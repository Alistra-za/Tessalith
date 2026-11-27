# Mistakes: Engine, CLI and encoders (crates/engine, crates/cli, crates/encoders-sys)

Format and rules: [README.md](README.md).

- 2026-11-01 Planned for a `cpm` target → targets are only `cs` and `vanilla`; the `cpm` target was removed and is now a hard error (Note AZ) (`--target`, `sdc.toml`)
- 2026-11-01 Assumed plain normal maps were OpenGL → plain `_normal` / `_nrm` / `_norm` default to DirectX green, and `_normalgl` flips (Note AW) (`engine::policy`)
- 2026-11-01 A merge edited a vendored file without updating its hash → update `crates/encoders-sys/vendor/SHA256SUMS` (the `vendor_manifest` test checks it) (`crates/encoders-sys/vendor/`)
- 2026-11-18 Added a marker (an `f` suffix) to the real set before the helper-fill step, and the fill resolved a filled fuzz map again and lost its `<base>_f.dds` → mark a clone for the members that need it and keep the real set unchanged (`crates/engine/src/api.rs`, commit eda22831, whose subject says the opposite; Note JQ52)
- 2026-11-23 Edited `bc7e.ispc` with the Edit tool, which stripped trailing whitespace from unchanged upstream lines and put them in the patch → edit vendored `.ispc` files with a Python byte script that asserts one match per hunk and keeps CRLF, so the diff touches only the patched lines (ruling BC7-T5-3) (`crates/encoders-sys/vendor/bc7e/bc7e.ispc`)
- 2026-11-26 tempfile's `persist_noclobber` passes raw paths to `MoveFileExW` on Windows, so new outputs over 260 characters failed with os error 3 while every `std::fs` write worked (std adds `\\?\` itself) → give a raw Win32 path call from a crate a verbatim path (`pipeline::long_form`), and test a write path with a folder over 260 characters (`crates/engine/src/pipeline.rs`, Note LV)
