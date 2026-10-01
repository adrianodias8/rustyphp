<?php
// Worker-mode front controller for Drupal 11: one kernel per worker, booted
// on the first request, then handle()/terminate() per request. Drupal is NOT
// patched; what leaks between requests is what bench/drupal/worker-leaks.sh
// measures, and DRUPAL_WORKER_RESET selects what this controller resets
// between requests (a comma list, see $resets below; empty = naive loop).
// Runs under ferro (`ferro -S … --worker`) and FrankenPHP worker mode.
//   DRUPAL_ROOT  the web root (default: the current directory)
use Drupal\Core\DrupalKernel;
use Symfony\Component\HttpFoundation\Request;

$root = getenv('DRUPAL_ROOT') ?: getcwd();
chdir($root);
$autoloader = require $root . '/autoload.php';
$resets = array_filter(explode(',', (string) getenv('DRUPAL_WORKER_RESET')));
// `recipe`: the minimal reset found by bench/drupal/worker-leaks.sh (step 2):
// rebuild the container from its cached definition, and reset the three
// Drupal statics that carry request state across it.
if (in_array('recipe', $resets, true)) {
    $resets = array_merge($resets, ['container', 'statics']);
    putenv('DRUPAL_WORKER_STATICS=' . implode(',', [
        'Drupal\\Core\\Render\\Renderer::$contextCollection',
        'Drupal\\Component\\Utility\\Html::$seenIds',
        'Drupal\\Component\\Utility\\Html::$seenIdsInit',
    ]));
}
$handle = function_exists('frankenphp_handle_request') ? 'frankenphp_handle_request' : 'ferro_handle_request';
$kernel = null;

$handler = static function () use (&$kernel, $autoloader, $resets) {
    $request = Request::createFromGlobals();
    if ($kernel === null) {
        $kernel = DrupalKernel::createFromRequest($request, $autoloader, 'prod');
    }
    $response = $kernel->handle($request);
    if (getenv('DRUPAL_WORKER_DEBUG')) {
        file_put_contents(getenv('DRUPAL_WORKER_DEBUG'), sprintf("%s %s route=%s status=%d len=%d ob=%d\n",
            $request->getMethod(), $request->getPathInfo(),
            $request->attributes->get('_route') ?? '-', $response->getStatusCode(),
            strlen((string) $response->getContent()), ob_get_level()), FILE_APPEND);
    }
    $response->send();
    $kernel->terminate($request, $response);
    // Between-request resets under test (each one named by what it clears).
    if (in_array('drupal_static', $resets, true)) {
        drupal_static_reset();
    }
    if (in_array('request_stack', $resets, true)) {
        $stack = \Drupal::requestStack();
        while ($stack->pop()) {
        }
    }
    if (in_array('container', $resets, true)) {
        $kernel->resetContainer();
    }
    if (in_array('kernel', $resets, true)) {
        $kernel = null;
    }
    // Every static property declared by a loaded Drupal class back to its
    // declared default; the ones that had changed are logged (the leak map).
    if (in_array('statics', $resets, true)) {
        // Process-level infrastructure the booted kernel relies on is kept;
        // DRUPAL_WORKER_STATICS (a comma list of Class::$prop) narrows the
        // reset to those names.
        $keep = ['Drupal\\Core\\DrupalKernel', 'Drupal\\Core\\Site\\Settings', 'Drupal\\Core\\Database\\Database',
            'Drupal\\Component\\FileCache\\FileCacheFactory', 'Drupal\\Component\\FileCache\\FileCache',
            'Drupal\\Core\\Extension\\ExtensionDiscovery', 'Drupal\\Component\\DependencyInjection\\ReverseContainer', 'Drupal'];
        $only = array_filter(explode(',', (string) getenv('DRUPAL_WORKER_STATICS')));
        $changed = [];
        foreach (get_declared_classes() as $c) {
            if (!str_starts_with($c, 'Drupal\\')) {
                continue;
            }
            $r = new \ReflectionClass($c);
            $defaults = $r->getDefaultProperties();
            foreach ($r->getProperties(\ReflectionProperty::IS_STATIC) as $p) {
                if ($p->getDeclaringClass()->getName() !== $c || $p->isReadOnly()) {
                    continue;
                }
                $name = $c . '::$' . $p->getName();
                if (in_array($c, $keep, true) || ($only && !in_array($name, $only, true))) {
                    continue;
                }
                $default = $defaults[$p->getName()] ?? null;
                if ($p->getValue() !== $default) {
                    $changed[] = $c . '::$' . $p->getName();
                    $p->setValue(null, $default);
                }
            }
        }
        if (getenv('DRUPAL_WORKER_DEBUG')) {
            file_put_contents(getenv('DRUPAL_WORKER_DEBUG'), '  statics reset: ' . implode(' ', $changed) . "\n", FILE_APPEND);
        }
    }
};

while ($handle($handler)) {
    gc_collect_cycles();
}
