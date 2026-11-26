# Research behind the defaults

For modders who ask why Tessalith picked a format, encoder or filter. One bullet per choice: what was measured, what it decided, and a link to the numbers in [docs/research](../research/README.md). The newest bench is round 3, with the BC1 Dual rescore of 2026-11-04; each research page gives the date and build it measured.

## What matters most

- Picking the right format and colour space moves quality by 5 to 15 dB, and a semantic mistake (wrong tag, lost alpha data) breaks the texture ([why](../research/encoder-benchmark.md#the-format-matters-more-than-the-encoder)).
- The top BC7 encoders sit within 0.16 to 0.32 dB of each other, so the encoder matters less than the format.

## Why these encoders?

- **BC7:** bc7e at `veryslow` (level 5) is the default. In the only independent multi-encoder benchmark (Aras Pranckevičius, 2020) bc7e "slow" beat ISPC TC "slow" by 1.1 dB at about 10 times the speed. Locally, level 5 is within ±0.03 dB of level 6 at 0.22 against 0.28 s/Mpix. Level 4 isn't offered: it lost to level 3 on gradients ([bc7e tuning](../research/bc7e-tuning.md)). Tessalith's bc7e carries fixes and a finer default ranking from patch 0005 ([kernel fixes](../research/bc7e-tuning.md#kernel-fixes-patch-0005-2026-11-25-note-lr)).
- **BC7 `max`:** an opt-in level. On 150 Skyrim textures it gained 0.18 dB RGB on diffuse and 0.24 dB on alpha over `veryslow`, at 2.7 to 2.9 times the time.
- **BC7 alpha weights:** a weighted BC7 can stand in for BC3 on data maps whose alpha matters ([the setting](GUI-Texture-Panel.md#bc7-weighting-against-bc3)).

![Grouped bar chart of mean RGB and alpha PSNR for BC3 and the BC7 weightings on three texture classes. On colour with smooth alpha, Alpha ×8 gives 51.40 dB alpha against 47.24 for BC3 and keeps 46.60 dB RGB against 41.00. On a normal map or packed rmaos with specular in alpha, BC3 keeps the highest alpha, and BC7 alpha rises with the weight.](../img/bc7-alpha-weights.svg)

- **texconv and Compressonator don't encode.** texconv was the weakest BC7 encoder at every setting, and Compressonator is unmaintained and writes BC7 headers with required flags missing. texconv is the test oracle only.
- **BC1 and BC3:** rgbcx level 18 led BC1 (38.47 dB, all encoders within 0.66 dB). `encoder.bc1_search = fast` runs level 12, which still beats NVTT 3 `-highest` on both decoders and encodes faster ([numbers](../research/encoder-benchmark.md#fast-bc1-search-2026-11-14)). <!-- D:HY16 -->
- **BC1 Dual:** rgbcx scores BC1 selectors against NVIDIA and AMD decoding together. It beat the Ideal target on all 93 textures on both vendors at 1.6 times the encode time, so BC1 and BC3 colour always use it and there's no decoder setting ([numbers](../research/encoder-benchmark.md#bc1-dual-nvidia-and-amd-scored-together-2026-11-02-note-fm17)). <!-- D:FM17 -->
- **The format picks the encoder:** rgbcx for BC1, BC3, BC4 and BC5 (Tessalith runs its own Rust port of it, which writes the same bytes in a fifth of the time or less: [numbers](../research/encoder-benchmark.md#encode-time-of-the-rust-port-of-rgbcx-2026-11-13-note-hy)), ISPC for BC6H, bc7e for BC7. Uncompressed formats run none. <!-- D:FM15 -->
- **AVX-512 for BC7 and BC6H:** on a CPU with AVX-512, bc7e and the ISPC Texture Compressor run ISPC's AVX-512 path. On the one CPU measured (a Ryzen 7 9800X3D) it took 0.61 to 0.81 of the AVX2 time at equal quality ([numbers](../research/avx512-ispc.md); [what it means for re-runs](Validation-and-Output.md#re-runs-and-if-changed)). <!-- D:HZ1 --> A CPU without AVX2 runs ISPC's SSE2 path. On the CPU measured it took 1.76 times the time of the AVX path for BC7 and 1.50 times for BC6H, and 1.29 and 1.09 times the time of the SSE4 path, which Tessalith no longer ships either ([numbers](../research/avx512-ispc.md#without-avx2-the-sse2-path)). <!-- D:JB1 -->
- **Perceptual weighting is off,** and locked off except for colour maps whose alpha isn't data. It raised normal-map angular error by 37% and cost 1.6 dB on packed data.
- **No RDO.** It only shrinks archives (42.25 dB fell to 39.51 dB at λ = 1 in one published run) and never reduces GPU memory.

## Why this format and tag?

- **BC7 keeps the blue channel.** Skyrim reads Z from blue on a base `_n`. BC5 drops both Z and specular alpha. Benchmarks that rebuild Z from BC5 measure a different shader path. See [Formats and mips](Texture-Types.md#formats-and-mips).
- **`_SRGB` only for TruePBR colour under `cs`.** Vanilla diffuse stores gamma-encoded colour in UNORM files, and an `_SRGB` tag darkens it in vanilla shaders, CS non-PBR paths and LOD. TruePBR expects albedo tagged `_SRGB`. `_SRGB` under a `lod` path is flagged ([the rule](Texture-Types.md#srgb-or-unorm)).
- **BC1 alpha is opt-in.** On cutouts it lost anti-aliased edges and visible RGB quality against BC7, so the compact presets use BC1 only for colour without alpha.
- **Small BC7 textures keep their format.** Full chains from 16 to 128 px ran in game without a crash (spike S2), so there's no fallback.

## Why these alpha rules?

- **Stray alpha is removed before mips.** It costs about 1 dB on real images and up to 17 dB on smooth ones.
- **Channel guard A (exact opaque) is on by default for `_m` and env masks only.** Without it bc7e decoded 254 instead of 255 on 28% of pixels of RGB-only input (spike S5), and 7 of 28 real `_m` files had 40% or more of their alpha off 255 (spike S6). CS, ENB and PGPatcher read that alpha. Forcing 255 costs 0.5 to 1.0 dB RGB on average; keeping alpha 0 and 255 exact in alpha blocks too lowers the combined colour and alpha score of colour maps with alpha by up to 0.21 dB ([A against the old flag](../research/bc7e-tuning.md#channel-guard-a-against-the-old-exact-opaque-flag)).
- **The complex-material gate is 251/255.** A RenderDoc read of the CS lighting shader (spike S8) showed the smallest mip's alpha must stay below 251/255. The `_m` margin check warns at 247.
- **Cutout mips keep coverage.** Box-filtered alpha thins out foliage at distance. Castaño's coverage-preserving method keeps the share of texels that pass the alpha test the same at every level. Default threshold 127 ([method](../research/mipmaps-and-alpha.md)).

## Why these mip filters?

- **Box is the default,** colour is filtered in linear light, and Kaiser is fixed at width 3, α 4: larger radii ring ([mip filters](../research/mipmaps-and-alpha.md)). No published PSNR benchmark of mip filters for game textures exists.
- **Tent, Mitchell and Kaiser build every level from mip 0.** Chaining them cost 0.8 to 2.7 dB against a direct Lanczos-8 reference. Box stays recursive, where that's exact.

## Why renormalise normal mips?

- A box of unit normals has a shorter average vector when the directions differ. Unity HDRP and other engines use this loss to widen GGX roughness. Tessalith uses a vMF fit to estimate the added variance from that average vector's length. See the [clamp and normal length settings](Project-Config.md#mipmaps) and the [variance and gloss bake](Texture-Packing.md#variance-and-gloss-bake).
- Quantisation noise alone on a flat normal map lifts roughness at lower mips, so a noise floor at L = 0.999 adds no variance (about a 2° tilt).

## Why the BC4/BC5 search?

- Upstream rgbcx modelled BC4 decoding with floored interpolants while GPUs round to nearest, so a wider search could make results worse. Tessalith's rgbcx rounds like the decoder.
- rgbcx anchored BC4's 6-value mode on the block's minimum and maximum. A wider block search in the vendored rgbcx gains 3.2 dB on cutout alpha at about twice the BC4 search time ([BC4-family encoding](../research/bc4-family.md)). <!-- D:FJ1,FJ2 -->

The encoder tables behind the README charts are in [encoder-benchmark](../research/encoder-benchmark.md).
