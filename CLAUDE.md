# Tessalith: notes for coding agents

Tessalith converts source images into DDS textures for Skyrim SE/AE. It has a GUI (`tessalith.exe`) and a CLI (`sdc.exe`). Both share one engine and one per-project `sdc.toml`, so they write the same bytes. Read README.md for what it does and CONTRIBUTING.md for the basics. This file covers what an agent tends to get wrong.

## Crates
| Package | Path | What it is |
|---|---|---|
| `engine` | crates/engine | Planning, policy, presets, packing, bake, RNM, mips, encode, DDS write, manifest |
| `sdc-gui` | crates/gui | egui + wgpu GUI (`tessalith.exe`); kittest tests |
| `sdc` | crates/cli | CLI (`sdc.exe`) |
| `encoders-sys` | crates/encoders-sys | FFI to vendored encoders (bc7e, ISPC TC, rgbcx); objects are vendored, no ISPC compiler needed |
| `modscan`, `vfs`, `esp`, `nif`, `render3d`, `pgpsim` | crates/… | MO2 scan store, BSA reading, plugin/NIF parsing, 3D preview, PGPatcher simulation |

A sibling "NIF team" owns `modscan`, `vfs`, `esp`, `nif`, `render3d` and `pgpsim`. It works on `nif/*` branches; the owner merges `nif/main` into `main`. Change those crates only when the task says so. Record what you changed in a DECISIONS note and in a line for the NIF team's handoff file (see CLAUDE.local.md).

