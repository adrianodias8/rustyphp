<?php
// Phase timing of one Drupal front-page request (Drupal itself unmodified):
// bootstrap (autoload + kernel creation + boot, i.e. everything up to
// $kernel->handle()), handle (routing, controller, render), send, terminate.
// Used as web/index.php by bench/drupal/phases.sh; appends one line per
// request to $PHASES_LOG (default /scratch/phases.log):
//   <engine> <bootstrap_ms> <handle_ms> <send_ms> <terminate_ms> <total_ms> <bytes>
use Drupal\Core\DrupalKernel;
use Symfony\Component\HttpFoundation\Request;
$t0 = hrtime(true);
$autoloader = require __DIR__ . '/autoload.php';
$request = Request::createFromGlobals();
$kernel = DrupalKernel::createFromRequest($request, $autoloader, 'prod');
$kernel->boot();
$t1 = hrtime(true);
$response = $kernel->handle($request);
$t2 = hrtime(true);
$response->send();
$t3 = hrtime(true);
$kernel->terminate($request, $response);
$t4 = hrtime(true);
$engine = function_exists('ferro_handle_request') ? 'ferro' : (PHP_SAPI === 'fpm-fcgi' ? 'php-fpm' : PHP_SAPI);
file_put_contents(getenv('PHASES_LOG') ?: '/scratch/phases.log', sprintf("%s %.3f %.3f %.3f %.3f %.3f %d\n",
    $engine, ($t1 - $t0) / 1e6, ($t2 - $t1) / 1e6, ($t3 - $t2) / 1e6, ($t4 - $t3) / 1e6, ($t4 - $t0) / 1e6,
    strlen((string) $response->getContent())), FILE_APPEND);
