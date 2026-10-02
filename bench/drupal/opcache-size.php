<?php
// What opcache holds for one Drupal front page, for MEMORY_DRUPAL.md: run the
// front controller once in the CLI with opcache on, then report opcache's
// memory, the scripts it cached and the PHP source they came from.
//   php -d opcache.enable_cli=1 -d opcache.memory_consumption=1024 \
//       -d opcache.interned_strings_buffer=256 opcache-size.php /path/to/web
$root = rtrim($argv[1] ?? getcwd(), '/');
chdir($root);
$_SERVER = array_merge($_SERVER, [
    'REQUEST_URI' => '/', 'SCRIPT_NAME' => '/index.php', 'PHP_SELF' => '/index.php',
    'SCRIPT_FILENAME' => "$root/index.php", 'REQUEST_METHOD' => 'GET', 'HTTP_HOST' => 'localhost',
    'SERVER_NAME' => 'localhost', 'SERVER_PORT' => '80', 'REMOTE_ADDR' => '127.0.0.1',
]);
register_shutdown_function(static function () {
    while (ob_get_level()) {
        ob_end_clean();
    }
    $s = opcache_get_status(false);
    $files = get_included_files();
    $m = $s['memory_usage'];
    $i = $s['interned_strings_usage'];
    // used_memory counts the interned-strings buffer (allocated from the same
    // shared segment) whole; scripts = used - buffer.
    fprintf(STDERR, "opcache: scripts %.1f MiB (used %.1f MiB incl. the %.1f MiB interned buffer, wasted %.1f MiB); "
        . "interned strings %.1f MiB in %d strings; %d scripts cached; %d files, %.1f MiB of PHP source\n",
        ($m['used_memory'] - $i['buffer_size']) / 2**20, $m['used_memory'] / 2**20, $i['buffer_size'] / 2**20,
        $m['wasted_memory'] / 2**20, $i['used_memory'] / 2**20, $i['number_of_strings'],
        $s['opcache_statistics']['num_cached_scripts'], count($files),
        array_sum(array_map('filesize', $files)) / 2**20);
});
ob_start();
require "$root/index.php";
