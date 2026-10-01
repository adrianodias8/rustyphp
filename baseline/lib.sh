# Shared helpers for baseline/run-baseline.sh and baseline/gate.sh.
# Both run INSIDE the dev container (docker/run.sh ...).
#
# The corpus is COPIED to the container-local /scratch volume before running:
# .phpt tests write temp files next to themselves, which would dirty the
# php-src checkout and put bind-mount I/O into every test. Test names are
# always recorded RELATIVE to the php-src root, so the lists are portable.

RUNNER="${RUNNER:-/target/release/phpt-runner}"
PHP_SRC="${PHP_SRC:-/work/php-src}"
CORPUS="${CORPUS:-/scratch/php-src}"
export PHPT_TIMEOUT_SECS="${PHPT_TIMEOUT_SECS:-10}"

stage_corpus() {
  local want have
  want="$(git -c safe.directory="*" -C "$PHP_SRC" rev-parse HEAD)"
  have="$(cat "$CORPUS/.staged-from" 2>/dev/null || true)"
  if [[ "$want" != "$have" ]]; then
    echo "staging corpus $PHP_SRC -> $CORPUS ($want)" >&2
    rm -rf "$CORPUS"; mkdir -p "$CORPUS"
    # Everything except .git: tests reference fixtures all over the tree.
    (cd "$PHP_SRC" && tar --exclude=.git -cf - .) | (cd "$CORPUS" && tar -xf -)
    echo "$want" >"$CORPUS/.staged-from"
  fi
}

# run_suite <name> <raw-dir> <dir>...   (dirs relative to the php-src root)
# Writes <raw-dir>/<name>.{log,rc,all,fails,skips,pass,summary}
run_suite() {
  local name="$1" raw="$2"; shift 2
  local -a dirs=() rel=("$@")
  local d
  for d in "${rel[@]}"; do dirs+=("$CORPUS/$d"); done
  mkdir -p "$raw"
  local t0 t1 rc=0
  t0=$(date +%s)
  # cwd = a throwaway dir: tests that write relative paths land there.
  local cwd="$raw/$name.cwd"; rm -rf "$cwd"; mkdir -p "$cwd"
  (cd "$cwd" && "$RUNNER" --isolate --list-fails --list-skips "${dirs[@]}") \
      >"$raw/$name.log" 2>"$raw/$name.stderr" || rc=$?
  t1=$(date +%s)
  echo "$rc" >"$raw/$name.rc"
  rm -rf "$cwd"
  # Every .phpt the runner was pointed at.
  for d in "${dirs[@]}"; do find "$d" -name '*.phpt' -type f; done \
      | sed "s|^$CORPUS/||" | LC_ALL=C sort -u >"$raw/$name.all"
  # Failures: "--- <path> ---" chunk headers after the "failures: N" line.
  tr -d '\0' <"$raw/$name.log" \
      | awk '/^failures: [0-9]+$/{f=1;next} f && /^--- .*\.phpt ---$/{sub(/^--- /,"");sub(/ ---$/,"");print}' \
      | sed "s|^$CORPUS/||" | LC_ALL=C sort -u >"$raw/$name.fails"
  # Skips: lines between "=== skips: N ===" and "=== phpt-runner ===", "<path>\t<category>\t<detail>".
  tr -d '\0' <"$raw/$name.log" \
      | awk '/^=== skips: [0-9]+ ===$/{s=1;next} /^=== phpt-runner ===$/{s=0} s && /\.phpt\t/{print}' \
      | sed "s|^$CORPUS/||" | LC_ALL=C sort -u >"$raw/$name.skips.tsv"
  cut -f1 "$raw/$name.skips.tsv" | LC_ALL=C sort -u >"$raw/$name.skips"
  LC_ALL=C comm -23 "$raw/$name.all" <(LC_ALL=C sort -u "$raw/$name.fails" "$raw/$name.skips") >"$raw/$name.pass"
  local total pass fail skip
  total=$(awk '/^total: /{print $2}' "$raw/$name.log" | tail -1)
  pass=$(awk '/^pass: /{print $2}' "$raw/$name.log" | tail -1)
  fail=$(awk '/^fail: /{print $2}' "$raw/$name.log" | tail -1)
  skip=$(awk '/^skip: /{print $2}' "$raw/$name.log" | tail -1)
  {
    echo "suite=$name"
    echo "dirs=\"${rel[*]}\""
    echo "runner_rc=$rc"
    echo "seconds=$((t1 - t0))"
    echo "total=${total:-?}"; echo "pass=${pass:-?}"; echo "fail=${fail:-?}"; echo "skip=${skip:-?}"
    echo "files_found=$(wc -l <"$raw/$name.all" | tr -d ' ')"
    echo "list_pass=$(wc -l <"$raw/$name.pass" | tr -d ' ')"
    echo "list_fail=$(wc -l <"$raw/$name.fails" | tr -d ' ')"
    echo "list_skip=$(wc -l <"$raw/$name.skips" | tr -d ' ')"
  } >"$raw/$name.summary"
}

# The lists must reproduce the runner's own counters, or the baseline is void.
check_suite_consistent() {
  local raw="$1" name="$2"
  # shellcheck disable=SC1090
  ( . "$raw/$name.summary"
    if [[ "$pass" != "$list_pass" || "$fail" != "$list_fail" || "$skip" != "$list_skip" || "$total" != "$files_found" ]]; then
      echo "INCONSISTENT $name: runner total/pass/fail/skip=$total/$pass/$fail/$skip vs lists files/pass/fail/skip=$files_found/$list_pass/$list_fail/$list_skip" >&2
      exit 1
    fi )
}
