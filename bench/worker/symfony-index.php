<?php
// One-shot front controller for the Symfony benchmark app (php-fpm, the
// FrankenPHP classic mode, `phpr -S` without --worker): boots per request.
require __DIR__ . '/symfony-kernel.php';

use Symfony\Component\HttpFoundation\Request;

$kernel = build_kernel(getenv('SYMFONY_DIR') ?: '/scratch/symfony-app');
$request = Request::createFromGlobals();
$response = $kernel->handle($request);
$response->send();
$kernel->terminate($request, $response);
