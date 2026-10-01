#!/usr/bin/env bash
# baseline/smoke-composer.sh — PLAN.md §1.2 smoke test (a):
# `composer require monolog/monolog` end to end under phpr, wall time vs oracle,
# then run the installed package on both engines and diff the output.
#
#   docker/run.sh ../baseline/smoke-composer.sh        (needs network)
#
# phpr cannot execute composer.phar (no Phar / __halt_compiler), so Composer is
# run from its EXTRACTED source on BOTH engines — the same thing upstream does.
# Each run gets an empty project dir, an empty COMPOSER_HOME and an empty cache:
# nothing is reused between runs. Order is ABBA (oracle, phpr, phpr, oracle) so a
# warmer network path does not favour one engine.
set -euo pipefail
PHPR="${PHPR:-/target/release/ferro}"
PHP="${PHP_ORACLE:-$(command -v php)}"
S="${SCRATCH:-/scratch}/smoke-composer"
OUT="${OUT:-$S/result.tsv}"
rm -rf "$S"; mkdir -p "$S"

cp "$(command -v composer)" "$S/composer.phar"      # Phar insists on the .phar extension
"$PHP" -d phar.readonly=0 -r '(new Phar($argv[1]))->extractTo($argv[2]);' "$S/composer.phar" "$S/composer-x"
COMPOSER_VERSION="$("$PHP" "$S/composer-x/bin/composer" --version --no-ansi 2>/dev/null | head -1)"
echo "composer: $COMPOSER_VERSION"

cat >"$S/use-monolog.php" <<'PHP'
<?php
require getcwd() . '/vendor/autoload.php';
use Monolog\Handler\StreamHandler;
use Monolog\Formatter\LineFormatter;
use Monolog\Level;
use Monolog\Logger;
$h = new StreamHandler('php://stdout', Level::Debug);
$h->setFormatter(new LineFormatter("%channel%.%level_name%: %message% %context% %extra%\n"));
$log = new Logger('smoke');
$log->pushHandler($h);
$log->pushProcessor(function ($record) { $record->extra['n'] = 42; return $record; });
$log->info('hello from {who}', ['who' => 'monolog']);
$log->warning('numbers', ['a' => 1, 'b' => [2, 3], 'c' => null]);
$log->error('exception', ['e' => get_class(new RuntimeException('x'))]);
echo 'monolog ', \Composer\InstalledVersions::getPrettyVersion('monolog/monolog'), "\n";
echo 'handlers ', count($log->getHandlers()), "\n";
PHP

printf 'run\tengine\trc\twall_s\tuser_s\tsys_s\trss_kb\tvendor_files\tmonolog\n' >"$OUT"
run() { # $1 run-id  $2 engine-label  $3... engine argv
  local id="$1" label="$2"; shift 2
  local d="$S/$id-$label"; mkdir -p "$d/project" "$d/home" "$d/cache"
  local rc=0
  ( cd "$d/project" && echo '{}' >composer.json && \
    COMPOSER_HOME="$d/home" COMPOSER_CACHE_DIR="$d/cache" COMPOSER_ALLOW_SUPERUSER=1 \
    /usr/bin/time -v -o "$d/time" "$@" "$S/composer-x/bin/composer" require monolog/monolog \
        --no-interaction --no-ansi --no-progress >"$d/stdout" 2>"$d/stderr" ) || rc=$?
  local wall user sys rss files ver="-"
  wall=$(awk -F': ' '/Elapsed \(wall clock\)/{n=split($2,a,":"); s=0; for(i=1;i<=n;i++) s=s*60+a[i]; print s}' "$d/time")
  user=$(awk -F': ' '/User time/{print $2}' "$d/time")
  sys=$(awk -F': ' '/System time/{print $2}' "$d/time")
  rss=$(awk -F': ' '/Maximum resident set size/{print $2}' "$d/time")
  files=$( (find "$d/project/vendor" -type f 2>/dev/null || true) | wc -l | tr -d ' ')
  if [[ -f "$d/project/vendor/autoload.php" ]]; then
    ( cd "$d/project" && "$PHP" -n "$S/use-monolog.php" >"$d/use.oracle.out" 2>&1 ) || true
    ( cd "$d/project" && "$PHPR"   "$S/use-monolog.php" >"$d/use.phpr.out"   2>&1 ) || true
    ver=$(awk '/^monolog /{print $2}' "$d/use.oracle.out")
    if cmp -s "$d/use.oracle.out" "$d/use.phpr.out"; then echo "  [$id $label] package output: IDENTICAL on both engines"
    else echo "  [$id $label] package output: DIFFERS"; diff "$d/use.oracle.out" "$d/use.phpr.out" | head -10; fi
  fi
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$id" "$label" "$rc" "$wall" "$user" "$sys" "$rss" "$files" "$ver" | tee -a "$OUT"
  if [[ "$rc" != 0 ]]; then echo "  stderr tail:"; tail -5 "$d/stderr" | sed 's/^/    /'; tail -3 "$d/stdout" | sed 's/^/    /'; fi
  return 0
}
run 1 oracle "$PHP"
run 2 phpr   "$PHPR"
run 3 phpr   "$PHPR"
run 4 oracle "$PHP"
echo
# Same lock file on both engines?  (resolution is deterministic given the same registry state)
if [[ -f "$S/1-oracle/project/composer.lock" && -f "$S/2-phpr/project/composer.lock" ]]; then
  "$PHP" -r '$a=json_decode(file_get_contents($argv[1]),true); $b=json_decode(file_get_contents($argv[2]),true);
    $f=fn($l)=>array_map(fn($p)=>$p["name"]."@".$p["version"], $l["packages"]);
    echo "resolved (oracle): ", implode(" ", $f($a)), "\nresolved (phpr):   ", implode(" ", $f($b)), "\n";
    echo $f($a)===$f($b) ? "resolution: IDENTICAL\n" : "resolution: DIFFERS\n";' \
    "$S/1-oracle/project/composer.lock" "$S/2-phpr/project/composer.lock"
  ( cd "$S/1-oracle/project/vendor" && find . -type f | LC_ALL=C sort | xargs sha256sum ) >"$S/vendor.oracle.sha"
  ( cd "$S/2-phpr/project/vendor"   && find . -type f | LC_ALL=C sort | xargs sha256sum ) >"$S/vendor.phpr.sha"
  if cmp -s "$S/vendor.oracle.sha" "$S/vendor.phpr.sha"; then echo "vendor/ tree: byte-IDENTICAL ($(wc -l <"$S/vendor.oracle.sha") files)"
  else echo "vendor/ tree: DIFFERS in $(diff "$S/vendor.oracle.sha" "$S/vendor.phpr.sha" | grep -c '^[<>]') lines:"; diff "$S/vendor.oracle.sha" "$S/vendor.phpr.sha" | head -10; fi
fi
