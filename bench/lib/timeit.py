#!/usr/bin/env python3
"""Time short-lived commands precisely: wall (perf_counter), user+sys CPU and
peak RSS (wait4 rusage) per run; prints medians.

usage: timeit.py <runs> <label>=<cmd...> [--- <label>=<cmd...>]...
Commands are interleaved round-robin (one of each per round) after one
discarded warm-up round. Output: TSV  label  wall_ms  user_ms  sys_ms  rss_kb  min_wall_ms  max_wall_ms  rc
"""
import os, shlex, statistics, subprocess, sys, time

runs = int(sys.argv[1])
groups, cur = [], []
for a in sys.argv[2:]:
    if a == "---":
        groups.append(cur); cur = []
    else:
        cur.append(a)
if cur:
    groups.append(cur)
cmds = []
for g in groups:
    label, first = g[0].split("=", 1)
    cmds.append((label, [first] + g[1:]))

def once(argv):
    t0 = time.perf_counter()
    p = subprocess.Popen(argv, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    _, status, ru = os.wait4(p.pid, 0)
    wall = time.perf_counter() - t0
    p.returncode = os.waitstatus_to_exitcode(status)
    return wall * 1e3, ru.ru_utime * 1e3, ru.ru_stime * 1e3, ru.ru_maxrss, p.returncode

data = {l: [] for l, _ in cmds}
for l, argv in cmds:      # warm-up round, discarded (page cache, opcache file cache)
    once(argv)
for _ in range(runs):
    for l, argv in cmds:
        data[l].append(once(argv))
print("label\twall_ms\tuser_ms\tsys_ms\trss_kb\tmin_wall_ms\tmax_wall_ms\trc")
for l, _ in cmds:
    d = data[l]
    med = lambda i: statistics.median(x[i] for x in d)
    rcs = sorted(set(x[4] for x in d))
    print(f"{l}\t{med(0):.2f}\t{med(1):.2f}\t{med(2):.2f}\t{int(med(3))}\t{min(x[0] for x in d):.2f}\t{max(x[0] for x in d):.2f}\t{','.join(map(str, rcs))}")
