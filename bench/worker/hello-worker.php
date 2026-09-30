<?php
// Worker-mode hello world: the smallest request loop. Runs under phpr
// (`phpr_handle_request`) and FrankenPHP (`frankenphp_handle_request`).
$handle = function_exists('frankenphp_handle_request') ? 'frankenphp_handle_request' : 'phpr_handle_request';
$n = 0;
while ($handle(static function () use (&$n) { $n++; echo "Hello, world"; })) {
}
