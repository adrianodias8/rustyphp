"""Where a warm Drupal request's time goes on ferro, by kind of work, against
what the same work costs on PHP 8.5.7 + opcache (NOTES.md session 11, step 4).

  python3 breakdown.py data/census-<date>.txt data/ns-kind-<date>.txt

Inputs:
- ferro's op-time census of one warm front page (bench/drupal/optime.sh,
  PHPR_OP_CENSUS_TOP large): per op kind, count and net ns; per named callee,
  calls and gross ns (a builtin's gross includes its work).
- ns-kind: which `ns:` callees (unqualified calls from namespaced code) are
  builtins and which are user functions (asked of the oracle).
- PHP's unit costs below, from bench/calls/callcost.php on the oracle with
  opcache on, JIT off, optimizer off (opcache.optimization_level=0: Drupal's
  calls are cross-file method calls the optimizer cannot remove), and
  bench/calls/unser-cache.php / a file_exists loop for builtin work.

Every category: ferro ms (census), count, and PHP ms = count x PHP unit cost;
gap = the difference. Categories whose PHP side was not measured say so.
"""
import re
import sys

census, nskind = sys.argv[1], sys.argv[2]

ops = {}       # name -> (net_ms, count)
callees = {}   # key -> (gross_ms, calls)
section = None
for line in open(census):
    if line.startswith("-- "):
        section = line
        continue
    f = line.split()
    if section and section.startswith("-- op time") and len(f) >= 6:
        ops[f[-1]] = (float(f[0]), int(f[-2]))
    elif section and section.startswith("-- call ops by callee") and len(f) >= 6:
        callees[f[-1]] = (float(f[0]), int(f[2]))

user_ns = {l.split()[1] for l in open(nskind) if l.startswith("user ")}

# PHP 8.5.7 + opcache (optimizer off) unit costs, ns per operation
PHP = {
    "func": 5.3, "method": 8.3, "static": 6.1, "closure": 9.0, "ctor": 8.3,
    "typed_param": 0.9,          # (f(int x3) - f($a,$b,$c)) / 3
    "builtin_call": 2.0,         # strlen / count call overhead
    "prop_get": 2.5, "prop_set": 2.6, "prop_isset": 4.5, "prop_chain_isset": 7.1, "prop_unset": 3.0,
    "dim": 3.5, "dim_write": 4.5, "foreach_elem": 0.9, "path": 4.0,
    "concat": 9.5, "simple_op": 1.0,
}
FERRO_BUILTIN_OVERHEAD = 49.0    # ferro strlen/count minus nothing: call machinery of a trivial builtin
UNSERIALIZE_RATIO = 5.42 / 2.88  # ferro / php on the site's cache rows
FILE_EXISTS_PHP_NS = 280.0       # php file_exists() on an existing file

def take(names):
    ms = sum(ops.get(n, (0, 0))[0] for n in names)
    cnt = sum(ops.get(n, (0, 0))[1] for n in names)
    for n in names:
        ops.pop(n, None)
    return ms, cnt

rows = []
def row(name, ferro_ms, count, php_ms, note=""):
    rows.append((name, ferro_ms, count, php_ms, note))

# ---- user calls: setup ops + Ret, one Ret per returning user frame ----
c = {k: ops.get(k, (0, 0))[1] for k in ops}
meth = c.get("MethodCall", 0) + c.get("ThisMethodCall", 0) + c.get("InvokeMethod", 0)
stat = c.get("StaticCall", 0) + c.get("StaticCallDynamic", 0)
ctor = c.get("InvokeCtor", 0) + c.get("InvokeCtorArgs", 0)
clos = c.get("CallValue", 0) + c.get("CallValueArgs", 0)
rets = c.get("Ret", 0)
plain = max(rets - meth - stat - ctor - clos, 0)   # functions, includes' returns, init thunks
user_ns_ms = sum(v[0] for k, v in callees.items() if k.startswith("ns:") and k[3:] in user_ns)
typed = c.get("CoerceParams", 0)
fm, _ = take(["Ret", "MethodCall", "ThisMethodCall", "InvokeMethod", "StaticCall", "StaticCallDynamic",
              "InvokeCtor", "InvokeCtorArgs", "CallValue", "CallValueArgs", "CoerceParams", "CoerceParam",
              "CheckArity", "FillDefault", "BindRefTo", "PushArgPlace", "ParkReturn", "MakeFcc"])
php_calls = (meth * PHP["method"] + stat * PHP["static"] + ctor * PHP["ctor"] + clos * PHP["closure"]
             + plain * PHP["func"] + typed * 2 * PHP["typed_param"]) / 1e6
row("user calls (setup + return)", fm + user_ns_ms, rets, php_calls,
    f"{meth} method, {stat} static, {ctor} ctor, {clos} dynamic, ~{plain} plain/unit returns")

# ---- builtin calls: machinery vs work ----
b_calls = sum(v[1] for k, v in callees.items() if k.startswith(("h:", "b:")) or (k.startswith("ns:") and k[3:] not in user_ns))
b_ms, _ = take(["CallNsFallback", "CallNsFallbackArgs", "CallHostBuiltin", "CallHostBuiltinOut", "CallHostBuiltinRef",
                "CallBuiltin", "CallBuiltinRef", "CallBuiltinRefCell"])
