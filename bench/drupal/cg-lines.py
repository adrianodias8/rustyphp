import sys, re, collections, subprocess
# per-line self Ir (via addr2line inline chains, outermost frame in FILE) of functions matching FN
sys.argv=[sys.argv[0]]+sys.argv[1:]
base0, run1, m, fnre, binary, fsub = sys.argv[1], sys.argv[2], int(sys.argv[3]), re.compile(sys.argv[4]), sys.argv[5], sys.argv[6]
g={"__file__":"/work/php-rust/bench/drupal/cg-handlers.py"}
src=open("/work/php-rust/bench/drupal/cg-handlers.py").read().replace("\nmain()\n","\n")
exec(compile(src,"h","exec"),g)
s0,_,_=g["parse"](base0); s1,_,t=g["parse"](run1)
base=g["load_base"](binary,t)
d=collections.Counter()
for (ob,fn,a),c in s1.items():
    if fn and fnre.search(fn): d[a]+=c-s0.get((ob,fn,a),0)
addrs=sorted(d)
inp="".join(f"0x{a-base:x}\n" for a in addrs)
out=subprocess.run([g["A2L"],"-e",binary,"-i","-a"],input=inp,capture_output=True,text=True).stdout
res={};cur=None
for l in out.splitlines():
    l=l.strip()
    if l.startswith("0x"): cur=int(l,16)+base; res[cur]=[]
    elif ":" in l: res[cur].append(l.split(" (disc")[0])
agg=collections.Counter()
for a,c in d.items():
    ch=res.get(a,[]); hit=None
    for fr in ch:
        if fsub in fr: hit=fr
    agg[(hit or (ch[-1] if ch else "?")).split("/")[-1]]+=c
tot=sum(agg.values())
print(f"total {tot/m:,.0f}")
for k,v in agg.most_common(25): print(f"{v/m:10,.0f} {k}")
