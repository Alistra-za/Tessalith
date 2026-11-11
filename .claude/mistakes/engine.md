# Mistakes: Engine, CLI and encoders (crates/engine, crates/cli, crates/encoders-sys)

Format and rules: [README.md](README.md).

- 2026-11-01 Planned for a `cpm` target → targets are only `cs` and `vanilla`; the `cpm` target was removed and is now a hard error (Note AZ) (`--target`, `sdc.toml`)
- 2026-11-01 Assumed plain normal maps were OpenGL → plain `_normal` / `_nrm` / `_norm` default to DirectX green, and `_normalgl` flips (Note AW) (`engine::policy`)
- 2026-11-01 A merge edited a vendored file without updating its hash → update `crates/encoders-sys/vendor/SHA256SUMS` (the `vendor_manifest` test checks it) (`crates/encoders-sys/vendor/`)
