#!/usr/bin/env python3
"""Compile trial of render3d's shader permutations with vkd3d-compiler, for the graphics-backend research.

Input: the folder `dump_permutations` wrote (one include-expanded <n>.hlsl and one <n>.json per permutation).
vkd3d-compiler has no -D option, so each define becomes a `#define K V` line ahead of the source, written to
<out>/src/<n>.hlsl. An error's line number is therefore the dump file's plus the number of defines (vkd3d 2.1 ignores
`#line`). Each compile is one process, run serially, timed from start to exit. Writes <out>/results.csv, the
compiler's output per compile in <out>/err, and prints pass counts and the distinct errors.

Python 3.10 or later, on the machine that has vkd3d-compiler (Linux).

  r3d-vkd3d-trial.py --dump <dir> --out <dir> --cmd "<vkd3d-compiler command>" [--targets dxbc-tpf,spirv-binary]
"""
import argparse, csv, json, os, re, shlex, statistics, subprocess, sys, time

ap = argparse.ArgumentParser()
ap.add_argument("--dump", required=True)
ap.add_argument("--out", required=True)
ap.add_argument("--cmd", required=True)
ap.add_argument("--targets", default="dxbc-tpf")
ap.add_argument("--timeout", type=int, default=180)
ap.add_argument("--only", default="")
a = ap.parse_args()
cmd = shlex.split(a.cmd)
for d in ("src", "bin", "err"):
    os.makedirs(os.path.join(a.out, d), exist_ok=True)


def norm(line):
    """The error without where it happened."""
    i = line.lower().find("error")
    s = line[i:] if i >= 0 else line
    s = re.sub(r"[\w./\\-]+\.hlsl:\d+:\d+:?", "", s)
    return re.sub(r"\s+", " ", s).strip()


def run(argv):
    t = time.perf_counter()
    try:
        p = subprocess.run(argv, capture_output=True, timeout=a.timeout)
        ok, text = p.returncode == 0, (p.stdout + b"\n" + p.stderr).decode("utf-8", "replace")
        if p.returncode < 0:
            text += f"\nerror: killed by signal {-p.returncode}"
    except subprocess.TimeoutExpired:
        ok, text = False, f"error: timeout after {a.timeout} s"
    return ok, (time.perf_counter() - t) * 1000, text


ids = sorted(int(f[:-5]) for f in os.listdir(a.dump) if f.endswith(".json"))
if a.only:
    ids = [i for i in ids if str(i) in a.only.split(",")]
rows = []
for n, i in enumerate(ids, 1):
    j = json.load(open(os.path.join(a.dump, f"{i}.json"), encoding="utf-8-sig"))
    raw = open(os.path.join(a.dump, f"{i}.hlsl"), "rb").read()
    head = ""
    for d in j["defines"]:
        k, _, v = d.partition("=")
        head += f"#define {k} {v if v else '1'}\n"
    src = os.path.join(a.out, "src", f"{i}.hlsl")
    with open(src, "wb") as f:
        f.write(head.encode() + raw)
    stat = []
    for t in a.targets.split(","):
        out = os.path.join(a.out, "bin", f"{i}.{t}")
        ok, ms, text = run(cmd + ["-x", "hlsl", "-p", j["target"], "-e", j["entry"], "-b", t, "-o", out, src])
        errs = [l.strip() for l in text.splitlines() if re.search(r"\berror\b", l, re.I)]
        if not ok and not errs:
            errs = [l.strip() for l in text.splitlines() if l.strip()][:1] or ["(no output)"]
        open(os.path.join(a.out, "err", f"{i}.{t}.txt"), "w", encoding="utf-8").write(text)
        rows.append({
            "id": i, "rel": j["rel"], "target": j["target"], "entry": j["entry"],
            "groups": " ".join(j.get("groups", [])), "compiler": f"vkd3d-{t}", "ok": ok, "ms": round(ms, 1),
            "bytes": os.path.getsize(out) if ok and os.path.exists(out) else 0,
            "defines": len(j["defines"]),
            "error": "" if ok else errs[0], "error_norm": "" if ok else norm(errs[0]),
            "errors_all": "" if ok else " || ".join(sorted({norm(e) for e in errs})),
            "warnings": len([l for l in text.splitlines() if re.search(r"\bwarning\b", l, re.I)]),
        })
        stat.append(f"{t}={'ok' if ok else 'FAIL'}")
    print(f"[{n}/{len(ids)}] {i} {j['rel']} {j['target']}: {' '.join(stat)}", flush=True)

with open(os.path.join(a.out, "results.csv"), "w", newline="", encoding="utf-8") as f:
    w = csv.DictWriter(f, fieldnames=list(rows[0].keys()))
    w.writeheader()
    w.writerows(rows)

print()
for c in sorted({r["compiler"] for r in rows}):
    cr = [r for r in rows if r["compiler"] == c]
    ok = [r for r in cr if r["ok"]]
    print(f"{c}: {len(ok)} of {len(cr)} ok; total {sum(r['ms'] for r in cr):.0f} ms; "
          f"median ok {statistics.median([r['ms'] for r in ok]) if ok else 0:.1f} ms")
    for t in sorted({r["target"] for r in cr}):
        tr = [r for r in cr if r["target"] == t]
        print(f"  {t}: {len([r for r in tr if r['ok']])} of {len(tr)}")
    first, every = {}, {}
    for r in cr:
        if r["ok"]:
            continue
        first.setdefault(r["error_norm"], []).append(r["id"])
        for e in r["errors_all"].split(" || "):
            every.setdefault(e, []).append(r["id"])
    print("  first errors:")
    for e, v in sorted(first.items(), key=lambda kv: -len(kv[1])):
        print(f"    {len(v):3} x {e}   (e.g. id {v[0]})")
    print("  every error (permutations reporting it):")
    for e, v in sorted(every.items(), key=lambda kv: -len(kv[1])):
        print(f"    {len(v):3} x {e}")
