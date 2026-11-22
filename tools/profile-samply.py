r"""samply frame attribution (plan 2026-11-21-engine-profile, Task 5; spec 7.5). Standard library only.

  py -3.13 -I -X utf8 tools\profile-samply.py <samply.json.gz> --out <file.md> [--syms <sidecar>]
  py -3.13 -I -X utf8 tools\profile-samply.py --self-test

Reads a processed profile (`threads[].stackTable`, `frameTable`, `funcTable`, `stringArray`, `samples`) and, for
frames the profile left as bare addresses, the `--unstable-presymbolicate` sidecar (`<profile>.syms.json`, or the
profile name without `.gz`). Samples are attributed by stack frame only: the recorder's clock and samply's timeline
share no epoch. Prints self and inclusive sample counts of spec 7.5's fixed frames, the top 30 functions by self
time, and the fixed frames never seen. EP-R3: --out is opened with mode "x" (it fails when the file exists); the
inputs are only read.

# shortcut: the sidecar layout is read from the field names samply 0.13.1 embeds (string_table; data[] with
# debug_name, debug_id, code_id, symbol_table[] of {rva, size, symbol}, known_addresses[] of [rva, symbol_table
# index]); no sample was on disk to confirm it. A missing key stops with a message; upgrade when a real run is seen.
"""
import argparse
import gzip
import json
import os
import re
import sys

TOP = 30


def word(name):
    return re.compile(r"(?<![A-Za-z0-9_])%s(?![A-Za-z0-9_])" % re.escape(name))


FIXED = [(n, word(n)) for n in (
    "srgb_to_linear", "linear_to_srgb", "powf", "to_encoder_input", "finalize", "normalize_vector", "downsample",
    "area_resample", "dilate_under_zero_alpha", "sha256_file", "gather_blocks", "bc7e_compress_blocks",
    "bc6h_compress_rows", "decode_level", "write_atomic", "FlushFileBuffers")] + [
    ("image/png/fdeflate decode frames", re.compile(r"(^|::)(image|png|fdeflate)::")),
    ("bcenc entry points", re.compile(r"(^|::)bcenc::")),
    ("serde_json serialisation", re.compile(r"serde_json::(ser|to_)")),
]
HEX = re.compile(r"^0x[0-9a-fA-F]+$")


class ProfileError(Exception):
    pass


