"""Doc lint: the mechanical rules of docs/DOCUMENTATION.md (sections 1, 4, 6) and the modder-voice skill.

  py -3.13 -I tools/doc-lint.py [--root .]
      Prints one line per failure as <file>:<line>: <rule>: <what to do>; exits 1 when any rule fails, 2 when it
      found no wiki pages. A warning prints "warning:" and does not change the exit code.

Fail rules:
  history      README.md, docs/gui.md, docs/wiki/*.md: "used to" (not after is, are, be, was or were: "is used to
               set" is current state), "previously", "since <version>" (since 1.x, since 3.0, since v3.0.0),
               "in earlier versions", "(new)" (section 1; history goes in CHANGELOG.md). "now" and "no longer" are
               not checked.
  as-above     docs/wiki/*.md: "as above" and "as below" (a section never leans on the reader's memory).
  changelog    CHANGELOG.md: each released section's ### headings are an ordered subset of Main, Engine, GUI, CLI,
               Common, with Main first (section 6). 0.8.0-alpha.1 is allowlisted by name.
  budget       docs/wiki/*.md: size limits of section 4. Every page not starting with "_" is linked from _Sidebar.md.
  callout      docs/wiki/*.md: at most two IMPORTANT lines per page. README.md, docs/gui.md and docs/wiki/*.md: no
               BOM, no image with empty alt text.
Warning rule: six or more top-level bullets in a row on a wiki page (_Sidebar.md excluded).
"""
# shortcut: the scans read text line by line, so a phrase split across two lines is missed and a phrase inside a
# code fence is judged like prose; upgrade when a page needs either.
import argparse
import pathlib
import re
import sys

# Section 4 numbers. "About" means 5 % over (and 5 % under for the lower bound).
KB = 1024
SLACK = 1.05
GENERAL_BYTES = 15 * KB
GENERAL_WORDS = 2500
TABLE_PAGES = {"Texture-Types.md", "Project-Config.md", "Diagnostic-Codes.md", "PBR-Reference.md"}
TABLE_BYTES = 30 * KB
NO_LIMIT_PAGES = {"CLI-Reference.md"}
MIN_BYTES = 2 * KB

HISTORY = [
    # "is used to <verb>" says what a thing is for, not what it did.
    (re.compile(r"(?<!\bis )(?<!\bare )(?<!\bbe )(?<!\bwas )(?<!\bwere )\bused to\b", re.I), "used to"),
    (re.compile(r"\bpreviously\b", re.I), "previously"),
    (re.compile(r"\bsince v?\d+\.(?:x|\d+)(?:\.\d+)?\b", re.I), "since <version>"),
    (re.compile(r"\bin earlier versions\b", re.I), "in earlier versions"),
    (re.compile(r"\(new\)", re.I), "(new)"),
]
ABOVE = re.compile(r"\bas (?:above|below)\b", re.I)
CALLOUT = re.compile(r"^\s*(?:[-*+>]\s+)*(?:\*\*|\[!)?IMPORTANT\b")
EMPTY_ALT = re.compile(r"!\[\]\(")
BULLET = re.compile(r"^[-*+] ")
HEADINGS = ["Main", "Engine", "GUI", "CLI", "Common"]
CHANGELOG_ALLOW = {"0.8.0-alpha.1"}
BULLET_RUN = 6


def read(path):
    raw = path.read_bytes()
    return raw, raw.decode("utf-8-sig").replace("\r\n", "\n")


