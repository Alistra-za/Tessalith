"""Copy the pinned Community Shaders include closure.

Usage: python -I tools/cs-bundle.py <Shaders dir> <SettingsDefault.json> <out dir> [--override <rel>=<path>]...

`--override` (repeatable) copies <path> in place of <Shaders dir>/<rel>, and follows its includes.

Walks `#include "..."` from the three entry files, ignoring preprocessor
conditions (so the result is the union), resolves each include against the
includer's folder and then the root, and copies the bytes unchanged. Prints the
file count and total size; exits 1 on an unresolved include.
"""
import posixpath
import re
import shutil
import sys
from pathlib import Path

ENTRIES = [
    "Lighting.hlsl",
    "DeferredCompositeCS.hlsl",
    "DynamicCubemaps/SpecularIrradianceCS.hlsl",
]
# shortcut: this walk and `includes::expand` differ on `..`/`.` segments and on `<...>` includes; this pin is
# unaffected, and the closure tests catch a difference at the next pin update.
INCLUDE = re.compile(r'^\s*#\s*include\s+"([^"]+)"', re.M)


def source(root: Path, rel: str, overrides: dict[str, Path]) -> Path:
    return overrides.get(rel, root / rel)


def closure(root: Path, overrides: dict[str, Path]) -> list[str]:
    seen: dict[str, None] = {}
    todo = list(ENTRIES)
    bad = []
    while todo:
        rel = todo.pop()
        if rel in seen:
            continue
        seen[rel] = None
        text = source(root, rel, overrides).read_text(encoding="utf-8", errors="replace")
        for inc in INCLUDE.findall(text):
            here = posixpath.normpath(posixpath.join(posixpath.dirname(rel), inc))
            there = posixpath.normpath(inc)
            hit = next((c for c in (here, there) if source(root, c, overrides).is_file()), None)
            if hit is None:
                bad.append(f"{rel}: unresolved include \"{inc}\"")
            else:
                todo.append(hit)
    if bad:
        print("\n".join(bad), file=sys.stderr)
        sys.exit(1)
    return sorted(seen)


def main() -> None:
    args = sys.argv[1:]
    overrides: dict[str, Path] = {}
    while "--override" in args:
        i = args.index("--override")
        if i + 1 >= len(args) or "=" not in args[i + 1]:
            sys.exit(__doc__)
        rel, _, path = args[i + 1].partition("=")
        overrides[posixpath.normpath(rel)] = Path(path)
        del args[i : i + 2]
    if len(args) != 3:
        sys.exit(__doc__)
    root, settings, out = Path(args[0]), Path(args[1]), Path(args[2])
    files = closure(root, overrides)
    missing = [r for r in overrides if r not in files]
    if missing:
        sys.exit(f"--override names a file outside the closure: {missing}")
    total = 0
    for rel in files:
        dst = out / rel
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source(root, rel, overrides), dst)
        total += dst.stat().st_size
    shutil.copyfile(settings, out / "SettingsDefault.json")
    print(f"{len(files)} shader files, {total / 1024:.1f} KiB (+ SettingsDefault.json)")


if __name__ == "__main__":
    main()
