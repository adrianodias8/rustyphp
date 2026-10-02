<?php
// php -S router for recording a zygote preload list: one request through
// preload-record.php (copied next to it in web/), static files served as is.
if (preg_match('#\.(css|js|png|svg|ico)$#', $_SERVER['REQUEST_URI'])) return false;
$_SERVER['SCRIPT_NAME'] = '/index.php';
require __DIR__ . '/preload-record.php';
