#!/usr/bin/env bash
# bench/worker/isolation.sh — the request-isolation gate of worker mode
# (DECISION_KERNEL.md §5): the same routes served by ONE worker across N
# requests and by the one-shot cli-server, compared byte for byte (status,
# the headers the script controls, body). Only /stateful may differ, in the
# documented way. Also runs the one-shot routes on the oracle (`php -S`) so
# the one-shot side is itself checked against PHP.
#   docker/run.sh /work/php-rust/bench/worker/isolation.sh
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PHPR="${PHPR:-/target/release/phpr}"; PHP="${PHP_ORACLE:-$(command -v php)}"
S="${SCRATCH:-/scratch}/isolation"; rm -rf "$S"; mkdir -p "$S"
ROUTES="/echo?a=1&b[]=2&b[]=3 /headers /warn /exit /throw /ob /ini /ini /handler /warn /shutdown /objects /stateful /stateful /nope /echo?x=y"
fetch() { # $1 base url  $2 out file
  : > "$2"
  for r in $ROUTES; do
    { echo "=== GET $r"; curl -s -D - -o "$S/body" -A "iso/1" -H "Cookie: k=v" "$1$r" \
        | grep -viE '^(Date|Connection|Host|Server|Content-Length):' | tr -d '\r'; cat "$S/body"; echo; } >> "$2"
  done
  { echo "=== POST /echo"; curl -s -D - -o "$S/body" -X POST -d 'p=1&q[]=2' "$1/echo" | grep -viE '^(Date|Connection|Host|Server|Content-Length):' | tr -d '\r'; cat "$S/body"; echo; } >> "$2"
  { echo "=== POST json /echo"; curl -s -D - -o "$S/body" -X POST -H 'Content-Type: application/json' -d '{"j":1}' "$1/echo" | grep -viE '^(Date|Connection|Host|Server|Content-Length):' | tr -d '\r'; cat "$S/body"; echo; } >> "$2"
}
cd "$HERE"
"$PHPR" -S 127.0.0.1:8101 --worker isolation-worker.php --workers 1 >"$S/worker.log" 2>&1 & W=$!
"$PHPR" -S 127.0.0.1:8102 isolation-oneshot.php >"$S/oneshot.log" 2>&1 & O=$!
"$PHP" -S 127.0.0.1:8103 isolation-oneshot.php >"$S/oracle.log" 2>&1 & P=$!
sleep 1
fetch http://127.0.0.1:8101 "$S/worker.txt"
fetch http://127.0.0.1:8102 "$S/oneshot.txt"
fetch http://127.0.0.1:8103 "$S/oracle.txt"
kill $W $O $P 2>/dev/null; wait 2>/dev/null
echo "--- worker vs one-shot (phpr):"; diff "$S/oneshot.txt" "$S/worker.txt" && echo IDENTICAL
echo "--- one-shot phpr vs oracle:"; diff "$S/oracle.txt" "$S/oneshot.txt" | sed 's/^/  /' | head -40
echo "(full transcripts in $S)"
