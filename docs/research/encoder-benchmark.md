# BCn encoders by format

Which encoder and setting gives the best quality for BC7, BC1, BC3, BC4, BC5 and BC6H, from a literature review and a local benchmark, with one section per format. The deeper BC7 and BC4-family studies have their own pages ([BC7 tuning and alpha weights](bc7e-tuning.md), [BC4-family block search](bc4-family.md)); [the index](README.md) lists every section by format.

The literature review and benchmark rounds 1 to 3 ran on 8 and 9 October 2026 (page first written 2026-10-16, commit 1bae037b). Later rows carry their own date and build. Where round 3 re-measured a claim on 100 images, its number is the one shown.

## Method

### Literature review

- Read the format specifications, encoder source code and official documentation.
- Collected every published measurement that compares encoders, and noted who made it.
- Covered bc7e and rgbcx (Rich Geldreich's bc7enc_rdo), Intel ISPC Texture Compressor, NVIDIA Texture Tools (NVTT) 2 and 3, DirectXTex/texconv, AMD Compressonator, Oodle Texture, ConvectionKernels, icbc, Ludicon Spark, Betsy and a few wrappers.

Each claim was labelled by its source: an independent measurement, a measurement by the encoder's author or a competitor, documentation or source code, an unsupported claim, or the review's own inference. PSNR values can only be compared inside one study. Studies differ in metric (RGB, luma, RGBA, RMSE, SSIMULACRA2), test images and whether perceptual modes were on.

The published data had large gaps, so the benchmark below was run. Where the two disagree, the benchmark wins.

### Benchmark: machine and tools

| Item | Value |
|---|---|
| CPU / GPU | AMD Ryzen 7 9800X3D (8 cores, 16 threads), NVIDIA RTX 4090 |
| OS | Windows 11 Pro 26200 |
| bc7enc_rdo | v1.08, commit b943862; bc7e built with ISPC 1.31.0 |
| Intel ISPC TC | commit 79ddbc9, through a small command-line wrapper (16 row strips on 16 threads) |
| NVIDIA Texture Tools | 3.2.5 (`nvcompress`) |
| DirectXTex texconv | 2026.5.8.1 |
| AMD Compressonator CLI | 4.5.52 |

### Images

- Round 1: 39 images in 10 sets, 2 to 7 real textures each (CC0 ambientCG, 1K) plus synthetic gradients. Round 2 added mask, normal, data and HDR sets of 2 to 7 images each (4 real and 4 synthetic HDR images).
- Round 3: 100 real images per set (18 HDR images), from 121 ambientCG materials, 191 ambientCG materials, decals and atlases with opacity, and 14 Poly Haven HDRIs. At most 4 assets per material family. All CC0.

| Set | What it holds | Main metric |
|---|---|---|
| `rgb_real` | colour maps | RGB PSNR |
| `rgb_grad` | synthetic gradients (round 1 only, 7 images) | RGB PSNR |
| `cutout_real` | colour with anti-aliased opacity | RGB PSNR over visible pixels, coverage error |
| `alpha_real` | smooth opacity (decals) | premultiplied RGB PSNR |
| `packed_real` | colour plus an unrelated map in alpha | RGB PSNR and alpha PSNR |
| `stray_real` | opaque images with 2% of texels at alpha 250 | RGB PSNR against the clean twin |
| `mask_real` | single-channel masks (roughness, AO, height, opacity, metalness) | PSNR |
| `normal_real` | DirectX normal maps | angular error in degrees |
| `normalh_real` | normal plus height in alpha | angle and height PSNR |
| `data_real` / `dataa_real` | three data maps in RGB, with and without a fourth map in alpha | RGB and alpha PSNR |
| `hdr_real` | HDR images | multi-exposure PSNR (mPSNR), RMSLE, peak ratio |

### Protocol

- **One decoder for every output**, mip 0 only, compared with the source in 8-bit code values. No colour-space conversion on either side. BC1 colours were decoded as `round((2a+b)/3)` ("IdealRound4").
- <a id="bench-decoder-floors-bc4"></a>**The bench decoder floors BC4 interpolants.** `bcdecode` is built on upstream rgbcx, which decodes the six interpolated BC4 values as `(6a + b) / 7` and so on, rounded down; bcdec and GPUs round to nearest. This affects BC4, both BC5 channels and BC3 alpha. Upstream rgbcx models the same floor, so its rows score well under this decoder. Every BC3, BC4 and BC5 output was later rescored with bcdec's round-to-nearest weights as well ([BC3, BC4 and BC5](#bc3-bc4-and-bc5-tessaliths-encoder-against-the-others)).
- **Normal maps** were scored the way a BC5 shader reads them: X and Y decoded, Z rebuilt. BC7 was also scored with its stored Z.
- **BC6H** used identical fp16 input for every tool. mPSNR uses 13 exposures from 2⁻⁶ to 2⁶. Peak ratio is decoded peak luminance divided by source peak; 1.0 means the brightest highlight survives.
- **Alpha flags were set the way a real pipeline would**: NVTT `-noalpha` on opaque sets and `-alpha` on alpha sets; ISPC alpha profiles on alpha sets.
- **Statistics (round 3).** For each claim "A against B", the per-image difference was averaged, with a 95% Student-t interval, a bootstrap interval and a bootstrap that resamples material families. A side is called better only when both the t-interval and the family interval exclude 0. Medians and win counts are given too, because a few near-lossless images can pull a mean.

In the tables, Δ is A minus B, so a positive number means A is better. For angles and RMSLE the sign is flipped so that + still means A is better.

### Metrics and their limits

- Metrics are PSNR in 8-bit code values, box-window SSIM, angular error, mPSNR and RMSLE. No perceptual metric such as SSIMULACRA2 was used.
- Only mip 0 was scored. Normal-map scores are pure angle, without shading.
- <a id="perceptual-metrics-on-linear-data"></a>**Perceptual weighting fits colour only.** Luma metrics, SSIM and VIF on luma, and bc7e's perceptual mode all weight error the way the eye sees colour. Normals, masks, height and packed data are linear values where luma means nothing, so those sets are scored per channel or by angle, and a perceptual encoder row on them measures the damage the weighting does.
- The new images all come from ambientCG and Poly Haven: mostly procedural PBR textures, not hand-painted or UI art.
- The smooth-alpha set covers only 15 material families (31 of its 100 images are road-line decals), so its family intervals are wide.
- Timings come from single runs on one machine. Round 3 timings are not comparable with rounds 1 and 2.

#### SSIM and VIF rescoring (2026-10-21)

The round 3 encodes were rescored with Gaussian SSIM and multi-scale VIFp (commit dbf54808), on the same pairs as the round 3 confidence-interval study. Nothing was re-encoded. SSIM uses an 11×11 Gaussian window (σ 1.5); VIFp uses 4 scales (windows 17, 9, 5, 3). Colour sets are scored on Rec.709 luma (cutout on visible luma, smooth alpha on premultiplied luma). Masks use R, normals use the stored R and G, data uses R, G and B, and alpha is scored on its own. Both metrics are on the 0 to 255 scale. SSIM and VIF are not defined for HDR without a tonemapping choice, so BC6H is not scored.

The format sections carry the results as extra columns: ΔSSIM ×10³ and ΔVIF ×10², each the study's mean difference with images won and lost by A (W/L), and a confidence interval only where the study printed one. **Bold** marks a value whose verdict (from the 95% t and family-cluster intervals) differs from the PSNR or angle verdict.

Of 55 paired comparisons, SSIM gives the same verdict as PSNR on 46 and VIF on 47. Most disagreements come from channel weighting: on colour, SSIM and VIF see luma only, so they side with luma PSNR. Where there is no luma step (masks, normals, data) the three metrics agree almost perfectly.

Not scored: Compressonator q1.0 on round 3 colour, cutout and alpha (never encoded; only 4 to 7 round 1 images exist), Compressonator on the round 2 sets, Compressonator and texconv CPU on the packed and stray sets, texconv CPU outside real colour (2 to 5 old images), the synthetic gradient sets and the round 1 variants round 3 dropped. texconv CPU on normal + height lacks 40 of 96 images, so its ranks use the 60 images where all variants exist.

## The format matters more than the encoder

- BC1 encoders were all within **0.66 dB** of each other in the 2020 independent benchmark by Aras Pranckevičius.
- BC3's colour block *is* a BC1 block, so BC3 never improves RGB over BC1. It only adds smooth alpha.
- On plain RGB PSNR the top BC7 encoders sat only a few tenths of a dB apart.

Format gap at best settings (round 1):

| | BC7 best | BC1 best | BC3 best |
|---|---|---|---|
| Opaque real (RGB PSNR, dB) | 50.96 | 41.24 (−9.7) | – |
| Opaque gradients | 62.28 (66.14 NVTT Production) | 48.48 (−14) | – |
| Smooth alpha real (premultiplied) | 55.42 | – | 50.43 (−5.0) |

The encoder moves BC7 quality by about 0.3 to 1.6 dB at top settings. The format moves it by 5 to 15 dB.

## BC7

### How the format works

Every BC7 block is 16 bytes for 16 texels and uses one of eight modes.

| Mode | Subsets | Partitions | Colour endpoints | Alpha | P-bits | Index bits |
|---|---|---|---|---|---|---|
| 0 | 3 | 16 | RGB 4.4.4 + p | none (255) | per endpoint | 3 |
| 1 | 2 | 64 | RGB 6.6.6 + p | none | shared per subset | 3 |
| 2 | 3 | 64 | RGB 5.5.5 | none | none | 2 |
| 3 | 2 | 64 | RGB 7.7.7 + p | none | per endpoint | 2 |
| 4 | 1 | – | RGB 5.5.5 | separate 6-bit | none | 2 and 3 |
| 5 | 1 | – | RGB 7.7.7 | separate 8-bit | none | 2 and 2 |
| 6 | 1 | – | RGBA 7.7.7.7 + p | combined | per endpoint | 4 |
| 7 | 2 | 64 | RGBA 5.5.5.5 + p | combined | per endpoint | 2 |

- Modes 0 to 3 have no alpha and decode it as 255. A block with even one texel below 255 cannot use them, which leaves mode 7 as the only partitioned mode. This is how alpha takes quality away from RGB, block by block.
- Mode 6 alone is not enough. In Geldreich's data (about 2018), a mode-6-only encode scored 42.8 dB average RGB PSNR against 45.8 to 46.0 dB with the partitioned modes.

### Published measurements

The only independent multi-encoder BC7 study is Aras Pranckevičius's (December 2020): 20 Unity textures, luma PSNR with perceptual modes on where available, 6-core i9-8950HK. NVTT, DirectXTex and Compressonator were not in it.

| Encoder / preset | Mpix/s | Average luma PSNR (dB) |
|---|---|---|
| ISPC TC basic | 4.53 | 46.07 |
| ISPC TC slow | 1.29 | 46.28 |
| bc7enc 64.4 (max) | 3.82 | 45.48 |
| bc7e fast | 30.97 | 46.18 |
| bc7e basic | 20.65 | 46.75 |
| bc7e slow | 13.04 | 47.40 |

- Per image, bc7e slow minus ISPC slow ranged from +3.10 dB to −0.77 dB (mean +1.13 dB). On the four normal maps the two were within ±0.26 dB.
- Part of bc7e's lead comes from its perceptual mode, which ISPC TC lacks, scored on a luma metric.

Geldreich's 2018 runs with perceptual off (31 images, linear RGB PSNR, author data):

| Encoder | Setting | Time (s) | RGB PSNR (dB) |
|---|---|---|---|
| ISPC TC | slow | 355.3 | 46.77 |
| Geldreich's ISPC encoder | uber 1 | 86.4 | 46.72 |
| Geldreich's ISPC encoder | uber 4, 64 partitions, p-bit search | 292.5 | 46.96 |
| Basis BC7 | – | 365.5 | 47.16 |
| NVTT 2 AVPCL | – | 28,061.9 | 47.14 |

NVTT 2's AVPCL is not NVTT 3's encoder, so these numbers do not describe NVTT 3.

What each encoder's top setting searches:

| Encoder at its top setting | Partitions fully evaluated | Endpoint search | Metric |
|---|---|---|---|
| ISPC TC `slow` | all 64 (modes 1/3) | 4 refinement iterations | linear |
| texconv `-bc x` | top 25% (16 of 64) | perturbation plus ±5 search; p-bits by majority vote | linear |
| bc7e `slowest` | top 4 | "uber" level 4, full p-bit search | linear or perceptual |
| Compressonator 1.0 | threshold-mapped, internals not examined | – | – |
| NVTT 3 Highest | undocumented, CPU only | – | – |

The review ranked the quality levers at top settings roughly as: whether partitioned modes and the mode 4/5 rotations are searched at all; how many partitions get a full evaluation; endpoint refinement depth; p-bit search; and last the metric, which changes what counts as best.

What the review could not answer, and the benchmark below measured:
- No public measurement compared NVTT 3 (Highest), Compressonator (`-Quality 1.0`), texconv and bc7e on one test set.
- No credible multi-encoder benchmark of BC7 with real alpha existed.
- NVTT 3 does not document what its "Highest" level does. NVIDIA staff did state that BC7 Production and Highest run on the CPU only.
- No study isolated the value of a proper p-bit search.

### BC7 on opaque colour

Round 3, `rgb_real`, n = 99, against bc7e level 6 (`-u6`); the luma, SSIM and VIF columns are the [2026-10-21 rescoring](#ssim-and-vif-rescoring-2026-10-21):

| Encoder | Mean Δ RGB (dB) [95% CI] | Median | Images won | Round 1 (n = 7) | Luma Δ (dB) | ΔSSIM ×10³ (W/L) | ΔVIF ×10² (W/L) |
|---|---|---|---|---|---|---|---|
| bc7e u5 | +0.03 [+0.01, +0.05] | +0.02 | 70/99 | +0.01 | −0.06 | **−0.058** [−0.070, −0.047] (14/85) | **−0.10** [−0.13, −0.08] (14/85) |
| bc7e u4 | −0.40 [−0.44, −0.36] | −0.38 | 0/99 | −0.39 | −0.45 | −0.27 (0/99) | −0.54 (0/99) |
| NVTT Production | −0.27 [−0.51, −0.02] | −0.03 | 32/99 | −0.32 | −0.52 | −0.180 [−0.215, −0.144] (18/81) | −0.46 [−0.55, −0.37] (10/89) |
| ISPC slow | −0.44 [−0.50, −0.38] | −0.36 | 0/99 | −0.59 | −0.58 | −0.28 (2/97) | −0.49 (7/92) |
| ISPC basic | −0.59 [−0.70, −0.49] | −0.43 | 0/99 | −0.75 | −0.77 | −0.32 (2/97) | −0.62 (6/93) |
| NVTT Normal (GPU) | −1.36 [−1.69, −1.02] | −0.92 | 0/99 | −1.42 | −1.83 | −0.80 (0/99) | −1.74 (0/99) |
| texconv `-bc x` | −1.47 [−1.77, −1.17] | −0.96 | 0/99 | −1.58 | −1.95 | −0.98 (0/99) | −2.05 (0/99) |
| texconv GPU | −1.76 [−1.99, −1.54] | −1.55 | 0/99 | −1.76 | −2.10 | −1.23 (0/99) | −2.40 (0/99) |
| bc7e u6 perceptual, RGB PSNR | −1.05 [−1.18, −0.91] | −1.07 | 5/99 | −1.03 | next row | **+1.63** [+1.32, +1.93] (99/0) | **+3.10** [+2.63, +3.56] (99/0) |
| bc7e u6 perceptual, luma PSNR | +3.69 [+3.31, +4.06] | +3.41 | 99/99 | +4.77 | | | |
| Compressonator `-Quality 1.0` | not re-measured | | 4/7 | −0.16 | −0.24 (n = 7) | −0.12 (2/5) | −0.16 (2/5) |

![Two bar charts of the SSIM and VIF difference against bc7e u6 on 99 real colour textures. bc7e perceptual is the only encoder above zero, at plus 1.63 SSIM and plus 3.10 VIF. The other encoders are at or below u6, and texconv GPU is furthest below at minus 2.40 VIF.](../img/encoder-ssim-vif.svg)

- **bc7e level 5 equals level 6 on RGB PSNR.** It is at least as good on average and 1.5× faster. SSIM and VIF prefer u6 on 85 of 99 images, as does luma PSNR: the u5 edge is in chroma.
- **NVTT Production is a practical tie on typical textures** (median −0.03 dB). Its mean is lower because of a few near-flat images where both encoders are far below one code value. On luma the gap is twice the RGB gap, and SSIM and VIF favour bc7e on 81 and 89 of 99 images.
- **Perceptual mode trades RGB for luma:** about −1 dB RGB PSNR for +3.7 dB luma PSNR. It wins every image on SSIM and VIF, which makes it the best BC7 encoder on opaque colour by those metrics; its RGB cost is chroma error they cannot see.
- **texconv is the weakest BC7 encoder at every setting.** `-bc x` (modes 0 and 2 on) adds only +0.04 dB, and SSIM and VIF call it a tie (49/42 images).

Speed, round 1, file-to-file time for a 1-Mpix texture (start-up cost subtracted):

| Encoder | s per Mpix | Δ RGB on real images (round 1) |
|---|---|---|
| NVTT Normal (GPU), texconv GPU | 0.03 to 0.09 | −1.4 / −1.8 |
| ISPC basic | 0.11 | −0.75 |
| bc7e u4 | 0.12 | −0.39 |
| bc7e u5 | 0.22 | +0.01 |
| bc7e u6 | 0.28 | 0 |
| ISPC slow | 0.30 | −0.59 |
| NVTT Production (= Highest) | about 14 | −0.32 |
| texconv CPU `-bc x` | about 16 to 20 | −1.6 |
| Compressonator `-Quality 1.0` | about 33 | −0.16 |

bc7e kernel speed by level (Mpix/s): u0 146, u1 44, u2 38, u3 19, u4 24, u5 6.7, u6 4.5. Levels 3 and 4 are out of order: level 3 is slower than level 4, and on gradients level 3 beats level 4.

#### Smooth gradients (round 1, 7 synthetic images)

| Encoder | Mean RGB PSNR (dB) | Δ against bc7e u6 |
|---|---|---|
| NVTT Production | 66.14 | +3.86 |
| Compressonator `-Quality 1.0` | 65.49 | +3.21 |
| ISPC slow | 63.01 | +0.73 |
| bc7e u6 / u5 | 62.28 / 62.27 | 0 / −0.01 |
| bc7e u4 | 59.89 | −2.39 |
| bc7e u6 perceptual | 58.42 | −3.86 |
| NVTT Normal | 56.08 | −6.20 |
| texconv (any setting) | 55.4 to 55.9 | −6.4 to −6.9 |

NVTT Production and Compressonator spend their much longer time fitting the ramp exactly. At 8 bits the gap is invisible: every BC7 encoder at a high setting kept the largest error at 1 to 2 code values on every gradient except the noise image. It matters only if the texture is later contrast-stretched (a lookup ramp or a brightness multiplier, for example).

### BC7 with alpha

Round 3, n = 100, against bc7e u6 (on the coverage row, SSIM and VIF score the alpha channel):

| Set, metric | Encoder | Mean Δ [95% CI] | Median | Wins | Verdict | ΔSSIM ×10³ (W/L) | ΔVIF ×10² (W/L) |
|---|---|---|---|---|---|---|---|
| Cutout, visible RGB PSNR (dB) | NVTT Production | −0.14 [−0.19, −0.08] | −0.06 | 29/100 | bc7e better, by a small margin | −0.088 (16/84) | −0.32 (10/90) |
| Cutout, coverage error (points) | NVTT Production | +0.005 [+0.001, +0.009] | +0.001 | 66/100 | negligible either way | +1.35 (99/1) | +0.33 (82/18) |
| Smooth alpha, premultiplied PSNR (dB) | NVTT Production | −0.06 [−0.15, +0.02] | −0.07 | 42/100 | tie | +0.13 [+0.04, +0.22], cluster [−0.17, +0.39] (49/51) | −0.06 (43/57) |
| Smooth alpha, premultiplied PSNR (dB) | ISPC slow | −0.58 [−0.65, −0.50] | −0.50 | 1/100 | bc7e better | −0.49 (2/98) | −0.45 (6/94) |
| Smooth alpha, premultiplied PSNR (dB) | texconv `-bc x` | −2.04 [−2.31, −1.78] | −1.74 | 1/100 | bc7e better | −0.91 (4/96) | −2.42 (2/98) |
| Packed, RGB PSNR (dB) | NVTT Production | +0.04 [−0.00, +0.09] | +0.04 | 69/100 | tie | −0.006 (41/59) | −0.12 (26/74) |
| Packed, alpha PSNR (dB) | NVTT Production | +0.27 [−0.05, +0.59] | +0.06 | 73/100 | tie, leaning NVTT | **+0.28** [+0.15, +0.42] (70/30) | +0.04 (62/38) |
| Packed, RGB PSNR (dB) | bc7e u6 perceptual | −2.89 [−3.07, −2.70] | −2.81 | 0/100 | bc7e u6 better | −3.84 (16/84) | −4.58 (13/87) |

- On cutouts every BC7 encoder kept the coverage error (texels flipping across alpha 128) at 0.05 to 0.13% in round 1.
- Compressonator was measured only in round 1 (n = 4 to 5): −1.50 dB on smooth alpha and −0.47 dB on cutouts. It used modes 4 and 5 in **0.0%** of blocks at every setting, where bc7e used them in 47% of alpha blocks and NVTT Production in 48%.
- Perceptual mode weights alpha at 256 against luma at 128, so with data in alpha it spends bits on alpha at the cost of colour.

From the review:
- An all-255 alpha costs nothing if the encoder classifies blocks one by one. bc7e, ISPC TC, DirectXTex and Compressonator all do.
- Stray alpha is the trap. bc7e sends any block whose lowest alpha is below 255 to its alpha path. A "nearly opaque" texture with alpha 254 loses modes 0 to 3 in those blocks. The review's advice was to strip or snap near-opaque alpha before encoding.
- None of the open encoders weights RGB error by alpha, so they spend bits on colour hidden under alpha 0.
- NVTT 3 before 3.2.2 (July 2023) had its opaque and transparent BC7 compressors swapped.

#### Stray alpha (round 1)

Opaque images where 2% of texels have alpha 250, compared with the same encoder on the clean image:

| Encoder | Mean Δ RGB (dB) | Worst image |
|---|---|---|
| bc7e u6 | −1.71 | −4.33 (linear gradient) |
| ISPC slow | −1.32 | −4.27 |
| NVTT Production | −2.47 | −6.98 |
| NVTT Production `-noalpha` | −2.03 | −6.98 |
| texconv CPU `-bc x` | −0.57 | −1.44 |
| Compressonator `-Quality 1.0` | −5.19 | −17.21 |

- Stray alpha costs about 1 dB on real textures and up to 17 dB on smooth ones, because blocks with alpha below 255 lose modes 0 to 3.
- NVTT's `-noalpha` does not strip the alpha. In round 3 it helped on 98 of 100 images (+0.58 dB [+0.50, +0.65]; SSIM +0.33 ×10³ and VIF +0.65 ×10², 98/0) but only partly.
- The only complete fix is to set alpha to 255, or drop the channel, before encoding.

#### "Opaque" BC7 is not always alpha 255

Even on RGB-only input, some encoders decode alpha as 254 in some mode-6 blocks:

| Encoder | Pixels with alpha ≠ 255 on opaque input |
|---|---|
| bc7e u6 | 28.0% (all 254, mode 6) |
| ISPC slow | 14.8% (254, mode 6) |
| texconv CPU | 2.4% |
| Compressonator `-Quality 1.0` | 1.1% |
| NVTT Production | 0.0% |

Mode 6 stores alpha in 7 bits plus a p-bit shared with RGB, so alpha 255 needs that p-bit set to 1. bc7e's opaque path picks the p-bit for RGB only. This matters only where something reads the alpha of an "opaque" texture, which in Skyrim is the `_m` environment mask ([Skyrim texture rules](design-overview.md#skyrim-texture-rules)).

#### Forcing alpha 255: spikes S5, S5b and S6 (2026-10-08 and 09)

Two ways to force 255 were measured with bc7e u5 at mip 0 on `rgb_real`, `rgb_grad`, `stray_real` and `stray_grad` (18 images, alpha set to 255 as `remove` does), against plain u5. The pass limit, a per-set mean RGB loss of at most 0.05 dB, was missed by both and not loosened.

| Mean Δ RGB PSNR (dB) | `rgb_real` | `rgb_grad` | `stray_real` | `stray_grad` | Worst image |
|---|---|---|---|---|---|
| Native patch 0001, p-bits chosen with the RGB fit (S5) | −0.515 | −1.020 | −0.078 | −0.382 | `subtle_dark` −5.166 (84.01 → 78.85) |
| Byte patch after encoding: mode-6 alpha endpoints 127, both p-bits 1 (S5b) | −2.696 | −8.410 | −1.015 | −4.899 | `subtle_dark` −28.15 (84.01 → 55.86); Metal049A −7.76 |

- Both give alpha 255 on every texel of all 18 images, and the native patch is better on every image. With its flag off, the native patch is bit-identical to upstream on 39 of 39 images.
- The native patch still costs RGB because, with both p-bits 1, mode-6 endpoints can only take odd 8-bit values (7-bit precision). bc7e re-fits under that constraint and falls back to modes 1 to 5 where they win, so the loss is the constraint itself. The byte patch moves every p = 0 endpoint up one code without a re-fit, on the 11 to 99% of blocks per image that use mode 6. Near-lossless images and dark gradients lose most.
- With perceptual mode off, opaque noise can also pick mode 4 with rotation 1 to 3, which decodes alpha 239 to 255. The encode stage fixes every all-255 block of that kind losslessly (rotated slot endpoints set to the maximum, RGB bit-identical), with or without the flag and under any alpha policy (Note A).
- Full chain (S5): with the flag off, alpha stays at 254 or more at every mip down to 1×1. The 254 share is 0% at 1×1 but 100% at 2×2 on Metal049A, 4×2 on PaintedPlaster017 and 4×4 on `subtle_dark`. With the flag on, and in BC1, alpha is 255 at every mip. The engine path reproduced the bench's bc7e u5 mip-0 bytes on 39 of 39 BC7 images and 14 of 14 under `remove`, and its quantised level equals the PNG's 8-bit RGBA on 53 of 53.
- The smallest-mip alpha check (`engine::validate::alpha`) agrees with Note C on bc7e u6 (Bricks104 254 share 20.96%, Note C 21.0%) and with NVTT (0%).
- **Spike S6, real `_m` with the flag off.** 28 plain `_m` from 28 of the owner's mods were decoded to PNG and converted as BC7 with `alpha.policy = remove` and `alpha.exact_opaque = false`. The share of decoded mip-0 alpha ≠ 255 ran from 1.57% to 57.95%, about 27% on average; 7 of 28 files were at 40% or more (worst `frozenmarshice_m` 57.95%, `hair_acc_m` 57.19%, `Dibella_m` 51.02%).

The per-image output is in the bench's `exact-opaque-native`, `exact-opaque-bytepatch` and `full-chain` gates (branch `bench`).

### BC7 for linear data

Round 3, n = 100, against NVTT Production:

| Set | Encoder | Mean Δ (dB) [95% CI] | Median | Wins | ΔSSIM ×10³ (W/L) | ΔVIF ×10² (W/L) |
|---|---|---|---|---|---|---|
| Data, opaque | bc7e u6 | −0.56 [−0.94, −0.18] | −0.30 | 4/100 | −0.97 (3/97) | **−0.70**, cluster [−1.47, +0.11] (8/92) |
| Data, opaque | ISPC slow | −0.74 [−1.19, −0.30] | −0.42 | 0/100 | −1.28 (0/100) | −0.95 (2/98) |
| Data, opaque | texconv GPU | −4.25 [−4.88, −3.62] | −3.44 | 0/100 | −16.8 (0/100) | −9.96 (2/98) |
| Data, opaque | bc7e u6 perceptual against linear | −2.37 [−2.72, −2.01] | −2.09 | 0/100 | −8.45 (5/95) | −4.59 (3/97) |
| Data + alpha, RGB | bc7e u6 | −0.27 [−0.38, −0.17] | −0.20 | 2/100 | −1.27 (4/96) | −0.62 (8/92) |
| Data + alpha, alpha | bc7e u6 | −0.32 [−0.43, −0.22] | −0.19 | 18/100 | −2.17 (21/79) | −0.89 (18/82) |
| Data + alpha, RGB | texconv GPU | −3.26 [−3.54, −2.98] | −2.93 | 0/100 | −32.2 (0/100) | −10.7 (1/99) |

- NVTT Production is the best BC7 encoder for linear data, on both RGB and alpha, but only by a few tenths of a dB over bc7e, at roughly 40 to 60× the time (round 2).
- In round 2, separate BC4 masks (about 49.8 dB) beat three maps sharing one BC7 (about 42.8 dB), at 1.5× the memory.

### BC7 on normal maps

Mean angle with the stored Z in round 3 (100 images), sign flipped so + favours A. BC7 against BC5 is in [Normal maps: BC5 against BC7](#normal-maps-bc5-against-bc7).

| Comparison | Mean Δ [95% CI] | Median | Wins | ΔSSIM ×10³ (W/L) | ΔVIF ×10² (W/L) |
|---|---|---|---|---|---|
| bc7e u6 against NVTT Production | −0.051° [−0.058, −0.044] | −0.044° | 3/100 | −1.29 (2/95) | −0.71 (9/88) |
| Perceptual against linear bc7e u6 | −0.363° [−0.441, −0.284] | −0.257° | 10/100 | −12.1 (13/87) | −4.40 (30/70) |
| Normal + height: bc7e u6 against NVTT, angle | −0.061° [−0.071, −0.051] | −0.052° | 7/100 | −1.77 (11/89) | −0.78 (12/88) |
| Normal + height: bc7e u6 against NVTT, height PSNR | −0.26 dB [−0.33, −0.18] | −0.17 dB | 15/100 | −4.16 (9/91) | −0.62 (23/77) |

NVTT Production is slightly better than bc7e on BC7 normals, on angle and on height.

### Perceptual mode

- bc7e's perceptual mode measures error in a scaled YCbCr space (default weights Y 128, Cr 64, Cb 16, A 256) and also skips modes 4 and 5 on opaque blocks.
- It helps sRGB colour on luma metrics (Geldreich reports up to 2 dB in luma terms).
- Its cost on linear data ([why](#perceptual-metrics-on-linear-data)) is in the tables above: −2.89 dB RGB on packed alpha, −2.37 dB on data, where it favours green at the cost of red and blue, and −0.363° on normals. DirectXTex's documentation also says to use uniform weighting when RGB holds other data.

### BC7 at the `max` level (2026-10-31)

`bc7e_max` is Tessalith's `Bc7Level::Max` (bc7e patch 0002, [bc7e tuning](bc7e-tuning.md)), run on every BC7 image of all three rounds through `sdc-bench encode-one --format bc7 --level max` and scored like every other encoder. That calls `engine::encode::encode_level` with the default preset's profile at effort `max` (perceptual off, no exact-opaque flag, no custom weights), so it writes the engine's bytes, rotated-block alpha fix included. At `max` bc7e already writes opaque alpha in rotated blocks: the fix changed one block in 925 images (a mode-4 block in `data_real`, whose alpha is not scored), so every score equals raw bc7e Max.

Round 3, mean Δ of `bc7e_max` against the other encoder [95% t-interval], wins out of n (+ = `max` better; angles in degrees, sign flipped):

| Set (n) | Metric | vs bc7e u6 | vs NVTT Production | vs ISPC slow |
|---|---|---|---|---|
| `rgb_real` (92) | RGB PSNR | +0.19 dB [±0.03], 92/92 | +0.45 [±0.26], 85/92 | +0.62 [±0.07], 92/92 |
| `cutout_real` (95) | visible RGB PSNR | +0.19 [±0.02], 95/95 | +0.34 [±0.05], 94/95 | +0.46 [±0.04], 94/95 |
| `alpha_real` (96) | premultiplied PSNR | +0.12 [±0.02], 89/96 | +0.17 [±0.08], 77/96 | +0.69 [±0.08], 95/96 |
| `packed_real` (96) | RGB / A PSNR | +0.12 / +0.16 | +0.07 / −0.11 [±0.33] | +0.56 / +0.58 |
| `stray_real` (98) | RGB PSNR | +0.30 [±0.22], 97/98 | +0.45 [±0.14] | +0.66 [±0.53] |
| `normal_real` (93) | mean angle | +0.028° [±0.005], 90/93 | −0.012° [±0.002], 8/93 | +0.036° [±0.007] |
| `normalh_real` (96) | mean angle / height PSNR | +0.034° / +0.23 dB | −0.017° / −0.02 dB [±0.05] | +0.047° / +0.42 dB |
| `data_real` (96) | RGB PSNR | +0.25 [±0.13], 96/96 | −0.13 [±0.03], 18/96 | +0.40 [±0.13] |
| `dataa_real` (96) | lowest-channel PSNR | +0.19 [±0.04], 92/96 | −0.08 [±0.04], 25/96 | +0.24 [±0.03] |

- **`max` beats bc7e u6 on every set in every round**, by 0.1 to 0.3 dB on real textures; cutout coverage error moves by a negligible 0.002 points. Round 1: +0.15 dB on `rgb_real` (n = 7), up to +3.2 dB on the `rgb_grad` gradients.
- **It beats NVTT Production on colour and alpha** (opaque, cutout, smooth alpha, stray). On linear data and normal maps NVTT Production stays ahead by about 0.1 dB, or 0.01 to 0.02°, at roughly 100× the time.
- **On gradients (rounds 1 and 2) it roughly ties NVTT Production** (`rgb_grad` −0.13 dB [±0.73]), where u6 was 3.9 dB behind.
- Against bc7e u6 perceptual it loses luma, as linear mode always does (`rgb_real` luma PSNR −3.5 dB).
- **Speed.** Round 1: 0.9 to 1.5× the time of bc7e u6 per set, 8.7 s against 8.2 s over all 39 images (process start included; 2.5 s against 2.2 s for the 7 `rgb_real` images). Round 3 measured 0.64 to 0.83× the time of u6, but the round-3 r1 and r2 passes that wrote the u6 rows ran at the same time (their logs overlap on 9 October) while `max` ran alone, so those ratios favour `max` and are not a speed claim.

The full per-set tables for all three rounds (including bc7e u6 perceptual, and the seconds) are in the bench folder, `results/summary_maxpeak.md` (`scripts/analyze_maxpeak.py`) and `results/summary_engine_bytes.md` (`scripts/analyze_engine_bytes.py`).

## BC1 and BC3

### Published measurements

From Aras's 2020 DXTC data (same images and metric as his BC7 test):

| Encoder / level | Average luma PSNR (dB) | Mpix/s |
|---|---|---|
| rgbcx 18 | 38.47 | 4.7 |
| rgbcx 10 | 38.41 | 36 |
| icbc 8 | 38.30 | 156 |
| stb_dxt | 38.01 | 244 |
| ISPC | 37.81 | 654 |

- BC1's 3-colour mode decodes the fourth palette entry as transparent black. bc7enc_rdo turns this on by default.
- BC1 decoding is not bit-identical across GPU vendors. rgbcx can encode for an NVIDIA, AMD or "ideal" decoder.

### BC1 and BC3 benchmark

Round 3, n = 100:

| Comparison | Mean Δ (dB) [95% CI] | Median | Wins | Luma Δ (dB) | ΔSSIM ×10³ (W/L) | ΔVIF ×10² (W/L) |
|---|---|---|---|---|---|---|
| BC1: NVTT `-highest` against rgbcx L18 `-r` (encoded for the rounding decoder) | −0.11 [−0.19, −0.03] | −0.02 | 4/100 | −0.10 | **−0.035** (58/42) | **+0.09** (68/32) |
| BC1: NVTT `-highest` against rgbcx L18 (default rounding) | +0.19 [−0.01, +0.40] | +0.06 | 97/100 | +0.31 | **+0.49** (93/7) | **+0.72** (91/9) |
| BC1: rgbcx L18 `-r` against default rgbcx L18 | +0.30 [+0.09, +0.52] | | 100/100 | +0.41 | +0.53 (95/5) | +0.63 (93/7) |
| BC1: Compressonator against rgbcx L18 | −0.49 [−0.56, −0.42] | −0.47 | 4/100 | −0.49 | −2.43 (8/92) | −1.42 (9/91) |
| BC1: texconv against rgbcx L18 | −2.13 [−2.49, −1.77] | −1.60 | 0/100 | −0.17 | **+0.68** [−0.71, +2.06] (55/45) | **+2.88** [+0.41, +5.34], cluster [+1.00, +5.63] (55/45) |
| BC3 premultiplied: NVTT `-highest` against rgbcx L18 `-r` | +0.09 [−0.04, +0.21] | −0.14 | 38/100 | | −0.07 (36/64) | +0.08 (38/62) |
| BC3 premultiplied: Compressonator against rgbcx L18 `-r` | −0.45 (a tie: the cluster interval spans 0) | | | | **−0.55**, cluster [−1.04, −0.21] (20/80) | −1.04, cluster [−2.12, +0.02] (24/76) |
| BC3 premultiplied: ISPC against rgbcx L18 `-r` | −1.18 [−1.30, −1.07] | −1.02 | 0/100 | | −0.83 (0/100) | −1.84 (0/100) |

- rgbcx at level 18 is the best BC1 and BC3 encoder when it is told which decoder rounding to expect. The decoder model matters more than the choice between the top encoders.
- SSIM and VIF rank texconv's default BC1 first, although it is 2.13 dB behind on RGB: it is perceptually weighted (its `-bc u` uniform variant ranks last), and on luma PSNR it already ties.
- On cutouts, BC1's 1-bit alpha keeps coverage exact but the visible RGB was **6.5 dB (NVTT) to 11.9 dB (Compressonator) worse than BC7** in round 1, and the anti-aliased edges are lost.
- BC3 alpha is a BC4 block; its rows are in [BC3, BC4 and BC5](#bc3-bc4-and-bc5-tessaliths-encoder-against-the-others) and the [BC4-family study](bc4-family.md).

### BC1 decoder model (2026-10-29)

Skyrim SE does no software decode: its DDS loader hands blocks to Direct3D 11 and the texture unit decodes on sample. A throwaway wgpu test (not kept) read 4096 random BC1 blocks back through a compute shader on the owner's NVIDIA GeForce RTX 4090 and compared them with rgbcx's four models. The card matches the **NVIDIA** model on every texel (0 of 65536), on DX12 and on Vulkan. Ideal differs on 14930 texels (up to 6/255 in green), AMD on 16677. AMD and Intel cards were not measured.

The model rgbcx encodes for changes the bytes, because it picks endpoints and selectors against that decoder's interpolation (a throwaway bench example, not kept: a 512x256 four-band gradient at level 18, one process per model):

| encoded for | blocks differing from Ideal (of 8192) | PSNR on NVIDIA | on AMD | on Ideal |
|---|---|---|---|---|
| Ideal | 0 | 48.47 | 48.97 | 51.02 |
| IdealRound4 | 6048 | 48.31 | 49.96 | 49.71 |
| NVIDIA | 7280 | **50.29** | 45.93 | 44.09 |
| AMD | 7776 | 44.45 | **52.45** | 48.78 |

To the eye all four decode to the same gradient; the difference is per-block texture of a few code values. Encoding for the card that decodes gains about 1.8 dB on NVIDIA and 3.5 dB on AMD over Ideal; encoding for the other vendor loses 2.5 to 4 dB against Ideal. This led to rgbcx's Dual mode (below).

### BC1 Dual: NVIDIA and AMD scored together (2026-11-02, Note FM17)

rgbcx `cBC1Dual` (mode 4, encode only) minimises the summed NVIDIA and AMD squared RGB error per texel, so one encoding serves both vendors. Measured on the bench's `research/bc1-dual` branch (commits 9363fa72 rgbcx patch, 85c11ef7 bench), level 18 with 3-colour blocks allowed, `rgb_real` (93 images), PSNR in dB by decoder model:

| encoded for | NVIDIA | AMD | Ideal |
|---|---|---|---|
| Dual | 39.51 | 39.74 | 39.37 |
| Ideal | 39.31 | 39.40 | 39.53 |

Dual against Ideal: +0.20 dB on NVIDIA, +0.35 dB on AMD, -0.16 dB on an exact (Ideal) decoder; better on both vendors for 93 of 93 images. On the 7 synthetic gradients (`rgb_grad`) the gain is +1.10 dB NVIDIA and +2.28 dB AMD. Encode time is 1.6 times Ideal's (15.2 s against 9.3 s for 92.8 Mpix on 16 threads). The existing rgbcx modes stay byte-identical. Report: `F:\ai\dds\reports\bc1-dual\REPORT-dual-mode.md`.

#### BC1 Dual rescore (2026-11-04)

Tessalith 2.0's BC1 encode (`encoders_sys::bc1_encode_block` as the engine calls it: 3-colour blocks on, no transparent black, not perceptual) against the round-3 BC1 files of the other encoders, all decoded with rgbcx's NVIDIA and AMD models, `rgb_real` (93 images). Mean RGB PSNR, then Dual's paired Δ against each [95% CI] and wins/losses:

| Encoder | NVIDIA (dB) | AMD (dB) | Dual Δ on NVIDIA | Dual Δ on AMD |
|---|--:|--:|---|---|
| rgbcx Dual (Tessalith 2.0) | 39.51 | 39.75 | | |
| NVTT 3 `-highest` | 39.38 | 39.62 | +0.13 [±0.08] 92/1 | +0.13 [±0.09] 92/1 |
| rgbcx L18, upstream | 39.32 | 39.41 | +0.19 [±0.08] 91/2 | +0.34 [±0.24] 92/1 |
| Compressonator q1.0 | 38.83 | 38.93 | +0.68 [±0.08] 93/0 | +0.82 [±0.23] 93/0 |
| Intel ISPC | 37.99 | 38.00 | +1.52 [±0.40] 93/0 | +1.75 [±0.60] 93/0 |
| texconv | 37.30 | 37.30 | +2.21 [±0.39] 93/0 | +2.45 [±0.58] 93/0 |

Dual is the best BC1 encoder measured on both vendors' decoders. NVTT 3 `-highest` is second, 0.13 dB behind. A throwaway engine example (not kept) produced the scores.

### Luma-weighted BC1 error (2026-10-31)

rgbcx's BC1 error is equal-weight RGB SSE. It is summed at 14 places in 5 functions of `rgbcx.cpp` (`bc1_find_sels4_fasterr`, `_check2_err`, `_fullerr`, the forced-selector path and `bc1_find_sels3_fullerr`), and under cBC1Dual in `bc1_dual_err`. Weighting only those sums leaves the PCA axis, the least-squares fits and the cluster orderings unweighted. Tessalith ships it as `encoder.perceptual` on BC1 and BC3 colour (Rec.709, FM42; `vanilla-compact` turns it on).

Two runs, both level 18 with 3-colour blocks allowed and no transparent black, both scored with the bench's decoder (`bcdecode.exe`) and `run_bench.metrics`. Besides the bench's luma metrics (`y_psnr`, and `ssim_y` = SSIM on Rec.709 luma), they add mean CIELAB ΔE76 (sRGB, D65) as an independent colour-perception metric.

- 2026-10-31, Ideal model: a scratch copy of the vendored rgbcx built as a separate tool (`F:\ai\dds\bench\scripts\bc1w\`). Its unweighted absolute values are 0.01 dB from the bench's own `bc1_rgbcx_L18` row (39.413 vs 39.424 dB), since bc7enc's CLI flags differ slightly.
- 2026-11-03, cBC1Dual: the shipped encoder (`encoders_sys::bc1_encode_block` with the flag off and on; `release/2.0` at 9c9fbd52 plus FM42, release build, 16 threads). Unweighted it scores 39.629 dB RGB and 41.264 dB luma.

Round 3 `rgb_real` (93 images), Δ against unweighted under the same model. For ΔE, + = lower (better):

| Weights, model | `rgb_psnr` | `y_psnr` | `ssim_y` | ΔE76 | time |
|---|---|---|---|---|---|
| Rec.709 (0.2126, 0.7152, 0.0722; ints 54/183/19), Dual (shipped) | −0.297 [±0.042] 0/93 | **+0.681 [±0.123] 93/0** | **+0.00351 [±0.00074] 93/0** | −0.063 [±0.014] 8/85 | same |
| Rec.709, Ideal | −0.284 [±0.041] 1/92 | +0.656 [±0.121] 93/0 | +0.00354 [±0.00075] 93/0 | −0.062 [±0.013] 8/85 | same |
| Rec.601 (0.299, 0.587, 0.114; 77/150/29), Ideal | −0.163 [±0.023] 1/92 | +0.488 [±0.095] 93/0 | +0.00275 [±0.00063] 93/0 | −0.038 [±0.010] 9/84 | same |

Round 1 (Ideal) gives the same picture: `rgb_real` (7) Rec.709 y +0.95 dB, ssim_y +0.0045 and ΔE −0.088; `rgb_grad` (7) y +0.98 dB. Under Dual the weighted Δs match the Ideal run within their intervals.

The weighting moves error out of luma into chroma, and both luma metrics use the same Rec.709 weights it optimises. On those metrics weighted rgbcx BC1 beats every BC1 row in the bench, NVTT −highest included (Ideal run: y 41.65 vs 41.33 dB; ssim_y 0.9824 vs 0.9793). ΔE76, which also sees chroma, gets worse on 85 of 93 images, and RGB PSNR drops on every image. It costs no time. Like bc7e's perceptual mode, it is a preference, not a free win, which is why it is off by default.

## BC4 and BC5

The review found no cross-encoder BC4/BC5 benchmark and expected competent encoders to converge, because BC4 is a one-dimensional fit. The benchmark showed otherwise. Published advice puts tangent-space normals in BC5 with Z rebuilt in the shader, which does not apply to Skyrim ([Skyrim texture rules](design-overview.md#skyrim-texture-rules)).

### BC4 (single-channel masks)

Round 3, n = 99, against bc7enc_rdo's rgbcx BC4 (`-4`), bench decoder:

| Encoder | Mean Δ (dB) [95% CI] | Median | Wins | ΔSSIM ×10³ (W/L) | ΔVIF ×10² (W/L) |
|---|---|---|---|---|---|
| rgbcx with search radius 10 (`-hr10`) | +0.13 [+0.07, +0.20] | +0.05 | 98/99 | +0.037 (93/5) | +0.14 (98/0) |
| NVTT `-highest` | −1.29 [−1.76, −0.83] | −0.89 | 10/99 | −0.80 | −0.94 |
| Compressonator | −1.16 [−1.56, −0.76] | −0.81 | 13/99 | −0.66 | −0.83 |
| ISPC | −2.98 [−3.69, −2.26] | −2.10 | 0/99 | −1.88 | −2.75 |
| texconv | −2.93 [−3.69, −2.17] | −1.99 | 1/99 | −1.45 | −2.13 |

The encoder matters more for BC4 than its reputation suggests. In round 2, ISPC and texconv could not reproduce a binary metalness map exactly and lost 6 to 8 dB on smooth gradients.

### BC4/BC5 search radius

rgbcx searches endpoint pairs within a radius around its first guess. The radius was tested on 12 October 2026 on the round-2 mask and normal sets (7 images each), decoded with bcdec.

The first run found that a wider search could make results worse (BC5 at radius 10 scored 47.92 dB against 57.99 dB at radius 5). The cause: upstream rgbcx modelled BC4 decoding with floored interpolants, while bcdec and the GPU round to nearest. A wider search found more endpoint pairs that were exact only because of the floor. rgbcx was patched to round like the decoder. After the patch:

| Format | Radius | Mean PSNR (dB) | Δ against radius 5 | Time against radius 5 |
|---|---|---|---|---|
| BC4 | 3 | 57.0994 | −0.1577 | 0.49× |
| BC4 | 4 | 57.1980 | −0.0591 | 0.73× |
| BC4 | 5 | 57.2571 | 0 | 1.00× |
| BC4 | 10 | 57.3802 | +0.1231 | 2.80× |
| BC5 | 3 | 60.4957 | −0.1066 | 0.47× |
| BC5 | 4 | 60.5661 | −0.0362 | 0.71× |
| BC5 | 5 | 60.6023 | 0 | 1.00× |
| BC5 | 10 | 60.6622 | +0.0599 | 2.96× |

- The patch raised the radius-5 result by 3.07 dB (BC4) and 2.61 dB (BC5).
- PSNR now rises with the radius on every one of the 14 images.
- Radius 10 gains little for about 3× the time. Radius 4 loses under 0.06 dB on the means for about 0.7× the time.

### Normal maps: BC5 against BC7

Round 3, n = 100, angular error (+ means A has less error). The BC7-only rows are in [BC7 on normal maps](#bc7-on-normal-maps).

| Comparison | Mean Δ [95% CI] | Median | Wins | ΔSSIM ×10³ (W/L) | ΔVIF ×10² (W/L) |
|---|---|---|---|---|---|
| BC7 (NVTT Production) against BC5 (rgbcx), both with Z rebuilt | −0.56° [−0.64, −0.49] | −0.52° | 3/100 | −17.8 (1/96) | −16.5 (2/95) |
| BC5: Compressonator against rgbcx | −0.039° [−0.049, −0.029] | −0.041° | 12/100 | −0.63 (7/90) | −0.77 (9/88) |
| BC5: NVTT `-highest` against rgbcx | −0.063° [−0.070, −0.056] | −0.070° | 3/100 | −0.78 (2/95) | −1.02 (4/93) |
| BC5: ISPC against rgbcx | −0.141° [−0.163, −0.120] | −0.143° | 9/100 | −2.12 | −3.13 |
| BC5: texconv against rgbcx | −0.144° [−0.160, −0.127] | −0.151° | 5/100 | −1.48 | −2.33 |
| BC5: rgbcx `-hr10` against rgbcx | +0.007° | | | +0.064 (92/5) | +0.17 (97/0) |

- **When the shader rebuilds Z, BC5 is the better normal format** at the same 8 bits per texel. BC5 gives X and Y two independent channels; BC7 shares endpoints across X, Y and Z.
- Round 2 had compared BC7's stored Z with BC5's rebuilt Z. That method is biased, because many ambientCG normal maps are not unit length: rebuilding Z from the source's own XY already differs by 0.43° on average for the round-2 maps and 1.47° for the new ones. The fair test rebuilds Z for both. Scored the round-2 way, the round 3 pair is a tie (+0.10°), while SSIM and VIF, which see only R and G, give the BC5 lead above.
- **Skyrim's shaders do not rebuild Z.** They read the blue channel directly, so the BC5 result does not apply to a Skyrim `_n` ([Skyrim texture rules](design-overview.md#skyrim-texture-rules)).

### BC3, BC4 and BC5: Tessalith's encoder against the others

`sdc rgbcx (rounded)` (CSV ids `sdc_bc3_rounded`, `sdc_bc4_rounded`, `sdc_bc5_rounded`) is the engine's own encode through `sdc-bench encode-one --format bc3|bc4|bc5 --level default`, re-run on 2026-11-01 with branch `research/bc4-family` (a3785aef, engine 1.2.0) on every BC3, BC4 and BC5 set of all three rounds. That is the vendored rgbcx with the round-to-nearest BC4 model (DECISIONS T13) and the BC4 block search (Note FJ, [BC4-family block search](bc4-family.md)), level 18, search radius 5. BC4 uses a `height` profile and BC5 a `flow` profile; BC3, which has no built-in sub-type, is a `diffuse` profile set to BC3 as a user override would, with colour encoded for the BC1 model of the time (`ideal`).

The upstream rows above (`bc3_rgbcx_*`, `bc4_rgbcx*`, `bc5_rgbcx*`) also use level 18 and radius 5 (`-hr10` is radius 10), so the differences are the decoder model and the block search. Because the bench decoder floors BC4 interpolants ([why it matters](#bench-decoder-floors-bc4)), every BC3, BC4 and BC5 output was scored twice: with that decoder (**floor**) and with bcdec's round-to-nearest weights (**round**, `scripts/rescore_rounded.py`). BC3 colour is decoded as IdealRound4 in both.

Round 3, mean Δ of `sdc rgbcx (rounded)` against the other encoder [95% t-interval], wins/losses (+ = Tessalith better):

| Set (n), metric | Against | Floor decoder | Round decoder |
|---|---|---|---|
| `mask_real` (93), BC4 PSNR (dB) | rgbcx `-4` | −1.67 [±0.74], 9/84 | +1.81 [±0.45], 93/0 |
| | rgbcx `-hr10` | −1.80 [±0.73], 5/88 | +1.87 [±0.55], 93/0 |
| | NVTT `-highest` | −0.33 [±0.27], 15/78 | +0.08 [±0.05], 46/47 |
| | NVTT default | +0.05 [±0.29], 82/11 | +0.89 [±0.10], 93/0 |
| | Compressonator | −0.47 [±0.37], 21/72 | +0.45 [±0.13], 88/5 |
| | ISPC | +1.32 [±0.13], 93/0 | +1.97 [±0.14], 93/0 |
| | texconv | +1.27 [±0.12], 93/0 | +2.27 [±0.59], 93/0 |
| `normal_real` (93), BC5 mean angle, Z rebuilt (°) | rgbcx `-5` | −0.069 [±0.009], 5/86 | +0.079 [±0.009], 89/2 |
| | rgbcx `-hr10` | −0.077 [±0.009], 3/88 | +0.094 [±0.015], 91/2 |
| | NVTT `-highest` | −0.004 [±0.006], 48/43 | +0.004 [±0.002], 77/14 |
| | Compressonator | −0.029 [±0.009], 13/78 | +0.010 [±0.006], 67/24 |
| | ISPC | +0.072 [±0.021], 83/9 | +0.102 [±0.024], 82/9 |
| | texconv | +0.074 [±0.012], 85/7 | +0.092 [±0.012], 86/6 |
| `alpha_real` (96), BC3 premultiplied PSNR (dB) | rgbcx L18 | +0.35 [±0.15], 55/41 | +0.55 [±0.15], 95/1 |
| | rgbcx L18 `-r` | +0.29 [±0.15], 43/53 | +0.46 [±0.15], 80/16 |
| | NVTT `-highest` | +0.20 [±0.04], 76/20 | +0.21 [±0.05], 76/20 |
| | Compressonator | +0.74 [±0.09], 95/1 | +0.77 [±0.08], 95/1 |
| | ISPC | +1.47 [±0.22], 96/0 | +1.47 [±0.23], 96/0 |
| `cutout_real` (95), BC3 alpha PSNR (dB) | rgbcx L18 | +3.01 [±0.47], 94/1 | +3.33 [±0.47], 95/0 |
| | NVTT `-highest` | +1.04 [±0.09], 95/0 | +1.18 [±0.10], 95/0 |
| | Compressonator | +0.83 [±0.13], 91/4 | +0.99 [±0.13], 92/3 |
| `packed_real` (95), BC3 alpha PSNR (dB) | rgbcx L18 | −2.31 [±0.76], 1/94 | +2.15 [±0.46], 95/0 |
| | NVTT `-highest` | −0.14 [±0.33], 76/19 | +0.97 [±0.14], 94/1 |
| | Compressonator | −0.81 [±0.41], 12/83 | +0.53 [±0.15], 89/6 |

- **Under the rounding decoder Tessalith's rgbcx beats upstream rgbcx** on BC4 (+1.8 dB), BC5 (+0.08°) and BC3 alpha (+0.5 to +3.3 dB), on almost every image. Under the floor decoder upstream stays ahead on BC4, BC5 and `packed_real`, where the gap is mostly the decoder model; on cutout and smooth alpha the block search outweighs it.
- **Under the rounding decoder Tessalith is ahead of NVTT `-highest` on BC3 alpha (cutout, smooth, packed) and on BC5 angle, and tied on BC4 and BC5 RG PSNR:** cutout alpha +1.18 dB (95/0), smooth alpha +0.21 dB premultiplied, packed alpha +0.97 dB, BC5 angle +0.004° (77/14). BC4 is +0.08 dB [±0.05] but wins only 46 of 93 images, and BC5 RG PSNR is +0.002 dB with 17/74, so those two are ties by count. Every other encoder is behind under the rounding decoder.
- **The wider search radius gains nothing once decoding rounds.** Upstream rgbcx `-hr10` against `-4` is +0.13 dB [±0.07] under the floor decoder and −0.06 dB [±0.16] under the rounding one; BC5 `-hr10` is −0.014° [±0.016].
- **BC3 cutouts:** the 2026-10-31 rows, before the block search, were about 2 dB behind NVTT and Compressonator on cutout alpha; the gap and its fix are in the [BC4-family study](bc4-family.md). Visible RGB is 0.075 dB behind NVTT `-highest` (3/92), from the colour half.
- **BC3 colour is exactly upstream rgbcx L18's** (Δ 0 on every image). Encoded for `ideal`, it is 0.10 to 0.32 dB behind `-r` under the bench's IdealRound4 colour decoder; that gap is the BC1 model setting, not the encoder.
- Rounds 1 and 2 (2 to 7 images per set) point the same way under the rounding decoder: against NVTT `-highest`, cutout alpha +1.06 dB (n = 4), BC4 `mask_real` −0.01 dB (a tie, n = 6), BC5 `normal_real` +0.002° (n = 7).
- Speed: the old-against-new end-to-end times, measured alternately per image, are in the [BC4-family study](bc4-family.md#shipped-end-to-end-and-bench-re-run-2026-11-01). A ratio to bc7enc's CLI from the bench's separate runs is not given: they ran under different machine load.

Per-set tables for all three rounds, with the mean of each variant under both decoders and the seconds, are in `results/summary_engine_bytes.md`; the change against the 2026-10-31 rows is in `results/summary_bc4_rerun.md` (`tools/compare_prev_sdc.py`).

## BC6H (HDR)

Published:
- Narkowicz's table (5 HDR images, RMSLE): ISPC TC slow/veryslow best on 4 of 5 images; DirectXTex worse than ISPC fast on every image, by up to about 1.8×.
- Ludicon's 2026 chart (27 HDR images, SSIMULACRA2, values read from an image, made by a competitor): Oodle about 91.0, Spark GPU about 90.5, ISPC TC about 88.6, Basis about 87.6, the "NVIDIA" series about 85.2.
- Unsigned BC6H (UF16) should be used for data that is never negative.

Round 2 (4 real HDR images):

| Encoder | mPSNR (dB) | RMSLE ×1000 | Worst peak ratio | s per Mpix |
|---|---|---|---|---|
| ISPC `bc6h_veryslow` | 51.50 | 3.53 | 0.22 (a sunset sun) | 0.19 |
| texconv CPU | 50.65 | 4.25 | 0.29 | 10.7 |
| Compressonator (any quality) | 50.32 | 4.32 | 0.29 | 1.2 to 16.7 |
| NVTT `-bc6 -production` (= `-highest`) | 50.01 | 3.30 | 0.95 | 2.9 |
| texconv GPU | 49.31 | 4.41 | 0.92 | 0.41 |

Round 3, n = 18, against ISPC `bc6h_veryslow`:

| Encoder | Metric | Mean Δ [95% CI] | Wins |
|---|---|---|---|
| texconv CPU | mPSNR | −0.76 dB [−1.05, −0.47] | 0/18 |
| NVTT Production | mPSNR | −2.38 dB [−3.36, −1.40] | 1/18 |
| NVTT Production | RMSLE | −0.00005 [−0.00048, +0.00037] | 11/18 |
| NVTT Production | peak ratio | +0.07 [−0.03, +0.17] | 9/18 |

- **ISPC `bc6h_veryslow` is the best general BC6H encoder**, and the fastest.
- ISPC (and texconv CPU) can clip a small, extremely bright highlight such as a visible sun. Round 3 showed this is specific to that kind of content, not general. Round 2's "NVTT has the best RMSLE" did not hold at n = 18.
- NVTT keeps highlights but is clearly weaker in darks and wide-range content.

### Peak-preserving ISPC mode (2026-10-31, Note FE)

**Why ISPC loses the sun.** ISPC TC measures error on the half-float bit patterns, a log-like scale (`kernel.ispc` `bc6h_setup`). The sun's texels come from an RGBE source, where blue is stored as exactly 0.0, and on that scale 0.0 is far from every other value. So blue dominates the PCA axis, the index choice and the least-squares endpoints. venice_sunset's 6464 peak decodes as 1464, at every effort level.

**The mode.** `Bc6hLevel::VeryslowPeak` (ISPC TC patch 0001) changes only highlight blocks: blocks whose brightest texel is at least 32× the block's geometric-mean luminance. In those blocks it keeps that texel at 0.9 or more of its source luminance whenever some candidate can. Every other block encodes exactly as veryslow.

Measured with `sdc-bench bc6-peak` against the `.f16` source as float (not the `.ref.npy` the Python bench used), n = 18 real images:

| Image | Peak ratio (veryslow → peak) | mPSNR Δ (dB) | RMSLE ×1000 (veryslow → peak) |
|---|---|---|---|
| venice_sunset | 0.219 → 0.923 | −0.674 | 3.96 → 3.34 |
| moonless_golf | 0.732 → 0.995 | −0.349 | 8.50 → 9.03 |
| dikhololo_night | 0.817 → 1.090 | −0.002 | 1.99 → 2.02 |
| dreifaltigkeitsberg | 0.849 → 1.008 | 0.000 | 5.57 → 5.59 |
| satara_night | 1.026 → 1.026 | −0.048 | 14.43 → 14.99 |
| the other 13 | unchanged | −0.007 to 0.000 | within ±0.01 |

- Mean Δ over the 18 real images:
  - peak ratio: +0.078 [−0.012, +0.168];
  - mPSNR: −0.061 dB [−0.147, +0.026];
  - RMSLE: +0.000028 [−0.00009, +0.00015], which is +0.6 % relative.
- No image's peak goes down.
- The four `hdr_grad` images encode identically.
- Encode time is unchanged within noise: about 0.1 s per 0.5 Mpix image, multi-threaded.
- The RMSLE bar ("not worse") is missed by that small, non-significant margin. The cost sits in moonless_golf and satara_night, where keeping the peak forces a two-region block to give up its dim texels.
- **Through the Python bench (2026-10-31),** as `bc6_ispc_veryslow_peak` beside the other BC6H rows (same texconv decode, scored against `.ref.npy`), n = 18 real images against `bc6h_veryslow`: peak ratio +0.078 [±0.090] (4 up, 0 down), mPSNR −0.061 dB [±0.086], RMSLE 0.00003 worse [±0.00012] (3 images better, 6 worse). That reproduces the small, non-significant RMSLE miss above. Against NVTT Production it keeps +2.32 dB [±0.96] mPSNR (17/18) and ties on RMSLE and peak ratio. The 4 `hdr_grad` images score identically to veryslow.
- Without the 32× contrast gate (first attempt), the guard re-encodes many ordinary blocks: mean mPSNR −0.168 dB and RMSLE +0.00023. Other effort levels do not use this mode.

## Other findings

### Rate-distortion optimisation

RDO always costs quality. In one published bc7enc_rdo run, PSNR fell from 42.25 dB with no RDO to 41.77 at λ = 0.1, 39.51 at λ = 1 and 29.18 at λ = 20. RDO only shrinks the file after LZ compression (inside an archive, for example). It never reduces the DDS size or GPU memory.

### DDS container rules

- BC6H, BC7, any `_SRGB` format and arrays need the DX10 header. BC1 to BC3 are most compatible with legacy FourCCs.
- BC4 and BC5 have ambiguous legacy names (`ATI1`/`BC4U`, `ATI2`/`BC5U`).
- bc7enc_rdo never writes `_SRGB` formats and makes no mipmaps, so any tool that uses its encoders has to write the DDS file itself.

### Settings that change nothing

- NVTT 3 BC7: `-fast` and the default give byte-identical output, and so do `-production` and `-highest` (39 of 39 files). The same holds for BC6H `-production` and `-highest`.
- NVTT's GPU and CPU paths at Normal give different bytes but the same quality (±0.01 dB).
- NVTT's `-normal` flag does not change the compressed data. It only sets the non-standard `DDPF_NORMAL` header flag.
- Compressonator BC6H `-Quality 0.05` and `1.0` give the same metrics, at 14× the time.

### Not re-measured in round 3

Round 3 was paused to free the CPU. These claims rest on the small round-1 and round-2 sets only:
- Compressonator `-Quality 1.0` on BC7 (opaque, cutout, smooth alpha, data, normals, stray alpha) and on BC6H.
- Smooth gradients, speed and the mode-usage counts.

### NVTT per-level feeding: spikes S7 and S7b (2026-10-09 and 13)

Tessalith 1.x could hand BC7 and BC6H to the user's own NVTT 3.2.5 `nvcompress` (removed in 2.0, Note FM14). These spikes checked that the engine can build the mip chain itself and feed NVTT one level at a time.
- One `nvcompress -nomips` process per level works to 1×1 (33 levels on the first 3 `rgb_real` and first 3 `hdr_real` images). Mip 0 matched the bench's direct `nvtt_production` and `bc6_nvtt_production` rows exactly (Δ 0.0000 dB), and BC6H payloads were `BC6H_UF16` of exactly `ceil(w/4)*ceil(h/4)*16` bytes.
- BC7 and BC6H Production run on the CPU even with CUDA available, and `-nocuda` gives byte-identical output (33 of 33 BC6H levels). The fixed cost of about 0.14 s per process is CUDA context creation; with `-nocuda` a tiny level takes 0.015 s. BC7 Production is slow: mip 0 of a 1 Mpix image took 14 to 27 s, about 13 cores busy.
- Uncompressed DDS input works only with a legacy header. nvcompress reads DX10 B8G8R8A8 and RGBA16F as garbage (RGB PSNR 5.05 dB); a legacy BGRA header, or FourCC 113 for RGBA16F, gives a payload byte-identical to the PNG feed.
- The no-argument run exits 1, and a truncated input exits 1 with no output.

### Encode time of the Rust port of rgbcx (2026-11-13, Note HY)

Tessalith encodes BC1, BC3, BC4 and BC5 with `bcenc`, a Rust port of its patched rgbcx. The port runs the same search and writes the same bytes. Three changes make it faster, and each gives the C++ result for every block:

- **A repeated pair of BC1 endpoints is scored once.** rgbcx's level-18 search reaches the same two endpoint colours many times in one block, by different routes. The port keeps the pairs it has scored for the block and skips a repeat.
- **A pair is scored against all 16 texels at once.** Finding each texel's nearest palette entry and summing the error is SSE2 integer code over the whole block, in BC1 and in BC4.
- **BC4 searches each mode once.** rgbcx searches the 8-value and the 6-value candidates, then searches one mode's candidates again when it refits. The port keeps the first search's result.

Time per 4×4 block in µs: one thread, median of 5 runs over 1,048,576 blocks (16 textures of 1024² from installed Skyrim mods), `bcenc` at c81ae94e against the vendored C++ on the [benchmark machine](#benchmark-machine-and-tools), with the arguments the engine passes (`crates/bcenc/examples/speed.rs`). The one-bit-alpha row has no real mask to use, so it marks a texel transparent when its green is below 64:

| Format | rgbcx (C++) | `bcenc` | `bcenc` / C++ |
|---|--:|--:|--:|
| BC1 | 27.20 | 5.38 | 0.20 |
| BC1, perceptual | 26.87 | 5.34 | 0.20 |
| BC1, one-bit alpha | 14.97 | 3.02 | 0.20 |
| BC3 | 25.34 | 4.85 | 0.19 |
| BC4 | 30.07 | 1.84 | 0.06 |
| BC5 | 58.24 | 3.67 | 0.06 |

The port before the three changes (945ad68c) took 0.69 of the C++ time on BC1 and BC3 and 0.82 on BC4 and BC5.

`sdc convert` of 300 PNGs with the format forced, Tessalith 3.0.0 against c81ae94e, in seconds (reading, mips and writing included; a test job ran at idle priority beside both builds):

| Forced format | 3.0.0 | c81ae94e |
|---|--:|--:|
| BC1 | 63.0 | 15.7 |
| BC3 | 61.8 | 15.1 |
| BC4 | 21.3 | 5.5 |
| BC5 | 26.4 | 6.0 |

Every DDS has the same SHA-256 from both builds, in these four configurations and seven more (perceptual, one-bit alpha, cutout, a BC3 alpha cut, the `vanilla-compact` preset).

Compared with the C++ block by block, c81ae94e has 0 mismatches in each:
- **Real textures:** every block of 10,000 textures as BC1, BC3, BC4 and BC5, and the 1,437 with transparent texels as BC1 with one-bit alpha: 2.72 billion comparisons. The port before the three changes (017fbc68) passed the same run.
- **Generated blocks:** 100 million BC1 blocks, every solid colour, 50 million one-bit-alpha blocks, 50 million BC3 blocks and 20 million BC4 blocks.

A sampling profile of c81ae94e on the same blocks shows where the rest of the time goes. BC1: the least-squares endpoint fits and their rounding to 5:6:5 38%, scoring texels and building palettes 33%, the table of scored pairs 14%. BC4: scoring texels 62%, building palettes 16%.

### Encoder maintenance (October 2026)

| Encoder | Status |
|---|---|
| ISPC TC | Archived (last push 2024-09-23). |
| bc7e | Repository archived (last push 2026-01-28). Its latest code lives in bc7enc_rdo. |
| bc7enc_rdo | Not archived, but no code change since March 2021. |
| Compressonator | Last release 4.5.52 (31 January 2024). |
| NVTT 3 | Last release 3.2.5.1 / Exporter 2024.1.1 (15 August 2024). |
| DirectXTex | Active, but its 2024 to 2026 releases contain no BC encoder changes. |

Open-source BC7 quality has not moved since 2021. No fixes are expected from the leading open encoders, so a project that uses them should keep its own pinned copy.

## What Tessalith does because of this

- **Correct format first.** The design ranks correct format and tag per map type first, then correct mips, then the best encoder at a practical speed ([Design overview](design-overview.md)).
- **bc7e for BC7, rgbcx for BC1/BC3/BC4/BC5, ISPC TC for BC6H**, vendored and pinned with SHA-256 hashes in [`crates/encoders-sys/vendor/`](../../crates/encoders-sys/vendor/README-VENDOR.md). The format alone picks the encoder (Note FM15). texconv and Compressonator are not used to encode; texconv is used only as a test oracle for headers.
- **bc7e at `veryslow` (level 5) is the default BC7 level.** Level 5 matches level 6 at 1.5× the speed. Level 4 is not offered, because level 3 beats it on gradients. The levels are in [`crates/engine/src/encode.rs`](../../crates/engine/src/encode.rs) and the default in [`presets/sdc-base.toml`](../../presets/sdc-base.toml). The `fast-preview` preset uses `fast`.
- <a id="vendored-encoders-reproduce-the-bench"></a>**The vendored encoders reproduce the bench.** Spike S3 (2026-10-08) built the workspace without ISPC on the path and matched the bench's bc7e level 5 result on `rgb_real` (mean 50.9771 dB against 50.977, largest per-image Δ 0.000000 dB over all 10 BC7 sets, 39 of 39 block payloads byte-identical) and rgbcx BC4 on `mask_real` (7 of 7 bit-exact). The Phase 0 gates (2026-10-09) added rgbcx BC1, BC3, BC4 and BC5 bit-exact (14, 25, 12 and 11 images; transparent black off on 14 of 14), ISPC BC6H `veryslow` at 51.4995 dB with 8 of 8 payloads byte-identical, and ISPC objects that regenerate byte for byte.
- **rgbcx runs at level 18, its highest.** BC1 and the colour half of BC3 encode with rgbcx Dual, which scores NVIDIA and AMD decoder error together; there is no decoder setting (Note FM17). BC3 is never picked to save space, because it is the same size as BC7.
- **BC4/BC5 search radius 5 is the default; radius 4 is the `fast` setting** used by the compact and preview presets; radius 10 is `max` ([`crates/encoders-sys/src/rgbcx.rs`](../../crates/encoders-sys/src/rgbcx.rs)). rgbcx is patched to round its BC4 palette like the decoder, and to search each BC4 block further for BC4, both BC5 channels and BC3 alpha ([BC4-family block search](bc4-family.md)). The patches and their tests are in [`README-VENDOR.md`](../../crates/encoders-sys/vendor/README-VENDOR.md).
- **Normals are BC7, not BC5**, because Skyrim does not rebuild Z. BC4 or BC5 on a normal map is the blocking warning `FMT-BC5-NORMAL`. BC5 is the default only for `_flow` and the user-picked `generic_two_channel`.
- **Perceptual mode is off by default** and refused for anything but colour without data in alpha (`FMT-PERCEPTUAL-DATA`, [`crates/engine/src/policy.rs`](../../crates/engine/src/policy.rs)). No RDO.
- **BC1 alpha is opt-in** (`encoder.bc1_alpha`, Note FM20), because BC1's 1-bit alpha loses anti-aliased edges and RGB quality against BC7.
- **The `remove` alpha policy drops the alpha channel before mips and encoding.** Source alpha whose lowest value is 251 or more is treated as opaque ([DECISIONS](../../.claude/DECISIONS.md) Note L), and so is alpha whose texels below 251 are fewer than 0.1 % of the image with no connected group over 16 texels (Note FM34; the survey behind it is in [Mipmaps, normal maps and alpha coverage](mipmaps-and-alpha.md#result-6-a-few-low-alpha-texels-are-noise-a-cutout-is-not)).
- **Exact alpha 255 where alpha is read.** The encode stage always applies a lossless fix to BC7 mode-4/5 blocks whose input alpha is 255. With `alpha.exact_opaque` on, a patched bc7e restricts mode-6 p-bits to 1 so alpha decodes to exactly 255. Its RGB cost is too high to turn on everywhere ([spikes S5 and S6](#forcing-alpha-255-spikes-s5-s5b-and-s6-2026-10-08-and-09)), so it is on by default only for `_m` and environment masks (owner, Phase 0), and turning it off on a BC7 `_m` is the blocking code `ALPHA-EXACT-OFF-M`; BC4 and BC3 `_m` never raise it ([DECISIONS](../../.claude/DECISIONS.md) Notes A and D; [`crates/encoders-sys/src/bc7_fixup.rs`](../../crates/encoders-sys/src/bc7_fixup.rs)).
- **BC6H is used only under the `cs` target** (vanilla has no BC6H path), for `hdr_sky` and for a `pbr_emissive` whose source is a float image that peaks above 1.0. Validation warns when the decoded peak ratio falls below 0.8 (`VAL-BC6H-PEAK`).
- **Tessalith writes its own DDS headers** ([`crates/engine/src/dds.rs`](../../crates/engine/src/dds.rs)): DX10 for BC6H, BC7 and every `_SRGB` format, legacy FourCC for BC1/BC3 UNORM under the `auto` header setting, and `ATI1`/`ATI2` never.
- **NVTT was an opt-in quality option for BC7 and BC6H in 1.x, at Production only, run as the user's own `nvcompress.exe`; 2.0 removed it** (Note FM14). The per-level feeding it used is in [spikes S7 and S7b](#nvtt-per-level-feeding-spikes-s7-and-s7b-2026-10-09-and-13).
- **The bench and its gates** (parity, exact-opaque, `radius45` for the search radius) live on branch `bench` (Note FM8).

## Summary tables behind the README charts

The data behind the README charts, drawn by `tools/readme-charts.py`. "—" means not measured; "n/a" means the encoder has no such format.

### BC7: GPU and CPU encoders, speed and quality

Δ is against bc7e u6, in dB. "Real" means real textures.

| Encoder | Speed (s/Mpix) | Δ RGB (real) | Δ (real alpha) |
|---|--:|--:|--:|
| NVTT Normal (GPU) / texconv GPU | 0.03–0.09 | −1.4 / −1.8 | −0.75 / −2.8 |
| ISPC basic | 0.11 | −0.75 | −0.69 |
| bc7e u4 | 0.12 | −0.39 | −0.36 |
| bc7e u5 | 0.22 | +0.01 | −0.03 |
| bc7e u6 | 0.28 | 0 | 0 |
| bc7e max‡ | ~0.6 | +0.15 | +0.04 |
| ISPC slow | 0.30 | −0.59 | −0.67 |
| NVTT Production/Highest | ~14 | −0.32 | −0.37 |
| texconv CPU (-bc x) | ~16–20 | −1.6 | −2.4 |
| Compressonator q1.0 | ~33 | −0.16 | −1.50 |

NVIDIA Texture Tools 3 is not part of Tessalith; its rows are a reference.

‡ `max` is opt-in (`encoder.effort = "max"`). Its Δ comes from the same bench round as the other rows (7 colour textures, 4 with alpha). On the 92 colour textures of bench round 3 it gains +0.19 dB RGB and beats u6 on all 92; on the 96 with smooth alpha it gains +0.12 dB premultiplied. It takes 2.7 times the time of u5 (`veryslow`) in the [bc7e tuning](bc7e-tuning.md) runs, about 0.6 s/Mpix on this table's scale.

### BC7: bc7e u5 against the other CPU encoders at maximum settings

| Measure | bc7e -u5 | NVTT 3 Production | Compressonator q1.0 | Intel ISPC slow | texconv -bc x |
|---|--:|--:|--:|--:|--:|
| Real colour textures (Δ dB) | +0.03 | −0.27 | −0.16† | −0.44 | −1.47 |
| Smooth gradients, synthetic (Δ dB) | 0.00 | +3.86 | +3.21 | +0.73 | −6.89 |
| Real decals, smooth alpha (Δ dB) | −0.02 | −0.06 | −1.50† | −0.58 | −2.04 |
| Packed data maps (RGB PSNR, dB) | 41.22 | 41.58 | —† | 41.20 | 40.38 |
| Normal maps (mean angle error) | 1.77° | 1.71° | —† | 1.79° | 2.19° |
| Opaque input decoded with alpha 254 | 28%* | 0% | 1.1% | 15% | 2.4% |
| Time per megapixel | 0.22 s | ~14 s | ~33 s | 0.30 s | ~20 s |

\* An optional patch (mode 6 p-bit override, exact opaque) is for when alpha 254 is not acceptable.

† Compressonator q1.0 was measured on the original 4–7 images only. On those images it was 0.15 dB behind NVTT on packed data and 0.27° behind on normals. Its alpha-254 and timing rows are from the original run.

### Other formats: best encoders per format

| Format and measure | rgbcx (Tessalith) | Intel ISPC | NVTT 3 -highest | Compressonator | texconv (CPU) |
|---|--:|--:|--:|--:|--:|
| BC1 colour, NVIDIA decoder (RGB PSNR, dB) | 39.51 | 37.99 | 39.38 | 38.83 | 37.30 |
| BC1 colour, AMD decoder (RGB PSNR, dB) | 39.75 | 38.00 | 39.62 | 38.93 | 37.30 |
| BC3 colour + smooth alpha (premultiplied PSNR, dB) | 49.50 | 48.03 | 49.29 | 48.74 | 48.08 |
| BC3 cutout alpha (alpha PSNR, dB) | 50.30 | 45.73 | 49.13 | 49.31 | 47.43 |
| BC4 masks (PSNR, dB) | 49.10 | 47.13 | 49.02 | 48.65 | 46.83 |
| BC5 normals (RG PSNR, dB) | 47.16 | 45.32 | 47.16 | 46.87 | 45.51 |
| BC5 normals (mean angle error) | 1.89° | 1.99° | 1.90° | 1.90° | 1.98° |
| BC6H HDR (multi-exposure PSNR, dB) | n/a | 50.38 | 48.00 | —† | 49.62 |
| BC6H brightest highlight kept (worst image) | n/a | 22% | 94% | 29%† | 29% |

Each row has its own image set, so compare encoders within a row, not across rows; the README chart plots each row against Tessalith's encoder. BC1 to BC5 are bench round 3 (93 colour textures, 96 with smooth alpha, 95 cutouts, 93 masks, 93 normal maps), scored the way GPUs decode: BC1 on the NVIDIA and AMD models, BC3 alpha, BC4 and BC5 rounding to nearest. The rgbcx column is Tessalith's patched build: BC1 Dual endpoints ([BC1 Dual rescore](#bc1-dual-rescore-2026-11-04)) and the wider BC4 block search above. Its BC3 colour half in this table is the Ideal-model encode, not Dual. On BC6H the texconv GPU path is slower than ISPC (0.41 against 0.19 s/Mpix) and 2.2 dB lower in mPSNR (4 HDR images).

† Original 4 HDRIs only; Compressonator's mPSNR there was 1.2 dB below ISPC.

## Sources

The main published sources were the [Aras 2020 benchmark](https://aras-p.info/blog/2020/12/08/Texture-Compression-in-2020/), Geldreich's posts ([2018 showdown](http://richg42.blogspot.com/2018/04/bc7-showdown-ispctexcomp-vs-my-ispc.html), [showdown 2](http://richg42.blogspot.com/2018/04/bc7-showdown-2-basis-ispc-vs-nvidia.html), [2021/02](https://richg42.blogspot.com/2021/02/)), the [bc7e README](https://github.com/BinomialLLC/bc7e), [bc7enc_rdo](https://github.com/richgel999/bc7enc_rdo), [ISPC TC](https://github.com/GameTechDev/ISPCTextureCompressor), [DirectXTex](https://github.com/microsoft/DirectXTex/wiki/Compress), the [Compressonator docs](https://compressonator.readthedocs.io/en/latest/), [Microsoft's BC7 format reference](https://learn.microsoft.com/en-us/windows/win32/direct3d11/bc7-format), [GPURealTimeBC6H](https://github.com/knarkowicz/GPURealTimeBC6H), [Ludicon Spark 1.4](https://www.ludicon.com/castano/blog/2026/09/spark-1-4/) and NVIDIA's [Texture Tools pages and forum posts](https://developer.nvidia.com/gpu-accelerated-texture-compression).
