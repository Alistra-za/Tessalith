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

#### Fast BC1 search (2026-11-14)

`encoder.bc1_search = fast` runs the BC1 search at a lower rgbcx level. Five levels of rgbcx (the C++ original, `cpp:<level>` in `crates/bcenc/examples/quality.rs`; `bcenc` matches it byte for byte at these levels) were scored on the `rgb_real` images (93), Dual endpoints, 3-colour blocks on, decoded with rgbcx's NVIDIA and AMD models. Mean RGB PSNR is the mean of per-image dB; Δ is the paired difference against level 18 [95% CI] with wins/losses over the 93 images; seconds are rgbcx's encode time for all 93 images on a busy 8-core machine. The Dual rescore table above reads 39.75 dB on AMD for level 18; that is its own, earlier run through an engine example.

| Encoder | NVIDIA (dB) | AMD (dB) | Δ NVIDIA | Δ AMD | rgbcx encode (s) |
|---|--:|--:|---|---|--:|
| rgbcx L18 (`default`) | 39.51 | 39.74 | | | 305.6 |
| rgbcx L15 | 39.48 | 39.72 | -0.025 [±0.005] 1/92 | -0.026 [±0.006] 1/92 | 145.6 |
| rgbcx L13 | 39.45 | 39.69 | -0.055 [±0.009] 0/93 | -0.057 [±0.010] 1/92 | 105.6 |
| rgbcx L12 (`fast`) | 39.41 | 39.65 | -0.097 [±0.018] 0/93 | -0.099 [±0.020] 0/93 | 83.3 |
| rgbcx L10 | 39.37 | 39.61 | -0.134 [±0.025] 0/93 | -0.137 [±0.028] 0/93 | 44.0 |
| NVTT 3 `-highest` | 39.38 | 39.62 | | | not timed |

The rule: the fastest level whose mean beats NVTT 3 `-highest` on both decoders. Level 10 falls 0.003 dB short on NVIDIA and 0.011 dB on AMD, so level 12 is the pick (Note HY16). Every faster level loses to level 18 on nearly every image, but only by 0.1 dB at level 12.

Per block, in the Rust encoder (`bcenc` `speed` example, 1,048,576 blocks, quiet machine, two passes), the fast level takes 0.375 and 0.353 of level 18's time on BC1 blocks and 0.357 and 0.322 on BC3 blocks. The Rust encoder at level 12 takes 0.199 and 0.203 of rgbcx's time at level 12 on BC1 blocks and 0.242 and 0.241 on BC3 blocks. The fast-to-default ratios differ from the rgbcx seconds above because the Rust port saves different amounts at each level than rgbcx does.

#### BC1 decoder weights (2026-11-15, Note HY18)

Question: does counting NVIDIA's error 2 or 3 times AMD's, instead of equally, give a better BC1? Measured on branch `feat/bcenc-weights` (tag `bcenc-plan-d-measured`, commit d892a3eb, unmerged) with `crates/bcenc/examples/quality.rs` and `examples/speed.rs`. Level 18, 3-colour blocks on, `rgb_real` (93 images), mean of per-image RGB PSNR. The baseline `rust:w11` is today's default, weights (1, 1). It equals `cpp:18` on every image (39.5085 dB NVIDIA, 39.7447 dB AMD; paired delta 0, 0 wins, 0 losses), so the run is valid. W21 is weights (2, 1), W31 is (3, 1), NVIDIA first.

The rule was fixed before the run. Per image the change in each decoder's PSNR is taken against the baseline. A candidate fails if any image has both decoders fall, or if NVIDIA's mean gain is not above zero, is below 0.02 dB, or has a 95% interval that includes zero. Otherwise it passes if AMD's mean also rises. If AMD does not rise, it passes only if AMD loses at most half of NVIDIA's gain and the gap between the decoders' means shrinks. It also has to encode faster than C++ would for one decoder model and use at most a third of the Rust port's time gain. The per-image reading of "both lower is a fail", the half-the-gain reading of "slight loss", the shrinking gap, the speed bar and the budget were pinned by the controller and confirmed by the owner on 2026-11-13. Only the 0.02 dB floor, the choice of candidates and dropping endpoint refinement were the acting owner's.

Endpoint refinement was ruled out before the run: the best pair within 1 code of level 18's gains +0.002 dB NVIDIA and +0.001 dB AMD, within 2 codes +0.003 and +0.002 (1,048,576 blocks, 2026-11-13); level 19 gains +0.002 dB at 5.5 times the time. All below the floor. The encoder is already within about 0.003 dB of the best its search space holds for the summed error, so new weights only trade one decoder against the other. Before the run the plan expected both candidates to fail, perhaps at the first step (an image with both decoders falling), and the likely end to be "measured, nothing passed, default unchanged". That was right except for the step: no image had both decoders fall in the plain rows, and both candidates failed on the 0.02 dB floor.

| Candidate | NVIDIA (dB) | AMD (dB) | Δ NVIDIA (wins/losses) | Δ AMD (wins/losses) | Verdict |
|---|--:|--:|---|---|---|
| (1, 1) default | 39.5085 | 39.7447 | | | |
| W21 | 39.5197 | 39.7274 | +0.0113 [±0.0040] 93/0 | -0.0173 [±0.0068] 0/93 | fail: below the gain floor |
| W31 | 39.5230 | 39.7181 | +0.0146 [±0.0050] 93/0 | -0.0266 [±0.0105] 0/93 | fail: below the gain floor |

Neither candidate reaches 0.02 dB on NVIDIA. AMD loses on every image, more than NVIDIA gains. The gap between the decoders shrinks (0.2362 dB to 0.2077 and 0.1950), but only because AMD falls.

Speed, per BC1 block (1,048,576 blocks, 5 runs, quiet machine): C++ 27.3 us, the Rust port at (1, 1) 4.02 us, W21 4.26 us, W31 4.03 us. The bar is 17.07 us and the budget 7.76 us above the Rust port's time, so both candidates pass on speed (W21 uses 0.24 us of the budget, W31 0.006 us). Speed does not rescue a candidate that fails the quality rule.

Report-only rows, each against the (1, 1) default under the same setting:

| Setting | W21 Δ NVIDIA / Δ AMD | W31 Δ NVIDIA / Δ AMD | Images with both decoders falling | Verdict |
|---|---|---|---|---|
| Perceptual weights | +0.0071 / -0.0155 | +0.0090 / -0.0237 | W21 2, W31 2 | fail for both: an image has both decoders fall |
| 3-colour blocks off (BC3's colour half) | +0.0105 / -0.0153 | +0.0137 / -0.0228 | W21 0, W31 0 | fail: below the gain floor |

Two limits of this measurement:

- Both NVIDIA-weighted candidates were affected by the [three-colour midpoint gap](#bc1-nvidia-three-colour-midpoint-2026-11-20-note-jz).
- **1-bit alpha gets worse under both candidates.** On generated blocks, total error against (1, 1) is +0.035% for W21 and +0.067% for W31. The `rgb_real` images are opaque and do not exercise the alpha path, so no image-set figure exists for it.

Result: nothing passed and the default stays (1, 1). The weight parameter and the measurement tool stay on the branch, unmerged.

#### BC1 NVIDIA three-colour midpoint (2026-11-20, Note JZ)

Does BC1 gain quality if the encoder scores the green midpoint that NVIDIA will decode from the packed endpoints? In the shipped encoder, `solid_block` and `bc1_encode3` can swap endpoints after scoring. That swap can change the green midpoint because rgbcx's NVIDIA lookup (`interp_half_6_nv`) is asymmetric. The trial kept rgbcx's NVIDIA table. It looked up green in the order of the two stored RGB565 words when scoring a three-colour fit or choosing endpoints for a solid block.

For example, stored words `0x0000 0x0200` have green codes (0, 16). Selector 2 gives green 33 on NVIDIA. With words `0x0200 0x0800`, red puts green 16 first. The NVIDIA midpoint is then 32. AMD gives 33 in both cases. On a test block where all 16 texels chose the midpoint, the default search scored 64 units of squared error across both decoders. The packed bytes scored 32.

The trial used level 18 on 93 opaque `rgb_real` images with transparent black off. Of 5,799,936 default blocks, 127,087 used three colours. In 7,693 blocks across 48 images, packing swapped the endpoints and changed a midpoint used by an image texel. These included 7,628 fitted blocks and 65 solids.

**Decoder check.** Before scoring, texcore and rgbcx decoded the same bytes with their NVIDIA models. All 16 RGBA texels agreed on every block across 19 rows (1,763,180,544 comparisons). The original rgbcx NVIDIA and AMD decoders graded in-image RGB. Task 3's restricted-minimum test, `nv_table_restricted_minima` in `crates/bcenc/src/bc1/nv_table.rs` (`research/bcenc-nv-table` at `472ede38`), checks every ordered green pair for all 256 targets at each trial weight. The hardware evidence is the [RTX 4090 readback](#bc1-decoder-model-2026-10-29) of 4,096 random blocks on DX12 and Vulkan. Hardware coverage is limited to that sample, with no AMD or Intel readback.

**Plain RGB.** C++ Dual (`cpp:18`) and the Rust default (`rust:w11`) matched byte for byte on every block and tied on both decoders on all 93 images. The per-image scores for W11, W21 and W31 reproduced the [decoder weights results](#bc1-decoder-weights-2026-11-15-note-hy18) to their published four decimal places.

Each gain below is the mean per-image change in RGB PSNR against `rust:w11`. Brackets give paired 95% Student-t intervals (92 degrees of freedom). Weights list NVIDIA first. The default gap between decoder means was 0.2362 dB.

| Candidate (weights) | NVIDIA gain [95% interval], dB | AMD gain [95% interval], dB | Decoder gap (dB) | Images where both fell |
|---|---:|---:|---:|---:|
| nv11 (1, 1) | +0.00139 [-0.00025, +0.00304] | +0.000004 [-0.000011, +0.000018] | 0.2348 | 0 |
| nv21 (2, 1) | +0.01324 [+0.00760, +0.01888] | -0.01728 [-0.02407, -0.01049] | 0.2057 | 0 |
| nv31 (3, 1) | +0.01705 [+0.00997, +0.02412] | -0.02661 [-0.03715, -0.01608] | 0.1926 | 0 |

Against the legacy encoder at the same weights, nv11 changed 59,176 blocks, nv21 changed 65,171 and nv31 changed 70,707. The NVIDIA gains from the correction alone were 0.00139, 0.00197 and 0.00246 dB respectively. Both weighted candidates beat the default on NVIDIA and lost to it on AMD on all 93 images.

**Other modes.** Each comparison uses the default (1, 1) encoder with the same settings. Masked blocks use weighted squared error, where a decrease is better.

| Diagnostic | nv11 | nv21 | nv31 |
|---|---:|---:|---:|
| Perceptual BC1, images where both scores fell (of 93) | 12 | 2 | 2 |
| Generated cutouts, blocks with higher error on both decoders (of 2,000) | 1 | 5 | 7 |
| Generated cutouts, change in total error using each candidate's weights | -0.015052% | +0.024751% | +0.047103% |
| Four-colour only, images where both scores fell (of 93) | 0 | 0 | 0 |

Perceptual BC1 fails the per-image rule. All three corrections lowered total masked error at fixed weights, but only nv11 beat the default on that measure. The four-colour control exercises BC3's colour half. All its bytes matched the legacy row at each fixed weight. Fast search, real cutouts and complete BC3 textures need their own measurements.

**Verdict.** All three candidates failed the 0.02 dB NVIDIA mean-gain floor. The interval for nv11 also includes zero. No candidate qualified. The speed run was not made because adoption required both quality and speed. Using medians from the same run, a candidate had to take less than C++ Dual time divided by 1.6. Its added time over Rust default could use at most one third of the time Rust saved over C++ Dual. For historical reference only, plan D measured 27.3 µs/block for C++ Dual, 4.02 for Rust default, 4.26 for legacy W21 and 4.03 for legacy W31.

Reproduce the tables at `472ede38` on `research/bcenc-nv-table`. The evidence folder is `F:\ai\dds\bench\nv-table-2026-11-20`. The per-image scores and run logs are `plain`, `p` and `4c` (CSV and TXT). The generated cutout results are in `masked.txt`.

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

Round 2 and Round 3 measured upstream ISPC TC, before patch 0004 ([what changed](#kernel-fixes-patch-0004-2026-11-22-note-lo)).

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

These rows measured upstream ISPC TC plus patch 0001, before patch 0004 ([kernel fixes](#kernel-fixes-patch-0004-2026-11-22-note-lo)).

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

### Kernel fixes (patch 0004, 2026-11-22, Note LO)

I audited the vendored ISPC TC BC6H kernel and fixed what reproduced, in patch 0004 (`crates/encoders-sys/vendor/ispctc/patches/0004-bc6h-kernel-fixes.patch`). Every BC6H row above, and the peak-mode numbers, measured upstream ISPC TC, before this patch.

**Fixed.** Each fix has a test that failed on the kernel before it and passes after (or, for #6 and #7, pins that stay byte for byte equal).
- #5, `get_span` returned its span through an `int`: a block whose widest channel spans 204.39 UF16 units was kept out of mode 2 at Fast and Basic, because 204.8 became 204. Changes bytes.
- #1, `block_quant` kept each texel's float error in an `int`: above 2^31 (a black texel beside one brighter than about 1344.0) it overflowed, and the overflowed value beat every honest candidate. Slow, Veryslow and VeryslowPeak turned such a block grey. Changes bytes.
- #3, the one-region encode submitted its last refinement pass, which can be worse than its first quantisation: it submits the best now. Changes bytes at Basic and slower.
- #2, endpoints rounded by `ep * (2^bits - 1) / 65535`, but the decoder rebuilds a code at (q + 0.5) * 65536 / 2^bits: the code could be one too high near 0 and one too low near the top. They take the code the decoder rebuilds nearest now. Changes bytes, and carries most of the mPSNR gain below (+0.066 of +0.069 dB).
- #6, `rgb_span` was declared with no elements and used with three: it has three now. No byte moves.
- #7, `bc6h_enc_2p` could read past its 32 partitions: it clamps. No profile asks for more, so no byte moves.

**Out of scope, not tested.** #4, negative and non-finite halves read as large values: Tessalith's `clamp_half` clamps every BC6H input first, so the kernel never sees them.

**Method.** `crates/encoders-sys/examples/bc6h_quality.rs` encodes the 18 `hdr_real` images (Poly Haven, 1024x512) and the 4 synthetic `hdr_grad` images at all six levels, one 4-row strip per call, decodes with `bcdec_rs`, and scores mPSNR, RMSLE and the peak ratio against the input as the engine sends it (every channel through `clamp_half`). The base is the kernel before patch 0004, the final is the kernel after it, both on the AVX2 path, paired per image: n = 18, 95 % Student-t intervals. Machine: Windows, Ryzen 7 9800X3D. A fix that lowered a metric's mean with an interval excluding 0 would have been dropped; only #2 trips that at Veryfast, and the owner accepted it.

Patch 0006 ([below](#audit-follow-up-speed-phase-patch-0006-2026-11-24-note-lt)) came after these measurements. It changes no byte and only the time, so the quality tables stand and the time table below is the kernel before it.

Base to final, mean delta (+ is better) with its 95 % half-width, and images better/worse:

| Level | mPSNR dB | RMSLE x1000 | Peak ratio |
|---|---|---|---|
| veryfast | -0.003 ±0.002, 2/14 | -0.002 ±0.003, 5/8 | +0.004 ±0.003, 7/0 |
| fast | +0.067 ±0.020, 16/2 | +0.006 ±0.010, 11/7 | +0.022 ±0.033, 9/2 |
| basic | +0.069 ±0.018, 18/0 | +0.007 ±0.008, 14/4 | +0.004 ±0.004, 9/1 |
| slow | +0.067 ±0.017, 17/1 | +0.009 ±0.008, 12/5 | +0.007 ±0.008, 9/1 |
| veryslow | +0.069 ±0.017, 18/0 | +0.007 ±0.008, 12/4 | +0.004 ±0.010, 8/2 |
| veryslow_peak | +0.069 ±0.018, 18/0 | +0.044 ±0.057, 14/2 | +0.014 ±0.026, 6/2 |

- Fast and slower gain about 0.07 dB of mPSNR, every interval clear of 0. The RMSLE and peak-ratio intervals include 0, except Slow RMSLE (+0.009 ±0.008, better).
- Veryfast loses 0.003 dB on average, and 0.10 dB on `sky_sun`. My guess, not checked: Veryfast does not refine, so it submits the unrefined line endpoints, and snapping those to the nearest code lowers the endpoint error without lowering the texel error. I accepted that (owner's call, "Keep it, accept Veryfast"): every slower effort gains about 0.07 dB from the same fix.
- No VeryslowPeak image falls below a peak ratio of 0.8 (lowest 0.934, 0.923 before). Peak mode's cost is unchanged: VeryslowPeak against Veryslow is -0.060 dB mPSNR after, -0.061 before, worst image venice_sunset (-0.696 after, -0.674 before).

**`hdr_grad`** (4 images, delta in mPSNR dB):
- veryfast: dim_ramp +0.088, log_ramp_color +0.083, noise_wide_range -0.001, sky_sun -0.100.
- fast: +0.382, +0.083, +0.075, sky_sun -0.032.
- basic and slower: every image gains, from +0.006 to +1.803 (slow, dim_ramp), except slow's sky_sun, which is unchanged (+0.000).
- veryslow_peak equals veryslow on all four.

**Changed images.** At every level, base against final on AVX2: 0 of 18 `hdr_real` and 0 of 4 `hdr_grad` images are byte-identical, so all six levels changed all 22. Per fix, against the one before: #6 and #7 changed none; #5 changed 15 of 18 `hdr_real` images at Veryfast, 17 at Basic and all 18 at the other levels; #1 changed none at Veryfast, 17 of 18 at Fast, Basic and Slow, and every image at Veryslow and VeryslowPeak; #3 changed Basic and slower; #2 changed every image at every level.

**SSE2 and AVX-512.** The same comparison on SSE2 and on AVX-512 agrees with AVX2 in direction at every level and metric, all means within 0.001 of AVX2 and the same single negative (Veryfast mPSNR, -0.003 ±0.002). The win and loss counts differ here and there.

**Time.** I timed each fix on a Linux VM: an Intel i9-13900K guest with 6 vCPUs, under Wine 11.0, on the AVX2 path with one thread. Each run encodes all 22 images, and each figure is the median of 3 rounds in alternating order, with no other process running. The VM's Intel CPU writes different BC6H bytes from the Ryzen behind the quality numbers, so these runs measure time only. Each ratio is after over before (1.10 = 10 % slower):

| level | #5 / base | #1 / #5 | #3 / #1 | #2 search / #3 | #2 / #3 | final / base |
|---|---|---|---|---|---|---|
| veryfast | 1.024 | 0.988 | 1.000 | 1.226 | 1.036 | 1.024 |
| fast | 1.011 | 0.994 | 1.006 | 1.139 | 1.027 | 1.037 |
| basic | 1.008 | 0.983 | 0.994 | 1.135 | 1.044 | 1.029 |
| slow | 1.000 | 0.997 | 0.999 | 1.150 | 1.045 | 1.039 |
| veryslow | 0.995 | 0.995 | 0.999 | 1.192 | 1.059 | 1.051 |
| veryslow_peak | 0.993 | 1.000 | 0.997 | 1.188 | 1.064 | 1.054 |
| total | 0.997 | 0.996 | 0.998 | 1.180 | 1.058 | 1.049 |

- #5, #1 and #3 cost nothing measurable: the rounds of one build differ by up to 1.4 %.
- #2's first form searched three candidate codes per endpoint and was 18 % slower in total. A single multiply finds the same code and writes the same bytes, so it replaced the search; #2 now costs 3 to 6 % per level. The first four columns come from one VM run and the last two from a second one.
- Veryfast takes 0.08 to 0.10 s for all 22 images, so its ratios move in coarse steps.

### Audit follow-up, speed phase (patch 0006, 2026-11-24, Note LT)

A second audit of the BC6H kernel (mine, plus a Codex one whose findings Opus checked one by one) left more open after patch 0004. I split the work in two: speed items that must not move a byte, which are patch 0006 (`crates/encoders-sys/vendor/ispctc/patches/0006-bc6h-speed.patch`) and are described here, and fixes that do move bytes, which are not in this patch. Patch 0006 is seven kept items and one dropped. BC6H output is byte for byte what patch 0004 wrote, on SSE2, AVX2 and AVX-512. Timed on the VM, the 22 images take 0.519 of the time end to end, about 1.9 times as fast (1.96 times at VeryslowPeak, the level BC6H's default effort, Slowest, uses).

**Diagnostics.** `bc6h_quality diag` (new, in `crates/encoders-sys/examples/bc6h_quality.rs`) encodes the 22 images at all six levels and counts what the audit asked about, without touching a kernel byte. AVX2 on the Ryzen, 18 `hdr_real` images (589,824 blocks) and 4 `hdr_grad`. The fixes these point at are not in 0006, so each is one line here and the counts are in Note LT:
- Decode bias (Q5): the decoded half is 0.41 to 0.51 codes below the source on average on `hdr_real` at every level (median 0, or -1 for Veryfast's blue).
- Collapsed regions (Q2): on `hdr_real`, 0.19 to 0.44 % of regions decode to one colour though the source has several, with at most 0.007 % of the squared error; `hdr_grad` reaches 1.03 % of regions and 0.264 % of the error.
- Two-region gate at Fast and Basic (Q4): a different gate would change the pick of 43,728 of 589,824 blocks at each level.
- Guarded blocks (Q6): 256 of 589,824 blocks take the peak search, and the lift changes no channel in 211 of them (that is what the skip-retry item removes).
- Three Codex findings (astra #5, #2 and #6): 5 blocks, 2 blocks and a source-side proxy; they wait for the byte-changing patch.
- Two tests that fail today are committed `#[ignore]`: a zero-covariance PCA block that decodes wrongly on the Ryzen's AVX-512 path only, and constant blocks that decode one half code low (2,194 of 4,536 at every level and path, every channel that misses being exactly one low).

**Kept.** Each item was checked to leave every byte as it was: the 18 pins, the random strips (48 hashes, 72 with AVX-512), the 22 measured images at all six levels on all three paths, and the engine's `hdr_sky.dds`.
- R2: the endpoint quantiser, its inverse and the PCA helpers touched slots nobody had written (the fourth channel of an endpoint, covariance entries the setup never filled); those loops now skip them. S2's cached copy of 16 floats and the best-endpoint copy still carry the unused slots along, which changes no output value. A correctness item, kept on identical bytes; it is also faster.
- Zero-div guard: a region with equal endpoints divided 0 by 0; it divides by at least 1. The numerator is still 0, so the index is as before, without the NaN. Kept as a correctness item; it costs 0.7 % at Slow and 0.5 % at VeryslowPeak, beyond the spreads, and is within noise at the other levels.
- Q6 skip-retry: VeryslowPeak ran its second search on every guarded block, and a block the lift does not change would repeat the first search exactly. It runs the second search only where the lift raised a channel.
- S1 rank-once: each two-region mode ranked the same 32 partitions of the block again (six times at Slow and slower). It is ranked once per block.
- S2 PCA once: each mode refitted the same line through the block for every ranked partition. The fits are cached per block (about 32.5 KiB of state per gang on AVX-512).
- 7a qbounds-once: at Veryfast, Fast and Basic each admitted mode built endpoint bounds that the next one overwrote. They are built once, right before the encode that uses them.
- S4 prune: at Slow and slower a mode is skipped when the distance of the block's texels to its endpoint window already exceeds the best error. The bound is below every candidate's error (it fired no assert in 15.9 million lane encodes; counts under "Mutations and proofs" below).

**Dropped.** S3, a BC6H `block_quant` that loads each region's endpoints once instead of gathering: both forms I tried (a hoisted reciprocal, then a per-texel division) changed bytes in `hdr_sky.dds`, which the engine's identity test caught. The first left the pins and the random strips equal. The second, on AVX2, changed some of the 22 images at every level (every metric delta +0.000, last-bit index flips). Two tries were the limit I set, so it is dropped and no time was measured.

**Method.** The same Linux VM as for patch 0004 (an Intel i9-13900K guest, 6 vCPUs, Wine 11.0), AVX2 path, one thread, 22 images, the median of 5 rounds in alternating order. A round with another process running was discarded and re-run. Every item is timed against the exe of the one kept before it, so the item's gain is the ratio of two runs made together. The VM's Intel CPU writes different bytes from the Ryzen, so these measure time only. The noise: an exe against itself gave ratios of 0.989 at Veryfast and 0.999 to 1.001 elsewhere, and each exe's slowest round over its fastest is in the spread table. A gain or loss counts only when the ratio is outside both exes' spreads; a ratio inside at least one exe's spread is called within noise. An item is kept when it is faster beyond the noise at the levels it names and no level is slower beyond it; R2 and the zero-div guard are kept on identical bytes alone unless slower, and the zero-div guard is slower by about 0.5 to 0.7 % at Slow and VeryslowPeak and kept anyway because it removes NaN-dependent behaviour.

Ratio after over before (below 1 is faster); Veryfast takes 0.08 to 0.09 s, so its figures move in steps of one hundredth:

| item | veryfast | fast | basic | slow | veryslow | veryslow_peak | total |
|---|---|---|---|---|---|---|---|
| R2 | 0.988 | 0.917 | 0.926 | 0.960 | 0.959 | 0.959 | 0.957 |
| zero-div guard | 0.988 | 1.017 | 1.004 | 1.007 | 1.007 | 1.005 | 1.007 |
| Q6 skip-retry | 0.988 | 0.987 | 0.997 | 0.998 | 0.998 | 0.994 | 0.996 |
| S1 rank-once | 1.024 | 1.012 | 0.773 | 0.742 | 0.872 | 0.873 | 0.848 |
| S2 PCA once | 0.976 | 0.993 | 0.908 | 0.714 | 0.652 | 0.652 | 0.680 |
| 7a qbounds-once | 1.000 | 0.981 | 0.987 | 0.998 | 0.999 | 0.998 | 0.995 |
| S4 prune | 1.000 | 1.010 | 0.999 | 0.935 | 0.938 | 0.939 | 0.943 |
| all seven, chained | 0.964 | 0.916 | 0.641 | 0.477 | 0.513 | 0.511 | 0.519 |

Median seconds, before / after, for all 22 images:

| item | veryfast | fast | basic | slow | veryslow | veryslow_peak | total |
|---|---|---|---|---|---|---|---|
| R2 | 0.09 / 0.09 | 0.64 / 0.59 | 1.61 / 1.49 | 6.83 / 6.56 | 14.71 / 14.11 | 14.83 / 14.21 | 38.71 / 37.04 |
| zero-div guard | 0.09 / 0.08 | 0.59 / 0.60 | 1.49 / 1.50 | 6.56 / 6.60 | 14.11 / 14.21 | 14.21 / 14.29 | 37.04 / 37.29 |
| Q6 skip-retry | 0.08 / 0.08 | 0.60 / 0.59 | 1.50 / 1.49 | 6.60 / 6.59 | 14.21 / 14.18 | 14.29 / 14.20 | 37.28 / 37.13 |
| S1 rank-once | 0.08 / 0.09 | 0.59 / 0.60 | 1.49 / 1.15 | 6.57 / 4.87 | 14.18 / 12.36 | 14.21 / 12.41 | 37.13 / 31.47 |
| S2 PCA once | 0.09 / 0.08 | 0.60 / 0.59 | 1.17 / 1.06 | 4.92 / 3.51 | 12.48 / 8.14 | 12.50 / 8.15 | 31.71 / 21.55 |
| 7a qbounds-once | 0.08 / 0.08 | 0.59 / 0.58 | 1.05 / 1.04 | 3.50 / 3.49 | 8.05 / 8.04 | 8.07 / 8.05 | 21.39 / 21.29 |
| S4 prune | 0.08 / 0.08 | 0.58 / 0.59 | 1.04 / 1.04 | 3.49 / 3.26 | 8.04 / 7.54 | 8.07 / 7.58 | 21.30 / 20.09 |

R2 and the zero-div guard were timed in one run against the base; each later item ran in its own VM session against the previous kept exe, so one item's "after" and the next item's "before" come from different sessions and differ a little (37.29 and 37.28 s, 31.47 and 31.71 s, 21.55 and 21.39 s), and the 38.71 s and 20.09 s ends come from different sessions too. The chained row multiplies the seven ratios; the end-to-end measure is 38.71 s (base, R2's "before") to 20.09 s (S4's "after"), 0.519 of the time, or about 1.9 times as fast. By level: Slow 6.83 to 3.26 s, Veryslow 14.71 to 7.54 s, VeryslowPeak 14.83 to 7.58 s.

Spread, each exe's slowest of 5 rounds over its fastest, before / after:

| item | veryfast | fast | basic | slow | veryslow | veryslow_peak | total |
|---|---|---|---|---|---|---|---|
| R2 | 1.023 / 1.037 | 1.024 / 1.012 | 1.014 / 1.008 | 1.006 / 1.005 | 1.001 / 1.009 | 1.003 / 1.004 | 1.002 / 1.006 |
| zero-div guard | 1.037 / 1.048 | 1.012 / 1.019 | 1.008 / 1.010 | 1.005 / 1.004 | 1.009 / 1.005 | 1.004 / 1.004 | 1.006 / 1.002 |
| Q6 skip-retry | 1.036 / 1.036 | 1.010 / 1.026 | 1.003 / 1.017 | 1.001 / 1.007 | 1.001 / 1.001 | 1.005 / 1.002 | 1.002 / 1.001 |
| S1 rank-once | 1.012 / 1.024 | 1.012 / 1.046 | 1.006 / 1.006 | 1.006 / 1.008 | 1.003 / 1.006 | 1.002 / 1.004 | 1.002 / 1.005 |
| S2 PCA once | 1.036 / 1.084 | 1.019 / 1.024 | 1.021 / 1.017 | 1.005 / 1.012 | 1.005 / 1.029 | 1.004 / 1.028 | 1.004 / 1.022 |
| 7a qbounds-once | 1.037 / 1.012 | 1.014 / 1.016 | 1.014 / 1.006 | 1.028 / 1.013 | 1.006 / 1.002 | 1.008 / 1.003 | 1.006 / 1.002 |
| S4 prune | 1.049 / 1.024 | 1.019 / 1.029 | 1.007 / 1.011 | 1.004 / 1.005 | 1.006 / 1.007 | 1.004 / 1.003 | 1.002 / 1.005 |

What the tables say, item by item:
- R2 is faster beyond the noise at every level but Veryfast (0.917 to 0.960).
- The zero-div guard costs 0.7 % at Slow and 0.5 % at VeryslowPeak. Fast's 1.017 is inside the guard exe's spread (1.019), Basic's 1.004 inside both, and Veryslow's 1.007 inside R2's (1.009), so those are within noise; only Slow (1.007) and VeryslowPeak (1.005) are outside both exes' spreads. I kept it anyway: it removes NaN-dependent behaviour, for 0.5 to 0.7 % at Slow and VeryslowPeak if the call is wrong.
- The skip-retry is faster at VeryslowPeak beyond the noise: its five rounds were 14.180 to 14.206 s against 14.278 to 14.347 s, no overlap, and the peak pass over Veryslow fell from 0.08 s to 0.02 s. That pass cost is what I keep it for. The 0.994 ratio is not: Veryslow, where the retry never runs, moved 0.998 with non-overlapping rounds too (14.174 to 14.182 against 14.206 to 14.219 s), which is probably code layout or alignment, not this change.
- S1 is 12.7 to 25.8 % faster at Basic and slower. Fast (1.012) and Veryfast (1.024) are each inside at least one exe's spread; Fast now ranks in every gang where it ranked only in gangs that entered a two-region mode, so a little more work there is plausible.
- S2 is 9.2 to 34.8 % faster at Basic and slower, the biggest single gain. Veryfast and Fast are within noise.
- 7a is faster at Fast by 1.9 %, beyond both spreads, and within noise elsewhere. A first form that always built with the widened-bounds helper added a loop per build and was replaced by two calls; two earlier runs of that form were partly loaded and are replaced by the one clean run in the tables.
- S4 is 6.1 to 6.5 % faster at Slow, Veryslow and VeryslowPeak, beyond both spreads (the rounds do not overlap); the other levels are within noise. 3.7 million of 19.7 million unguarded lane-modes were skipped. It prunes a mode only when all 16 lanes of an AVX-512 gang prune, so its gain there may differ from the VM's AVX2.

**Mutations and proofs.** Each byte-neutral item was broken on purpose after its commit, to show that a check sees it (R2 and the zero-div guard change no output value, so none is possible; the zero-div one is the old kernel).
- Skip-retry, `if (!lifted)`: `the_peak_mode_is_byte_identical_to_the_recorded_objects` (the VeryslowPeak pins) and `peak_mode_keeps_a_blue_zero_sun` fail on SSE2, AVX2 and AVX-512, and the VeryslowPeak strip hash of `a` moves on SSE2 and AVX2.
- S1: deleting the ranking call in `bc6h_setup` fails `shipped_levels_are_byte_identical_to_the_recorded_objects`, `the_peak_mode_is_byte_identical_to_the_recorded_objects` and `the_plain_call_encodes_on_the_process_target` on all three paths and moves 60 strip hashes. Deleting the call in `bc6h_peak_retry` moves nothing, no pin, strip hash or test: VeryslowPeak searches all 32 partitions (`fastSkipTreshold` 32), so list order matters only for exact ties. The call stays as the exact equivalent of the old per-call ranking; a change that lowers that threshold must add a check.
- S2: (a) every rank copying rank 0's endpoints fails the same two pins on all three paths and moves 30 of the 48 SSE2 and AVX2 strip hashes; (b) deleting the refill in `bc6h_peak_retry` fails `the_peak_mode_is_byte_identical_to_the_recorded_objects` and `peak_mode_keeps_a_blue_zero_sun` on all three paths and moves the VeryslowPeak strip hash of `a` (the stale list alone moves nothing, so this is the endpoints).
- 7a: dropping the bounds build before `bc6h_enc_1p` at Fast and Basic (run on the first form, the one that always called the widened-bounds helper) fails `shipped_levels_are_byte_identical_to_the_recorded_objects` and `refinement_never_ends_worse_than_veryfast` on all three paths and moves 4 Fast and 4 Basic of the 48 strip hashes; dropping it at Veryfast too crashes the `bc6h` binary with a stack buffer overrun. Dropping the build before `bc6h_enc_2p` at Fast and Basic (final form) leaves the bounds uninitialised and aborts the binary on a kernel assert, so no hash is compared. The replacement of the first form by two calls was forced by its cost (an extra loop per build), not by an identity failure.
- S4: the bound is proved by a scratch build that asserted `best_err >= bound * (1 - 1e-4) - 1` on each encoded mode of unguarded slow-level lanes. It fired on none of 15,922,981 lane encodes of the 22 images on AVX2 (Slow and Veryslow together 10,616,599, VeryslowPeak 5,306,382; 3,735,259 of 19,658,240 unguarded lane-modes pruned, 3,010 guarded lane-modes not checked), nor on 314,139 in `random_blocks_encode_at_every_level_on_every_path` with AVX-512 (209,426 without). A control with an always-false assert aborted the test at its first Slow call, so the assert was live. The mutation `bound * 2 > best_err` passes the pins and strips; `encode_versions` fails (`hdr_sky.dds`) and the 22-image compare says 0/18 and 1/4 identical at Slow, Veryslow and VeryslowPeak.
- S3 in detail: the first attempt hoisted `1.0f / div` per region (with `bc6h_peak_guard` given the same select, then again with its gathers restored); its `bc6h` binary on all three paths and its 48 and 72 strip hashes equalled the previous kept ones, but `encode_versions` failed. The second divided per texel (`proj / div_j`) and failed `encode_versions` too; against the previous kept CSV on AVX2 the hdr_real / hdr_grad images identical were 17/18 and 3/4 at Veryfast, 5/18 and 3/4 at Fast, 5/18 and 2/4 at Basic, 4/18 and 2/4 at Slow, 3/18 and 2/4 at Veryslow and VeryslowPeak. The smallest reproduction is the one-region path at Veryfast on `studio_small_08` and `dim_ramp`. My guess, not tested: under fast-math the loop-invariant division compiles to a reciprocal multiply, so both attempts compile alike.

**SSE2 and AVX-512 on the PC.** The whole speed phase was also timed end to end on the Ryzen: the base exe (8f98807a, patch 0004's kernel) against the speed-end exe (8543ef55, patch 0006; its kernel differs from the last kernel commit, 9c452929, by one comment). One thread, the same 22 images, the median of 3 clean rounds with no other session running, SSE2 then AVX-512 in each round. Each round timed the base, the speed-end exe and a second copy of the base (the A/A pair), in that order in rounds 1 and 3 and reversed in round 2. Seconds are summed over the 22 images (a level's median over the 3 rounds); the ratio is the speed-end median over the base median, formed from the unrounded seconds, so it can differ from the rounded seconds shown in the last digit (below 1 is faster); "spread" is an exe's slowest round over its fastest.

SSE2:

| level | base s | speed-end s | ratio | spread base / speed-end |
|---|---|---|---|---|
| Veryfast | 0.18 | 0.16 | 0.88 | 1.00 / 1.01 |
| Fast | 1.57 | 1.48 | 0.95 | 1.00 / 1.01 |
| Basic | 3.95 | 2.68 | 0.68 | 1.00 / 1.01 |
| Slow | 17.22 | 8.47 | 0.49 | 1.00 / 1.01 |
| Veryslow | 37.30 | 19.68 | 0.53 | 1.01 / 1.01 |
| VeryslowPeak | 37.44 | 19.77 | 0.53 | 1.01 / 1.01 |
| TOTAL | 97.73 | 52.27 | 0.54 | 1.01 / 1.01 |

AVX-512:

| level | base s | speed-end s | ratio | spread base / speed-end |
|---|---|---|---|---|
| Veryfast | 0.09 | 0.09 | 0.99 | 1.08 / 1.02 |
| Fast | 0.51 | 0.48 | 0.94 | 1.03 / 1.01 |
| Basic | 1.31 | 0.96 | 0.73 | 1.02 / 1.00 |
| Slow | 5.86 | 3.50 | 0.60 | 1.01 / 1.00 |
| Veryslow | 12.90 | 8.54 | 0.66 | 1.02 / 1.00 |
| VeryslowPeak | 13.04 | 8.54 | 0.66 | 1.01 / 1.01 |
| TOTAL | 33.69 | 22.09 | 0.66 | 1.01 / 1.00 |

The speed-end exe is faster than the base at every level on SSE2, and on AVX-512 at every level except Veryfast, which is within noise; it is slower at none. At the default effort's level (VeryslowPeak) it takes 0.53 of the time on SSE2 (1.9 times as fast) and 0.66 on AVX-512 (1.5 times as fast), against 0.511 on the VM's AVX2 path, so the AVX-512 gain is smaller than the VM's. I did not time the items one by one on the PC, so I do not know which one gives less there; the S4 prune, which skips a mode only when all 16 lanes of a gang agree, is a candidate.

Noise: the A/A ratio (second base copy over the first) is 1.00 at every level except AVX-512 Veryfast (1.06; 1.01 on SSE2). AVX-512 Veryfast takes 0.09 s for all 22 images, and its 0.99 is inside the spreads (1.08 and 1.02), so it is within noise. SSE2 Veryfast's 0.88 is outside both exes' spreads (1.00 and 1.01) and the A/A ratio, so by the rule above it counts as a gain, on a level that takes 0.16 to 0.18 s. Spreads are 1.00 to 1.02 except AVX-512 Veryfast (1.08 for the base) and Fast (1.03 for the base), and the second base copy's on AVX-512: 1.10 at Veryfast and 1.13 to 1.15 at Fast and slower (one slow run in one round; its median still equals the base's). By the noise rule above every ratio at Fast and slower, and SSE2 Veryfast's, is outside both exes' spreads.

Hash check: the output hashes of the base, the second base copy and the speed-end exe are identical in all 3 rounds on both paths (12 comparisons, all equal), so the PC runs also show the bytes unchanged.

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

Tessalith encodes BC1, BC3, BC4 and BC5 with `bcenc`, a Rust port of its patched rgbcx. The port runs the same search and writes the same bytes. Four changes make it faster, and each gives the C++ result for every block:

- **A repeated pair of BC1 endpoints is scored once.** rgbcx's level-18 search reaches the same two endpoint colours many times in one block, by different routes. The port keeps the pairs it has scored for the block and skips a repeat.
- **A pair is scored against all 16 texels at once.** Finding each texel's nearest palette entry and summing the error is SSE2 integer code over the whole block, in BC1 and in BC4.
- **BC4 searches each mode once.** rgbcx searches the 8-value and the 6-value candidates, then searches one mode's candidates again when it refits. The port keeps the first search's result.
- **A BC1 total-ordering pass fits all its orderings at once.** A 4-colour pass has 128 orderings and a 3-colour pass 32. Each gets its endpoint pair from the pass's prefix sums and the block's totals, so the port computes them together on SSE2 vectors, the float code included, and scores them in rgbcx's order.

Time per 4×4 block in µs: one thread, median of 5 runs over 1,048,576 blocks (16 textures of 1024² from installed Skyrim mods), two passes, the mean of the two medians, `bcenc` at b26f0004 against the vendored C++ on the [benchmark machine](#benchmark-machine-and-tools), with the arguments the engine passes (`crates/bcenc/examples/speed.rs`). The one-bit-alpha row has no real mask to use, so it marks a texel transparent when its green is below 64:

| Format | rgbcx (C++) | `bcenc` | `bcenc` / C++ |
|---|--:|--:|--:|
| BC1 | 28.86 | 4.16 | 0.14 |
| BC1, perceptual | 28.37 | 4.24 | 0.15 |
| BC1, one-bit alpha | 15.97 | 2.53 | 0.16 |
| BC3 | 27.00 | 4.18 | 0.15 |
| BC4 | 30.56 | 2.03 | 0.07 |
| BC5 | 60.07 | 4.07 | 0.07 |

The port takes 0.14 to 0.16 of the C++ time on BC1 and BC3 and 0.07 on BC4 and BC5; before the BC1 fit change (9ec06da4) it took 0.19 to 0.20 on BC1 and BC3. The port before the first three changes (945ad68c) took 0.69 of the C++ time on BC1 and BC3 and 0.82 on BC4 and BC5.

`sdc convert` of 300 PNGs with the format forced, Tessalith 3.0.0 against b26f0004, in seconds, the lower of two rounds (reading, mips and writing included; another job ran beside both builds):

| Forced format | 3.0.0 | b26f0004 |
|---|--:|--:|
| BC1 | 67.7 | 14.1 |
| BC3 | 62.7 | 13.9 |
| BC4 | 20.4 | 4.8 |
| BC5 | 27.8 | 5.7 |

Every DDS has the same SHA-256 from both builds, in forced BC1, BC1 perceptual, BC1 cutout (plain and perceptual), BC3 with two alpha cuts, BC4 and BC5 (a 300-PNG slice).

Compared with the C++ block by block, b26f0004 has 0 mismatches in each:
- **Real textures:** every block of 10,000 textures as BC1, BC3, BC4 and BC5, and the 1,437 with transparent texels as BC1 with one-bit alpha: 2.72 billion comparisons. The port before the first three changes (017fbc68) passed the same run.
- **Generated blocks:** 100 million BC1 blocks, every solid colour, 50 million one-bit-alpha blocks, 50 million BC3 blocks and 20 million BC4 blocks.

A sampling profile of c81ae94e on the same blocks shows where the rest of the time goes. BC1: the least-squares endpoint fits and their rounding to 5:6:5 38%, scoring texels and building palettes 33%, the table of scored pairs 14%. BC4: scoring texels 62%, building palettes 16%.

#### Speed per path on CPUs with AVX2 (2026-11-22, Note HY)

`bcenc` has two code paths, `base` and `avx2`, and writes the same bytes on both. `Path::best()` picks `avx2` on every CPU and OS that has AVX2 and `base` otherwise, once per call, with no check of the CPU's make. `avx2` compiles the BC1 encode a second time with AVX2 allowed, runs the BC1 selector search on 256-bit vectors, and runs BC4's search (BC5's two channels, BC3's alpha block) scoring two endpoint pairs per vector. Zen 1, which splits 256-bit operations in two, is not measured.

Method: the 16 textures above (1,048,576 blocks), one thread, `speed.exe` with five runs per invocation, three alternating rounds per path, the lowest median of the rounds for each of Rust and C++ (µs per block = median ÷ 1,048,576). Every row printed `same_bytes yes`. `bc1-fast` and `bc3-fast` are the `fast` BC1 search (rgbcx level 12). A round is clean when no other job was running at its start and end; each figure below gives its clean count.

**Shipped code (204fe8d3), Intel Core i9-13900K host, Linux KVM guest with 6 vCPUs pinned to P-cores, `speed.exe` under Wine 11.0; 3 of 3 clean rounds on both paths.** I took this table here because the Windows PC stayed busy with other builds. Its C++ times are not the 9800X3D's, so compare ratios, not microseconds, across the two tables.

| Row | C++ µs | `base` µs | `base` / C++ | `avx2` µs | `avx2` / C++ | `avx2` / `base` |
|---|--:|--:|--:|--:|--:|--:|
| BC1 | 32.75 | 4.60 | 0.141 | 4.13 | 0.126 | 0.897 |
| BC1, perceptual | 32.44 | 4.57 | 0.141 | 4.09 | 0.126 | 0.896 |
| BC1, 4-colour only | 28.75 | 3.89 | 0.135 | 3.48 | 0.121 | 0.896 |
| BC1, one-bit alpha | 18.07 | 2.64 | 0.146 | 2.33 | 0.129 | 0.882 |
| BC3 | 30.11 | 4.51 | 0.150 | 4.11 | 0.136 | 0.911 |
| BC4 | 29.67 | 2.50 | 0.084 | 2.36 | 0.080 | 0.945 |
| BC5 | 57.93 | 5.00 | 0.086 | 4.73 | 0.082 | 0.947 |
| BC1, `fast` | 9.18 | 1.60 | 0.174 | 1.43 | 0.156 | 0.896 |
| BC3, `fast` | 6.53 | 1.49 | 0.229 | 1.39 | 0.213 | 0.933 |

**BC1 changes alone, Ryzen 7 9800X3D (Zen 5), Windows 11, build c2df2021 (2026-11-15); 3 of 3 clean rounds.** The BC3 and `bc3-fast` rows show only the gain of BC3's colour half, and BC4 and BC5 are unchanged, because these rows are from before the BC4 change. The last column here is (`avx2` / C++) / (`base` / C++), the two C++-normalised times, not the ratio of the Rust times; HY21's figures are the same quantity:

| Row | C++ µs | `base` µs | `base` / C++ | `avx2` µs | `avx2` / C++ | (`avx2` / C++) / (`base` / C++) |
|---|--:|--:|--:|--:|--:|--:|
| BC1 | 27.12 | 3.94 | 0.145 | 3.53 | 0.130 | 0.898 |
| BC1, perceptual | 26.86 | 3.90 | 0.145 | 3.50 | 0.130 | 0.897 |
| BC1, 4-colour only | 24.06 | 3.28 | 0.136 | 2.95 | 0.123 | 0.900 |
| BC1, one-bit alpha | 14.90 | 2.27 | 0.152 | 2.01 | 0.135 | 0.884 |
| BC3 | 25.27 | 3.81 | 0.151 | 3.48 | 0.138 | 0.913 |
| BC4 | 28.35 | 1.85 | 0.065 | 1.85 | 0.065 | 1.00 |
| BC5 | 54.83 | 3.68 | 0.067 | 3.69 | 0.067 | 1.00 |
| BC1, `fast` | 7.44 | 1.44 | 0.193 | 1.30 | 0.175 | 0.904 |
| BC3, `fast` | 5.58 | 1.28 | 0.230 | 1.23 | 0.220 | 0.959 |

BC4's change alone, `avx2` after over before: on the i9-13900K VM `bc4` 0.901, `bc5` 0.904, `bc3` 0.988, `bc3-fast` 0.965, the BC1 rows 0.995 to 1.001 (3 of 3 clean rounds). On the 9800X3D it gave `bc4` 0.975, `bc5` 0.976, `bc3` 0.990, but only one round of three was clean, so read that as a hint, not a result. BC4 and BC5 gain about 10% on that Intel CPU and 2 to 3% on the Zen 5 desktop. On the VM the `base` path stayed within 0.952 to 1.003 of its earlier time on every row.

Tried and not shipped (the two measured ones had to cut a targeted row's `rust/cpp` by 3%, with no row more than 2% worse):
- **BC4 palettes from a 4 KB table indexed by endpoint difference**, on the 9800X3D (verdict on round 3, the clean one): `bc4` went from 0.063-0.064 to 0.067-0.070 and `bc3-fast` from 0.225-0.232 to 0.242-0.253. Looking the entry up costs more than computing it.
- **BC1 ordering fits two at a time in 256-bit vectors**, on the 9800X3D (3 of 3 clean rounds), after over before: BC1 rows 0.983 to 0.991, `bc3-fast` 1.001. A steady 1 to 1.7% is under the 3% bar.
- **An AVX-512 path** (a frame, plus BC4 scoring four pairs per vector): dropped on the owner's word with no quiet measurement. The one reading, from a busy machine, had BC4 slower.

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
- **bc7e for BC7, rgbcx for BC1/BC3/BC4/BC5, ISPC TC for BC6H** (with patches 0001, 0003, 0004 and 0006; [0004's kernel fixes](#kernel-fixes-patch-0004-2026-11-22-note-lo), [0006's speed items](#audit-follow-up-speed-phase-patch-0006-2026-11-24-note-lt)), vendored and pinned with SHA-256 hashes in [`crates/encoders-sys/vendor/`](../../crates/encoders-sys/vendor/README-VENDOR.md). The format alone picks the encoder (Note FM15). texconv and Compressonator are not used to encode; texconv is used only as a test oracle for headers.
- **bc7e at `veryslow` (level 5) is the default BC7 level.** Level 5 matches level 6 at 1.5× the speed. Level 4 is not offered, because level 3 beats it on gradients. The levels are in [`crates/engine/src/encode.rs`](../../crates/engine/src/encode.rs) and the default in [`presets/sdc-base.toml`](../../presets/sdc-base.toml). The `fast-preview` preset uses `fast`.
- <a id="vendored-encoders-reproduce-the-bench"></a>**The vendored encoders reproduce the bench.** Spike S3 (2026-10-08) built the workspace without ISPC on the path and matched the bench's bc7e level 5 result on `rgb_real` (mean 50.9771 dB against 50.977, largest per-image Δ 0.000000 dB over all 10 BC7 sets, 39 of 39 block payloads byte-identical) and rgbcx BC4 on `mask_real` (7 of 7 bit-exact). The Phase 0 gates (2026-10-09) added rgbcx BC1, BC3, BC4 and BC5 bit-exact (14, 25, 12 and 11 images; transparent black off on 14 of 14), ISPC BC6H `veryslow` at 51.4995 dB with 8 of 8 payloads byte-identical, and ISPC objects that regenerate byte for byte.
- **rgbcx runs at level 18, its highest.** BC1 and the colour half of BC3 encode with rgbcx Dual, which scores NVIDIA and AMD decoder error together; there is no decoder setting (Note FM17). BC3 is never picked to save space, because it is the same size as BC7.
- **BC4/BC5 search radius 5 is the default; radius 4 is the `fast` setting** used by the compact and preview presets; radius 10 is `slowest` ([`crates/encoders-sys/src/rgbcx.rs`](../../crates/encoders-sys/src/rgbcx.rs)); `max` adds the pair search ([BC4-family block search](bc4-family.md#max-near-exhaustive)). rgbcx is patched to round its BC4 palette like the decoder, and to search each BC4 block further for BC4, both BC5 channels and BC3 alpha ([BC4-family block search](bc4-family.md)). The patches and their tests are in [`README-VENDOR.md`](../../crates/encoders-sys/vendor/README-VENDOR.md).
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
