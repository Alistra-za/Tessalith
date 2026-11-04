# How Tessalith came about: design overview

Written 2026-10-16 (commit 1bae037b).

Tessalith started in October 2026 as a written plan for an open-source DDS converter for Skyrim SE/AE modding, with a GUI and a CLI on one engine. The plan drew on the [encoder literature review and local benchmark](encoder-benchmark.md), the [mipmap research](mipmaps-and-alpha.md) and a review of Skyrim's texture conventions. It listed open questions, and each was settled by a short test ("spike") or by the project owner. This page describes the design that resulted and how it differs from the first plan. The decision record is [DECISIONS.md](../../.claude/DECISIONS.md).

## The main idea: correct Skyrim policy matters more than the encoder

The benchmark showed that the top BC7 encoders are within 0.16 to 0.32 dB of each other on real colour, while the wrong format costs 5 to 15 dB and stray alpha 1 to 17 dB. Skyrim adds errors that are not gradual at all: a vanilla diffuse tagged `_SRGB` renders too dark, a landscape diffuse saved as BC1 loses its snow mask or parallax height, a BC5 normal decodes with Z = −1, and a complex-material `_m` without a full mip chain can switch its shading off.

So the design ranks quality in three layers:

1. **Correct format, colour tag and alpha handling** for each map type and target.
2. **Correct mips.**
3. **The best measured encoder at a practical speed.**

Most of the original work went into the first two. The encoders were taken from the benchmark unchanged.

## Who it is for

Texture artists converting a folder of source images (PNG, TGA, TIFF, EXR, PSD and others) to DDS. It is not a tool for "fixing" an installed game folder; DDS input is secondary. File names may suggest how a texture is treated, but the user always decides ([DECISIONS](../../.claude/DECISIONS.md), "Scope clarification").

## Skyrim texture rules

These come from the Skyrim research notes, the Community Shaders (CS) source and the owner's answers.

