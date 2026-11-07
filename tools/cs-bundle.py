"""Copy the pinned Community Shaders include closure.

Usage: python -I tools/cs-bundle.py <Shaders dir> <SettingsDefault.json> <out dir>

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
INCLUDE = re.compile(r'^\s*#\s*include\s+"([^"]+)"', re.M)


def closure(root: Path) -> list[str]:
    seen: dict[str, None] = {}
    todo = list(ENTRIES)
    bad = []
    while todo:
        rel = todo.pop()
        if rel in seen:
            continue
        seen[rel] = None
        text = (root / rel).read_text(encoding="utf-8", errors="replace")
        for inc in INCLUDE.findall(text):
            here = posixpath.normpath(posixpath.join(posixpath.dirname(rel), inc))
            there = posixpath.normpath(inc)
            hit = next((c for c in (here, there) if (root / c).is_file()), None)
            if hit is None:
                bad.append(f"{rel}: unresolved include \"{inc}\"")
            else:
                todo.append(hit)
    if bad:
        print("\n".join(bad), file=sys.stderr)
        sys.exit(1)
    return sorted(seen)


def main() -> None:
    if len(sys.argv) != 4:
        sys.exit(__doc__)
    root, settings, out = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
    files = closure(root)
    total = 0
    for rel in files:
        dst = out / rel
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(root / rel, dst)
        total += dst.stat().st_size
    shutil.copyfile(settings, out / "SettingsDefault.json")
    print(f"{len(files)} shader files, {total / 1024:.1f} KiB (+ SettingsDefault.json)")


if __name__ == "__main__":
    main()
