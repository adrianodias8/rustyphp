<?php
// Shared micro-harness for bench/*.php. Same source runs on phpr and the oracle.
//   TIME <name> <ms>        wall time of one section (hrtime, monotonic)
//   RESULT <name> <value>   checksum; bench/run.sh diffs these across engines,
//                           so a benchmark that computes something different
//                           on phpr is reported as INVALID instead of "fast".
function bench(string $name, callable $fn): void {
    $t0 = hrtime(true);
    $r = $fn();
    $ms = (hrtime(true) - $t0) / 1e6;
    printf("TIME %s %.3f\n", $name, $ms);
    printf("RESULT %s %s\n", $name, is_scalar($r) ? (string)$r : md5(serialize($r)));
}
