#!/usr/bin/env python3
"""Aggregate bench/run.sh raw output into bench/results/<date>.md.

usage: summarize.py <raw-dir> <out.md> "<bench names>"

Per (bench, engine): median over runs of wall, user CPU, peak RSS (from
/usr/bin/time -v). Per section: median of the benchmark's own timings
(TIME lines from lib/harness.php, or the native tables of Zend/bench.php and
Zend/micro_bench.php). A section whose RESULT checksum differs from
php-noopc's is flagged INVALID rather than reported as a time.
"""
import os, re, statistics, sys
from collections import defaultdict

raw, out, names = sys.argv[1], sys.argv[2], sys.argv[3].split()
ENGINES = ["phpr", "php-noopc", "php-opc", "php-opc-warm"]
REF = "php-noopc"

def parse_time_file(p):
    d = {}
    for line in open(p, errors="replace"):
        line = line.strip()
        if line.startswith("Elapsed (wall clock)"):
            t = line.split(": ", 1)[1]
            parts = [float(x) for x in t.split(":")]
            d["wall"] = sum(v * 60 ** i for i, v in enumerate(reversed(parts)))
        elif line.startswith("User time (seconds)"):
            d["user"] = float(line.split(": ")[1])
        elif line.startswith("System time (seconds)"):
            d["sys"] = float(line.split(": ")[1])
        elif line.startswith("Maximum resident set size"):
            d["rss_kb"] = int(line.split(": ")[1])
    return d

# "name   0.123" (bench.php) or "name   0.123    0.100" (micro_bench.php: raw, then raw minus
# loop overhead). The first number — the raw time — is the one recorded.
ZEND_LINE = re.compile(r"^(\S.*?)\s+(\d+\.\d{3})(?:\s+-?\d+\.\d{3})?\s*$")

def parse_out(bench, p):
    """-> (times {section: seconds}, results {section: checksum}, info {k: v})"""
    times, results, info = {}, {}, {}
    for line in open(p, errors="replace"):
        line = line.rstrip("\n")
        if line.startswith("TIME "):
            _, name, ms = line.split(" ", 2)
            times[name] = float(ms) / 1000.0
        elif line.startswith("RESULT "):
            _, name, val = line.split(" ", 2)
            results[name] = val
        elif line.startswith("INFO "):
            _, k, v = line.split(" ", 2)
            info[k] = v
        elif bench.startswith("zend_"):
            m = ZEND_LINE.match(line)
            if m and not set(m.group(1)) <= set("-"):
                times[m.group(1).strip()] = float(m.group(2))
    return times, results, info

# Rows that exist in the output but do not measure what their name says.
NOT_MEASURED = {
    ("zend_micro_bench", "isset(Foo::$x)"):
        "statement patched out on both engines: phpr rejects `isset(Class::$static)` at parse time",
}

def med(xs):
    return statistics.median(xs) if xs else None

def fmt(x, nd=3):
    return "—" if x is None else f"{x:.{nd}f}"

def ratio(a, b):
    return "—" if not a or not b else f"{a / b:.2f}×"

meta = {}
for line in open(os.path.join(raw, "meta.env")):
    if "=" in line:
        k, v = line.rstrip("\n").split("=", 1)
        meta[k] = v

L = []
w = L.append
w(f"# Benchmark results — {os.path.splitext(os.path.basename(out))[0]}")
w("")
w(f"Measured {meta.get('date_utc', '?')} (UTC, container clock).")
w("")
w("Produced by `bench/run.sh` (PLAN.md §2.1). Numbers are medians over "
  f"**{meta.get('runs')} runs**, engines interleaved per round.")
w("")
w("| | |")
w("|---|---|")
for k, label in [
    ("phpr_sha", "phpr git SHA"), ("phpr_dirty", "phpr tracked source files modified vs SHA"),
    ("phpr_bin_sha256", "phpr binary sha256 (first 16)"), ("phpr_bin_bytes", "phpr binary size (bytes)"),
    ("oracle_version", "oracle PHP version"), ("oracle_opcache", "oracle opcache"),
    ("php_src_tag", "php-src tag (Zend/bench.php source)"), ("rustc", "rustc"),
    ("kernel", "kernel"), ("cpus", "CPUs visible"), ("host", "host"),
    ("mem_kb", "memory (kB)"), ("loadavg_start", "loadavg at start"), ("loadavg_end", "loadavg at end"),
]:
    w(f"| {label} | `{meta.get(k, '?')}` |")
w("")
w("Engines: `phpr` · `php-noopc` = `php -n -d opcache.enable_cli=0` · "
  "`php-opc` = `php -d opcache.enable_cli=1` (one-shot CLI: optimizer on, but every file is still "
  "compiled each run) · `php-opc-warm` = `php-opc` + primed `opcache.file_cache` (compiled scripts "
  "loaded from cache — the closest CLI analogue of a warm FPM/worker). JIT is off in all oracle runs.")
w("")

summary_rows = []
sections_md = []
problems = []

