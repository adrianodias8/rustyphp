<?php
// Records, in declaration order, the classes, interfaces and traits a
// request declares (on top of Drupal's own index.php), for the zygote's
// optional preload (bench/drupal/drupal-zygote.php, ZYGOTE_PRELOAD): opcache
// preloading's idea — the zygote compiles and declares that code before it
// forks, so request children inherit it instead of compiling it each time.
register_shutdown_function(static function () {
    $names = array_merge(get_declared_interfaces(), get_declared_traits(), get_declared_classes());
    // Declaration-only files a request loads outside the class autoloader
    // (module/include/theme files, Twig's compiled templates), as "file:".
    foreach (get_included_files() as $f) {
        if (preg_match('/\.(module|inc|theme|engine|profile)$/', $f) || str_contains($f, '/files/php/twig/')) {
            $names[] = 'file:' . $f;
        }
    }
    file_put_contents(getenv('PRELOAD_LIST') ?: '/scratch/drupal-preload.txt', implode("\n", $names) . "\n");
});
require __DIR__ . '/index.php';
