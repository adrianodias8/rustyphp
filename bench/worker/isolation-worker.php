<?php
// Request-isolation battery for worker mode (DECISION_KERNEL.md §5 gate):
// every route below is answered by this script in worker mode and by
// isolation-oneshot.php in one-shot mode; bench/worker/isolation.sh diffs
// the two byte for byte and expects exactly the documented differences.
require __DIR__ . '/isolation-routes.php';
// Captured at boot, before the first request: must be what a one-shot request sees.
$GLOBALS['boot_sg'] = implode(',', array_map('gettype', [$_GET, $_POST, $_COOKIE, $_FILES, $_SERVER]));
$handle = function_exists('frankenphp_handle_request') ? 'frankenphp_handle_request' : 'ferro_handle_request';
while ($handle(static function () { isolation_route(); })) {
    gc_collect_cycles();
}
