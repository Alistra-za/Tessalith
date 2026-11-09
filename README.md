<p align="center"><img src="docs/assets/tessalith-emblem.png" alt="Tessalith texture tools" width="640"></p>

<h1 align="center">Tessalith</h1>

<p align="center">Converts texture source images to DDS for Skyrim SE/AE.</p>

---

For Skyrim SE/AE mod authors who want a folder of source images turned into finished DDS sets. Tessalith works out what each file is from its name, groups files into material sets, packs loose maps into Skyrim's layouts, builds the mips and picks the format and colour tag each shader needs. The GUI (`tessalith.exe`) and the CLI (`sdc`) share one engine and one per-project `sdc.toml`, so both write the same bytes. It makes new textures; it doesn't fix an installed game folder.

## Get it

Download the zip from [Releases](https://github.com/Alistra-za/Tessalith/releases), unzip it anywhere and run `tessalith.exe`. Windows 10 or 11, x64. The exes are unsigned, so SmartScreen warns on the first run ([Installation](docs/wiki/Installation.md#download-and-run)). Building from source is in [CONTRIBUTING.md](CONTRIBUTING.md#build-from-source). What changed in each version is in the [CHANGELOG](CHANGELOG.md).

## Features

**Encoding and checks**
- BC1, BC3, BC4, BC5, BC6H, BC7 and uncompressed output, each with the [best encoder for its format](#uses-the-best-encoder-available). BC1 is scored for NVIDIA and AMD decoders together.
- [BC7 weighting](docs/wiki/GUI-Texture-Panel.md#bc7-weighting-against-bc3) for data maps whose alpha matters.
- Every encode is decoded and checked against Skyrim's rules. DDS files from other tools can be validated too.
- [Artist checks](docs/wiki/Texture-Types.md#artist-checks) warn about a flipped normal, missing UV coverage and similar. They never block an export.

**Maps and packing**
- Inputs: PNG, TGA, TIFF, EXR, HDR, BMP, JPEG, WebP, PSD/PSB.
- sRGB or UNORM chosen per map type and target shader. Sizes round to a power of two.
- Cutout alpha keeps its coverage in every mip. Normal mips are renormalised, and their variance goes into TruePBR roughness.
- Loose maps pack into `_rmaos`, `_rfaos` and complex `_m`, with suffix-based channel remapping. The complex-material `_m` gate is checked.
- Detail normals ([Detail names](docs/wiki/Texture-Types.md#detail-names)), complex grass and [cubemaps](docs/wiki/Cubemaps.md).

**Skyrim and mod aware**
- A mod scan lets the meshes decide alpha and shader ([Meshes and mod scanning](docs/wiki/Mod-Scanning.md)).
- TruePBR: a PGPatcher JSON per set, validated, with every option exposed, plus the `_m` sidecar.
- PBR Landscape JSONs built from the EDIDs in your load order.
- [Also export as Vanilla and Complex material](docs/wiki/Set-Shaders.md): converts PBR sets to those two formats.
- Runs standalone or from MO2.

**GUI**
- Encoded-mip preview with lens and difference heatmap.
- A [3D preview](docs/wiki/Previews.md) that matches Community Shaders' deferred rendering.

**NIF editing**
- Fields, shader flags, controllers, a key editor and clip playback with an in-game preview.

**Command line**
- Batch convert, inspect and validate. See the [CLI reference](docs/wiki/CLI-Reference.md).

**What sets it apart.** It starts from what the file is: suffix and folder give a sub-type, and the sub-type sets format, tag, alpha and mips. Unclear files are skipped until you decide them. It blocks combinations Skyrim mishandles, such as `_SRGB` outside TruePBR colour under `cs`, or BC4/BC5 on a normal map. A manifest lets re-runs redo only what changed.

## Uses the best encoder available

Each format uses the encoder that measured best. Why, with the numbers: [Research](docs/wiki/Research.md).

### BC7

![Back-to-back bar chart of BC7 encoders. Left bars show speed in seconds per megapixel on a log scale, where shorter is faster. Right bars show quality in dB against bc7e u6. Tessalith's opt-in bc7e max is best at plus 0.15 dB RGB and plus 0.04 dB alpha, about 0.6 s per megapixel. The default bc7e u5 is level with u6 at 0.22 s. NVTT Production, Compressonator and texconv CPU take 14 to 33 s and do not beat it. GPU encoders take 0.03 to 0.09 s and lose 1.4 to 2.8 dB.](docs/img/encoder-bc7.svg)

`max` is opt-in. NVTT isn't part of Tessalith; it's charted as a reference.

### Other formats

![Bar chart grouped by format (BC1 on the NVIDIA and AMD decoders, BC3 smooth and cutout alpha, BC4, BC5, BC6H). Each bar is an encoder's PSNR difference in dB from Tessalith's encoder for that format, which is 0; each format uses its own images, so bars compare only within a format. Tessalith's patched rgbcx is best on BC1, BC3 and BC4 and ties NVTT 3 on BC5; NVTT 3 is closest elsewhere, 0.08 to 1.17 dB behind. On BC6H Tessalith's Intel ISPC leads. Missing values are marked not measured.](docs/img/encoder-formats.svg)

Each format has its own images, so compare bars within a format only.

## Quick start

- **GUI:** run `tessalith.exe`, **Open folder…**, pick the **Mode**, settle any amber-bordered texture, **Export**.
- **CLI:** `sdc convert textures --recursive -o out --accept-suggestions high`
- Back up your mod folders before you convert in place.

## Documentation

[Wiki home](docs/wiki/Home.md) · [Getting started](docs/wiki/Getting-Started.md) · [Troubleshooting](docs/wiki/Troubleshooting.md) · [CLI reference](docs/wiki/CLI-Reference.md)

Contributing and the design records are in [CONTRIBUTING.md](CONTRIBUTING.md).

## Credits and licence

Tessalith is licensed under GPL-3.0-or-later; see [LICENSE](LICENSE). The conversion engine (`engine`, `encoders-sys`) is MPL-2.0. It builds on bc7e and rgbcx (Rich Geldreich / Binomial LLC), the Intel ISPC Texture Compressor and bcdec, among others. Third-party licences and the changes made to vendored code are listed in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) and [LICENSES/](LICENSES/). Credits for ideas and prior work, including AlphaBloom's Auto Coverage, Ignacio Castaño's coverage-preserving mips and jiayev's TruePBR-Manager, are in [docs/CREDITS.md](docs/CREDITS.md).
