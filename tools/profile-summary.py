r"""Engine profiling summariser (plan 2026-11-21-engine-profile, Task 5). Standard library only.

  py -3.13 -I -X utf8 tools\profile-summary.py --accept <list> --runroot <RunRoot> --out <new folder>
  py -3.13 -I -X utf8 tools\profile-summary.py --check <attempt folder> --runroot <RunRoot> [--reference <attempt folder>]
  py -3.13 -I -X utf8 tools\profile-summary.py --mipq <json>... --out <new folder>
  py -3.13 -I -X utf8 tools\profile-summary.py --self-test

--accept: the list names accepted attempt folders (names inside RunRoot), one per line, grouped under
`[<label> <cohort>]` headers; one `reference <attempt>` line per label (under any header of that label). An
attempt's meta.json label and cohort must equal its header's, except on an explicit `relabel <attempt> from
<recorded cohort>` line (EP-C9, review I5): only an `overhead` attempt of the `release` build may be listed
under a `warm-fresh` header. Each relabel is printed and recorded in summary.json and summary.md. Every
listed attempt, and every reference, is checked (spec 7.3, EP-R4) before anything is written; a failed check is
exit 1 with the reason. A cold-fresh attempt whose meta.json cold verdict is false is refused (EP-C5), and so is
a per-file eviction attempt without a true verdict. Output (all created new): summary.json, summary.md, timeline-<label>-<cohort>.csv,
cpu-<label>.svg. --allow-contaminated is for fixture checks only: it keeps the runner's contamination verdict
out of the acceptance check and marks the summary NOT ACCEPTANCE EVIDENCE.

--check: runs the acceptance checks on one attempt (exit 0 or 1). Without --reference the attempt is checked
against the expected set alone (a reference is checked this way). expected-<label>.txt is read from RunRoot.

--mipq: aggregates quality reports (spec 9.3). Scored records are read as
  {"output","subtype","format","levels","per_level":[{"level","texels","mse":{<name>: <8-bit-code MSE>},
    "edge":{"band_texels","band_mse","interior_texels","interior_mse"}?}]}
and skipped ones as {"output","subtype","format","levels","why"}.  # shortcut: the scored-record fields are
this script's contract for Task 7's writer (the plan fixes only the skip fields); upgrade when Task 7 lands.

EP-R3: nothing here deletes, empties, moves or overwrites a file or folder. --out is created without
exist_ok and every file is opened with mode "x". The DDS files of an attempt are never read (the controller
may have moved them, EP-R5): only hashes.csv, _sdc/manifest.jsonl, meta.json, the report and the sampler.

Clocks: the recorder's timeline and sampler.csv's t_ms share no zero. The CPU buckets assume both start
together (shortcut: the offset is tens of ms; the unrecorded-CPU column is informational only, spec 4.2).
"""
import argparse
import contextlib
import csv
import fnmatch
import io
import json
import math
import os
import re
import statistics
import sys
import tempfile
import time
from fractions import Fraction

NS = 1_000_000_000
GAP_NS = 50_000  # spec 6.4: coalescing joins spans less than 50 us apart
BUCKET_MS = 250
RECORDER_BUILDS = ("release", "profiling")
MANIFEST_IGNORED = {"ts", "output_stamp"}
STALE_COHORTS = ("warm-stale",)
FRESH_LIKE = ("warm-fresh", "cold-fresh", "overhead", "parity")
# EP-C9 (review I5): the only relabels --accept takes, (recorded cohort, listed cohort) -> allowed builds.
RELABELS = {("overhead", "warm-fresh"): ("release",)}
EVICT_METHOD = "per-file-no-buffering"
ENCODE_STAGE = "encode_band"

# The stage table of spec section 5 (53 stages), used only to build synthetic reports in the self-test; real
# reports carry their own `stages`.
WAITS = {"plan_admit_wait", "run_admit_wait", "run_classify", "scope_join", "row_join", "events_wait",
         "manifest_wait", "write"}
STAGE_NAMES = (
    "plan plan_prepare plan_discover plan_file plan_load plan_analyse plan_resolve plan_pack plan_fill plan_derive "
    "plan_admit_estimate plan_admit_wait pool_build dispatch scope_join run run_admit_estimate run_admit_wait "
    "run_classify currency currency_hash file stamp_check hash_source bake_load decode pack_assemble derive_assemble "
    "fill cube_prepare preprocess dilate srgb_decode resize resize_rows mips filter_rows row_join coverage "
    "detail_check quantize encode encode_gather encode_pack encode_band events_wait events dds_assemble validate "
    "manifest_record write manifest_wait manifest_append").split()


# ---------------------------------------------------------------- interval helpers

def merge(ivs):
    out = []
    for a, b in sorted(i for i in ivs if i[1] > i[0]):
        if out and a <= out[-1][1]:
            out[-1] = (out[-1][0], max(out[-1][1], b))
        else:
            out.append((a, b))
    return out


def total(ivs):
    return sum(b - a for a, b in ivs)


def intersect(xs, ys):
    """Both lists merged."""
    out, i, j = [], 0, 0
    while i < len(xs) and j < len(ys):
        a, b = max(xs[i][0], ys[j][0]), min(xs[i][1], ys[j][1])
        if b > a:
            out.append((a, b))
        if xs[i][1] < ys[j][1]:
            i += 1
        else:
            j += 1
    return out


def subtract(ivs, cuts):
    cuts = merge(cuts)
    out = []
    for a, b in ivs:
        cur = a
        for c, d in cuts:
            if d <= cur or c >= b:
                continue
            if c > cur:
                out.append((cur, c))
            cur = max(cur, d)
        if cur < b:
            out.append((cur, b))
    return out


# ---------------------------------------------------------------- the time accounting (spec 4)

def span_list(report):
    out = []
    for i, (stage, ident, thread, start, end, busy, pieces) in enumerate(report["spans"]):
        out.append({"i": i, "stage": stage, "id": ident, "thread": thread, "start": start, "end": end,
                    "busy": busy, "pieces": pieces})
    return out


def interval(s):
    """A coalesced span is placed at its end, since its gaps are not kept (spec 6.4)."""
    return (s["end"] - s["busy"], s["end"]) if s["pieces"] > 1 else (s["start"], s["end"])


def nest(spans):
    """Exclusive busy ns per span (busy minus direct same-thread children) and the child map. Spans are pushed
    when they close, so of two identical intervals the later one is the outer."""
    by_i = {s["i"]: s for s in spans}
    by_thread = {}
    for s in spans:
        by_thread.setdefault(s["thread"], []).append(s)
    children = {s["i"]: [] for s in spans}
    for lst in by_thread.values():
        lst.sort(key=lambda s: (s["start"], -s["end"], -s["i"]))
        stack = []
        for s in lst:
            while stack and not (stack[-1]["start"] <= s["start"] and s["end"] <= stack[-1]["end"]):
                stack.pop()
            if stack:
                children[stack[-1]["i"]].append(s["i"])
            stack.append(s)
    excl = {i: max(0, s["busy"] - sum(by_i[c]["busy"] for c in children[i])) for i, s in by_i.items()}
    return excl, children


def analyse(report, command_wall_ns=None):
    """command_wall_ns is the runner's external wall (meta.json); the recorder's own wall is kept as a second figure."""
    stages = report["stages"]
    names = [s["name"] for s in stages]
    is_wait = [s["class"] == "wait" for s in stages]
    spans = span_list(report)
    by_i = {s["i"]: s for s in spans}
    excl, children = nest(spans)
    calls = sorted(report.get("calls", []), key=lambda c: c["start_ns"])

    def call_of(t):
        """The one call a span start belongs to: the last call starting at or before t, when t is not past its end."""
        hit = None
        for c in calls:
            if c["start_ns"] <= t:
                hit = c
            else:
                break
        return hit if hit is not None and t <= hit["end_ns"] else None

    per_acc = {c["call"]: [0, 0] for c in calls}  # work ns, encode ns per call

    work_ns, wait_ns, wait_threads = {}, {}, {}
    by_kind = {}
    cap_num = {}
    union_ivs, union_bound = {}, {}
    work_own = []
    enc_ivs, other_ivs, admit_ivs = [], [], []
    split_bound = 0
    for s in spans:
        name = names[s["stage"]]
        ns = excl[s["i"]]
        c = call_of(s["start"])
        kind = c["kind"] if c else "none"
        if is_wait[s["stage"]]:
            wait_ns[name] = wait_ns.get(name, 0) + ns
            wait_threads.setdefault(name, set()).add(s["thread"])
        else:
            work_ns[name] = work_ns.get(name, 0) + ns
            if c:
                cap_num[name] = cap_num.get(name, 0) + ns
                per_acc[c["call"]][0] += ns
                if name == ENCODE_STAGE:
                    per_acc[c["call"]][1] += ns
        by_kind.setdefault(kind, {})
        by_kind[kind][name] = by_kind[kind].get(name, 0) + ns
        union_ivs.setdefault(name, []).append(interval(s))
        if s["pieces"] > 1:
            union_bound[name] = union_bound.get(name, 0) + (s["pieces"] - 1) * GAP_NS
            split_bound += (s["pieces"] - 1) * GAP_NS
        if name in ("plan_admit_wait", "run_admit_wait"):
            admit_ivs.append(interval(s))
        # own time: the interval minus the direct children's intervals
        own = subtract([interval(s)], [interval(by_i[ch]) for ch in children[s["i"]]])
        if not is_wait[s["stage"]]:
            work_own.extend(own)
            (enc_ivs if name == ENCODE_STAGE else other_ivs).extend(own)
    total_work = sum(work_ns.values())
    encode = work_ns.get(ENCODE_STAGE, 0)
    recorder_wall = report.get("process", {}).get("wall_ns") or max((s["end"] for s in spans), default=0)
    wall = command_wall_ns or recorder_wall
    enc_m, oth_m = merge(enc_ivs), merge(other_ivs)
    mixed = total(intersect(enc_m, oth_m))
    cap_den = sum(c["pool_threads"] * (c["end_ns"] - c["start_ns"]) for c in calls)
    per_call = []
    for c in calls:
        w, e = per_acc[c["call"]]
        den = c["pool_threads"] * (c["end_ns"] - c["start_ns"])
        per_call.append({"call": c["call"], "kind": c["kind"], "pool_threads": c["pool_threads"],
                         "wall_ns": c["end_ns"] - c["start_ns"], "work_ns": w, "encode_ns": e,
                         "work_share_of_capacity": Fraction(w, den) if den else None,
                         "budget_limit": c.get("budget_limit"), "budget_peak": c.get("budget_peak")})
    return {
        "work_ns": work_ns, "wait_ns": wait_ns,
        "wait_threads": {k: len(v) for k, v in wait_threads.items()},
        "total_work_ns": total_work, "encode_ns": encode,
        "encode_share": Fraction(encode, total_work) if total_work else None,
        # EP-R1, integers: stop when 10 x encode > 9 x work
        "stop": total_work > 0 and 10 * encode > 9 * total_work,
        "capacity": {k: Fraction(v, cap_den) for k, v in cap_num.items()} if cap_den else {},
        "union_ns": {k: total(merge(v)) for k, v in union_ivs.items()}, "union_bound_ns": union_bound,
        "split": {"wall_ns": wall, "recorder_wall_ns": recorder_wall, "encode_only_ns": total(enc_m) - mixed, "mixed_ns": mixed,
                  "no_encode_ns": wall - total(enc_m), "bound_ns": split_bound},
        "by_kind": by_kind, "per_call": per_call, "work_own": work_own, "admit_ivs": merge(admit_ivs),
        "wall_ns": wall, "recorder_wall_ns": recorder_wall,
    }


