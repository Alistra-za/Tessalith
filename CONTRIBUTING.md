# Contributing

Bug reports and pull requests are welcome. For a large change, open an issue first.

## Build and gates

Rust stable with the MSVC Build Tools (see [Build from source](#build-from-source)); `rust-toolchain.toml` pins the rest. No ISPC install is needed.

```
cargo build --release -p sdc -p sdc-gui
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --workspace -- -D warnings
cargo test --workspace
```

CI runs the same on Windows. Tests that need owner data or a GPU are `#[ignore]`d or skip themselves.

## Build from source

Most people should use the [release zip](docs/wiki/Installation.md). Build from source to change the code.

### Requirements

| Requirement | Detail |
|---|---|
| Operating system | Windows, x86-64. The build target is fixed to `x86_64-pc-windows-msvc`. Other platforms are not supported. |
| Rust | Installed through `rustup`. The repository's `rust-toolchain.toml` selects the stable channel, the MSVC target, and the `clippy` and `rustfmt` components. |
| Minimum Rust version | 1.88 for the workspace. The GUI crate needs 1.95 or later because of its `eframe` 0.36.2 dependency. A current stable toolchain covers both. |
| C++ compiler | Microsoft Visual C++ Build Tools (MSVC) with C++17 support. The `encoders-sys` crate compiles the rgbcx encoder, the ISPC Texture Compressor host code and a small C++ shim. |
| ISPC | Not needed for a normal build. See [ISPC objects](#ispc-objects). |

### Build steps

1. Install the Visual Studio Build Tools with the "Desktop development with C++" workload.
2. Install Rust with `rustup`; `rust-toolchain.toml` in the repository root makes `cargo` fetch the right toolchain, target and components.
3. Clone the repository and build in release mode:

```powershell
git clone https://github.com/Alistra-za/Tessalith.git Tessalith
cd Tessalith
cargo build --release
```

`.cargo/config.toml` sets the build target to `x86_64-pc-windows-msvc` and links the C runtime statically (`+crt-static`), so the programs need no separate Visual C++ redistributable.

The binaries land in the target-specific folder:

```text
target\x86_64-pc-windows-msvc\release\sdc.exe
target\x86_64-pc-windows-msvc\release\tessalith.exe
```

To build only one of them, use `cargo build --release -p sdc` or `cargo build --release -p sdc-gui`. `sdc --version` lists the encoder libraries that were linked in.

### Workspace layout

| Crate | Role |
|---|---|
| `crates/engine` | The conversion engine: discovery, grouping, config, packing, mips, encoding, validation. |
| `crates/texcore` | The encoder-free base: DDS reader and writer, pixel decoders, colour curves, cube maths, image loading. `engine` and `render3d` build on it. |
| `crates/bcenc` | rgbcx's block encoders in Rust (BC1, BC3, BC4 and BC5); the engine encodes those formats with it. No dependencies; tests compare it with `encoders-sys`. |
| `crates/encoders-sys` | Native encoders: bc7e, ISPC Texture Compressor (BC6H), and bc7enc_rdo's rgbcx, kept as the reference `bcenc` is tested against. |
| `crates/cli` | The `sdc` program. |
| `crates/gui` | The GUI, `tessalith.exe` (Cargo package `sdc-gui`). |
| `crates/render3d` | Direct3D 11 renderer for the GUI's 3D set preview and mesh view. |
| `crates/modscan`, `crates/nif`, `crates/esp`, `crates/vfs`, `crates/pgpsim` | Mod scanning: MO2 resolution, BSA archives, NIF meshes, ESP plugins, PGPatcher simulation. |

### ISPC objects

The bc7e BC7 encoder and the ISPC Texture Compressor are written in ISPC. Building them from source needs the ISPC compiler, so the repository ships them precompiled in `crates/encoders-sys/vendor/ispc-objs`. A normal `cargo build` links those objects and never runs ISPC.

Every vendored file is listed with its SHA-256 hash in `crates/encoders-sys/vendor/SHA256SUMS`. `crates/encoders-sys/vendor/README-VENDOR.md` says where each piece comes from.

#### Which ISPC path tests run on

The objects hold five ISPC targets: SSE2, SSE4, AVX, AVX2 and AVX-512 (`avx512skx-x16`). ISPC's dispatcher picks the highest one the CPU has, and BC7 and BC6H bytes differ between the AVX-512 path, the AVX2 path and the older paths.

- Test builds encode on the AVX2 path at most, so tests and pinned hashes give the same result on every machine. `encoders-sys`'s `test-hooks` feature sets this cap, and a plain `cargo build` never turns it on. `cargo build` with `--tests`, `--examples` or `--all-targets` (and `cargo test --release`) does, and leaves a capped `sdc.exe` in the release folder that `tools\package-release.ps1 -NoBuild` would ship. Two CLI tests deliberately spawn a lifted `sdc convert`, so on an AVX-512 machine a default `cargo test` does encode that fixture on the AVX-512 path.
- Set `SDC_TEST_AVX512=1` to lift the cap. Only then are the AVX-512 pins (`level_identity` and `bc6h` in `crates/encoders-sys/tests`) checked, and only on a CPU with AVX-512. `tools\gate.ps1` does this in its `avx512` step.

#### tools\regen-ispc.ps1

Use this script only to check the vendored objects or to change the ISPC sources. It needs two compilers and refuses any other version: ISPC 1.31.0 builds every file except the two AVX-512 objects, and ISPC 1.32.0 builds `bc7e_avx512skx.obj` and `kernel_avx512skx.obj`, because it fixes an AVX-512 miscompile of 1.31.0 (ISPC issue 3891). Without `-Ispc512` the check reports those two objects as `SKIP`; `-Update` needs both compilers.

```powershell
# Rebuild into a temporary folder and compare with the vendored objects
.\tools\regen-ispc.ps1 -Ispc C:\tools\ispc-1.31.0\bin\ispc.exe -Ispc512 C:\tools\ispc-1.32.0\bin\ispc.exe

# Rebuild and replace the vendored objects and headers
.\tools\regen-ispc.ps1 -Ispc C:\tools\ispc-1.31.0\bin\ispc.exe -Ispc512 C:\tools\ispc-1.32.0\bin\ispc.exe -Update

# With -Update: replace only the bc7e objects, leave the ISPC Texture Compressor objects alone
.\tools\regen-ispc.ps1 -Ispc C:\tools\ispc-1.31.0\bin\ispc.exe -Ispc512 C:\tools\ispc-1.32.0\bin\ispc.exe -Update -Bc7eOnly
```

| Parameter | Default | Meaning |
|---|---|---|
| `-Ispc` | `ispc.exe` (from `PATH`) | Path to the ISPC 1.31.0 compiler. |
| `-Ispc512` | none | Path to the ISPC 1.32.0 compiler, for the two AVX-512 objects. Required by `-Update`. |
| `-Bc7eSource` | `crates\encoders-sys\vendor\bc7e\bc7e.ispc` | The bc7e source to compile. |
| `-Update` | off | Copy the rebuilt objects and headers into `vendor\ispc-objs`. |
| `-Bc7eOnly` | off | With `-Update`, replace only the bc7e objects. |

The script compiles for x86-64 Windows with the five targets, from the repository root with repository-relative paths so the output is reproducible. It reports `MATCH`, `DIFF` or `SKIP` for each object and exits with 0 when every object reports `MATCH` or `SKIP`.

#### tools\hash-vendor.ps1

After you change anything under `crates\encoders-sys\vendor`, run `.\tools\hash-vendor.ps1` to rewrite `SHA256SUMS`. It takes no parameters and hashes every vendored file except `SHA256SUMS` itself.

## Conventions

- Licence: GPL-3.0-or-later. By contributing you agree your change is under it. Do not copy code from projects without a compatible licence.
- Design decisions are recorded in [.claude/DECISIONS.md](.claude/DECISIONS.md) as numbered notes. A change that alters behaviour adds or amends a note, and updates the wiki pages it touches.
- Output changes: a change that alters written bytes needs a line in [CHANGELOG.md](CHANGELOG.md) (and [docs/presets-changelog.md](docs/presets-changelog.md) for presets).
- A pub item in `sdc-gui` (or a counter it reads) that only tests use goes behind `#[cfg(feature = "test-hooks")]`; the crate's own dev-dependency turns the feature on for tests, and a release build never does. `encoders-sys` has the same feature: in a test build it keeps BC7 and BC6H on the AVX2 path at most, and `SDC_TEST_AVX512=1` lifts that.
- A new GUI test file, `crates/gui/tests/<name>.rs`, gets its `#[path]` and `mod` lines in `crates/gui/tests/it/main.rs`. That file's header says when a test file needs a `[[test]]` entry in `crates/gui/Cargo.toml` instead.
- No game assets in PRs: no `.dds`, `.nif`, `.bsa`, `.esp` or textures from Skyrim or from mods you do not own. Make small test fixtures yourself.

## Design records

- [.claude/DECISIONS.md](.claude/DECISIONS.md) holds the numbered rulings, [docs/presets-changelog.md](docs/presets-changelog.md) the preset changes, and [docs/research/](docs/research/README.md) the measurements behind the encoder, mip-filter and coverage choices.

## Host plugins

Optional plugins for Photoshop, GIMP 3, Paint.NET and chaiNNer that open and save DDS live on the [`host-plugins` branch](https://github.com/Alistra-za/Tessalith/tree/host-plugins). It gets bug fixes but is not kept in step with the engine.
