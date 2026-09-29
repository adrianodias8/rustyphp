<?php
// PLAN §2.1: boot a minimal Symfony HttpKernel app and handle 200 requests
// in-process. Closest proxy to a Drupal/Laravel request until those run:
// autoloading ~150 framework classes, event dispatch, routing, controller
// resolution, argument resolving via Reflection, Response rendering.
//   SYMFONY_DIR=<dir with bench/symfony-app/composer.json installed>
require __DIR__ . '/lib/harness.php';

$dir = getenv('SYMFONY_DIR') ?: '/scratch/symfony-app';
$nreq = (int) (getenv('SYMFONY_REQUESTS') ?: 200);

use Symfony\Component\EventDispatcher\EventDispatcher;
use Symfony\Component\HttpFoundation\JsonResponse;
use Symfony\Component\HttpFoundation\Request;
use Symfony\Component\HttpFoundation\RequestStack;
use Symfony\Component\HttpFoundation\Response;
use Symfony\Component\HttpKernel\Controller\ArgumentResolver;
use Symfony\Component\HttpKernel\Controller\ControllerResolver;
use Symfony\Component\HttpKernel\EventListener\ResponseListener;
use Symfony\Component\HttpKernel\EventListener\RouterListener;
use Symfony\Component\HttpKernel\HttpKernel;
use Symfony\Component\Routing\Matcher\UrlMatcher;
use Symfony\Component\Routing\RequestContext;
use Symfony\Component\Routing\Route;
use Symfony\Component\Routing\RouteCollection;

final class BenchController
{
    public function home(): Response
    {
        return new Response('<html><body><h1>home</h1></body></html>');
    }

    public function user(Request $request, int $id, string $slug = 'none'): Response
    {
        $rows = [];
        for ($i = 0; $i < 20; $i++) {
            $rows[] = sprintf('<li>%s</li>', htmlspecialchars("$slug item $i of user $id & co"));
        }
        return new Response('<ul>' . implode('', $rows) . '</ul>', 200, ['X-User' => (string) $id]);
    }

    public function api(Request $request, string $resource): JsonResponse
    {
        $data = ['resource' => $resource, 'q' => $request->query->all(), 'items' => []];
        for ($i = 0; $i < 25; $i++) {
            $data['items'][] = ['id' => $i, 'name' => "$resource-$i", 'ok' => ($i % 2) === 0];
        }
        return new JsonResponse($data);
    }
}

$kernel = null;

bench('boot_autoload_and_kernel', function () use ($dir, &$kernel) {
    require $dir . '/vendor/autoload.php';

    $routes = new RouteCollection();
    $routes->add('home', new Route('/', ['_controller' => [BenchController::class, 'home']]));
    $routes->add('user', new Route('/user/{id}/{slug}', ['_controller' => [BenchController::class, 'user'], 'slug' => 'none'], ['id' => '\d+']));
    $routes->add('api', new Route('/api/{resource}', ['_controller' => [BenchController::class, 'api']]));
    // Filler routes so the matcher has a realistic table to walk.
    for ($i = 0; $i < 60; $i++) {
        $routes->add("filler_$i", new Route("/section$i/{a}/{b}", ['_controller' => [BenchController::class, 'home']], ['a' => '\d+']));
    }

    $stack = new RequestStack();
    $matcher = new UrlMatcher($routes, new RequestContext());
    $dispatcher = new EventDispatcher();
    $dispatcher->addSubscriber(new RouterListener($matcher, $stack));
    $dispatcher->addSubscriber(new ResponseListener('UTF-8'));

    $kernel = new HttpKernel($dispatcher, new ControllerResolver(), $stack, new ArgumentResolver());
    return get_class($kernel);
});

$paths = [
    '/',
    '/user/42/alice',
    '/api/widgets?page=2&sort=name',
    '/user/7',
    '/api/orders?x[]=1&x[]=2',
];

// First request pays for lazily-autoloaded classes; reported on its own.
bench('first_request', function () use (&$kernel, $paths) {
    $request = Request::create($paths[1]);
    $response = $kernel->handle($request);
    $kernel->terminate($request, $response);
    return $response->getStatusCode() . ':' . md5($response->getContent());
});

bench('handle_requests', function () use (&$kernel, $paths, $nreq) {
    $h = '';
    $status = 0;
    for ($i = 0; $i < $nreq; $i++) {
        $request = Request::create($paths[$i % count($paths)], 'GET', [], ['sid' => 'abc' . $i], [], ['HTTP_ACCEPT' => 'text/html', 'REMOTE_ADDR' => '10.0.0.' . ($i % 250)]);
        $response = $kernel->handle($request);
        $status += $response->getStatusCode();
        $h = md5($h . $response->getContent());
        $kernel->terminate($request, $response);
    }
    return $status . ':' . $h;
});

printf("INFO requests %d\n", $nreq);
// (get_included_files() is not implemented by phpr, so it is not reported.)
printf("INFO declared_classes %d\n", count(get_declared_classes()));
