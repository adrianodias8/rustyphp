<?php
// unserialize() on a Drupal site's real cache rows (every serialized row of
// every cache_* table), php vs ferro: bench/calls/README / NOTES session 11.
//   php|ferro unser-cache.php /scratch/drupal-census/web/default/files/.ht.sqlite [R]
// Classes are not loaded (no autoloader), so objects come back as
// __PHP_Incomplete_Class on both engines: the parse and build work is what is
// timed. Prints rows, bytes and the best-of-R total ms.
$db = new PDO('sqlite:' . $argv[1]);
$R = (int) ($argv[2] ?? 5);
$rows = [];
foreach ($db->query("SELECT name FROM sqlite_master WHERE type = 'table' AND name LIKE 'cache_%'")->fetchAll(PDO::FETCH_COLUMN) as $t) {
    try {
        $q = $db->query("SELECT data FROM $t WHERE serialized = 1");
    } catch (PDOException) {
        continue; // not a cache bin (no data/serialized columns)
    }
    foreach ($q->fetchAll(PDO::FETCH_COLUMN) as $d) {
        $rows[] = $d;
    }
}
$bytes = array_sum(array_map('strlen', $rows));
$best = INF;
for ($r = 0; $r < $R; $r++) {
    $t = hrtime(true);
    foreach ($rows as $d) {
        @unserialize($d);
    }
    $best = min($best, (hrtime(true) - $t) / 1e6);
}
printf("%d rows, %.1f KiB, unserialize all: %.2f ms (best of %d), %.0f ns/KiB\n", count($rows), $bytes / 1024, $best, $R, $best * 1e6 / ($bytes / 1024));
