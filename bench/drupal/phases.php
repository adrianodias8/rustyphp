<?php
// Phase timing of one Drupal front-page request (Drupal itself unmodified;
// copy into the web root and point a `-S` router at it):
// autoload, kernel boot (settings, container load), handle (routing, render),
// send+terminate. Run from the web root with the CLI request globals set.
use Drupal\Core\DrupalKernel;
use Symfony\Component\HttpFoundation\Request;
$t0 = hrtime(true);
$autoloader = require __DIR__ . '/autoload.php';
$t1 = hrtime(true);
$request = Request::createFromGlobals();
$kernel = DrupalKernel::createFromRequest($request, $autoloader, 'prod');
$kernel->boot();
$t2 = hrtime(true);
$response = $kernel->handle($request);
$t3 = hrtime(true);
ob_start(); $response->send(); ob_end_clean();
$kernel->terminate($request, $response);
$t4 = hrtime(true);
file_put_contents('php://stderr', sprintf("autoload %.2f  boot %.2f  handle %.2f  send+terminate %.2f  total %.2f ms (%d bytes)\n",
  ($t1-$t0)/1e6, ($t2-$t1)/1e6, ($t3-$t2)/1e6, ($t4-$t3)/1e6, ($t4-$t0)/1e6, strlen($response->getContent())));
