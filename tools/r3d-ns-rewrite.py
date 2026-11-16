# Rewrites the cbuffers that CS declares inside namespaces, for the graphics-backend research (2026-11-16).
# FXC scopes such a cbuffer's members to the namespace (`SharedData::MipBias`); DXC and glslang make them global, so
# every use fails. This is an experiment to see what else stops those compilers, not a proposed patch.
#
# Each member `M` of a cbuffer declared inside namespace `N` becomes a global-safe `N_M`, and a line
# `static const T M = N_M;` is added inside the namespace right after the cbuffer, so `N::M` still resolves.
# Preprocessor lines inside the cbuffer are repeated around the aliases.
#
# Input: the folder `dump_permutations` wrote (crates/render3d/tests/backend_baseline.rs). Output: the same file
# names, `.hlsl` rewritten and `.json` copied, for tools\r3d-dxc-trial.ps1.
#
#   py -3.13 -I tools\r3d-ns-rewrite.py <dump dir> <out dir>
import re
import shutil
import sys
from pathlib import Path

NS = re.compile(r"\bnamespace\s+(\w+)")
CBUF = re.compile(r"^\s*cbuffer\s+\w+")
DIRECTIVE = re.compile(r"^\s*#\s*(\w+)")
MEMBER = re.compile(
    r"^(?P<indent>\s*)(?P<type>(?:(?:row_major|column_major)\s+)?[\w:]+(?:<[^>]*>)?)\s+"
    r"(?P<name>\w+)(?P<dims>(?:\s*\[[^\]]*\])*)\s*(?P<pack>:\s*packoffset\([^)]*\))?\s*;(?P<rest>.*)$"
)


def strip_comments(line: str, in_block: bool) -> tuple[str, bool]:
    """The code on `line` without // and /* */ comments, and whether a block comment is still open."""
    out, i = [], 0
    while i < len(line):
        if in_block:
            j = line.find("*/", i)
            if j < 0:
                return "".join(out), True
            i, in_block = j + 2, False
        elif line.startswith("//", i):
            break
        elif line.startswith("/*", i):
            in_block, i = True, i + 2
        else:
            out.append(line[i])
            i += 1
    return "".join(out), in_block


def rewrite(text: str, name: str) -> tuple[str, int]:
    out: list[str] = []
    stack: list[str | None] = []  # a namespace name, or None for any other brace scope
    conds: list[list[str | None]] = []  # `stack` as it was at each open #if
    pending: str | None = None
    in_block = False
    cb: dict | None = None  # the namespaced cbuffer being read
    renamed = 0
    for line in text.split("\n"):
        code, in_block = strip_comments(line, in_block)
        if cb is not None:
            if not cb["open"]:
                if "{" in code:
                    cb["open"] = True
                out.append(line)
                continue
            if code.strip().startswith("}"):
                out.append(line)
                out.extend(cb["alias"])
                cb = None
                continue
            if not code.strip():
                out.append(line)
                continue
            if code.lstrip().startswith("#"):
                out.append(line)
                cb["alias"].append(line)
                continue
            m = MEMBER.match(line)
            if not m:
                raise SystemExit(f"{name}: cannot read cbuffer member: {line!r}")
            g = m.groupdict()
            new = f"{cb['ns']}_{g['name']}"
            out.append(f"{g['indent']}{g['type']} {new}{g['dims']}{' ' + g['pack'] if g['pack'] else ''};{g['rest']}")
            cb["alias"].append(f"{g['indent']}static const {g['type']} {g['name']}{g['dims']} = {new};")
            renamed += 1
            continue
        if code.lstrip().startswith("#"):
            # The dump is not preprocessed: `#if A` ... `{` ... `#else` ... `{` ... `#endif` opens one scope, not
            # two. Each later branch restarts from the scopes open at its `#if`.
            d = DIRECTIVE.match(code)
            word = d.group(1) if d else ""
            if word in ("if", "ifdef", "ifndef"):
                conds.append(list(stack))
            elif word in ("else", "elif") and conds:
                stack = list(conds[-1])
            elif word == "endif" and conds:
                conds.pop()
            out.append(line)
            continue
        if CBUF.match(code) and stack and all(s is not None for s in stack):
            cb = {"ns": "_".join(s for s in stack if s), "open": "{" in code, "alias": []}
            out.append(line)
            continue
        m = NS.search(code)
        if m:
            pending = m.group(1)
        for ch in code:
            if ch == "{":
                stack.append(pending)
                pending = None
            elif ch == "}":
                if not stack:
                    raise SystemExit(f"{name}: unbalanced braces")
                stack.pop()
        out.append(line)
    if stack or cb is not None:
        raise SystemExit(f"{name}: unbalanced braces at the end ({len(stack)} open)")
    return "\n".join(out), renamed


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit(__doc__ or "usage: r3d-ns-rewrite.py <dump dir> <out dir>")
    src, dst = Path(sys.argv[1]), Path(sys.argv[2])
    dst.mkdir(parents=True, exist_ok=True)
    files = sorted(src.glob("*.hlsl"), key=lambda p: int(p.stem))
    total = 0
    for f in files:
        text, n = rewrite(f.read_text(encoding="utf-8", newline=""), f.name)
        (dst / f.name).write_text(text, encoding="utf-8", newline="")
        shutil.copyfile(f.with_suffix(".json"), dst / f.with_suffix(".json").name)
        total += n
    print(f"{len(files)} files, {total} members renamed, written to {dst}")


if __name__ == "__main__":
    main()