for b in names:
    per = {}
    for e in ENGINES:
        runs, secs, res, infos, rcs = [], defaultdict(list), {}, {}, []
        i = 1
        while os.path.exists(os.path.join(raw, f"{b}.{e}.{i}.time")):
            base = os.path.join(raw, f"{b}.{e}.{i}")
            rc = int(open(base + ".rc").read().strip() or "0")
            rcs.append(rc)
            t = parse_time_file(base + ".time")
            if rc == 0 and t:
                runs.append(t)
                ts, rs, inf = parse_out(b, base + ".out")
                for k, v in ts.items():
                    secs[k].append(v)
                res.update(rs)
                infos.update(inf)
            i += 1
        per[e] = dict(runs=runs, secs=secs, res=res, info=infos, rcs=rcs)
        bad = [rc for rc in rcs if rc != 0]
        if bad:
            err = ""
            ep = os.path.join(raw, f"{b}.{e}.1.err")
            op = os.path.join(raw, f"{b}.{e}.1.out")
            for p in (ep, op):
                if os.path.exists(p):
                    txt = open(p, errors="replace").read().strip()
                    if txt:
                        err = txt.splitlines()[-1][:300]
                        if "Fatal" in txt or "error" in txt.lower():
                            for ln in txt.splitlines():
                                if "Fatal" in ln or "rror" in ln:
                                    err = ln[:300]
                                    break
                            break
            problems.append(f"`{b}` under `{e}`: {len(bad)}/{len(rcs)} runs exited non-zero "
                            f"(rc={sorted(set(bad))}). Last message: `{err}`")

    def agg(e, key):
        return med([r[key] for r in per[e]["runs"] if key in r])

    row = [b]
    for e in ENGINES:
        row += [fmt(agg(e, "wall")), fmt(agg(e, "user")),
                "—" if agg(e, "rss_kb") is None else f"{agg(e, 'rss_kb') / 1024:.1f}"]
    pw, pu = agg("phpr", "wall"), agg("phpr", "user")
    row += [ratio(pw, agg("php-noopc", "wall")), ratio(pw, agg("php-opc", "wall")),
            ratio(pw, agg("php-opc-warm", "wall")), ratio(pu, agg("php-noopc", "user")),
            ratio(agg("phpr", "rss_kb"), agg("php-noopc", "rss_kb"))]
    summary_rows.append(row)

    # section table
    order = []
    for e in [REF] + ENGINES:
        for k in per[e]["secs"]:
            if k not in order:
                order.append(k)
    if order:
        S = [f"### `{b}` — per-section times (seconds, median of {meta.get('runs')})", ""]
        if per[REF]["info"]:
            S.append("Info: " + " · ".join(f"{k}={v}" for k, v in per[REF]["info"].items()))
            S.append("")
        S.append("| section | phpr | php-noopc | php-opc | php-opc-warm | phpr / noopc | phpr / opc |")
        S.append("|---|---:|---:|---:|---:|---:|---:|")
        for k in order:
            if (b, k) in NOT_MEASURED:
                S.append(f"| {k} | not measured | — | — | — | — | — |")
                problems.append(f"`{b}` / `{k}`: {NOT_MEASURED[(b, k)]}.")
                continue
            vals = {e: med(per[e]["secs"].get(k, [])) for e in ENGINES}
            ref_res = per[REF]["res"].get(k)
            p_res = per["phpr"]["res"].get(k)
            pcell = fmt(vals["phpr"], 4)
            if vals["phpr"] is None:
                pcell = "MISSING"
                if per["phpr"]["runs"]:   # engine ran, this section alone is absent
                    problems.append(f"`{b}` / `{k}`: no timing from phpr (section did not run).")
            elif ref_res is not None and p_res != ref_res:
                pcell = f"INVALID ({fmt(vals['phpr'], 4)})"
                problems.append(f"`{b}` / `{k}`: phpr RESULT `{p_res}` != oracle `{ref_res}` — timing not comparable.")
            S.append(f"| {k} | {pcell} | {fmt(vals['php-noopc'], 4)} | {fmt(vals['php-opc'], 4)} | "
                     f"{fmt(vals['php-opc-warm'], 4)} | {ratio(vals['phpr'], vals['php-noopc'])} | "
                     f"{ratio(vals['phpr'], vals['php-opc'])} |")
        S.append("")
        sections_md.append("\n".join(S))

w("## Whole-process summary")
w("")
w("wall / user in seconds, RSS = peak resident set in MiB (`/usr/bin/time -v`). Whole-process figures "
  "include each engine's startup and, for the harness benchmarks, fixture construction outside the timed sections.")
w("")
hdr = ["benchmark"]
for e in ENGINES:
    hdr += [f"{e} wall", f"{e} user", f"{e} RSS"]
hdr += ["phpr/noopc wall", "phpr/opc wall", "phpr/opc-warm wall", "phpr/noopc user", "phpr/noopc RSS"]
w("| " + " | ".join(hdr) + " |")
w("|---|" + "---:|" * (len(hdr) - 1))
for row in summary_rows:
    w("| " + " | ".join(row) + " |")
w("")
unp = os.path.join(raw, "zend_micro_bench.unpatched.phpr.txt")
if os.path.exists(unp):
    msg = open(unp, errors="replace").read().strip()
    if "rror" in msg:
        problems.insert(0, "`Zend/micro_bench.php` **unmodified** does not run under phpr: "
                           f"`{msg[:200]}`. The table below uses a copy with that one statement neutralised.")
w("## Problems")
w("")
if problems:
    for p in dict.fromkeys(problems):
        w(f"- {p}")
else:
    w("None: every run exited 0 and every phpr RESULT checksum matched the oracle's.")
w("")
w("## Per-section detail")
w("")
L.extend(sections_md)

os.makedirs(os.path.dirname(out), exist_ok=True)
open(out, "w").write("\n".join(L) + "\n")