def lint(root):
    """([failure], [warning]) as strings; root is the repo root."""
    fails, warns = [], []
    root = pathlib.Path(root)

    def rel(p):
        return p.relative_to(root).as_posix()

    wiki = sorted((root / "docs/wiki").glob("*.md"))
    prose = [p for p in [root / "README.md", root / "docs/gui.md"] if p.exists()] + wiki

    for p in prose:
        raw, text = read(p)
        for n, line in enumerate(text.split("\n"), 1):
            for rx, word in HISTORY:
                if rx.search(line):
                    fails.append(f"{rel(p)}:{n}: history: \"{word}\" is history, and docs describe the current state "
                                 f"only. Say what the feature is, and put what changed in CHANGELOG.md "
                                 f"(docs/DOCUMENTATION.md section 1).")
            if EMPTY_ALT.search(line):
                fails.append(f"{rel(p)}:{n}: alt-text: an image has empty alt text. Write what the image shows "
                             f"(docs/DOCUMENTATION.md section 5).")
        if raw.startswith(b"\xef\xbb\xbf"):
            fails.append(f"{rel(p)}:1: bom: the file starts with a byte-order mark. Save it as UTF-8 without BOM.")

    for p in wiki:
        raw, text = read(p)
        lines = text.split("\n")
        for n, line in enumerate(lines, 1):
            if ABOVE.search(line):
                fails.append(f"{rel(p)}:{n}: as-above: \"as above\" and \"as below\" lean on the reader's memory. "
                             f"Name the thing it points at, or link to its section.")
        marks = [n for n, line in enumerate(lines, 1) if CALLOUT.match(line)]
        if len(marks) > 2:
            fails.append(f"{rel(p)}:{marks[2]}: callout: {len(marks)} IMPORTANT lines (lines "
                         f"{', '.join(map(str, marks))}); a page keeps at most two, for what silently breaks a game "
                         f"or a load order. Make the others plain text.")
        if p.name.startswith("_"):
            continue
        run, start = 0, 0
        for n, line in enumerate(lines, 1):
            if BULLET.match(line):
                if run == 0:
                    start = n
                run += 1
            elif line.strip() and not line.startswith((" ", "\t")):
                if run >= BULLET_RUN:
                    warns.append(f"{rel(p)}:{start}: warning: {run} top-level bullets in a row; past five, split them "
                                 f"under sub-headings by job (modder-voice).")
                run = 0
        if run >= BULLET_RUN:
            warns.append(f"{rel(p)}:{start}: warning: {run} top-level bullets in a row; past five, split them under "
                         f"sub-headings by job (modder-voice).")
        size = len(text.encode("utf-8"))
        words = len(text.split())
        if p.name in NO_LIMIT_PAGES:
            pass
        elif p.name in TABLE_PAGES:
            if size > TABLE_BYTES * SLACK:
                fails.append(f"{rel(p)}:1: budget: {size} bytes; a lookup-table page holds at most about 30 KB "
                             f"({int(TABLE_BYTES * SLACK)} bytes). Split it by command or section group.")
        else:
            if size > GENERAL_BYTES * SLACK or words > GENERAL_WORDS * SLACK:
                fails.append(f"{rel(p)}:1: budget: {size} bytes and {words} words; a page holds at most about 15 KB "
                             f"and 2,500 words ({int(GENERAL_BYTES * SLACK)} bytes, {int(GENERAL_WORDS * SLACK)} "
                             f"words). Split it and update _Sidebar.md.")
        if size < MIN_BYTES / SLACK:
            fails.append(f"{rel(p)}:1: budget: {size} bytes; a page under about 2 KB merges into its parent.")

    sidebar = root / "docs/wiki/_Sidebar.md"
    linked = read(sidebar)[1] if sidebar.exists() else ""
    for p in wiki:
        if not p.name.startswith("_") and f"({p.name}" not in linked:
            fails.append(f"docs/wiki/_Sidebar.md:1: sidebar: {p.name} is not linked from _Sidebar.md. Add it in the "
                         f"commit that adds the page.")

    changelog = root / "CHANGELOG.md"
    if changelog.exists():
        version, seen, in_code = None, [], False

        def check():
            if version is None or version == "Unreleased" or version in CHANGELOG_ALLOW or not seen:
                return
            names = [h for _, h in seen]
            order = [HEADINGS.index(h) for h in names if h in HEADINGS]
            if len(order) != len(names) or order != sorted(set(order)) or names[0] != "Main":
                fails.append(f"CHANGELOG.md:{seen[0][0]}: changelog: the ### headings of {version} are "
                             f"{', '.join(names)}; a released section uses Main first, then Engine, GUI, CLI, "
                             f"Common, in that order, an empty one left out (docs/DOCUMENTATION.md section 6).")

        for n, line in enumerate(read(changelog)[1].split("\n"), 1):
            if line.startswith("```"):
                in_code = not in_code
            if in_code:
                continue
            if line.startswith("## "):
                check()
                version, seen = line[3:].split()[0] if line[3:].split() else "", []
            elif line.startswith("### ") and version is not None:
                seen.append((n, line[4:].strip()))
        check()
    return fails, warns, len(wiki)


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--root", default=".")
    a = ap.parse_args()
    fails, warns, pages = lint(a.root)
    if pages == 0:
        print(f"no wiki pages under {pathlib.Path(a.root).resolve()}: run from the repo root or pass --root",
              file=sys.stderr)
        return 2
    for w in warns:
        print(w)
    for f in fails:
        print(f)
    print(f"doc-lint: {pages} wiki pages, {len(fails)} failures, {len(warns)} warnings")
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
