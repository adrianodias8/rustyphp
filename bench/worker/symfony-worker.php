<?php
// Worker-mode front controller for the Symfony benchmark app: boot once,
// then serve. Runs under phpr (`ferro -S … --worker symfony-worker.php`) and
// under FrankenPHP worker mode (`frankenphp_handle_request`) unchanged.
//   SYMFONY_DIR = the directory holding vendor/ (default: /scratch/symfony-app)
require __DIR__ . '/symfony-kernel.php';

use Symfony\Component\HttpFoundation\Request;

$dir = getenv('SYMFONY_DIR') ?: '/scratch/symfony-app';
$kernel = build_kernel($dir);
$handle = function_exists('frankenphp_handle_request') ? 'frankenphp_handle_request' : 'ferro_handle_request';

$handler = static function () use ($kernel) {
    $request = Request::createFromGlobals();
    $response = $kernel->handle($request);
    $response->send();
    $kernel->terminate($request, $response);
};

$max = (int) (getenv('WORKER_MAX_REQUESTS') ?: 0);
for ($n = 0; $max === 0 || $n < $max; $n++) {
    if (!$handle($handler)) {
        break;
    }
    gc_collect_cycles();
}
