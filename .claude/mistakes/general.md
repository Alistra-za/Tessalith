# Mistakes: Machine, shell, git and build mistakes that are not tied to one section

Format and rules: [README.md](README.md).

- 2026-11-01 The Bash tool returned no output on this Windows machine → use the PowerShell tool (Bash tool)
- 2026-11-01 `python -` / `python3 -` heredocs hung the shell → write a script file and run it with `py -3.13 -I <script>` (any shell)
- 2026-11-01 Read on `.claude/DECISIONS.md` failed: the file is too large → Grep it, never Read it whole (`.claude/DECISIONS.md`)
- 2026-11-01 Shell rewrites (.NET ReadAllText/WriteAllText, sed) mangled CRLF files → use the Edit tool (any CRLF file)
- 2026-11-01 Files are CRLF in the working copy (autocrlf); LF-to-CRLF warnings on commit are harmless → never mass-normalise line endings (git)
- 2026-11-01 Merging a branch that stops tracking files (`git rm --cached`) deleted those files from the other checkout's disk → back them up first (git merge)
- 2026-11-01 A shared CARGO_TARGET_DIR on C: grew to 432 GB and filled the drive → check its size and put it on F: (`CARGO_TARGET_DIR`)
- 2026-11-01 `cargo clean` missed the real build folder → pass `--target x86_64-pc-windows-msvc` too (`cargo clean`)
- 2026-11-01 `tools/decisions-archive.py` crashed with UnicodeEncodeError (cp1252) printing a failure, and `PYTHONIOENCODING` is ignored under `-I` → run it as `py -3.13 -I -B -X utf8 tools/decisions-archive.py ...` (decisions-archive.py --check)
- 2026-11-01 A `///` doc-comment change on a `JsonSchema` field left `schema/*.json` stale and failed `committed_schemas_match_the_types` in the full gate → after editing such a comment run `sdc presets schema all --out schema` and commit the diff (`crates/engine/tests/schemas.rs`)
- 2026-11-03 Splicing a line into CHANGELOG.md with List.AddRange in PowerShell wiped the file to one line (it was caught by git diff --stat) -> insert with the Edit tool anchored on a nearby line, and read git diff --stat after any scripted doc edit (CHANGELOG.md)
- 2026-11-03 Write to a new test file name that already existed (crates/gui/tests/skin_card.rs) overwrote a tracked test suite → run `git ls-files <path>` before creating a file, and pick a name no module or test uses yet (Write tool)
- 2026-11-03 Subagent-written text passed through PowerShell double-quoted strings turned `` `apply_compiled `` and `` `edit_changed `` into BEL/ESC control bytes ("pply_compiled", "dit_changed") in committed comments → write Markdown-backticked code names with the Edit tool or single-quoted strings, and scan changed files for bytes < 0x20 other than tab/CR/LF before committing (PowerShell `"…`a…"` / `"…`e…"`)
- 2026-11-04 A PowerShell find/replace table `@{ f = @(@(old, new)) }` flattened to one array, so `$p[0]`/`$p[1]` were the first two characters and every `(` in six source files became a letter; the recovery map (a PowerShell hashtable, case-insensitive) then put back wrong-case lines ("AO" as "ao") → do source edits with the Edit tool; if a script must map lines, use `[Collections.Generic.Dictionary[string,string]]::new([StringComparer]::Ordinal)`, and read `git diff --stat` right after (PowerShell `@(@(..))`, `@{}`)

- 2026-11-04 Restoring a source file from a scratch copy with Copy-Item kept the copy's older mtime, so cargo reused the stale mutated build and a GREEN run failed → after restoring, set the file's LastWriteTime to now (or restore with the Edit tool) before re-running cargo (`Copy-Item` during mutation checks)
- 2026-11-04 rustfmt pushed a comment-only line far right because the line above ended in a trailing `// …` comment (it aligns it as a continuation) → make it a trailing comment on the next code line, or put code between them (`cargo fmt`, Rust test fixtures)
- 2026-11-04 Bash and Monitor both failed with ENAMETOOLONG (uv_spawn) in a long session → use the PowerShell tool and read background task output files instead of Monitor (Bash/Monitor tools)
- 2026-11-05 `cargo test ... -- --skip render_ --no-fail-fast` built for 6 minutes, then the first test binary rejected `--no-fail-fast` and nothing ran → `--no-fail-fast` is a cargo flag: put it before `--` (`cargo test`)
- 2026-11-06 Implementers wrote to the main checkout twice: a relative path in a .NET call or a file tool resolves against the process cwd `F:\ai\dds\converter`, not the worktree → use absolute worktree paths in every script and file call; the controller checks `git -C F:\ai\dds\converter status --short` after a task (any scripted edit in a worktree)