## Gates
During a plan: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo clippy --workspace -- -D warnings`, and tests of the changed crates only (owner, 2026-10-29). Once at plan end, after the final review's fixes, and before a merge into `main`, run `tools\gate.ps1 -Out <log folder>`. It adds the whole test suite, `render_` alone and the doc checks, and prints PASS or FAIL per step.
- **`.githooks/pre-commit`** checks fmt, control bytes and duplicate note headings. Turn it on in a clone with `git config core.hooksPath .githooks`.
- **GPU tests (`render_*`) run alone.** They can hang for 30+ minutes when the GPU is busy.
- **`cargo test -p engine --test golden`** is a quick pre-check; the workspace run already includes it.
- **`cargo test -p engine --test encode_versions`** pins encoder inputs and built-in profile hashes; a change it accepts rewrites `crates/engine/tests/encode_versions/*.json`. Commit that diff with the change.
- **Capture test output to a file** and read the `test result:` lines. Never pipe it through `head`: truncated output has been mistaken for a pass.
- **On Windows, set `CARGO_BUILD_JOBS=4`** when several builds run at once. Otherwise linking fails with os error 1450 ("insufficient system resources"). That error is not a code failure.
- **Release exes land in `target\x86_64-pc-windows-msvc\release\`**, because `.cargo/config` sets the target triple, not in `target\release\`. An `sdc-gui.exe` there is stale; the GUI is `tessalith.exe`.

## Disk
Build folders fill the drive fast: one worktree's `target/` reaches 50–170 GB.
- Use at most one target dir per active worktree, or one shared `CARGO_TARGET_DIR`. Never give each subagent its own.
- Check free space before a full gate, and stop below ~20 GB.
- Delete a worktree's target dir before removing the worktree.

## Hard rules
- **Goldens are byte-identical.** Encoded output must not change unless a DECISIONS note says why. If a golden would change, stop and say so.
- **Tests never touch the real user data folders**: `%LOCALAPPDATA%\sdc` (scan.db, logs) and `%APPDATA%\sdc` (gui.toml, presets). Use temp dirs.
- **Tests that need the owner's game or mod data** stay `#[ignore]` or gated on an `SDC_*` environment variable.
- **No game or mod assets in the repo**: no `.dds`, `.bsa`, `.esp` or real textures. Fixtures are generated in tests. The one exception is the pinned CS shader source in `crates/render3d/shaders/cs/` (Note GT).
- **Files the user edits are written with `engine::dds::write_atomic`**, which flushes before the rename. That covers `sdc.toml`, `sdc-project.toml`, `gui.toml`, presets and exported JSON. DDS outputs use it too (owner decision, Note BC10).
- **Never overwrite a file Tessalith did not write.** This matters most next to PGPatcher's outputs. Exports check the recorded hash first, as the PGPatcher, CS PBR and complex-material JSON exports do.
- **Version:** the workspace version is the engine version, and it is part of every output's identity. Bump it only on purpose. An engine change that alters a type's output bytes or validation verdict sets that type's arm in `engine::manifest::encode_version` to the new engine version (Note DM). New `Profile` fields are `#[serde(default, skip_serializing_if = ...)]`. A release commit sets `RELEASED_ENGINE_VERSION` to the released version. The version follows semantic versioning (Note DM8): MAJOR breaks existing use, MINOR adds a backward-compatible feature, PATCH fixes a bug.
- **Schemas:** after adding a code, shader or config field, regenerate with `sdc presets schema all --out schema` and commit only real diffs. A `///` doc comment on an enum variant turns that schema into `oneOf`, so use `//` there.

## How work is recorded
- **Three files hold the rulings.** `.claude/DECISIONS.md` has an index (one line per crate or area, naming its notes), a digest of the 1.x rulings still in force, then the 2.x notes in full. `.claude/DECISIONS-1.x.md` is the 1.x record, frozen at 2.0: history only, never edited, its IDs still citable. `.claude/DECISIONS-ARCHIVE.md` holds superseded 2.x rulings.
- **Every judgement call becomes a ruling in `.claude/DECISIONS.md`.** Append a new `## Note XX — <topic> (<who>, <date>)` at the end with the next free letters. Number the items XX1, XX2, and so on. **When you add a note, add its letter to the index line of every crate or area it governs.**
- **Note letters are split by team (owner, 2026-10-28).** The main team uses the F block (FA, FB, ... FZ), then H, J and so on; the NIF team uses G, then I, K and so on. Each team takes the next free letter in its own block, so the two never collide. Parallel branches of one team can still collide: pick the next free letter in your block as it stands on `main` at merge time, and rename the note if it's taken. A letter is free only when none of the three files uses it. A merge must leave no duplicate headings in `DECISIONS.md` or `DECISIONS-ARCHIVE.md` (the frozen 1.x file is never edited). Check each with `grep -oE '^## Note [A-Z]+' <file> | sort | uniq -d`, which must print nothing.
- **DECISIONS.md holds live rulings only.** A fully reverted or superseded 2.x ruling moves verbatim to `.claude/DECISIONS-ARCHIVE.md` under its original ID; a 1.x ruling a new note overrides is dropped from the digest (the note says what it supersedes); a partly superseded item is edited to the current rule and its old wording archived. IDs are never reused. Archived rulings are not in force: don't cite them. Details: `docs/DOCUMENTATION.md` §7.
- **Documentation follows `docs/DOCUMENTATION.md`:** current state only (history goes in CHANGELOG.md), one home per fact (at most two mentions, three if essential, full explanation once), page size budget, generated screenshots refreshed with a difference overlay before a push. Read it before editing README.md, docs/wiki, docs/gui.md or DECISIONS.md.
- **Behaviour recorded as a ruling is intended.** Don't "fix" it without a new ruling.
- **A comment starting `shortcut:`** marks a known limit, and says when to upgrade it.

## Workflow
- Do feature work on a branch in `.worktrees/<name>`, not on `main`.
- Commit explicit paths. Never `git add -A`.
- Bug fixes start with a failing test. Make one commit per fix.
- Merges into `main` are local. The owner pushes.
- When two branches both append a note to `.claude/DECISIONS.md`, keep both.
- Temporary files go in `.claude/scratch/` (gitignored; delete your own when done), never loose in the repo root or `crates/`.
- **Every commit builds.** Don't split a shared-file registration from the code that uses it.
- **Review conflicted merges.** After a merge that resolved conflicts, check it with `git show --remerge-diff <merge>`.

## Gotchas
- **Path case folding:** game-path keys (meshes, scan, BSA) fold ASCII only, while set ids and `[files]` keys fold Unicode (Note BF).

## Mistakes log
- Before you start, read `.claude/mistakes/<section>.md` for the area you work in, and `.claude/mistakes/general.md`. The sections are the section table of `.claude/workflows/trace-debug.js` (E1–E6, G1–G7, S1, S2, R1–R4), as lower-case file names.
- When something goes wrong that the next agent would repeat, append one line: `- YYYY-MM-DD <what went wrong> → <what to do instead> (<where: file or command>)`. `.claude/mistakes/README.md` has the rules.

## Where things are
- `.claude/DECISIONS.md`: every live ruling: the index, the digest of 1.x rulings in force, and the 2.x notes. Read the index and the digest section for your area before changing behaviour. `.claude/DECISIONS-1.x.md`: the frozen 1.x record behind each digest cite (rulings under its `# Archived` heading are not in force). `.claude/DECISIONS-ARCHIVE.md`: superseded 2.x rulings, for history only.
- `CODING_STANDARDS.md`: judgement rules a reviewer enforces; name it in every review dispatch.
- `docs/DOCUMENTATION.md`: the documentation policy (what goes where, repetition limits, screenshots).
- `docs/wiki/`: user-facing behaviour; the GUI pages are `docs/wiki/GUI-*.md` (`docs/gui.md` is a link to them). Update them when the UI changes, following `docs/DOCUMENTATION.md`. `docs/wiki` is the source of the GitHub wiki: after editing it, run `tools\publish-wiki.ps1` (`-Out <dir>` previews the converted pages).
- `docs/research/`: the measurements behind encoder, mip-filter and coverage choices.
- `.claude/workflows/trace-debug.js`: a whole-repo bug hunt with skeptical review.
- `.claude/workflows/quality-audit.js`: a whole-repo quality pass (dead code, efficiency, duplication, architecture, test gaps, shortcut debt) with skeptical checks, a delete list and refactor briefs. It uses the same section table as trace-debug.
- `.claude/workflows/in-game-capture.js`: launch the game through MO2, take RenderDoc captures of test meshes without a keyboard, restore settings, and turn the captures into numbers.
- `.claude/workflows/golden-capture.js` (NIF team): Phase D golden captures with a CS settings profile (A, B, A_DC). Usage and plan format are in its header comment. File work lives in `golden-capture-lib.mjs`; run `node .claude/workflows/golden-capture.test.mjs` for the offline test (no game launch).
- `.claude/workflows/ui-proof.js`: headless UI proof of one feature: owner-approved goals, blind explorers, frame checks and a verified, ranked report; usage in its header. It drives the app through `crates/gui/tests/ui_drive` (`tools\ui-drive.ps1`); journeys are tests in `crates/gui/tests/ui_proof.rs`.
- `tools/scan-query.py`: read-only queries against a scan database: any SQL, `--tables`, `--txst <plugin>`. Run it with `py -3.13 -I`.
- `tools/package-release.ps1`: builds the release zip and SHA256SUMS.
- `tools/doc-shots.ps1`: re-renders the documentation screenshots and writes a difference overlay for each changed one (run before a push).
- `tools/doc-dupes.py`: finds facts repeated across the user-facing docs (one home per fact); `--lost` lists what an edit dropped.
- `tools/doc-markers.ps1`: checks the hidden `<!-- D:ID -->` ruling markers in the user-facing docs (run before a push; `-SelfTest`).
- `tools/decisions-archive.py`: moves superseded rulings from DECISIONS.md to DECISIONS-ARCHIVE.md from a `tools/decisions-archive/<date>.tsv` table (`--apply`, `--check --base <commit>`); a row already archived (also in the 1.x file) is skipped.