- **Suffixes are a convention only.** The mesh's texture slot and shader type decide how a texture is read. `_s` alone has three meanings, and `_m` is plain or complex depending on its content.
- **Alpha often holds data.** The `_n` alpha is a specular mask; the landscape diffuse alpha is the snow mask (vanilla) or parallax height (CS); a complex `_m`'s alpha is height. Diffuse alpha means transparency only when the mesh says so.
- **Normal maps keep three channels.** Skyrim's `_n` stores XYZ in RGB and the shader reads Z directly. CS never rebuilds Z. A BC5 `_n` would decode with Z = −1 and lose its specular alpha. This is why the benchmark's "BC5 beats BC7 for normals" does not apply here.
- **TruePBR sets** live under `textures/pbr/`. `_rmaos` is roughness, metallic, AO and specular in RGBA. `_p` is height in red (0.5 neutral). `_s`, `_cnr`, `_f` and `_g` are subsurface/coat colour, coat normal, fuzz and emissive.
- **Complex material `_m`**: red environment mask, green gloss, blue metalness, alpha height (255 means flat). It needs a real alpha channel, and CS reads its smallest mip to decide whether it is complex ([spike S8](#spike-s8-the-cs-complex-material-gate-2026-10-09)).
- **Colour space.** Vanilla stores gamma-encoded colour in UNORM files; an `_SRGB` tag makes it too dark in vanilla shaders, CS non-PBR paths and LOD. TruePBR expects colour maps tagged `_SRGB`. Data maps are always UNORM.
- **Loader limits.** SE/AE loads BC1 to BC7 and some uncompressed formats. It loads BC4 and BC5 with either the legacy `BC4U`/`BC5U` or the DX10 header, unsigned only. Mipmaps are expected everywhere except `interface/` and `sky/`.

### Spike S8: the CS complex-material gate (2026-10-09)

The gate was read from the shader, because parallax cannot be judged on a flat test map. One frame was captured with Community Shaders' RenderDoc feature (RenderDoc 1.43) in `DLC1VampireCastleGuildhall`, with `EnableComplexMaterial`, `EnableParallax` and `EnableParallaxWarpingFix` on. Two test `_m` were placed with DevBench: `heightmask_m` on a Comfy Coffins `SCoffin01`, and `m_250_m` on a septim pile. The CS Lighting pixel shader decides this way:
- It samples `TexEnvMask` at LOD 15, the smallest mip.
- `grey` means every pair of R, G and B differs by less than 4/255. `black` means R, G and B are all below 4/255 and 4/255 < alpha < 251/255.
- The material is complex when it is not (`grey` and not `black`) and alpha < 251/255 (0.984314).
- Parallax then runs only on texels whose full-resolution alpha is in (4/255, 251/255).

The decoded smallest mips agree: `m_250_m` (80, 160, 40, 250) is complex, and `heightmask_m` (0, 0, 0, 126) is complex through the black exception. On the test set (RGB 80/160/40), alpha 245, 248 and 250 are complex and 252 and 255 are not. So `complex_m_warn_alpha` stays 247, four codes below the gate; `VAL-M-COMPLEX-MARGIN` says how many codes below the gate a file is, or that complex is off, and `VAL-M-GRAYSCALE` applies the shader's grey rule to the smallest mip. The capture scripts are `F:/ai/dds/tools/rdc-scripts/s8_stage1.py` and `s8_stage2.py`.

## Targets

The first plan had three targets: Community Shaders, Complex Parallax Materials (`cpm`) and vanilla. The `cpm` target was removed before the first release ([DECISIONS](../../.claude/DECISIONS.md) Note AZ). Today there are two:

| Target | What it allows |
|---|---|
| `cs` (default) | Everything: TruePBR, Advanced skin, `_SRGB` on PBR colour maps, BC6H and RGBA16F for HDR. |
| `vanilla` | Bethesda shaders, plus the complex-material `_m` for a set whose shader is complex material, and complex grass. No `_SRGB`, no BC6H, no PBR. |

See [Presets and targets](../wiki/Presets-and-Targets.md).

## Language and architecture

The plan compared three shapes (one language for engine, CLI and GUI; a CLI engine driven by a GUI over JSON Lines; a native core DLL with a GUI in another language) and three languages (Rust, C++20, C#/.NET).

**Chosen: one Rust workspace**, Windows x64 only. The reasons in the plan were one image-I/O code path, memory-safe parsing of files downloaded from mod sites, GPU decoding of BC textures for the preview, preset types that generate their own JSON Schema, and a single-file exe. C++20 was a close second. The owner confirmed Rust after spikes S3 and S4.

| Crate | Role |
|---|---|
| `engine` | Planning, policy, presets, mips, DDS writer, validation |
| `encoders-sys` | Vendored bc7e, rgbcx and ISPC objects with a small C++ shim |
| `cli` | `sdc`, the command-line tool |
| `gui` | `tessalith.exe`, built on eframe/egui with wgpu |
| `bench` | `sdc-bench`, the benchmark gates (on branch `bench` since Note FM8) |
| `nif`, `esp`, `vfs`, `modscan`, `pgpsim`, `render3d` | Added later: mod scanning (meshes, plugins, BSA archives) and the 3D preview |

The engine reports progress as typed events. `sdc --json` prints the same events as JSON Lines, so the GUI and the CLI cannot drift apart.

The first plan listed mesh (NIF) parsing and BSA reading as non-goals. Both were added later, so that a mod scan can tell Tessalith what a texture's alpha means and which shader its set uses ([Mod scanning](../wiki/Mod-Scanning.md)).

## Encoders

| Component | Role | How it is used |
|---|---|---|
| bc7e (from bc7enc_rdo) | BC7 | Prebuilt ISPC objects, vendored with SHA-256 hashes, so building needs no ISPC compiler. Local patches add exact-opaque alpha and the `max` level. |
| rgbcx (from bc7enc_rdo) | BC1, BC3, BC4, BC5 | Plain C++. Local patches round its BC4 palette like the decoder, widen the BC4 block search and add the BC1 Dual mode. |
| Intel ISPC TC | BC6H | Prebuilt ISPC objects. One local patch adds the peak-preserving mode. |
| bcdec_rs | Decoding for validation and preview | Rust crate. |
| NVTT 3 | Optional BC7 and BC6H at Production in 1.x; removed in 2.0 (Note FM14) | The user's own `nvcompress.exe`, run as a separate process. Never linked or shipped, because its licence forbids use that would place it under an open-source licence. |
| DirectXTex / texconv | Test oracle only | Not shipped. |
| Compressonator, Oodle | Not used | |

The vendored files and patches are listed in [`README-VENDOR.md`](../../crates/encoders-sys/vendor/README-VENDOR.md).

### Spike S4: a scalar bc7e (2026-10-09)

The spike tested basis_universal's scalar C++ port of bc7e as a fallback for the ISPC objects: [`encoder/basisu_bc7e_scalar.cpp`](https://raw.githubusercontent.com/BinomialLLC/basis_universal/99f52d63aa6799cbdaecfe977111dc5ec3b31d47/encoder/basisu_bc7e_scalar.cpp) and its header `encoder/basisu_bc7e_scalar.h` at commit `99f52d63aa6799cbdaecfe977111dc5ec3b31d47` (Apache-2.0), SHA-256 `684b106e6037042bee065694e5956a97ba91cf2b2417c1a44217e0087a5e808f` and `5395f673cbdb9b4ce5115d18369f36b6002712bf88ac3ce71170cdeace889b79`. It is self-contained and was compiled unmodified with MSVC `/O2 /EHsc /std:c++17 -DNDEBUG`, baseline SSE2.
- **Quality** (`veryslow`, linear, `rgb_real` and `alpha_real`, 11 images, against the ISPC u5 payloads): the largest per-image |ΔPSNR| was 0.0759 dB (Metal049A, alpha channel), over the 0.01 dB limit; the other 10 images were within 0.0088 dB, with deltas of mixed sign (mean about 0.003 dB). No image was byte-identical, which the port's differences from the ISPC source (a denominator bias, for example) explain.
- **Speed** (u5, three runs each): 0.18 Mpix/s on one thread and 2.07 to 2.11 Mpix/s on 16, against ISPC's 0.83 to 0.89 and 7.9 to 8.2, so 4.8× slower on one thread and 3.8 to 3.95× on 16, against a 4× limit.
- It was not adopted; adopting it would need a looser per-image tolerance. The spike folder was removed on 2026-10-13 (Note Y10); it can be rebuilt from the commit and hashes above if the ISPC objects ever fail on a needed configuration.

## The conversion pipeline

The research fixed the order of the steps. A batch goes through these stages:

| Stage | What happens | Why it is here |
|---|---|---|
| Discover, Group | Find files, take the game-relative path, group files into material sets | Paths decide colour tags; sets decide packing |
| Load, Analyse | Decode to float32 RGBA; measure channels, alpha, normal length | 16-bit and HDR sources are not cut to 8 bits early |
| Resolve | Presets, rules, suggestions and user choices become one fixed job per file; `--dry-run` stops here | GUI and CLI share one settings model |
| Preprocess | Alpha policy, channel swizzles, green flip, colour fill under alpha 0, resize, sRGB decode for colour | Stray alpha must go before mips and encoding |
| Mipmaps | Filter per channel group, renormalise normals, bake roughness, preserve coverage | Coverage comes after filtering and before encoding |
| Compress | Encode each level in parallel bands; apply the alpha fixes | The alpha-254 values come from the encoder |
| Validate, Write | Decode what was encoded, check headers, mips, alpha rules and errors; write to a temporary file and rename | The smallest mip's alpha only shows its real value after encoding |

Every output is written atomically, and a batch manifest records how each file was made, so `--on-existing if-changed` can skip files that are up to date.

## How the open questions were settled

| Question in the plan | Answer | Where |
|---|---|---|
| Two-channel maps beyond `_flow` | Only `_flow` by default; BC5 stays selectable | DECISIONS #1 |
| BC4/BC5 header form | SE loads both; unsigned DX10 by default, legacy as an option, `ATI1`/`ATI2` never written | DECISIONS #2, [`dds.rs`](../../crates/engine/src/dds.rs) |
| BC6H for emissive and sky | Usable under CS, not in vanilla | DECISIONS #3 |
| `_SRGB` in vanilla | Not usable | DECISIONS #4 |
| `_g` and `_f` colour space | Any colour data in a PBR set is `_SRGB`; BC6H by override for an emissive `_g` | DECISIONS #5, Spike S1 (not run in game; owner answer, 2026-10-09) |
| Normal length | Renormalise; bake variance into roughness when the set has an `_rmaos` or `_rfaos` | DECISIONS #6, M4 rulings |
| Small BC7 textures crashing | 66 BC7 textures of 16 to 128 px with full chains (plus seven `_n` re-encoded at 16 to 64 px) ran in game for about 10 minutes with no crash, and again in the S8 session; the crash reports that blamed small textures were most likely misattributed. No fallback | Spike S2 (2026-10-09) |
| Forcing alpha 255 | Remove the alpha channel by default; exact-opaque BC7 only for `_m` and environment masks, because it costs about 0.5 to 1 dB RGB | Notes A and D, [Spikes S5 and S6](encoder-benchmark.md#forcing-alpha-255-spikes-s5-s5b-and-s6-2026-10-08-and-09) |
| Coverage threshold | Default 127, set per texture; coverage on for cutout and cutout + blend | DECISIONS #10, Note E |
| BC1 decoder model | rgbcx Dual, scored for NVIDIA and AMD together; no setting | Note FM17 |
| BC1 transparent black | BC1 is opaque unless the user turns on `encoder.bc1_alpha` (1-bit alpha) | Notes F and FM20 |
| Linux | Windows only | DECISIONS #12 |
| Coverage implementation | Rebuild the coverage idea in the engine, with credits ([CREDITS](../CREDITS.md)) | DECISIONS #13 |
| Rust or C++ | Rust | [Phase 0 exit](#phase-records-2026-10-09) |
| Hash in the DDS header | Not done; a batch manifest records provenance, per-file sidecars are optional | DECISIONS #15, Note F |
| 16K previews | Default limit 8192, 16384 as a setting | DECISIONS #19 |
| Driving NVTT per mip level | Works: one `-nomips -nocuda` process per level, fed as a legacy-header DDS (NVTT removed in 2.0) | [Spikes S7 and S7b](encoder-benchmark.md#nvtt-per-level-feeding-spikes-s7-and-s7b-2026-10-09-and-13), Note NV |
| CS complex-material gate | Exact at 251/255 on the smallest mip; Tessalith warns at 247 | [Spike S8](#spike-s8-the-cs-complex-material-gate-2026-10-09) |

## Phase records (2026-10-09)

- **Phase 0 exit.** Its gates are the [vendored-encoder check](encoder-benchmark.md#vendored-encoders-reproduce-the-bench) and spikes [S4](#spike-s4-a-scalar-bc7e-2026-10-09), [S5](encoder-benchmark.md#forcing-alpha-255-spikes-s5-s5b-and-s6-2026-10-08-and-09) and [S7](encoder-benchmark.md#nvtt-per-level-feeding-spikes-s7-and-s7b-2026-10-09-and-13). The owner then confirmed Rust, set `alpha.exact_opaque` on by default only for `_m` and environment masks (S5 missed its 0.05 dB condition), and moved the variance bake to milestone M4, with the MVP renormalising `_n` mips.
- **Phase 2 exit, the CLI MVP.** `sdc-bench --gates` passed 13 of 13 gate reports, and the workspace ran 472 tests with 0 failed and 4 ignored by design (the S5b and S5c ΔRGB tests, the S7 NVTT test and the 512 MiB decode-limit test). nvddsinfo read back the format, size and mip count of the sampled outputs, and texconv decoded each one. On 110 PNG sources from three of the owner's mods, capped at 2048, the first run converted 85 and exposed a false block on near-opaque normal alpha (Note L). The re-run converted 97, skipped 13 `pbr_albedo` files with `SUG-LOW-CONFIDENCE` and failed none; `sdc validate` passed all 97. The MVP work also fixed rotated BC7 mode 4/5 alpha below 254 without exact-opaque, a DDS next to its PNG counting as a duplicate, and validate judging stored alpha as removed.
- **Final review of `feat/phase0-2` (59cf60c).** 0 critical, 8 important and 16 minor findings, and no wrong bytes or exit codes on a shipped path. Fixed then: per-file panic isolation in planning (I1), the blocking `IN-FLOAT-LDR` plus `LOSS-CLAMP-HDR` for a float source written to an LDR format, until the M6 transfer split (I4), and size and mtime recorded at plan time and compared before Load (I7).

## Other changes since the plan

- **Plain `_normal` files are assumed DirectX**, with an info message, instead of blocking until the user states the convention ([DECISIONS](../../.claude/DECISIONS.md) Note AW).
- **Version 0.8.0** is the first public alpha ([DECISIONS](../../.claude/DECISIONS.md) Note AX).
- **Channel packing, RNM normal layers, complex grass, PGPatcher JSON export and a 3D preview** were added after the plan. See the [wiki](../wiki/Home.md).