b_ms -= user_ns_ms
overhead_ms = b_calls * FERRO_BUILTIN_OVERHEAD / 1e6
row("builtin calls: call machinery", overhead_ms, b_calls, b_calls * PHP["builtin_call"] / 1e6,
    f"{FERRO_BUILTIN_OVERHEAD:.0f} ns vs {PHP['builtin_call']} ns per call (trivial builtin)")
uns = callees.get("ns:unserialize", (0, 0))
row("builtin work: unserialize", uns[0] - uns[1] * FERRO_BUILTIN_OVERHEAD / 1e6, uns[1],
    (uns[0] - uns[1] * FERRO_BUILTIN_OVERHEAD / 1e6) / UNSERIALIZE_RATIO, f"ferro/php {UNSERIALIZE_RATIO:.2f} on the site's cache rows")
fe = callees.get("ns:file_exists", (0, 0))
row("builtin work: file_exists", fe[0] - fe[1] * FERRO_BUILTIN_OVERHEAD / 1e6, fe[1], fe[1] * FILE_EXISTS_PHP_NS / 1e6,
    f"ferro {fe[0] * 1e6 / max(fe[1], 1):.0f} ns/call in the request vs php {FILE_EXISTS_PHP_NS:.0f} ns (micro)")
pdo = sum(v[0] for k, v in callees.items() if k.startswith("h:__pdo"))
pdo_n = sum(v[1] for k, v in callees.items() if k.startswith("h:__pdo"))
row("builtin work: PDO / SQLite", pdo, pdo_n, None, "PHP side not measured (same SQLite library)")
rest_b = b_ms - overhead_ms - (uns[0] - uns[1] * FERRO_BUILTIN_OVERHEAD / 1e6) - (fe[0] - fe[1] * FERRO_BUILTIN_OVERHEAD / 1e6) - pdo
row("builtin work: all other builtins", rest_b, b_calls - uns[1] - fe[1] - pdo_n, None, "PHP side not measured")

# ---- properties ----
pg, pgn = take(["ThisPropGet", "PropGetSlot", "PropGetSilent", "PropGet", "PropGetSlotRecv", "PropGetDynamic",
                "StaticPropGet", "StaticPropGetDynamic", "PropDimGetConst"])
row("property reads", pg, pgn, pgn * PHP["prop_get"] / 1e6)
ps, psn = take(["PropSetPop", "FieldAssign", "PropSet", "StaticPropSet", "BinaryTCPropSetPop", "PropIncDec"])
row("property writes", ps, psn, psn * PHP["prop_set"] / 1e6)
pi, pin = take(["FieldIsset", "PropIssetFetchGate", "PropIsset", "FieldEmpty"])
row("property isset/empty", pi, pin, pin * PHP["prop_chain_isset"] / 1e6, "priced as isset($o->a->b)")
pu, pun = take(["FieldUnset", "PropUnset"])
row("property unset", pu, pun, pun * PHP["prop_unset"] / 1e6)

# ---- arrays ----
ar, arn = take(["FetchDim", "CoalesceFetchDim", "FetchDimList"])
row("array reads", ar, arn, arn * PHP["dim"] / 1e6)
aw, awn = take(["ArrayInsert", "ArrayInit", "ArrayPush", "ArrayAppendSpread"])
row("array writes / literals", aw, awn, awn * PHP["dim_write"] / 1e6)
fe_ms, fen = take(["IterNext", "IterInit", "IterPop", "IterNextRef", "IterInitRef"])
row("foreach", fe_ms, fen, fen * PHP["foreach_elem"] / 1e6)
pa, pan = take(["AssignPath", "IssetPath", "EmptyPath", "UnsetPath", "AssignOpPath"])
row("nested paths ($a[..][..], $o->a[..])", pa, pan, pan * PHP["path"] / 1e6)

# ---- strings ----
st, stn = take(["ConcatN", "ConcatNConst", "Stringify", "StringifySlot", "ConcatAssignSlot"])
row("string concat / interpolation", st, stn, stn * PHP["concat"] / 1e6)

# ---- includes and declarations ----
inc, incn = take(["Include", "DeclareDeferred", "DeclareFn", "DefineConst"])
row("include / class declaration", inc, incn, None, "PHP side not measured (opcache include + early binding)")

# ---- everything else: loads, stores, constants, jumps, compares, arithmetic ----
rest_ms = sum(v[0] for v in ops.values())
rest_n = sum(v[1] for v in ops.values())
row("everything else (loads, stores, jumps, compares, objects, ...)", rest_ms, rest_n, rest_n * PHP["simple_op"] / 1e6,
    "priced at 1 ns per op")

tot_f = sum(r[1] for r in rows)
known = [r for r in rows if r[3] is not None]
print(f"{'category':58} {'ferro ms':>9} {'count':>8} {'php ms':>8} {'gap ms':>8}  note")
for name, fm_, n, pm, note in sorted(rows, key=lambda r: -(r[1] - (r[3] or 0))):
    gap = f"{fm_ - pm:8.2f}" if pm is not None else "       ?"
    pms = f"{pm:8.2f}" if pm is not None else "       ?"
    print(f"{name:58} {fm_:9.2f} {n:8d} {pms} {gap}  {note}")
print(f"{'total (census, whole request)':58} {tot_f:9.2f}")
print(f"gap explained by priced categories: {sum(r[1] - r[3] for r in known):.2f} ms")