def shared_summary():
    """tools/profile-summary.py holds the one Python copy of the spec 7.8 path check (it parses profile-paths.ps1).
    Loaded by path, because -I keeps the script folder off sys.path."""
    import importlib.util
    sys.dont_write_bytecode = True  # no __pycache__ beside the tools
    path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "profile-summary.py")
    spec = importlib.util.spec_from_file_location("profile_summary", path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def need(d, key, what):
    if not isinstance(d, dict) or key not in d:
        raise ProfileError("the profile has no %s (missing key %r)" % (what, key))
    return d[key]


def load_json(path):
    opener = gzip.open if path.endswith(".gz") else open
    with opener(path, "rt", encoding="utf-8") as f:
        return json.load(f)


def sidecar_candidates(path):
    c = [path + ".syms.json"]
    if path.endswith(".gz"):
        c.append(path[:-3] + ".syms.json")
    return c


def check_sidecar(syms):
    data = need(syms, "data", "sidecar `data` array")
    need(syms, "string_table", "sidecar `string_table`")
    for e in data:
        for k in ("debug_name", "symbol_table", "known_addresses"):
            need(e, k, "sidecar library entry")
    return syms


def lookup(syms, debug_name, addr):
    """The symbol name for an address inside one library of the sidecar, or None."""
    strings = syms["string_table"]
    for e in syms["data"]:
        if e["debug_name"] != debug_name:
            continue
        table = e["symbol_table"]
        for a, idx in e["known_addresses"]:
            if a == addr:
                return strings[table[idx]["symbol"]]
        for s in table:
            if s["rva"] <= addr < s["rva"] + s["size"]:
                return strings[s["symbol"]]
    return None


def thread_names(profile, thread, syms):
    """Resolved function name per frame index; unresolved ones keep their address text. Returns (names, unresolved)."""
    strings = thread.get("stringArray") or profile.get("shared", {}).get("stringArray") or profile.get("stringArray")
    if strings is None:
        raise ProfileError("the profile has no stringArray (thread, shared or profile level)")
    ft, fn = need(thread, "frameTable", "frameTable"), need(thread, "funcTable", "funcTable")
    func_of = need(ft, "func", "frameTable.func")
    names = need(fn, "name", "funcTable.name")
    res_of = fn.get("resource")
    rt = thread.get("resourceTable") or {}
    libs = profile.get("libs") or []
    addr_of = ft.get("address")
    out, unresolved = [], 0
    for f, fi in enumerate(func_of):
        name = strings[names[fi]]
        if HEX.match(name):
            addr = addr_of[f] if addr_of and addr_of[f] is not None and addr_of[f] >= 0 else int(name, 16)
            lib = None
            if res_of is not None and rt.get("lib") and res_of[fi] is not None and res_of[fi] >= 0:
                li = rt["lib"][res_of[fi]]
                if li is not None and 0 <= li < len(libs):
                    lib = libs[li].get("debugName") or libs[li].get("name")
            hit = lookup(syms, lib, addr) if syms is not None and lib else None
            if hit is None:
                unresolved += 1
            name = hit or name
        out.append(name)
    return out, unresolved


def attribute(profile, syms, fixed=FIXED):
    total = 0
    self_c, incl_c = {}, {}
    fixed_self = {n: 0 for n, _ in fixed}
    fixed_incl = {n: 0 for n, _ in fixed}
    unresolved = 0
    for thread in need(profile, "threads", "threads array"):
        st, smp = need(thread, "stackTable", "stackTable"), need(thread, "samples", "samples")
        frame_of, prefix = need(st, "frame", "stackTable.frame"), need(st, "prefix", "stackTable.prefix")
        names, un = thread_names(profile, thread, syms)
        unresolved += un
        stacks = need(smp, "stack", "samples.stack")
        weights = smp.get("weight")
        for k, s in enumerate(stacks):
            if s is None:
                continue
            w = weights[k] if weights and weights[k] is not None else 1
            total += w
            chain = []
            while s is not None:
                chain.append(names[frame_of[s]])
                s = prefix[s]
            leaf = chain[0]
            self_c[leaf] = self_c.get(leaf, 0) + w
            for n in set(chain):
                incl_c[n] = incl_c.get(n, 0) + w
            for label, rx in fixed:
                if rx.search(leaf):
                    fixed_self[label] += w
                if any(rx.search(n) for n in chain):
                    fixed_incl[label] += w
    if total == 0:
        raise ProfileError("the profile holds no samples")
    never = [n for n, _ in fixed if fixed_incl[n] == 0]
    top = sorted(self_c.items(), key=lambda kv: (-kv[1], kv[0]))[:TOP]
    return {"samples": total, "unresolved_frames": unresolved, "fixed_self": fixed_self, "fixed_incl": fixed_incl,
            "never_seen": never, "top_self": top}


def render(res, source):
    n = res["samples"]
    L = ["# samply attribution (%s)" % source, "",
         "%d samples; %d frame(s) stayed unresolved after the sidecar. CPU samples show executing code only: blocked time "
         "(sync_all, lock waits, disk reads) is visible in span time, not here." % (n, res["unresolved_frames"]), "",
         "## Fixed frames (spec 7.5)", "", "| frame | self samples | self share | inclusive samples | inclusive share |", "|---|---|---|---|---|"]
    for label in res["fixed_self"]:
        s, i = res["fixed_self"][label], res["fixed_incl"][label]
        L.append("| %s | %d | %.1f %% | %d | %.1f %% |" % (label, s, 100.0 * s / n, i, 100.0 * i / n))
    L += ["", "## Top %d functions by self time" % TOP, "", "| function | self samples | share |", "|---|---|---|"]
    for name, c in res["top_self"]:
        L.append("| %s | %d | %.1f %% |" % (name.replace("|", "\\|"), c, 100.0 * c / n))
    L += ["", "## Fixed frames never seen", "", ", ".join(res["never_seen"]) if res["never_seen"] else "none", ""]
    return "\n".join(L)


# ---------------------------------------------------------------- self-test

def synthetic_profile():
    strings = ["main", "finalize", "normalize_vector", "0x2000", "0x3000", "sdc.pdb"]
    thread = {
        "stringArray": strings,
        "funcTable": {"name": [0, 1, 2, 3, 4], "resource": [-1, -1, -1, 0, 0]},
        "resourceTable": {"lib": [0], "name": [5]},
        "frameTable": {"func": [0, 1, 2, 3, 4], "address": [-1, -1, -1, 0x2000, 0x3000]},
        "stackTable": {"frame": [0, 1, 2, 3, 4], "prefix": [None, 0, 1, 0, 3]},
        "samples": {"stack": [2, 1, 4]},
    }
    return {"libs": [{"debugName": "sdc.pdb", "breakpadId": "AB"}], "threads": [thread]}


def synthetic_syms():
    return {"string_table": ["write_atomic", "FlushFileBuffers"],
            "data": [{"debug_name": "sdc.pdb", "debug_id": "AB", "code_id": None,
                      "symbol_table": [{"rva": 0x2000, "size": 0x10, "symbol": 0}, {"rva": 0x3000, "size": 0x10, "symbol": 1}],
                      "known_addresses": [[0x2000, 0], [0x3000, 1]]}]}


def self_test():
    fails = 0

    def ok(name, cond, detail=""):
        nonlocal fails
        print("%s %s%s" % ("PASS" if cond else "FAIL", name, "" if cond else ": " + str(detail)))
        fails += 0 if cond else 1
    r = attribute(synthetic_profile(), check_sidecar(synthetic_syms()))
    ok("frames_attribute_by_stack: finalize inclusive 2, self 1", (r["fixed_incl"]["finalize"], r["fixed_self"]["finalize"]) == (2, 1), r["fixed_incl"])
    ok("frames_attribute_by_stack: normalize_vector self 1", r["fixed_self"]["normalize_vector"] == 1)
    ok("frames_attribute_by_stack: FlushFileBuffers self 1 (sidecar)", r["fixed_self"]["FlushFileBuffers"] == 1 and r["fixed_incl"]["write_atomic"] == 1,
       (r["fixed_self"], r["fixed_incl"]))
    ok("frames_attribute_by_stack: powf is listed as never seen", "powf" in r["never_seen"] and "finalize" not in r["never_seen"], r["never_seen"])
    ok("frames_attribute_by_stack: no frame stays unresolved", r["unresolved_frames"] == 0, r["unresolved_frames"])
    bare = attribute(synthetic_profile(), None)
    ok("frames_attribute_by_stack: without the sidecar the two frames are counted unresolved, not silently lost",
       bare["unresolved_frames"] == 2 and bare["fixed_self"]["FlushFileBuffers"] == 0, bare["unresolved_frames"])
    try:
        check_sidecar({"data": []})
        ok("a sidecar without string_table stops loudly", False)
    except ProfileError:
        ok("a sidecar without string_table stops loudly", True)
    broken = synthetic_profile()
    del broken["threads"][0]["frameTable"]
    try:
        attribute(broken, None)
        ok("a profile without a frameTable stops loudly", False)
    except ProfileError:
        ok("a profile without a frameTable stops loudly", True)
    ok("the markdown lists the top functions and the never-seen frames", "## Top 30" in render(r, "x") and "powf" in render(r, "x"))
    import contextlib
    import io
    import tempfile
    tmp = tempfile.mkdtemp(prefix="profile-samply-selftest-")  # left in place (EP-R3)
    src = os.path.join(tmp, "p.json")
    with open(src, "x", encoding="utf-8") as f:
        json.dump(synthetic_profile(), f)
    with open(src + ".syms.json", "x", encoding="utf-8") as f:
        json.dump(synthetic_syms(), f)
    # a stand-in protected list that names a folder inside tmp, so a regressed check can only write into tmp
    mod = shared_summary()
    mod.PATHS_PS1 = os.path.join(tmp, "stand-in-paths.ps1")
    prot_dir = os.path.join(tmp, "protected-area")
    with open(mod.PATHS_PS1, "x", encoding="utf-8") as f:
        f.write("function Get-ProtectedList {\n    @(\n        '%s'\n    )\n}\n" % prot_dir)
    real = shared_summary
    globals()["shared_summary"] = lambda: mod
    buf = io.StringIO()
    try:
        with contextlib.redirect_stdout(buf):
            code = main([src, "--out", os.path.join(prot_dir, "out.md")])
    finally:
        globals()["shared_summary"] = real
    ok("a protected --out is refused before anything is created", code == 1 and "protected" in buf.getvalue()
       and not os.path.exists(os.path.join(prot_dir, "out.md")), (code, buf.getvalue()))
    out_md = os.path.join(tmp, "out.md")
    with contextlib.redirect_stdout(io.StringIO()):
        code1 = main([src, "--out", out_md])
    buf = io.StringIO()
    with contextlib.redirect_stdout(buf):
        code2 = main([src, "--out", out_md])
    ok("an existing --out is a FAIL line (not a traceback) and the file is not replaced", code1 == 0 and code2 == 1 and "FileExistsError" in buf.getvalue()
       and "powf" in open(out_md, encoding="utf-8").read(), (code1, code2, buf.getvalue()))
    print("%s: %d failed" % ("FAIL" if fails else "ALL PASS", fails))
    return 1 if fails else 0


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("profile", nargs="?")
    ap.add_argument("--out")
    ap.add_argument("--syms")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args(argv)
    if args.self_test:
        return self_test()
    if not (args.profile and args.out):
        ap.error("a profile and --out are required")
    try:
        why = shared_summary().out_problem(args.out)  # spec 7.8, before anything is read or created
        if why:
            print("FAIL --out %s %s" % (args.out, why))
            return 1
        profile = load_json(args.profile)
        sp = args.syms or next((c for c in sidecar_candidates(args.profile) if os.path.isfile(c)), None)
        syms = check_sidecar(load_json(sp)) if sp else None
        if syms is None:
            print("warning: no .syms.json sidecar found; address-only frames stay unresolved", file=sys.stderr)
        text = render(attribute(profile, syms), os.path.basename(args.profile))
        with open(args.out, "x", encoding="utf-8", newline="\n") as f:  # never replaces a file (EP-R3)
            f.write(text)
    except (ProfileError, OSError, ValueError, KeyError, IndexError, TypeError) as e:
        print("FAIL %s: %s" % (type(e).__name__, e))
        return 1
    print("wrote %s" % args.out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