def cpu_buckets(an, sampler, logical):
    """Per 250 ms bucket (spec 4.2): cores used (sampler), idle cores, recorded work threads, unrecorded CPU.
    One pass: the sampler is walked with a moving index and every work interval is added to the buckets it spans."""
    if len(sampler) < 2:
        return []
    ts = [r["t_ms"] for r in sampler]
    cpu = [r["cpu_ms"] for r in sampler]
    n = int((ts[-1] + 1e-9) // BUCKET_MS)
    if n <= 0:
        return []
    k = [1]

    def cum(t):  # t only grows between calls
        if t <= ts[0]:
            return cpu[0]
        if t >= ts[-1]:
            return cpu[-1]
        while ts[k[0]] < t:
            k[0] += 1
        j = k[0]
        f = (t - ts[j - 1]) / (ts[j] - ts[j - 1]) if ts[j] > ts[j - 1] else 1.0
        return cpu[j - 1] + f * (cpu[j] - cpu[j - 1])

    width = BUCKET_MS * 1_000_000
    rec = [0] * n
    for x, y in an["work_own"]:
        if y <= x:
            continue
        for i in range(max(0, x // width), min(n - 1, (y - 1) // width) + 1):
            rec[i] += min(y, (i + 1) * width) - max(x, i * width)
    adm = [False] * n
    for x, y in an["admit_ivs"]:
        for i in range(max(0, x // width), min(n - 1, (y - 1) // width) + 1):
            adm[i] = True
    out = []
    prev = cum(0)
    for i in range(n):
        nxt = cum((i + 1) * BUCKET_MS)
        used = (nxt - prev) / BUCKET_MS
        prev = nxt
        recorded = rec[i] / width
        out.append({"t_ms": i * BUCKET_MS, "cores_used": used, "idle_cores": logical - used, "recorded": recorded,
                    "unrecorded": used - recorded, "too_many": recorded > logical,
                    "idle_while_admitting": adm[i] and recorded < logical / 2})
    return out

# ---------------------------------------------------------------- statistics (spec 7.3)

def stats(vals):
    v = sorted(vals)
    d = {"n": len(v), "min": v[0], "median": statistics.median(v), "max": v[-1]}
    if len(v) >= 5:  # five repeats: inclusive quartiles; three give min, median, max only
        q = statistics.quantiles([float(x) for x in v], n=4, method="inclusive")
        d["q1"], d["q3"] = q[0], q[2]
    return d


def overhead_outcome(diffs_pct):
    """Spec 6.5. Each entry is one round's paired off-minus-base difference as a percent of base."""
    if all(d <= 1.0 for d in diffs_pct):
        return "within 1 %"
    if all(d > 1.0 for d in diffs_pct):
        return "over 1 %"
    return "inconclusive"


def overhead_rounds(attempts):
    """attempts in run order; a round is one base, one release-off and one release (on) attempt."""
    rounds = []
    for k in range(0, len(attempts), 3):
        chunk = attempts[k:k + 3]
        by = {a["meta"].get("build"): a["meta"].get("externalWallMs") for a in chunk}
        if len(chunk) != 3 or set(by) != {"base", "release-off", "release"}:
            raise ValueError("overhead round %d does not hold one base, one release-off and one release attempt"
                             % (k // 3 + 1))
        base, off, on = by["base"], by["release-off"], by["release"]
        rounds.append({"base_ms": base, "off_ms": off, "on_ms": on,
                       "off_minus_base_pct": 100.0 * (off - base) / base,
                       "on_minus_off_pct": 100.0 * (on - off) / base})
    return {"rounds": rounds, "outcome": overhead_outcome([r["off_minus_base_pct"] for r in rounds]) if rounds else "none"}


# ---------------------------------------------------------------- protected paths (spec 7.8)

PATHS_PS1 = os.path.join(os.path.dirname(os.path.abspath(__file__)), "profile-paths.ps1")


def protected_list(ps1=None):
    """The one definition lives in tools/profile-paths.ps1 (Get-ProtectedList); it is parsed, never copied, so the
    Python and PowerShell tools cannot drift. Entries: quoted paths, and `(Join-Path $env:NAME 'sub')`."""
    with open(ps1 or PATHS_PS1, "r", encoding="utf-8") as f:
        text = f.read()
    m = re.search(r"function Get-ProtectedList \{(.*?)\n\}", text, re.S)
    if not m:
        raise ValueError("Get-ProtectedList not found in %s" % ps1)
    out = []
    for ln in m.group(1).splitlines():
        ln = ln.strip().rstrip(",")
        j = re.match(r"\(Join-Path \$env:(\w+) '([^']+)'\)$", ln)
        if j:
            base = os.environ.get(j.group(1))
            if not base:
                raise ValueError("%s is not set, so the protected path %s\\%s cannot be resolved" % (j.group(1), j.group(1), j.group(2)))
            out.append(os.path.join(base, j.group(2)))
        else:
            out.extend(re.findall(r"'([^']+)'", ln))
    if not out:
        raise ValueError("Get-ProtectedList in %s holds no path" % ps1)
    return out


def _parts(p):
    """Resolved (junctions followed where the path exists) whole components, case-folded."""
    r = os.path.realpath(os.path.abspath(p))
    drive, rest = os.path.splitdrive(r)
    return [drive.casefold()] + [x.casefold() for x in re.split(r"[\\/]+", rest) if x]


def _contained(child, parent):
    c, p = _parts(child), _parts(parent)
    return len(p) <= len(c) and c[:len(p)] == p


def out_problem(path, protected=None):
    """Why an output path is refused (spec 7.8, as Get-RunRootProblem does), or None."""
    if path.startswith("\\\\") or path.startswith("//"):
        return "is a UNC path (refused)"
    for p in (protected if protected is not None else protected_list()):
        if _contained(path, p) or _contained(p, path):
            return "overlaps the protected path %s" % p
    comp = _parts(path)
    if len(comp) > 3 and comp[:3] == ["f:", "ai", "dds"] and fnmatch.fnmatch(comp[3], "converter-nif-*"):
        return "is inside a converter-nif-* checkout"
    cur = os.path.realpath(os.path.abspath(path))
    while True:
        for name in ("sdc.toml", "sdc-project.toml"):
            if os.path.exists(os.path.join(cur, name)):
                return "is inside %s, which holds %s (an owner project)" % (cur, name)
        up = os.path.dirname(cur)
        if up == cur:
            return None
        cur = up


def check_out(path):
    why = out_problem(path)
    if why:
        raise ValueError("--out %s %s" % (path, why))


# ---------------------------------------------------------------- loading (disk to dicts)

def read_text(path):
    with open(path, "r", encoding="utf-8") as f:
        return f.read()


def read_json(path):
    with open(path, "r", encoding="utf-8-sig") as f:
        return json.load(f)


def lines_of(text):
    return [ln.strip() for ln in text.splitlines() if ln.strip()]


def parse_expected(text):
    return set(lines_of(text))


def parse_planned_not_written(text):
    """key, state, status json (tab separated, spec 7.3 and EP-T4-2 b)."""
    return [ln.split("\t") for ln in lines_of(text)]


def parse_skipped_inputs(text):
    return [ln.split("\t") for ln in lines_of(text)]


def parse_manifest(text):
    recs = {}
    for ln in text.splitlines():
        try:
            r = json.loads(ln)
        except ValueError:
            continue
        if isinstance(r, dict) and "output" in r:
            recs[r["output"]] = r  # the last line per key wins
    return recs


def parse_hashes(text):
    out = {}
    for ln in text.splitlines()[1:]:
        if ln.strip():
            k, _, h = ln.rpartition(",")
            out[k] = h
    return out


def load_sampler(path):
    rows = []
    with open(path, "r", encoding="utf-8", newline="") as f:
        for r in csv.DictReader(f):
            rows.append({"t_ms": float(r["t_ms"]), "cpu_ms": float(r["cpu_ms"]), "working_set": int(r["working_set"]),
                         "private_bytes": int(r["private_bytes"]), "threads": int(r["threads"])})
    return rows


def try_read(path):
    return read_text(path) if os.path.isfile(path) else None


def load_attempt(folder):
    att = {"dir": folder, "name": os.path.basename(folder.rstrip("\\/"))}
    mp = os.path.join(folder, "meta.json")
    att["meta"] = read_json(mp) if os.path.isfile(mp) else {}
    h = try_read(os.path.join(folder, "hashes.csv"))
    att["hashes"] = parse_hashes(h) if h is not None else {}
    m = try_read(os.path.join(folder, "out", "_sdc", "manifest.jsonl"))
    att["manifest"] = parse_manifest(m) if m is not None else None
    prof = os.path.join(folder, "profile")
    reps = sorted(f for f in os.listdir(prof) if f.startswith("profile-") and f.endswith(".json")) \
        if os.path.isdir(prof) else []
    att["report"] = read_json(os.path.join(prof, reps[0])) if len(reps) == 1 else None
    att["report_files"] = len(reps)
    att["files_before"] = try_read(os.path.join(folder, "files-before.csv"))
    att["files_after"] = try_read(os.path.join(folder, "files-after.csv"))
    sp = os.path.join(folder, "sampler.csv")
    att["sampler"] = load_sampler(sp) if os.path.isfile(sp) else []
    return att


def load_copied_manifest(runroot, att):
    """The manifest of the attempt a stale or noop copy was made from (its records before the run)."""
    src = (att["meta"].get("copy") or {}).get("from")
    if not src:
        return None
    m = try_read(os.path.join(runroot, src, "out", "_sdc", "manifest.jsonl"))
    return parse_manifest(m) if m is not None else None


# ---------------------------------------------------------------- acceptance (spec 7.3, EP-R4)

def head(items, n=3):
    items = list(items)
    s = ", ".join(items[:n])
    return s + (" (+%d more)" % (len(items) - n) if len(items) > n else "")


def stale_span_problems(att, expected, copied):
    """Spec 7.3 warm-stale: for each expected job whose copied record passed validation, one currency_hash span
    and one file span holding a hash_source span (EP-T4-2 keeps the job id = the run-side id)."""
    rep = att["report"]
    if rep is None:
        return ["no profile report to check the stale re-hash spans against"]
    if copied is None:
        return ["the copy source's manifest is missing, so the eligible jobs cannot be listed"]
    names = [s["name"] for s in rep["stages"]]
    idx = {n: i for i, n in enumerate(names)}
    ids = {}
    for j in rep.get("jobs", []):
        ids.setdefault(j["output"], set()).add(j["id"])
    by_id = {}
    for s in span_list(rep):  # grouped once: a scan per expected key would be quadratic
        if s["stage"] in (idx["currency_hash"], idx["file"], idx["hash_source"]):
            by_id.setdefault(s["id"], []).append(s)
    probs = []
    for key in sorted(expected):
        rec = copied.get(key)
        if not rec or (rec.get("validation") or {}).get("passed") is not True:
            continue
        jid = ids.get(key)
        if not jid:
            probs.append("%s: no job in the report" % key)
            continue
        mine = [s for i in jid for s in by_id.get(i, [])]
        ch = [s for s in mine if s["stage"] == idx["currency_hash"]]
        files = [s for s in mine if s["stage"] == idx["file"]]
        hs = [s for s in mine if s["stage"] == idx["hash_source"]]
        holding = [f for f in files if any(h["thread"] == f["thread"] and f["start"] <= h["start"]
                                           and h["end"] <= f["end"] for h in hs)]
        if len(ch) != 1:
            probs.append("%s: %d currency_hash span(s), want 1" % (key, len(ch)))
        if len(holding) != 1:
            probs.append("%s: %d file span(s) holding a hash_source span, want 1" % (key, len(holding)))
    return probs


def check_attempt(att, expected, ref=None, copied=None, allow_contaminated=False):
    """Every reason this attempt is refused (an empty list accepts it). `ref` is another loaded attempt."""
    p = []
    m = att["meta"]
    cohort = m.get("cohort")
    if not m:
        return ["no meta.json (the attempt did not get as far as writing one)"]
    if cohort == "warmup" or m.get("calibrate"):
        p.append("a warmup or calibration attempt is never accepted")
    if m.get("finished") is not True:
        p.append("the attempt did not finish" + (": " + m["error"] if m.get("error") else ""))
    elif m.get("exitCode") != 0:
        p.append("exit code %s, not 0" % m.get("exitCode"))
    if not isinstance(m.get("externalWallMs"), (int, float)) or isinstance(m.get("externalWallMs"), bool):
        p.append("meta.json has no numeric externalWallMs (the command wall)")
    cont = m.get("contention")
    if not isinstance(cont, dict) or "contaminated" not in cont:
        if not allow_contaminated:
            p.append("no contention verdict in meta.json")
    elif cont["contaminated"] and not allow_contaminated:
        p.append("contaminated: " + "; ".join(cont.get("reasons") or ["no reason recorded"]))
    if cohort == "cold-fresh" and not m.get("purgeStamp"):
        p.append("a cold attempt must name the purge stamp it consumed (purgeStamp is empty)")
    if cohort == "cold-fresh":
        # EP-C5: the runner's cold verdict (a dict with "cold"); a purge stamp has none and needs none
        cv = m.get("cold")
        flag = cv.get("cold") if isinstance(cv, dict) else cv
        if flag is False:
            p.append("not cold (%s): %s" % (m.get("coldMethod") or "method not recorded",
                                            (cv.get("reason") if isinstance(cv, dict) else None) or "the runner's cold verdict is false"))
        elif m.get("coldMethod") == EVICT_METHOD and flag is not True:
            p.append("a per-file eviction attempt has no cold verdict in meta.json (cold.cold is not true)")
    if m.get("build") in RECORDER_BUILDS and att["report"] is None:
        p.append("no single profile report under profile\\ (%d found)" % att.get("report_files", 0))
    # the key set, both ways (EP-R4): never a count
    written = set(att["hashes"])
    missing, extra = sorted(expected - written), sorted(written - expected)
    if missing:
        p.append("%d expected key(s) not written: %s" % (len(missing), head(missing)))
    if extra:
        p.append("%d unexpected key(s) written: %s" % (len(extra), head(extra)))
    if ref is not None:
        rh = ref["hashes"]
        bad = sorted(k for k in expected & written if att["hashes"][k] != rh.get(k))
        if bad:
            p.append("%d hash(es) differ from the reference: %s" % (len(bad), head(bad)))
        if att["manifest"] is None:
            p.append("no _sdc/manifest.jsonl to read validation from")
        elif ref["manifest"] is None:
            p.append("the reference has no _sdc/manifest.jsonl")
        else:
            nov, vbad, mbad = [], [], []
            cross = (m.get("exeSha256") or "") != (ref["meta"].get("exeSha256") or "")
            for k in sorted(expected):
                a, r = att["manifest"].get(k), ref["manifest"].get(k)
                if a is None or r is None:
                    nov.append(k)
                    continue
                if a.get("validation") != r.get("validation"):
                    vbad.append(k)
                if cohort in FRESH_LIKE:
                    ignored = set(MANIFEST_IGNORED) | ({"engine_commit"} if cross else set())
                    fields = sorted(f for f in set(a) | set(r) if f not in ignored and a.get(f) != r.get(f))
                    if fields:
                        mbad.append("%s [%s]" % (k, ", ".join(fields)))
            if nov:
                p.append("%d expected key(s) have no manifest record (here or in the reference): %s" % (len(nov), head(nov)))
            if vbad:
                p.append("%d validation object(s) differ from the reference: %s" % (len(vbad), head(vbad)))
            if mbad:
                p.append("%d manifest record(s) differ from the reference in: %s" % (len(mbad), head(mbad)))
    if cohort in STALE_COHORTS:
        p.extend(stale_span_problems(att, expected, copied))
    if cohort == "warm-noop":
        if att["files_before"] is None or att["files_after"] is None:
            p.append("files-before.csv or files-after.csv is missing")
        elif att["files_before"] != att["files_after"]:
            p.append("files-after.csv differs from files-before.csv (the noop changed its copy)")
    return p


# ---------------------------------------------------------------- per-attempt figures

def disk_figures(meta):
    d = meta.get("disk")
    if not d:
        return None
    out = {}
    try:
        for which in ("corpus", "output"):
            s, e = d["start"][which], d["end"][which]
            out[which] = {"disk": e.get("name"), "read_bytes": e["read"] - s["read"], "write_bytes": e["write"] - s["write"]}
    except (KeyError, TypeError):
        return {"unreadable": "meta.json disk counters are incomplete"}
    return out


def attempt_figures(att, runroot):
    m = att["meta"]
    fig = {"attempt": att["name"], "build": m.get("build"), "cohort": m.get("cohort"),
           "wall_ms": m.get("externalWallMs"), "disk": disk_figures(m), "purgeStamp": m.get("purgeStamp"),
           "coldMethod": m.get("coldMethod"), "cold": m.get("cold"), "validate": m.get("validate") is True,
           "contaminated": (m.get("contention") or {}).get("contaminated"), "quiet": m.get("quiet")}
    if m.get("purgeStamp") and runroot:
        t = try_read(os.path.join(runroot, m["purgeStamp"]))
        if t:
            fig["purge"] = json.loads(t)
    rep = att["report"]
    if rep is None:
        return fig, None
    wall_ms = m.get("externalWallMs")
    an = analyse(rep, int(wall_ms * 1_000_000) if wall_ms else None)  # the brief's command wall, from meta.json
    logical = m.get("logicalCores") or rep.get("meta", {}).get("logical_cores") or 1
    buckets = cpu_buckets(an, att["sampler"], logical)
    fig["an"] = an
    fig["isa"] = rep.get("meta", {}).get("isa")
    fig["recorder_wall_ms"] = an["recorder_wall_ns"] / 1e6
    fig["pool_threads"] = {str(c["call"]): c["pool_threads"] for c in rep["calls"]}
    last_file = max((s["start"] for s in span_list(rep)
                     if rep["stages"][s["stage"]]["name"] == "file"), default=None)
    fig["tail_ms"] = (an["recorder_wall_ns"] - last_file) / 1e6 if last_file is not None else None
    fig["plan_wall_ms"] = sum(c["wall_ns"] for c in an["per_call"] if c["kind"] == "plan") / 1e6
    fig["run_wall_ms"] = sum(c["wall_ns"] for c in an["per_call"] if c["kind"] == "run") / 1e6
    pr = {"peak_working_set": 0, "peak_commit": 0}
    pr.update(rep.get("process", {}))
    peaks = [c["budget_peak"] for c in an["per_call"] if c.get("budget_peak")]
    fig["memory"] = {"peak_working_set": pr["peak_working_set"], "peak_commit": pr["peak_commit"],
                     "sampler_max_working_set": max((r["working_set"] for r in att["sampler"]), default=None),
                     "sampler_max_threads": max((r["threads"] for r in att["sampler"]), default=None),
                     "budget": [{"call": c["call"], "limit": c["budget_limit"], "peak": c["budget_peak"]} for c in an["per_call"]],
                     "real_over_admitted_peak": (pr["peak_working_set"] / max(peaks)) if peaks else None}
    fig["cpu"] = {"buckets": len(buckets),
                  "cores_used_mean": statistics.fmean(b["cores_used"] for b in buckets) if buckets else None,
                  "idle_cores_mean": statistics.fmean(b["idle_cores"] for b in buckets) if buckets else None,
                  "unrecorded_mean_signed": statistics.fmean(b["unrecorded"] for b in buckets) if buckets else None,
                  "buckets_too_many_threads": sum(b["too_many"] for b in buckets),
                  "buckets_idle_while_admitting": sum(b["idle_while_admitting"] for b in buckets)}
    fig["buckets"] = buckets
    return fig, an


SKIP_ROWS = [
    ("D1/D5 plan-side decode, analyse and assembly", [("plan", s) for s in
        ("plan_load", "plan_analyse", "decode", "pack_assemble", "derive_assemble")]),
    ("D2/D3 source re-reads (hash_source, currency, currency_hash)", [(None, s) for s in
        ("hash_source", "currency", "currency_hash")]),
    ("D4 bake_load", [(None, "bake_load")]),
    ("D6 copies", None),
    ("D7 validate", [(None, "validate")]),
    ("D8 write (wait)", [(None, "write")]),
    ("D9 events, events_wait", [(None, "events"), (None, "events_wait")]),
    ("D10 manifest_wait, manifest_append", [(None, "manifest_wait"), (None, "manifest_append")]),
    ("D11 pool_build", [(None, "pool_build")]),
    ("D12 admission estimates", [(None, "plan_admit_estimate"), (None, "run_admit_estimate")]),
]


def stage_ns(an, kind, stage):
    if kind is None:
        return an["work_ns"].get(stage, 0) + an["wait_ns"].get(stage, 0)
    return an["by_kind"].get(kind, {}).get(stage, 0)


def skip_table(ans):
    rows = []
    for label, parts in SKIP_ROWS:
        if parts is None:
            rows.append({"candidate": label, "seconds": None, "note": "no span covers it; samply (profile-samply.py) attributes it"})
            continue
        secs = [sum(stage_ns(an, k, s) for k, s in parts) / NS for an in ans]
        works = [(sum(stage_ns(an, k, s) for k, s in parts) / an["total_work_ns"]) if an["total_work_ns"] else None
                 for an in ans]
        rows.append({"candidate": label, "seconds_median": statistics.median(secs),
                     "share_of_work_median": statistics.median([w for w in works if w is not None]) if any(
                         w is not None for w in works) else None,
                     "label": "upper bound of the gain from removing it"})
    return rows


# ---------------------------------------------------------------- the list file and the run

def parse_accept(text):
    groups, refs, cur = [], {}, None
    for n, raw in enumerate(text.splitlines(), 1):
        ln = raw.strip()
        if not ln or ln.startswith("#"):
            continue
        if ln.startswith("["):
            if not ln.endswith("]") or len(ln[1:-1].split()) != 2:
                raise ValueError("line %d: a header is [<label> <cohort>]" % n)
            label, cohort = ln[1:-1].split()
            cur = {"label": label, "cohort": cohort, "attempts": [], "relabels": {}}
            groups.append(cur)
        elif cur is None:
            raise ValueError("line %d: an attempt before any [label cohort] header" % n)
        elif ln.startswith("relabel "):
            parts = ln.split()
            if len(parts) != 4 or parts[2] != "from":
                raise ValueError("line %d: a relabel line is 'relabel <attempt> from <recorded cohort>'" % n)
            cur["attempts"].append(parts[1])
            cur["relabels"][parts[1]] = parts[3]
        elif ln.startswith("reference "):
            name = ln[len("reference "):].strip()
            if cur["label"] in refs and refs[cur["label"]] != name:
                raise ValueError("line %d: a second reference for label %s" % (n, cur["label"]))
            refs[cur["label"]] = name
        else:
            cur["attempts"].append(ln)
    for g in groups:
        if g["label"] not in refs:
            raise ValueError("label %s has no 'reference <attempt>' line" % g["label"])
    return groups, refs


def load_expected(runroot, label):
    p = os.path.join(runroot, "expected-%s.txt" % label)
    if not os.path.isfile(p):
        raise FileNotFoundError("no expected-%s.txt in %s (run profile-run.ps1 -Expected)" % (label, runroot))
    exp = parse_expected(read_text(p))
    pnw = try_read(os.path.join(runroot, "planned-not-written-%s.txt" % label))
    sk = try_read(os.path.join(runroot, "skipped-inputs-%s.txt" % label))
    return exp, parse_planned_not_written(pnw) if pnw else [], parse_skipped_inputs(sk) if sk else []


def run_check(args):
    runroot = os.path.abspath(args.runroot)
    att = load_attempt(os.path.abspath(args.check))
    label = att["meta"].get("label")
    if not label:
        print("FAIL: %s has no label in meta.json" % att["name"])
        return 1
    expected, _, _ = load_expected(runroot, label)
    ref = load_attempt(os.path.abspath(args.reference)) if args.reference else None
    probs = check_attempt(att, expected, ref, load_copied_manifest(runroot, att))  # never relaxed here
    if ref is not None:
        probs += ["reference: " + x for x in check_attempt(ref, expected, None, None)]
    for x in probs:
        print("FAIL %s: %s" % (att["name"], x))
    if not probs:
        print("OK %s" % att["name"])
    return 1 if probs else 0


def pct(x):
    return "n/a" if x is None else "%.1f %%" % (100.0 * float(x))


def secs(ns):
    return "%.3f" % (ns / NS)


def build_summary(groups, refs, runroot, allow_contaminated):
    problems, loaded, expected_by = [], {}, {}
    for g in groups:
        label = g["label"]
        if label not in expected_by:
            expected_by[label] = load_expected(runroot, label)
    refloaded = {}
    for label, name in refs.items():
        ref = load_attempt(os.path.join(runroot, name))
        refloaded[label] = ref
        for x in check_attempt(ref, expected_by[label][0], None, None, allow_contaminated):
            problems.append("reference %s: %s" % (name, x))
    relabelled = []
    for g in groups:
        g["loaded"] = []
        for name in g["attempts"]:
            att = load_attempt(os.path.join(runroot, name))
            recorded, build = att["meta"].get("cohort"), att["meta"].get("build")
            rel_from = g.get("relabels", {}).get(name)
            if att["meta"].get("label") != g["label"]:
                problems.append("%s: meta.json says label %s, the list says %s" % (name, att["meta"].get("label"), g["label"]))
            if rel_from is None:
                if recorded != g["cohort"]:
                    problems.append("%s: meta.json says %s %s, the list says %s %s" % (
                        name, att["meta"].get("label"), recorded, g["label"], g["cohort"]))
            elif (rel_from, g["cohort"]) not in RELABELS:
                problems.append("%s: relabel from %s to %s is not allowed (allowed: %s)" % (
                    name, rel_from, g["cohort"], ", ".join("%s to %s" % k for k in RELABELS)))
            elif recorded != rel_from:
                problems.append("%s: the relabel line says from %s, meta.json says %s" % (name, rel_from, recorded))
            elif build not in RELABELS[(rel_from, g["cohort"])]:
                problems.append("%s: only a %s build may be relabelled from %s to %s (this one is %s)" % (
                    name, " or ".join(RELABELS[(rel_from, g["cohort"])]), rel_from, g["cohort"], build))
            else:
                relabelled.append({"attempt": name, "label": g["label"], "recorded": recorded, "build": build, "listed": g["cohort"]})
            for x in check_attempt(att, expected_by[g["label"]][0], refloaded[g["label"]],
                                   load_copied_manifest(runroot, att), allow_contaminated):
                problems.append("%s: %s" % (name, x))
            g["loaded"].append(att)
    for g in groups:
        if not g["attempts"]:
            problems.append("[%s %s] lists no attempts" % (g["label"], g["cohort"]))
    if problems:
        return None, problems
    summary, figs_by = summarise(groups, refs, expected_by, runroot, allow_contaminated)
    summary["relabelled"] = relabelled
    return (summary, figs_by), []


def summarise(groups, refs, expected_by, runroot, allow_contaminated):
    """The figures of already loaded and accepted attempts (g["loaded"]); pure but for the purge stamp read."""
    out = {"accepted": not allow_contaminated, "labels": {}, "groups": [], "overhead": {}}
    for label, (exp, pnw, sk) in expected_by.items():
        out["labels"][label] = {"expected_keys": len(exp), "planned_not_written": [r[:2] for r in pnw],
                                "skipped_inputs": len(sk), "reference": refs[label]}
    figs_by = {}
    for g in groups:
        figs, ans = [], []
        for att in g["loaded"]:
            f, an = attempt_figures(att, runroot)
            figs.append(f)
            if an is not None:
                ans.append(an)
        figs_by[(g["label"], g["cohort"])] = (g, figs)
        walls = [f["wall_ms"] for f in figs]
        grp = {"label": g["label"], "cohort": g["cohort"], "attempts": [f["attempt"] for f in figs],
               "wall_ms": stats(walls), "per_attempt": []}
        shares = [a["encode_share"] for a in ans if a["encode_share"] is not None]
        if shares:
            med = statistics.median(shares)
            stops = [a["stop"] for a in ans]
            grp["encode"] = {"share_median": float(med), "share_min": float(min(shares)), "share_max": float(max(shares)),
                             "split": any(stops) and not all(stops),
                             "isa": sorted({f.get("isa") for f in figs if f.get("isa")})}
            if g["cohort"] == "warm-fresh":  # spec 4.3: the corpus verdict is the warm-fresh median's; other cohorts are informational
                grp["encode"]["verdict"] = "stop" if 10 * med > 9 else "no stop"
                out["labels"][g["label"]]["stop_verdict"] = grp["encode"]["verdict"]
        if ans:
            names = sorted({n for a in ans for n in list(a["work_ns"]) + list(a["wait_ns"])})
            st = {}
            for n in names:
                st[n] = {"seconds": stats([(a["work_ns"].get(n, 0) + a["wait_ns"].get(n, 0)) / NS for a in ans]),
                         "class": "wait" if any(n in a["wait_ns"] for a in ans) else "work",
                         "share_of_work": stats([float(Fraction(a["work_ns"].get(n, 0), a["total_work_ns"]))
                                                 if a["total_work_ns"] else 0.0 for a in ans]),
                         "share_of_capacity": stats([float(a["capacity"].get(n, 0)) for a in ans]),
                         "union_share_of_command_wall": stats([a["union_ns"].get(n, 0) / a["wall_ns"] if a["wall_ns"] else 0.0
                                                       for a in ans]),
                         "union_bound_ms": max(a["union_bound_ns"].get(n, 0) for a in ans) / 1e6}
            grp["stages"] = st
            grp["wall_split_share"] = {k: stats([a["split"][k] / a["split"]["wall_ns"] if a["split"]["wall_ns"] else 0.0
                                                 for a in ans]) for k in ("encode_only_ns", "mixed_ns", "no_encode_ns")}
            grp["wall_split_bound_ms"] = max(a["split"]["bound_ns"] for a in ans) / 1e6
            if g["cohort"] == "warm-fresh":
                grp["skip_work"] = skip_table(ans)
        for f in figs:
            row = {k: v for k, v in f.items() if k not in ("an", "buckets")}
            if "an" in f:
                a = f["an"]
                row["encode_share"] = float(a["encode_share"]) if a["encode_share"] is not None else None
                row["encode_stop"] = a["stop"]
                row["split"] = {k: v for k, v in a["split"].items()}
                row["per_call"] = [{k: (float(v) if isinstance(v, Fraction) else v) for k, v in c.items()} for c in a["per_call"]]
            grp["per_attempt"].append(row)
        out["groups"].append(grp)
        if g["cohort"] == "overhead":
            out["overhead"][g["label"]] = overhead_rounds(g["loaded"])
    return out, figs_by


def timeline_rows(buckets):
    return [(b["t_ms"], b["cores_used"], b["idle_cores"], b["recorded"], b["unrecorded"]) for b in buckets]


def median_attempt(figs):
    ordered = sorted((f for f in figs if f.get("buckets")), key=lambda f: f["wall_ms"])
    return ordered[len(ordered) // 2] if ordered else None


def svg_timeline(buckets, logical, title):
    w, h, pad = 900, 300, 40
    if not buckets:
        return None
    tmax = buckets[-1]["t_ms"] + BUCKET_MS
    ymax = max(logical, max(b["cores_used"] for b in buckets), max(b["recorded"] for b in buckets)) * 1.05

    def pts(key):
        return " ".join("%.1f,%.1f" % (pad + (b["t_ms"] + BUCKET_MS / 2) / tmax * (w - 2 * pad),
                                        h - pad - b[key] / ymax * (h - 2 * pad)) for b in buckets)
    ly = h - pad - logical / ymax * (h - 2 * pad)
    return ('<svg xmlns="http://www.w3.org/2000/svg" width="%d" height="%d" font-family="sans-serif" font-size="11">\n'
            '<rect width="100%%" height="100%%" fill="white"/>\n<text x="%d" y="16">%s</text>\n'
            '<line x1="%d" y1="%.1f" x2="%d" y2="%.1f" stroke="#999" stroke-dasharray="4"/>'
            '<text x="%d" y="%.1f" fill="#666">%d logical cores</text>\n'
            '<polyline fill="none" stroke="#1f77b4" stroke-width="1.5" points="%s"/>\n'
            '<polyline fill="none" stroke="#2ca02c" stroke-width="1.5" points="%s"/>\n'
            '<text x="%d" y="%d" fill="#1f77b4">cores used (sampler)</text>'
            '<text x="%d" y="%d" fill="#2ca02c">recorded work threads</text>\n'
            '<text x="%d" y="%d">time, 0 to %.1f s</text></svg>\n') % (
        w, h, pad, title.replace("&", "&amp;").replace("<", "&lt;"), pad, ly, w - pad, ly, w - pad - 110, ly - 4, logical,
        pts("cores_used"), pts("recorded"), pad + 10, h - 8, pad + 160, h - 8, w // 2 - 40, h - 22, tmax / 1000.0)


def write_new(path, text):
    with open(path, "x", encoding="utf-8", newline="\n") as f:
        f.write(text)


def fmt_stat(d, scale=1.0, nd=3):
    f = lambda x: ("%." + str(nd) + "f") % (float(x) * scale)
    s = "%s [%s, %s]" % (f(d["median"]), f(d["min"]), f(d["max"]))
    if "q1" in d:
        s += " IQR %s-%s" % (f(d["q1"]), f(d["q3"]))
    return s


def cold_text(a):
    """What was done before a cold run, by the method meta.json records (EP-C5); '' for a warm one."""
    stamp = a.get("purgeStamp")
    if a.get("coldMethod") == EVICT_METHOD:
        cv = a.get("cold") if isinstance(a.get("cold"), dict) else {}
        return ("; cold: each corpus file's cached pages were dropped (per-file no-buffering open); the exe, DLLs and "
                "file system metadata stay cached (evict stamp %s; corpus disk read %s of %s corpus bytes)" % (
                    stamp, cv.get("diskReadBytes", "n/a"), cv.get("corpusBytes", "n/a")))
    if stamp:
        return "; cold: the standby list was purged before the run (purge stamp %s)" % stamp
    return ""


def render_md(summary):
    L = []
    if not summary["accepted"]:
        L += ["**NOT ACCEPTANCE EVIDENCE: contaminated attempts were allowed (--allow-contaminated).**", ""]
    L += ["# Engine profile summary", "",
          "Median [min, max] per cohort (n = repeats); five repeats add the inclusive IQR. Work seconds are exclusive "
          "busy thread-seconds of work stages; wait seconds are exclusive time in wait stages and never count as work.", ""]
    for label, info in summary["labels"].items():
        L += ["## Corpus %s" % label, "",
              "Expected keys: %d. Planned, not written: %d. Skipped inputs: %d. Reference: %s. Stop verdict (warm-fresh median): %s." % (
                  info["expected_keys"], len(info["planned_not_written"]), info["skipped_inputs"], info["reference"],
                  info.get("stop_verdict", "n/a (no warm-fresh group with a report)")), ""]
        for g in [x for x in summary["groups"] if x["label"] == label]:
            L += ["### %s (n = %d)" % (g["cohort"], g["wall_ms"]["n"]), "",
                  "Command wall (ms): %s" % fmt_stat(g["wall_ms"], 1.0, 0), ""]
            if "encode" in g:
                e = g["encode"]
                verdict = ("Verdict (the corpus verdict, from this warm-fresh median): **%s**%s." % (
                    e["verdict"], " (split: the repeats straddle 90 %)" if e["split"] else "")) if "verdict" in e else \
                    "Informational only, no verdict: the corpus verdict is the warm-fresh median's."
                L += ["Encode share of work (EP-R1): median %s, min %s, max %s. %s ISA: %s. "
                      "Wall shares are reported, not deciding." % (pct(e["share_median"]), pct(e["share_min"]), pct(e["share_max"]),
                                                                    verdict, ", ".join(e["isa"]) or "n/a"), ""]
                ws = g["wall_split_share"]
                L += ["Wall split (share of command wall from meta.json; recorder wall is a second figure per attempt below; "
                      "error bound %.3f ms): encode-only %s, no-encode %s, mixed %s." % (
                    g["wall_split_bound_ms"], pct(ws["encode_only_ns"]["median"]), pct(ws["no_encode_ns"]["median"]),
                    pct(ws["mixed_ns"]["median"])), ""]
                L += ["| stage | class | seconds | share of work | share of pool capacity | union of command wall | union bound ms |",
                      "|---|---|---|---|---|---|---|"]
                for n, s in sorted(g["stages"].items(), key=lambda kv: -kv[1]["seconds"]["median"]):
                    L.append("| %s | %s | %s | %s | %s | %s | %.3f |" % (
                        n, s["class"], fmt_stat(s["seconds"]), pct(s["share_of_work"]["median"]) if s["class"] == "work" else "wait",
                        pct(s["share_of_capacity"]["median"]), pct(s["union_share_of_command_wall"]["median"]), s["union_bound_ms"]))
                L.append("")
            if "skip_work" in g:
                L += ["Skip-work candidates (each gain is an **upper bound**; seconds are thread-seconds, median):", "",
                      "| candidate | seconds | share of work | label |", "|---|---|---|---|"]
                for r in g["skip_work"]:
                    if r.get("seconds_median") is None and "note" in r:
                        L.append("| %s | n/a | n/a | %s |" % (r["candidate"], r["note"]))
                    else:
                        L.append("| %s | %.3f | %s | %s |" % (r["candidate"], r["seconds_median"], pct(r["share_of_work_median"]), r["label"]))
                L.append("")
            L += ["Per attempt (n/a where the attempt has no recorder report; unrecorded CPU is informational only and printed with its sign):", "",
                  "| attempt | command wall ms | recorder wall ms | plan ms | run ms | tail ms | cores used | idle cores | unrecorded CPU | peak WS MB | contaminated |",
                  "|---|---|---|---|---|---|---|---|---|---|---|"]
            for a in g["per_attempt"]:
                cpu = a.get("cpu") or {}
                mem = a.get("memory") or {}
                L.append("| %s | %s | %s | %s | %s | %s | %s | %s | %s | %s | %s |" % (
                    a["attempt"] + (" (--validate)" if a.get("validate") else ""), a["wall_ms"], "%.0f" % a["recorder_wall_ms"] if "recorder_wall_ms" in a else "n/a",
                    "%.0f" % a["plan_wall_ms"] if "plan_wall_ms" in a else "n/a",
                    "%.0f" % a["run_wall_ms"] if "run_wall_ms" in a else "n/a",
                    "%.0f" % a["tail_ms"] if a.get("tail_ms") is not None else "n/a",
                    "%.2f" % cpu["cores_used_mean"] if cpu.get("cores_used_mean") is not None else "n/a",
                    "%.2f" % cpu["idle_cores_mean"] if cpu.get("idle_cores_mean") is not None else "n/a",
                    "%+.2f" % cpu["unrecorded_mean_signed"] if cpu.get("unrecorded_mean_signed") is not None else "n/a",
                    "%.0f" % (mem["peak_working_set"] / 1048576) if mem.get("peak_working_set") else "n/a", a.get("contaminated")))
            L.append("")
            L += ["Disk (bytes during the run; a cold run's line says what was done before it, nothing more is claimed):", ""]
            for a in g["per_attempt"]:
                d = a.get("disk")
                if d and "unreadable" in d:
                    L.append("- %s: %s%s" % (a["attempt"], d["unreadable"], cold_text(a)))
                elif d:
                    L.append("- %s: corpus disk read %d write %d; output disk read %d write %d%s" % (
                        a["attempt"], d["corpus"]["read_bytes"], d["corpus"]["write_bytes"], d["output"]["read_bytes"],
                        d["output"]["write_bytes"], cold_text(a)))
            L.append("")
    if summary.get("relabelled"):
        L += ["## Relabelled attempts (EP-C9)", "",
              "Listed under a cohort other than the one the runner recorded; only an overhead attempt of the release build may count as warm-fresh.", ""]
        for r in summary["relabelled"]:
            L.append("- %s (%s): recorded %s, %s build; counted as %s" % (r["attempt"], r["label"], r["recorded"], r["build"], r["listed"]))
        L.append("")
    for label, o in summary["overhead"].items():
        L += ["## Overhead (%s)" % label, "", "Outcome: **%s**." % o["outcome"], "",
              "| round | base ms | off ms | on ms | off - base | on - off |", "|---|---|---|---|---|---|"]
        for k, r in enumerate(o["rounds"], 1):
            L.append("| %d | %s | %s | %s | %+.2f %% | %+.2f %% |" % (k, r["base_ms"], r["off_ms"], r["on_ms"],
                                                                    r["off_minus_base_pct"], r["on_minus_off_pct"]))
        L.append("")
    return "\n".join(L) + "\n"


def run_accept(args):
    check_out(args.out)  # before anything is read or created
    runroot = os.path.abspath(args.runroot)
    groups, refs = parse_accept(read_text(args.accept))
    res, problems = build_summary(groups, refs, runroot, args.allow_contaminated)
    if problems:
        for x in problems:
            print("FAIL " + x)
        return 1
    summary, figs_by = res
    for r in summary.get("relabelled", []):
        print("RELABEL %s (%s): recorded %s, %s build; counted as %s" % (r["attempt"], r["label"], r["recorded"], r["build"], r["listed"]))
    os.makedirs(args.out)  # fails when the folder exists (EP-R3)
    write_new(os.path.join(args.out, "summary.json"), json.dumps(summary, indent=2, default=float) + "\n")
    write_new(os.path.join(args.out, "summary.md"), render_md(summary))
    for (label, cohort), (g, figs) in figs_by.items():
        med = median_attempt(figs)
        if med is None:
            continue
        rows = ["t_ms,cores_used,idle_cores,recorded_work_threads,unrecorded_cpu"] + [
            "%d,%.3f,%.3f,%.3f,%+.3f" % r for r in timeline_rows(med["buckets"])]
        write_new(os.path.join(args.out, "timeline-%s-%s.csv" % (label, cohort)), "\n".join(rows) + "\n")
        if cohort == "warm-fresh":
            logical = next((a["meta"].get("logicalCores") for a in g["loaded"]), None) or 1
            svg = svg_timeline(med["buckets"], logical, "%s warm-fresh, median attempt %s" % (label, med["attempt"]))
            if svg:
                write_new(os.path.join(args.out, "cpu-%s.svg" % label), svg)
    print("wrote %s" % args.out)
    return 0


# ---------------------------------------------------------------- mip quality (spec 9.3)

def psnr_text(mse):
    return "exact" if mse == 0 else "%.2f dB" % (10 * math.log10(255.0 * 255.0 / mse))


def aggregate_mipq(reports):
    """reports: {corpus name: report dict}."""
    out = {"corpora": {}}
    for corpus, rep in reports.items():
        groups = {}

        def group(kind, value):
            return groups.setdefault((kind, value), {"scored": 0, "skipped": 0, "reasons": {}, "views": {}, "empty_edge_bands": 0})
        for j in rep.get("jobs", []):
            for kind in ("subtype", "format"):
                g = group(kind, j[kind])
                g["scored"] += 1
                for lv in j.get("per_level", []):
                    top = lv["level"]
                    bottom = j["levels"] - 1 - lv["level"]
                    for view, idx, lim in (("top", top, 3), ("bottom", bottom, 2)):
                        if idx >= lim:
                            continue
                        cell = g["views"].setdefault("%s%d" % (view, idx), {"texels": 0, "metrics": {}, "edge": {}})
                        cell["texels"] += lv["texels"]
                        for name, mse in lv["mse"].items():
                            m = cell["metrics"].setdefault(name, {"sum": 0.0, "texels": 0, "per_texture": [], "exact_levels": 0})
                            m["sum"] += mse * lv["texels"]
                            m["texels"] += lv["texels"]
                            m["per_texture"].append(mse)
                            m["exact_levels"] += 1 if mse == 0 else 0
                        e = lv.get("edge")
                        if e is not None:
                            if e["band_texels"] == 0:
                                g["empty_edge_bands"] += 1
                            else:
                                ed = cell["edge"].setdefault("band", {"sum": 0.0, "texels": 0, "inter_sum": 0.0, "inter_texels": 0})
                                ed["sum"] += e["band_mse"] * e["band_texels"]
                                ed["texels"] += e["band_texels"]
                                ed["inter_sum"] += e["interior_mse"] * e["interior_texels"]
                                ed["inter_texels"] += e["interior_texels"]
        for j in rep.get("skipped", []):
            for kind in ("subtype", "format"):
                g = group(kind, j[kind])
                g["skipped"] += 1
                g["reasons"][j["why"]] = g["reasons"].get(j["why"], 0) + 1
        res = {}
        for (kind, value), g in sorted(groups.items()):
            sampled = g["scored"] + g["skipped"]
            views = {}
            for v, cell in sorted(g["views"].items()):
                mets = {}
                for name, m in sorted(cell["metrics"].items()):
                    pooled = m["sum"] / m["texels"] if m["texels"] else 0.0
                    mets[name] = {"pooled_mse": pooled, "psnr": psnr_text(pooled), "per_texture_median_mse": statistics.median(m["per_texture"]),
                                  "levels": len(m["per_texture"]), "exact_levels": m["exact_levels"]}
                ed = cell["edge"].get("band")
                views[v] = {"texels": cell["texels"], "metrics": mets,
                            "edge_band_mse": ed["sum"] / ed["texels"] if ed else None,
                            "interior_mse": ed["inter_sum"] / ed["inter_texels"] if ed and ed["inter_texels"] else None}
            res["%s=%s" % (kind, value)] = {"sampled": sampled, "scored": g["scored"], "skipped": g["skipped"],
                                            "skip_reasons": g["reasons"], "under_half_scored": g["scored"] * 2 < sampled,
                                            "empty_edge_bands": g["empty_edge_bands"], "views": views}
        out["corpora"][corpus] = {"isa": rep.get("isa"), "self_check": rep.get("self_check"), "groups": res}
    return out


def render_mipq_md(agg):
    L = ["# Mip quality summary", "", "MSE is in 8-bit code units, pooled over texels; the per-texture median is beside it. "
         "Levels are counted from the top (top0, top1, top2) and from the bottom (bottom0 = smallest). Exact levels are "
         "counted, not averaged.", ""]
    for corpus, c in agg["corpora"].items():
        L += ["## %s" % corpus, "", "ISA: %s. Self-check: %s." % (c["isa"], c["self_check"]), "",
              "| group | sampled | scored | skipped | skip reasons | flag |", "|---|---|---|---|---|---|"]
        for k, g in c["groups"].items():
            L.append("| %s | %d | %d | %d | %s | %s |" % (k, g["sampled"], g["scored"], g["skipped"],
                                                           ", ".join("%s: %d" % kv for kv in sorted(g["skip_reasons"].items())) or "-",
                                                           "under half scored" if g["under_half_scored"] else ""))
        L.append("")
        for k, g in c["groups"].items():
            for v, cell in g["views"].items():
                for name, m in cell["metrics"].items():
                    L.append("- %s %s %s: pooled MSE %.4f (%s), per-texture median %.4f, %d level(s), %d exact" % (
                        k, v, name, m["pooled_mse"], m["psnr"], m["per_texture_median_mse"], m["levels"], m["exact_levels"]))
                if cell["edge_band_mse"] is not None:
                    L.append("- %s %s edge band MSE %.4f, interior %s" % (k, v, cell["edge_band_mse"],
                                                                           "n/a" if cell["interior_mse"] is None else "%.4f" % cell["interior_mse"]))
            if g["empty_edge_bands"]:
                L.append("- %s: %d empty edge band(s) left out" % (k, g["empty_edge_bands"]))
        L.append("")
    return "\n".join(L) + "\n"


def run_mipq(args):
    check_out(args.out)
    reps = {os.path.splitext(os.path.basename(p))[0]: read_json(p) for p in args.mipq}
    agg = aggregate_mipq(reps)
    os.makedirs(args.out)
    write_new(os.path.join(args.out, "mipq.json"), json.dumps(agg, indent=2) + "\n")
    write_new(os.path.join(args.out, "mipq.md"), render_mipq_md(agg))
    print("wrote %s" % args.out)
    return 0


# ---------------------------------------------------------------- self-test

def mk_report(spans, calls, wall=None, jobs=None):
    """spans: (stage name, thread, start_ns, end_ns[, id, busy, pieces]), emitted in the order given (inner first)."""
    idx = {n: i for i, n in enumerate(STAGE_NAMES)}
    raw = []
    for s in spans:
        name, th, a, b = s[:4]
        ident = s[4] if len(s) > 4 else 0
        busy = s[5] if len(s) > 5 else b - a
        pieces = s[6] if len(s) > 6 else 1
        raw.append([idx[name], ident, th, a, b, busy, pieces])
    end = max(r[4] for r in raw)
    return {"schema": 1, "meta": {"isa": "avx2", "logical_cores": 16},
            "stages": [{"name": n, "class": "wait" if n in WAITS else "work"} for n in STAGE_NAMES],
            "threads": [], "calls": calls, "inputs": [], "jobs": jobs or [], "spans": raw,
            "process": {"wall_ns": wall or end, "peak_working_set": 0, "peak_commit": 0}}


def call(pool, end, start=0, kind="run", idx=0):
    return {"call": idx, "kind": kind, "pool_threads": pool, "start_ns": start, "end_ns": end, "budget_limit": 0, "budget_peak": 0}


def S(x):
    return int(round(x * NS))


def US(x):
    return int(x * 1000)


class T:
    def __init__(self):
        self.fail = 0

    def ok(self, name, cond, detail=""):
        print("%s %s%s" % ("PASS" if cond else "FAIL", name, "" if cond else ": " + str(detail)))
        if not cond:
            self.fail += 1


def selftest_accounting(t):
    def share(rep):
        a = analyse(rep)
        return a, a["encode_share"], a["stop"]
    # encode_share_ignores_idle_workers
    for pool in (2, 16):
        rep = mk_report([("encode_band", 1, 0, S(10)), ("decode", 2, 0, S(10))], [call(pool, S(10))])
        a, sh, stop = share(rep)
        t.ok("encode_share_ignores_idle_workers: pool %d is 50 %% and no stop" % pool, sh == Fraction(1, 2) and not stop, (sh, stop))
    a, sh, stop = share(mk_report([("encode_band", 1, 0, S(10))], [call(16, S(10))]))
    t.ok("one_worker_encoding_alone_stops", sh == 1 and stop, (sh, stop))
    spans = []
    for i in range(16):
        o = S(0.5 * i)
        spans += [("decode", i, o, o + S(0.5)), ("encode_band", i, o + S(0.5), o + S(10))]
    a, sh, stop = share(mk_report(spans, [call(16, S(17.5))]))
    t.ok("steady_state_95_percent_encode_stops", sh == Fraction(95, 100) and stop, (sh, stop))
    a, sh, stop = share(mk_report([("encode_band", 1, 0, S(9)), ("decode", 2, 0, S(1))], [call(2, S(9))]))
    t.ok("exactly_90_percent_does_not_stop", sh == Fraction(9, 10) and not stop, (sh, stop))
    a, sh, stop = share(mk_report([("encode_band", 1, 0, S(9.5)), ("decode", 2, 0, S(0.5)), ("write", 2, S(0.5), S(10))],
                                  [call(2, S(10))]))
    t.ok("a_blocked_write_counts_as_wait: 95 % and stop", sh == Fraction(95, 100) and stop and a["wait_ns"]["write"] == S(9.5), (sh, stop, a["wait_ns"]))
    rep = mk_report([("decode", 0, S(2), S(6), 1), ("file", 0, S(2), S(6), 1), ("row_join", 0, 0, S(10), 0)], [call(1, S(10))])
    a = analyse(rep)
    t.ok("nested_job_inside_row_join_is_not_charged_to_the_wait: row_join wait 6 s, decode work 4 s",
         a["wait_ns"]["row_join"] == S(6) and a["work_ns"]["decode"] == S(4) and a["work_ns"].get("file", 0) == 0, (a["wait_ns"], a["work_ns"]))
    # scope_join: the coordinator's run (0-10) holds scope_join (0-10) holds dispatch (0-1); closing order inner first
    rep = mk_report([("dispatch", 0, 0, S(1)), ("scope_join", 0, 0, S(10)), ("run", 0, 0, S(10)), ("encode_band", 1, S(1), S(10))],
                    [call(16, S(10))])
    a = analyse(rep)
    t.ok("scope_join_is_a_wait: wait 9 s, encode 90 % and no stop", a["wait_ns"]["scope_join"] == S(9) and a["encode_share"] == Fraction(9, 10)
         and not a["stop"] and a["work_ns"]["dispatch"] == S(1), (a["wait_ns"], a["work_ns"], a["encode_share"]))
    spans = [("plan_file", 0, 0, S(3))] + [("encode_band", i, S(3), S(10)) for i in range(16)]
    rep = mk_report(spans, [call(1, S(3), 0, "plan", 0), call(16, S(10), S(3), "run", 1)], wall=S(10))
    a = analyse(rep, S(10))
    t.ok("wall_shares_are_reported_not_deciding: 112/115 stop, no-encode wall 30 %",a["encode_share"] == Fraction(112, 115) and a["stop"]
         and a["split"]["no_encode_ns"] == S(3) and a["split"]["wall_ns"] == S(10), (a["encode_share"], a["split"]))
    # unrecorded_cpu_is_informational
    spans = []
    for i in range(16):
        o = S(0.5 * i)
        spans += [("decode", i, o, o + S(0.5)), ("encode_band", i, o + S(0.5), o + S(10))]
    rep = mk_report(spans, [call(16, S(17.5))])
    base = analyse(rep)

    def sampler(cores, end_ms=17500):
        return [{"t_ms": float(t_), "cpu_ms": cores * t_, "working_set": 0, "private_bytes": 0, "threads": 1}
                for t_ in range(0, end_ms + 1, 250)]
    hi = cpu_buckets(base, sampler(19), 16)
    lo = cpu_buckets(base, sampler(8), 16)
    b9 = next(b for b in hi if b["t_ms"] == 9000)
    l9 = next(b for b in lo if b["t_ms"] == 9000)
    t.ok("unrecorded_cpu_is_informational: +3 with 3 extra cores", abs(b9["unrecorded"] - 3) < 1e-6 and abs(b9["recorded"] - 16) < 1e-6, b9)
    t.ok("unrecorded_cpu_is_informational: negative when cores used are below the recorded work", l9["unrecorded"] < 0, l9)
    fa = mk_att("u1", build="release", hashes={}, report=rep, logicalCores=16, externalWallMs=17500)
    fa["sampler"] = sampler(19)
    fb = mk_att("u2", build="release", hashes={}, report=rep, logicalCores=16, externalWallMs=17500)
    fb["sampler"] = sampler(8)
    f1, _ = attempt_figures(fa, None)
    f2, _ = attempt_figures(fb, None)
    t.ok("unrecorded_cpu_is_informational: attempt figures keep the verdict and share, only the unrecorded column moves (+ and -)",
         f1["an"]["stop"] == f2["an"]["stop"] and f1["an"]["encode_share"] == f2["an"]["encode_share"]
         and f1["cpu"]["unrecorded_mean_signed"] > 0 > f2["cpu"]["unrecorded_mean_signed"], (f1["cpu"], f2["cpu"]))
    # coalesced rows
    rep = mk_report([("encode_band", 1, 0, US(50), 0, US(30), 3)], [call(1, US(50))])
    a = analyse(rep)
    t.ok("coalesced_rows_use_busy_time_and_bound_the_union: 30 us of work, bound 100 us",
         a["work_ns"]["encode_band"] == US(30) and a["union_bound_ns"]["encode_band"] == US(100) and a["union_ns"]["encode_band"] == US(30),
         (a["work_ns"], a["union_bound_ns"], a["union_ns"]))
    rep = mk_report([("encode_band", 1, 0, US(50), 0, US(30), 3), ("decode", 2, 0, US(20))], [call(2, US(50))])
    a = analyse(rep)
    true_mixed = US(10)
    t.ok("coalesced_gaps_bound_the_wall_split: computed mixed 0, true 10 us, bound 100 us covers it",
         a["split"]["mixed_ns"] == 0 and a["split"]["bound_ns"] == US(100) and abs(a["split"]["mixed_ns"] - true_mixed) <= a["split"]["bound_ns"], a["split"])


def selftest_stats(t):
    s = stats([10, 11, 12, 13, 30])
    t.ok("quartiles_follow_the_inclusive_method: five walls", (s["median"], s["q1"], s["q3"], s["min"], s["max"]) == (12, 11, 13, 10, 30), s)
    s3 = stats([10, 12, 30])
    t.ok("quartiles_follow_the_inclusive_method: three walls give no quartiles", "q1" not in s3 and s3["median"] == 12, s3)
    t.ok("overhead_outcomes: within 1 %", overhead_outcome([0.2, 0.4, 0.6]) == "within 1 %")
    t.ok("overhead_outcomes: over 1 %", overhead_outcome([1.5, 2, 1.2]) == "over 1 %")
    t.ok("overhead_outcomes: inconclusive", overhead_outcome([0.5, 1.8, 0.3]) == "inconclusive")
    mk = lambda b, w: {"meta": {"build": b, "externalWallMs": w}}
    o = overhead_rounds([mk("base", 1000), mk("release-off", 1005), mk("release", 1010)])
    r = o["rounds"][0]
    t.ok("overhead_outcomes: a round's paired differences are shares of base", abs(r["off_minus_base_pct"] - 0.5) < 1e-9
         and abs(r["on_minus_off_pct"] - 0.5) < 1e-9 and o["outcome"] == "within 1 %", o)


def mk_att(name, cohort="warm-fresh", build="release-off", exe="E1", hashes=None, manifest=None, report=None, **meta):
    m = {"attempt": name, "label": "fx", "cohort": cohort, "build": build, "exeSha256": exe, "finished": True, "exitCode": 0,
         "contention": {"contaminated": False, "reasons": []}, "externalWallMs": 100}
    m.update(meta)
    return {"dir": "", "name": name, "meta": m, "hashes": hashes or {}, "manifest": manifest, "report": report,
            "files_before": None, "files_after": None, "sampler": []}


def rec(key, **kw):
    r = {"output": key, "ts": "t0", "engine_commit": "c1", "profile": {"mips": {"edge": "wrap"}}, "profile_sha256": "p1",
         "provenance": {"mips.edge": "builtin"}, "source_sha256": "s1", "validation": {"passed": True, "levels": 3},
         "output_stamp": {"size": 1, "mtime_ns": 1}}
    r.update(kw)
    return r


def selftest_acceptance(t):
    n = "acceptance_failures_name_their_reason"
    exp = {"a.dds", "b.dds"}
    H = lambda **k: {key: "h-" + key for key in k}
    good = dict(hashes={"a.dds": "ha", "b.dds": "hb"}, manifest={"a.dds": rec("a.dds"), "b.dds": rec("b.dds")})
    ref = mk_att("ref", build="base", exe="B1", **good)
    t.ok(n + ": a clean attempt is accepted", check_attempt(mk_att("x", **good), exp, ref) == [], check_attempt(mk_att("x", **good), exp, ref))
    # first: one expected key missing and one unexpected key written (same count)
    bad = mk_att("x", hashes={"a.dds": "ha", "c.dds": "hc"}, manifest={"a.dds": rec("a.dds"), "c.dds": rec("c.dds")})
    pr = check_attempt(bad, exp, None)
    t.ok(n + ": first, a missing key and an extra key (same count) are refused and named",
         any("b.dds" in x and "not written" in x for x in pr) and any("c.dds" in x and "unexpected" in x for x in pr), pr)
    # second: reference and attempt both miss the same ready key
    miss = dict(hashes={"a.dds": "ha"}, manifest={"a.dds": rec("a.dds")})
    r2, a2 = mk_att("ref2", build="base", exe="B1", **miss), mk_att("x2", **miss)
    pr_r, pr_a = check_attempt(r2, exp, None), check_attempt(a2, exp, r2)
    t.ok(n + ": second, a reference and an attempt that both miss the same ready key are both refused",
         any("b.dds" in x for x in pr_r) and any("b.dds" in x for x in pr_a), (pr_r, pr_a))
    # third: the expected set from a dry run with one ready and one unresolved job
    pnw_text = 'textures/b.dds\tunresolved\t{"state":"unresolved"}\n'
    exp3, pnw = parse_expected("textures/a.dds\n"), parse_planned_not_written(pnw_text)
    ok3 = mk_att("x3", hashes={"textures/a.dds": "h"}, manifest={"textures/a.dds": rec("textures/a.dds")})
    t.ok(n + ": third, the unresolved key is listed as planned, not written, and the attempt writing only the ready key is accepted",
         exp3 == {"textures/a.dds"} and pnw == [["textures/b.dds", "unresolved", '{"state":"unresolved"}']] and check_attempt(ok3, exp3, None) == [],
         (exp3, pnw, check_attempt(ok3, exp3, None)))
    # fourth: hashes, profile_sha256, engine_commit
    hb = mk_att("x4", hashes={"a.dds": "ZZ", "b.dds": "hb"}, manifest=good["manifest"])
    pr = check_attempt(hb, exp, ref)
    t.ok(n + ": fourth, a hash differing from the reference is refused and named", any("hash" in x and "a.dds" in x for x in pr), pr)
    pm = dict(good, manifest={"a.dds": rec("a.dds", profile_sha256="p2"), "b.dds": rec("b.dds")})
    pr = check_attempt(mk_att("x5", **pm), exp, ref)
    t.ok(n + ": a fresh attempt's record differing in profile_sha256 is refused and the field named", any("profile_sha256" in x for x in pr), pr)
    ec = dict(good, manifest={"a.dds": rec("a.dds", engine_commit="c2"), "b.dds": rec("b.dds")})
    pr = check_attempt(mk_att("x6", exe="B1", **ec), exp, ref)
    t.ok(n + ": a same-build pair differing in engine_commit is refused and the field named", any("engine_commit" in x for x in pr), pr)
    pr = check_attempt(mk_att("x7", exe="E1", **ec), exp, ref)
    t.ok(n + ": a cross-build pair differing only in engine_commit is accepted", pr == [], pr)
    # stale
    jobs = [{"id": 0, "call": 1, "output": "a.dds"}, {"id": 1, "call": 1, "output": "b.dds"}]
    sp = [("currency_hash", 0, 10, 20, 0), ("hash_source", 0, 30, 40, 0), ("file", 0, 25, 50, 0),
          ("currency_hash", 1, 12, 22, 1), ("hash_source", 1, 32, 42, 1), ("file", 1, 27, 52, 1)]
    sp = [(s[0], 0, s[2], s[3], s[4]) for s in sp]
    stale_manifest = {"a.dds": rec("a.dds", profile={"mips": {"edge": "clamp"}}, profile_sha256="pX", provenance={"x": "cli"}),
                      "b.dds": rec("b.dds", profile={"mips": {"edge": "clamp"}}, profile_sha256="pX", provenance={"x": "cli"})}
    rp = mk_report(sp, [call(2, 100, 0, "run", 1)], jobs=jobs)
    copied = {"a.dds": rec("a.dds"), "b.dds": rec("b.dds")}
    st = mk_att("s1", cohort="warm-stale", hashes=good["hashes"], manifest=stale_manifest, report=rp)
    pr = check_attempt(st, exp, ref, copied)
    t.ok(n + ": a stale attempt whose records differ in profile, profile_sha256 and provenance but whose bytes and validation match is accepted", pr == [], pr)
    st = mk_att("s2", cohort="warm-stale", hashes={"a.dds": "ZZ", "b.dds": "hb"}, manifest=stale_manifest, report=rp)
    pr = check_attempt(st, exp, ref, copied)
    t.ok(n + ": the same with one byte changed is refused", any("hash" in x for x in pr), pr)
    rp2 = mk_report([s for s in sp if not (s[0] == "currency_hash" and s[4] == 1)], [call(2, 100, 0, "run", 1)], jobs=jobs)
    st = mk_att("s3", cohort="warm-stale", hashes=good["hashes"], manifest=stale_manifest, report=rp2)
    pr = check_attempt(st, exp, ref, copied)
    t.ok(n + ": one lacking currency_hash for an eligible job is refused and the job named", any("b.dds" in x and "currency_hash" in x for x in pr), pr)
    cop2 = {"a.dds": rec("a.dds"), "b.dds": rec("b.dds", validation={"passed": False, "levels": 3})}
    pr = check_attempt(st, exp, ref, cop2)
    t.ok(n + ": one lacking it for a job whose copied record has validation.passed false is accepted", pr == [], pr)
    # noop
    noop = mk_att("n1", cohort="warm-noop", **good)
    noop["files_before"], noop["files_after"] = "path,size\na,1\n", "path,size\na,2\n"
    pr = check_attempt(noop, exp, ref)
    t.ok(n + ": a noop whose files-after.csv differs is refused", any("files-after.csv differs" in x for x in pr), pr)
    noop["files_after"] = noop["files_before"]
    t.ok(n + ": a noop with equal file lists is accepted", check_attempt(noop, exp, ref) == [])
    cold = mk_att("c1", cohort="cold-fresh", purgeStamp=None, **good)
    t.ok(n + ": a cold attempt without a consumed purge stamp is refused", any("purge stamp" in x for x in check_attempt(cold, exp, ref)))
    # EP-C5: the runner's cold verdict
    ev = dict(cohort="cold-fresh", purgeStamp="evict-20261126-000000.json", coldMethod=EVICT_METHOD)
    pr = check_attempt(mk_att("c2", cold={"method": EVICT_METHOD, "cold": False, "reason": "the run read 10 bytes from the corpus disk"},
                              **ev, **good), exp, ref)
    t.ok(n + ": a cold attempt whose cold verdict is false is refused with the runner's reason and method",
         any("not cold (per-file-no-buffering)" in x and "read 10 bytes" in x for x in pr), pr)
    pr = check_attempt(mk_att("c3", cold=False, **ev, **good), exp, ref)
    t.ok(n + ": a bare cold = false is refused too", any(x.startswith("not cold") for x in pr), pr)
    pr = check_attempt(mk_att("c4", cold={"method": EVICT_METHOD, "cold": True, "corpusBytes": 100, "diskReadBytes": 99}, **ev, **good), exp, ref)
    t.ok(n + ": a per-file eviction attempt with a true verdict is accepted", pr == [], pr)
    pr = check_attempt(mk_att("c5", **ev, **good), exp, ref)
    t.ok(n + ": a per-file eviction attempt without a verdict is refused", any("no cold verdict" in x for x in pr), pr)
    pr = check_attempt(mk_att("c6", cohort="cold-fresh", purgeStamp="purge-20261126-000000.json", coldMethod="standby-purge",
                              cold={"method": "standby-purge", "check": "not applied"}, **good), exp, ref)
    t.ok(n + ": a standby-purge attempt (no byte check) is accepted", pr == [], pr)
    pr = check_attempt(mk_att("c7", cohort="cold-fresh", purgeStamp="purge-20261126-000000.json", **good), exp, ref)
    t.ok(n + ": an older purge attempt without coldMethod is accepted", pr == [], pr)
    cont = mk_att("k1", **good)
    cont["meta"]["contention"] = {"contaminated": True, "reasons": ["banned process appeared: cargo(1)"]}
    t.ok(n + ": a contaminated attempt is refused with the runner's reason", any("cargo(1)" in x for x in check_attempt(cont, exp, ref)))
    t.ok("parse_accept: groups and a reference line",
         parse_accept("[fx warm-fresh]\na1\na2\nreference r1\n[fx warm-noop]\nn1\n")[1] == {"fx": "r1"})


def selftest_mipq(t):
    rep = {"isa": "avx2", "self_check": "ok",
           "jobs": [{"output": "a.dds", "subtype": "diffuse", "format": "bc7", "levels": 3,
                     "per_level": [{"level": 0, "texels": 1024, "mse": {"d_rgb": 0.0}}]}],
           "skipped": [{"output": "b.dds", "subtype": "diffuse", "format": "bc7", "levels": 3, "why": "rebuild mismatch"}]}
    g = aggregate_mipq({"fx": rep})["corpora"]["fx"]["groups"]["subtype=diffuse"]
    m = g["views"]["top0"]["metrics"]["d_rgb"]
    t.ok("mipq_aggregation_counts_skips_and_exact_levels: scored 1, skipped 1 with its reason", (g["scored"], g["skipped"], g["skip_reasons"]) == (1, 1, {"rebuild mismatch": 1}), g)
    t.ok("mipq_aggregation_counts_skips_and_exact_levels: the exact level is counted, not averaged", m["exact_levels"] == 1 and m["psnr"] == "exact", m)
    t.ok("mipq_aggregation_counts_skips_and_exact_levels: the format group counts it too", aggregate_mipq({"fx": rep})["corpora"]["fx"]["groups"]["format=bc7"]["skipped"] == 1)


def _slow_cpu_buckets(an, sampler, logical):
    """The first (quadratic) version, kept as the reference the linear one is checked against."""
    if len(sampler) < 2:
        return []
    ts = [r["t_ms"] for r in sampler]
    cpu = [r["cpu_ms"] for r in sampler]

    def cum(t):
        if t <= ts[0]:
            return cpu[0]
        if t >= ts[-1]:
            return cpu[-1]
        for k in range(1, len(ts)):
            if t <= ts[k]:
                f = (t - ts[k - 1]) / (ts[k] - ts[k - 1]) if ts[k] > ts[k - 1] else 1.0
                return cpu[k - 1] + f * (cpu[k] - cpu[k - 1])
        return cpu[-1]
    out = []
    t = 0
    while t + BUCKET_MS <= ts[-1] + 1e-9:
        a, b = t * 1_000_000, (t + BUCKET_MS) * 1_000_000
        used = (cum(t + BUCKET_MS) - cum(t)) / BUCKET_MS
        recorded = sum(max(0, min(y, b) - max(x, a)) for x, y in an["work_own"]) / (b - a)
        waiting = any(y > a and x < b for x, y in an["admit_ivs"])
        out.append({"t_ms": t, "cores_used": used, "idle_cores": logical - used, "recorded": recorded,
                    "unrecorded": used - recorded, "too_many": recorded > logical,
                    "idle_while_admitting": waiting and recorded < logical / 2})
        t += BUCKET_MS
    return out


def _slow_stale(att, expected, copied):
    """The first version of the stale span check (a scan of all spans per key), as the reference."""
    rep = att["report"]
    idx = {n: i for i, n in enumerate(s["name"] for s in rep["stages"])}
    ids = {}
    for j in rep.get("jobs", []):
        ids.setdefault(j["output"], set()).add(j["id"])
    spans = span_list(rep)
    probs = []
    for key in sorted(expected):
        rec_ = copied.get(key)
        if not rec_ or (rec_.get("validation") or {}).get("passed") is not True:
            continue
        jid = ids.get(key)
        if not jid:
            probs.append("%s: no job in the report" % key)
            continue
        mine = [s for s in spans if s["id"] in jid]
        ch = [s for s in mine if s["stage"] == idx["currency_hash"]]
        files = [s for s in mine if s["stage"] == idx["file"]]
        hs = [s for s in mine if s["stage"] == idx["hash_source"]]
        holding = [f for f in files if any(h["thread"] == f["thread"] and f["start"] <= h["start"]
                                           and h["end"] <= f["end"] for h in hs)]
        if len(ch) != 1:
            probs.append("%s: %d currency_hash span(s), want 1" % (key, len(ch)))
        if len(holding) != 1:
            probs.append("%s: %d file span(s) holding a hash_source span, want 1" % (key, len(holding)))
    return probs


def selftest_paths(t):
    n = "protected_out_paths"
    prot = protected_list()
    for path, refused in (("G:\\x", True), ("C:\\_mount\\x", True), ("B:\\testdata\\x", True), ("B:\\testdata2\\x", False),
                          ("C:\\_mount2\\x", False), ("\\\\server\\share\\x", True),
                          (os.path.join(os.environ.get("LOCALAPPDATA", "C:\\nolocal"), "sdc", "x"), True)):
        why = out_problem(path, prot)
        t.ok("%s: %s is %s" % (n, path, "refused" if refused else "allowed (a sibling prefix is not inside)"), (why is not None) == refused, why)
    body = re.search(r"function Get-ProtectedList \{(.*?)\n\}", read_text(PATHS_PS1), re.S).group(1)
    quoted = [q for q in re.findall(r"'([^']+)'", body) if q != "sdc"]
    joins = len(re.findall(r"Join-Path \$env:", body))
    t.ok(n + ": the parsed list is the .ps1's list (every quoted path and both env entries)", len(prot) == len(quoted) + joins and "G:\\" in prot
         and "B:\\testdata" in prot and "C:\\_mount" in prot, (len(prot), len(quoted), joins))
    tmp = tempfile.mkdtemp(prefix="profile-summary-selftest-")  # left in place (EP-R3)
    fake = os.path.join(tmp, "paths.ps1")
    write_new(fake, "function Get-ProtectedList {\n    @(\n        'Z:\\extra', 'G:\\',\n        (Join-Path $env:LOCALAPPDATA 'sdc')\n    )\n}\n")
    got = protected_list(fake)
    t.ok(n + ": a changed .ps1 list is what is read (the two cannot drift)", got[0] == "Z:\\extra" and got[1] == "G:\\" and len(got) == 3, got)
    link = os.path.join(tmp, "link")
    target = os.path.dirname(PATHS_PS1)
    r = os.system('cmd /c mklink /J "%s" "%s" >nul' % (link, target))
    t.ok(n + ": a junction into a protected folder is resolved and refused", r == 0 and out_problem(os.path.join(link, "x"), [target]) is not None, r)


def selftest_scale(t):
    n = "scale_and_equivalence"
    # equivalence with the slow references on a small input
    spans = []
    for i in range(16):
        o = S(0.5 * i)
        spans += [("decode", i, o, o + S(0.5)), ("encode_band", i, o + S(0.5), o + S(10)), ("plan_admit_wait", i, o, o + S(0.2))]
    rep = mk_report(spans, [call(16, S(17.5))])
    an = analyse(rep)
    smp = [{"t_ms": float(x), "cpu_ms": 13.0 * x + (x % 7), "working_set": 0, "private_bytes": 0, "threads": 1} for x in range(0, 17501, 130)]
    fast, slow = cpu_buckets(an, smp, 16), _slow_cpu_buckets(an, smp, 16)
    same = len(fast) == len(slow) and all(
        abs(a[k] - b[k]) < 1e-6 for a, b in zip(fast, slow) for k in ("cores_used", "idle_cores", "recorded", "unrecorded")) and all(
        a["too_many"] == b["too_many"] and a["idle_while_admitting"] == b["idle_while_admitting"] for a, b in zip(fast, slow))
    t.ok(n + ": cpu_buckets equals the quadratic reference on a small input", same and len(fast) > 50, (len(fast), len(slow)))
    jobs = [{"id": 0, "call": 0, "output": "a.dds"}, {"id": 1, "call": 0, "output": "b.dds"}, {"id": 2, "call": 0, "output": "c.dds"}]
    sp = [("currency_hash", 0, 1, 2, 0), ("hash_source", 0, 4, 5, 0), ("file", 0, 3, 8, 0),
          ("hash_source", 0, 14, 15, 1), ("file", 0, 13, 18, 1), ("currency_hash", 0, 21, 22, 2), ("currency_hash", 0, 23, 24, 2)]
    srep = mk_report(sp, [call(1, 100)], jobs=jobs)
    exp = {"a.dds", "b.dds", "c.dds"}
    copied = {k: rec(k) for k in exp}
    att = mk_att("sc", cohort="warm-stale", report=srep)
    f, s = stale_span_problems(att, exp, copied), _slow_stale(att, exp, copied)
    t.ok(n + ": stale_span_problems equals the per-key scan on a small input (and finds b.dds and c.dds)", f == s and len(f) == 3, (f, s))
    # a large report: 5,000 keys, 205,000 spans
    big_spans, big_jobs = [], []
    for i in range(5000):
        th, b = i % 16, (i // 16) * US(50000)
        big_jobs.append({"id": i, "call": 0, "output": "k%d.dds" % i})
        big_spans += [("currency_hash", th, b, b + US(1000), i), ("hash_source", th, b + US(2000), b + US(4000), i)]
        big_spans += [("encode_band", th, b + US(5000 + 1000 * k), b + US(6000 + 1000 * k), i) for k in range(37)]
        big_spans.append(("file", th, b, b + US(44000), i))
    big = mk_report(big_spans, [call(16, S(20))], jobs=big_jobs)
    sm = [{"t_ms": float(x), "cpu_ms": 8.0 * x, "working_set": 1, "private_bytes": 1, "threads": 16} for x in range(0, 20001, 250)]
    keys = {"k%d.dds" % i for i in range(5000)}
    battr = mk_att("big", cohort="warm-stale", report=big)
    t0 = time.time()
    a_big = analyse(big)
    bk = cpu_buckets(a_big, sm, 16)
    pr = stale_span_problems(battr, keys, {k: rec(k) for k in keys})
    took = time.time() - t0
    t.ok(n + ": %d spans and %d keys finish in under 10 s (took %.1f s) with consistent results" % (len(big["spans"]), len(keys), took),
         len(big["spans"]) >= 200000 and took < 10 and len(bk) == 80 and pr == [] and a_big["work_ns"]["encode_band"] == 5000 * 37 * US(1000),
         (len(big["spans"]), took, len(bk), pr[:2]))


def selftest_cli(t):
    n = "summary_cli_and_render"
    # the summary's wall split (labelled) and the corpus verdict only from warm-fresh
    spans = [("plan_file", 0, 0, S(3))] + [("encode_band", i, S(3), S(10)) for i in range(16)]
    rep = mk_report(spans, [call(1, S(3), 0, "plan", 0), call(16, S(10), S(3), "run", 1)], wall=S(10))
    mk = lambda name, cohort: mk_att(name, cohort=cohort, build="release", hashes={"a.dds": "h"}, manifest={"a.dds": rec("a.dds")},
                                     report=rep, externalWallMs=10000, logicalCores=16, **({"purgeStamp": "p"} if cohort == "cold-fresh" else {}))
    groups = [{"label": "fx", "cohort": "warm-fresh", "attempts": ["w"], "loaded": [mk("w", "warm-fresh")]},
              {"label": "fx", "cohort": "cold-fresh", "attempts": ["c"], "loaded": [mk("c", "cold-fresh")]}]
    out, _ = summarise(groups, {"fx": "w"}, {"fx": ({"a.dds"}, [], [])}, None, False)
    md = render_md(out)
    t.ok(n + ": the wall split reads no-encode 30.0 % and is labelled as command wall", "no-encode 30.0 %" in md and "command wall" in md, md[:400])
    t.ok(n + ": one corpus verdict, from warm-fresh; the cold group is informational", out["labels"]["fx"]["stop_verdict"] == "stop"
         and md.count("Verdict (the corpus verdict") == 1 and "Informational only" in md, md.count("Verdict"))
    out2, _ = summarise(groups, {"fx": "w"}, {"fx": ({"a.dds"}, [], [])}, None, True)
    t.ok(n + ": contaminated attempts allowed marks the markdown NOT ACCEPTANCE EVIDENCE", "NOT ACCEPTANCE EVIDENCE" in render_md(out2))
    # the cold line says what was done, by the recorded method (EP-C5)
    disk = {"start": {"corpus": {"name": "2 F:", "read": 0, "write": 0}, "output": {"name": "2 F:", "read": 0, "write": 0}},
            "end": {"corpus": {"name": "2 F:", "read": 990, "write": 0}, "output": {"name": "2 F:", "read": 0, "write": 5}}}
    cold_ev = mk_att("ce", cohort="cold-fresh", build="release-off", hashes={"a.dds": "h"}, manifest={"a.dds": rec("a.dds")}, disk=disk,
                     purgeStamp="evict-20261126-000000.json", coldMethod=EVICT_METHOD,
                     cold={"method": EVICT_METHOD, "cold": True, "corpusBytes": 1000, "diskReadBytes": 990})
    cold_pu = mk_att("cp", cohort="cold-fresh", build="release-off", hashes={"a.dds": "h"}, manifest={"a.dds": rec("a.dds")}, disk=disk,
                     purgeStamp="purge-20261126-000000.json", coldMethod="standby-purge")
    out3, _ = summarise([{"label": "fx", "cohort": "cold-fresh", "attempts": ["ce", "cp"], "loaded": [cold_ev, cold_pu]}],
                        {"fx": "w"}, {"fx": ({"a.dds"}, [], [])}, None, False)
    md3 = render_md(out3)
    line_ev = next((x for x in md3.splitlines() if x.startswith("- ce:")), "")
    line_pu = next((x for x in md3.splitlines() if x.startswith("- cp:")), "")
    t.ok(n + ": a per-file eviction attempt's disk line says what EP-C5 did, not that the standby list was purged",
         "each corpus file's cached pages were dropped (per-file no-buffering open); the exe, DLLs and file system metadata stay cached" in line_ev
         and "standby" not in line_ev and "read 990 of 1000 corpus bytes" in line_ev, line_ev)
    t.ok(n + ": a standby-purge attempt's disk line says the standby list was purged", "standby list was purged" in line_pu
         and "per-file" not in line_pu, line_pu)
    t.ok(n + ": the disk header claims no purge for every cold run", "the standby list was purged before a cold run" not in md3, md3[:300])
    val = mk_att("v1", build="release-off", hashes={"a.dds": "h"}, manifest={"a.dds": rec("a.dds")}, validate=True)
    plain = mk_att("v2", build="release-off", hashes={"a.dds": "h"}, manifest={"a.dds": rec("a.dds")})
    out4, _ = summarise([{"label": "fx", "cohort": "warm-fresh", "attempts": ["v1", "v2"], "loaded": [val, plain]}],
                        {"fx": "w"}, {"fx": ({"a.dds"}, [], [])}, None, False)
    md4 = render_md(out4)
    t.ok(n + ": an attempt run with -Validate says so in its line, and only that one",
         any(x.startswith("| v1 (--validate) |") for x in md4.splitlines()) and any(x.startswith("| v2 |") for x in md4.splitlines()), md4)
    # disk-backed: run_check exit codes, build_summary and run_accept
    tmp = tempfile.mkdtemp(prefix="profile-summary-selftest-")  # left in place (EP-R3)
    rr = os.path.join(tmp, "runroot")
    os.makedirs(rr)
    write_new(os.path.join(rr, "expected-fx.txt"), "a.dds\nb.dds\n")

    def mk_dir(name, cohort, build, exe, keys):
        d = os.path.join(rr, name)
        os.makedirs(os.path.join(d, "out", "_sdc"))
        meta = {"attempt": name, "label": "fx", "cohort": cohort, "build": build, "exeSha256": exe, "finished": True, "exitCode": 0,
                "externalWallMs": 100, "logicalCores": 16, "contention": {"contaminated": False, "reasons": []}}
        write_new(os.path.join(d, "meta.json"), json.dumps(meta))
        write_new(os.path.join(d, "hashes.csv"), "path,sha256\n" + "".join("%s,h%s\n" % (k, k) for k in keys))
        write_new(os.path.join(d, "out", "_sdc", "manifest.jsonl"), "".join(json.dumps(rec(k)) + "\n" for k in keys))
        if build in RECORDER_BUILDS:
            os.makedirs(os.path.join(d, "profile"))
            write_new(os.path.join(d, "profile", "profile-1.json"), json.dumps(rep))
        return d
    ref_d = mk_dir("ref", "warm-fresh", "base", "B1", ["a.dds", "b.dds"])
    ok_d = mk_dir("ok", "warm-fresh", "release-off", "E1", ["a.dds", "b.dds"])
    bad_d = mk_dir("bad", "warm-fresh", "release-off", "E1", ["a.dds"])
    ns = lambda **k: argparse.Namespace(**dict(dict(check=None, reference=None, runroot=rr, out=None, accept=None, allow_contaminated=False), **k))

    def captured(fn, *a):
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            code = fn(*a)
        return code, buf.getvalue()
    code, text = captured(run_check, ns(check=ok_d, reference=ref_d))
    t.ok(n + ": run_check exits 0 on a good attempt", code == 0 and text.startswith("OK"), (code, text))
    code, text = captured(run_check, ns(check=bad_d, reference=ref_d))
    t.ok(n + ": run_check exits 1 naming the missing key", code == 1 and "b.dds" in text and "not written" in text, (code, text))
    code, text = captured(run_check, ns(check=ref_d))
    t.ok(n + ": run_check on a reference alone checks it against the expected set", code == 0, (code, text))
    acc = os.path.join(tmp, "accept.txt")
    write_new(acc, "[fx warm-fresh]\nok\nreference ref\n")
    out_dir = os.path.join(tmp, "summary-out")
    code, text = captured(run_accept, ns(accept=acc, out=out_dir))
    t.ok(n + ": run_accept writes summary.json and summary.md from accepted attempts", code == 0 and os.path.isfile(os.path.join(out_dir, "summary.md"))
         and os.path.isfile(os.path.join(out_dir, "summary.json")), (code, text))
    write_new(acc + "2", "[fx warm-fresh]\nbad\nreference ref\n")
    code, text = captured(run_accept, ns(accept=acc + "2", out=os.path.join(tmp, "summary-out2")))
    t.ok(n + ": run_accept refuses an attempt that fails its check (exit 1, reason, nothing created)",
         code == 1 and "b.dds" in text and not os.path.exists(os.path.join(tmp, "summary-out2")), (code, text))
    # the protected folder is one inside tmp, named by a stand-in list, so a regressed check can only write into tmp
    global PATHS_PS1
    keep, PATHS_PS1 = PATHS_PS1, os.path.join(tmp, "stand-in-paths.ps1")
    try:
        prot_dir = os.path.join(tmp, "protected-area")
        write_new(PATHS_PS1, "function Get-ProtectedList {\n    @(\n        '%s'\n    )\n}\n" % prot_dir)
        with contextlib.redirect_stdout(io.StringIO()) as buf:
            code = main(["--accept", acc, "--runroot", rr, "--out", os.path.join(prot_dir, "out")])
    finally:
        PATHS_PS1 = keep
    t.ok(n + ": a protected --out is refused before anything is created", code == 1 and "protected" in buf.getvalue()
         and not os.path.exists(os.path.join(prot_dir, "out")), (code, buf.getvalue()))
    try:
        with contextlib.redirect_stderr(io.StringIO()):
            main(["--check", ok_d, "--runroot", rr, "--allow-contaminated"])
        t.ok(n + ": --allow-contaminated is refused with --check", False)
    except SystemExit as e:
        t.ok(n + ": --allow-contaminated is refused with --check (usage error, exit 2)", e.code == 2, e.code)
    write_new(acc + "3", "[fx warm-fresh]\nreference ref\n")
    code, text = captured(run_accept, ns(accept=acc + "3", out=os.path.join(tmp, "summary-out3")))
    t.ok(n + ": a header with no attempts is a named failure, not a traceback", code == 1 and "lists no attempts" in text, (code, text))
    code, text = captured(main, ["--accept", acc, "--runroot", os.path.join(tmp, "nope"), "--out", os.path.join(tmp, "summary-out4")])
    t.ok(n + ": a missing run root is a FAIL line, not a traceback", code == 1 and text.startswith("FAIL"), (code, text))
    # EP-C9 (review I5): an overhead release attempt counts as warm-fresh only through an explicit, logged relabel
    mk_dir("ov-base", "overhead", "base", "B1", ["a.dds", "b.dds"])
    mk_dir("ov-off", "overhead", "release-off", "E1", ["a.dds", "b.dds"])
    mk_dir("ov-rel", "overhead", "release", "E1", ["a.dds", "b.dds"])
    overhead = "[fx overhead]\nov-base\nov-off\nov-rel\nreference ref\n"

    def accept(tag, warm_lines):
        path = os.path.join(tmp, "accept-%s.txt" % tag)
        write_new(path, overhead + "[fx warm-fresh]\n" + warm_lines)
        out = os.path.join(tmp, "summary-%s" % tag)
        code, text = captured(main, ["--accept", path, "--runroot", rr, "--out", out])
        return code, text, out
    code, text, out = accept("plain", "ov-rel\n")
    t.ok(n + ": an overhead attempt listed under warm-fresh without a relabel line is refused", code == 1
         and "meta.json says fx overhead, the list says fx warm-fresh" in text and not os.path.exists(out), (code, text))
    code, text, out = accept("relabel", "relabel ov-rel from overhead\n")
    md = read_text(os.path.join(out, "summary.md")) if code == 0 else ""
    sj = json.loads(read_text(os.path.join(out, "summary.json"))) if code == 0 else {}
    t.ok(n + ": a relabelled overhead release attempt is accepted as warm-fresh, printed and recorded", code == 0
         and "RELABEL ov-rel (fx): recorded overhead, release build; counted as warm-fresh" in text
         and "## Relabelled attempts" in md and "ov-rel (fx): recorded overhead, release build; counted as warm-fresh" in md
         and "### warm-fresh" in md and sj.get("relabelled") == [{"attempt": "ov-rel", "label": "fx", "recorded": "overhead", "build": "release", "listed": "warm-fresh"}],
         (code, text))
    code, text, out = accept("off", "relabel ov-off from overhead\n")
    t.ok(n + ": a relabel of a release-off overhead attempt is refused", code == 1 and "only a release build may be relabelled" in text, (code, text))
    code, text, out = accept("wrongfrom", "relabel ref from overhead\n")
    t.ok(n + ": a relabel line whose recorded cohort is wrong is refused", code == 1
         and "the relabel line says from overhead, meta.json says warm-fresh" in text, (code, text))
    path = os.path.join(tmp, "accept-stale.txt")
    write_new(path, overhead + "[fx warm-noop]\nrelabel ov-rel from overhead\n")
    code, text = captured(main, ["--accept", path, "--runroot", rr, "--out", os.path.join(tmp, "summary-stale")])
    t.ok(n + ": a relabel to any cohort but warm-fresh is refused", code == 1 and "relabel from overhead to warm-noop is not allowed" in text, (code, text))
    try:
        parse_accept("[fx warm-fresh]\nrelabel ov-rel overhead\nreference ref\n")
        t.ok(n + ": a malformed relabel line is a named parse error", False)
    except ValueError as e:
        t.ok(n + ": a malformed relabel line is a named parse error", "relabel <attempt> from <recorded cohort>" in str(e), str(e))


def self_test():
    t = T()
    for fn in (selftest_accounting, selftest_stats, selftest_acceptance, selftest_mipq, selftest_paths, selftest_scale, selftest_cli):
        try:
            fn(t)
        except Exception as e:  # a crashing case is a failure, with its traceback
            import traceback
            traceback.print_exc()
            t.ok(fn.__name__ + " ran", False, repr(e))
    print("%s: %d failed" % ("FAIL" if t.fail else "ALL PASS", t.fail))
    return 1 if t.fail else 0


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--accept")
    ap.add_argument("--check")
    ap.add_argument("--reference")
    ap.add_argument("--runroot")
    ap.add_argument("--out")
    ap.add_argument("--mipq", nargs="+")
    ap.add_argument("--allow-contaminated", action="store_true")
    args = ap.parse_args(argv)
    if args.self_test:
        return self_test()
    if args.allow_contaminated and not args.accept:
        ap.error("--allow-contaminated is only for --accept (a --check gate is never relaxed)")
    try:
        if args.check:
            if not args.runroot:
                ap.error("--check needs --runroot")
            return run_check(args)
        if args.accept:
            if not (args.runroot and args.out):
                ap.error("--accept needs --runroot and --out")
            return run_accept(args)
        if args.mipq:
            if not args.out:
                ap.error("--mipq needs --out")
            return run_mipq(args)
    except ValueError as e:
        # a protected --out is `--out <path> overlaps the protected path ...`
        print("FAIL %s" % e)
        return 1
    except (OSError, KeyError, TypeError, IndexError, AttributeError) as e:
        print("FAIL %s: %s (a missing or malformed input, or an unusable path)" % (type(e).__name__, e))
        return 1
    ap.error("one of --accept, --check, --mipq or --self-test")


if __name__ == "__main__":
    sys.exit(main())