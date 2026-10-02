<?php
// Zygote front controller for Drupal 11 (`ferro -S … --worker drupal-zygote.php
// --zygote`): the zygote boots the kernel once, up to the handle boundary
// (autoload, settings, container, boot), with a synthetic request; then
// ferro_handle_request() forks a child per request, which handles the real
// request on the inherited post-boot state and exits. Drupal is not patched
// and nothing is reset: no request state outlives its child.
use Drupal\Core\DrupalKernel;
use Symfony\Component\HttpFoundation\Request;

$root = getenv('DRUPAL_ROOT') ?: getcwd();
chdir($root);
$autoloader = require $root . '/autoload.php';
$boot = Request::create('http://localhost/', 'GET', [], [], [], [
    'SCRIPT_NAME' => '/index.php',
    'SCRIPT_FILENAME' => $root . '/index.php',
]);
$kernel = DrupalKernel::createFromRequest($boot, $autoloader, 'prod');
$kernel->boot();

// Optional preload (ZYGOTE_PRELOAD=<list from preload-record.php>): declare
// the code a request needs before forking, so children inherit it compiled
// (opcache preloading). Code only — no request runs, no request state.
if (($list = getenv('ZYGOTE_PRELOAD')) && is_file($list)) {
    foreach (file($list, FILE_IGNORE_NEW_LINES | FILE_SKIP_EMPTY_LINES) as $name) {
        try {
            if (str_starts_with($name, 'file:')) {
                // Declaration-only files Drupal loads with require_once (and
                // Twig templates, whose classes Twig checks before including).
                require_once substr($name, 5);
            } else {
                class_exists($name) || interface_exists($name) || trait_exists($name);
            }
        } catch (\Throwable) {
            // A name this engine cannot load (an extension class's polyfill
            // whose parent is missing): skipped, as opcache preload would.
        }
    }
}

ferro_handle_request(static function () use ($kernel) {
    $request = Request::createFromGlobals();
    $response = $kernel->handle($request);
    $response->send();
    $kernel->terminate($request, $response);
});
