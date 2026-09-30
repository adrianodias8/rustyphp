<?php
// The Symfony HttpKernel application of bench/symfony-boot.php, as a
// reusable boot: `build_kernel($dir)` returns the HttpKernel; the controller
// and routes are byte-for-byte those of the benchmark (same three routes,
// 60 filler routes), so the responses are comparable across SAPIs.
//   $dir = the directory holding vendor/ (bench/symfony-app installed).

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

function build_kernel(string $dir): HttpKernel
{
    require_once $dir . '/vendor/autoload.php';

    $routes = new RouteCollection();
    $routes->add('home', new Route('/', ['_controller' => [BenchController::class, 'home']]));
    $routes->add('user', new Route('/user/{id}/{slug}', ['_controller' => [BenchController::class, 'user'], 'slug' => 'none'], ['id' => '\d+']));
    $routes->add('api', new Route('/api/{resource}', ['_controller' => [BenchController::class, 'api']]));
    for ($i = 0; $i < 60; $i++) {
        $routes->add("filler_$i", new Route("/section$i/{a}/{b}", ['_controller' => [BenchController::class, 'home']], ['a' => '\d+']));
    }

    $stack = new RequestStack();
    $matcher = new UrlMatcher($routes, new RequestContext());
    $dispatcher = new EventDispatcher();
    $dispatcher->addSubscriber(new RouterListener($matcher, $stack));
    $dispatcher->addSubscriber(new ResponseListener('UTF-8'));

    return new HttpKernel($dispatcher, new ControllerResolver(), $stack, new ArgumentResolver());
}
