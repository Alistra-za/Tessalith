# BC7: bc7e tuning, the `max` level and alpha weights

BC7 studies of Tessalith's encoder settings: whether bc7e has quality left above `veryslow` (2026-10-18, commit 65c6db00), which became the `max` level; [whether BC7 with a higher alpha weight can replace BC3](#bc7-alpha-weights-against-bc3-2026-10-21) for delicate alpha (2026-10-21, commit dbf54808), which became the *BC7 weighting* options; and bc7e patch 0005 (2026-11-23 to 2026-11-25): [its kernel fixes](#kernel-fixes-patch-0005-2026-11-25-note-lr), [the per-type flat tolerance and channel guard](#per-type-tuning-flat-tolerance-and-the-channel-guard-2026-11-23-note-lr), [×16 on colour and the alpha weights again](#x16-on-colour-and-the-alpha-weights-again-2026-11-24-note-lr) and [the estimators' precision](#estimator-precision-2026-11-23-note-lr). The encoder comparison they build on is [BCn encoders by format](encoder-benchmark.md#bc7).

Every table from Method to the alpha weights study measured bc7e before patch 0005. The patch 0005 sections measure the encoder Tessalith ships.

Tessalith encodes BC7 with bc7e (bc7enc_rdo b943862, patch 0001) at its `veryslow` level, which the
[encoder benchmark](encoder-benchmark.md#bc7-on-opaque-colour) found ties or beats NVTT Production on real textures. This study asks
whether bc7e, as vendored, can be made measurably better for Skyrim textures, and if so what it costs.
Everything here was measured on 18 October 2026 in a Linux container; see "How to read the timings".

## Method

1. **Baseline.** bc7e at Tessalith's settings (`veryslow`, linear weights, exact-opaque off) over the corpus.
2. **Headroom.** A near-exhaustive reference encoder on a 10-image subset. If bc7e is within 0.1 dB of it,
   there is nothing to gain in quality and the study stops.
3. **Targeted changes**, one at a time, each measured for quality against time: partition and mode search
   breadth, endpoint refinement, p-bit choice, per-channel weighting for normal maps.
4. **Recommendation**: adopt as a new off-by-default level, adopt as default (needs an owner ruling, since
   the goldens change), or drop.

### Corpus

Public, redistributable images only: Poly Haven textures, all CC0 (https://polyhaven.com/license), 1k PNG,
one asset per material family, fetched through the Poly Haven API on 18 October 2026. The download list with
Poly Haven's MD5, the SHA-256 of each download and the SHA-256 of each PNG written is
`docs/research/bc7e-tuning/MANIFEST.tsv` on branch `bench`.

| Class | n | Source | Metric |
|---|---|---|---|
| `diffuse` | 35 | the asset's `Diffuse` map, alpha 255 | RGB PSNR (luma PSNR also reported) |
| `normal` | 35 | the asset's `nor_dx` map (DirectX green, as Skyrim stores it), alpha 255 | mean angular error of the stored (x, y, z), both normalised |
| `cutout` | 15 | `Diffuse` RGB with alpha from the asset's `Displacement` (or `AO`) map thresholded at its median and blurred one texel, so the edge is anti-aliased | RGB PSNR, alpha PSNR |
| `smooth` | 15 | `Diffuse` RGB with alpha = the asset's `Displacement` (or `AO`) map, a smooth ramp | premultiplied RGB PSNR, alpha PSNR |

Poly Haven textures carry no alpha or opacity maps, so the two alpha classes are compositions of each asset's
own maps rather than real decals. They exercise the alpha paths (modes 4 to 7, rotations, the alpha index set)
with real colour and real alpha statistics, but their alpha is correlated with the colour in a way a hand-painted
decal's is not; treat the alpha-class numbers as a guide, not as decal results.

### Metrics

All scoring decodes with `bcdec_rs` and compares 8-bit code values with the source, mip 0 only, as the
encoder benchmark did. RGB PSNR over the three colour channels; luma PSNR over Rec. 709 luma of the decoded
8-bit values; alpha PSNR over the alpha channel; premultiplied PSNR over RGB × alpha / 255 on both sides; angular
error as the mean angle in degrees between the source and decoded vectors (x, y, z) = c / 255 × 2 − 1, each
normalised. Per class, the per-image difference against `base` is averaged with a 95% Student-t interval; a
change is called real only when the interval excludes 0.

### Tools

- `sdc-bench bc7-tune` (`crates/bench/src/bc7tune.rs`, on branch `bench` since Note FM8): encodes every image with named bc7e variants through
  `encoders_sys::bc7e_compress_blocks_tuned`, a measurement-only overlay on a level's parameters, and writes one
  CSV row per image and variant. `--reference N` adds the reference encoder. Main's `Bc7Tune` now holds only
  `weights`; the other knobs this study used (partitions, uber level, refinement, p-bit, endpoint search) live in
  git history before Note FZ and on branch `bench`.
- The reference encoder (`crates/bench/src/bc7ref.rs`) tries every mode, every partition (16 or 64), all four
  rotations of modes 4 and 5 and both index selectors of mode 4; fits each subset with a PCA start, iterated
  least squares under every p-bit combination, and a greedy ±1 local search over the quantised endpoints and
  p-bits with the indices re-assigned at every step; and gives the `N` best partitions per mode the full fit.
  It is exhaustive over structure and locally optimal over endpoints, not a global optimum. A test checks that its
  packer and error model agree with the decoder on 400 random blocks across all eight modes.
- `crates/encoders-sys/tests/level_identity.rs` pins the SHA-256 of every shipped level's output over four
  deterministic images, for both perceptual settings and both exact-opaque settings, so a patch or a rebuilt
  object cannot change a shipped level unnoticed.

### How to read the timings

The study ran on Linux objects built from the vendored `bc7e.ispc` with the same ISPC 1.31.0 and the same
flags as `tools/regen-ispc.ps1`, except `--target-os=linux --pic`. They do not reproduce the Windows objects'
output byte for byte ([correction](#correction-the-linux-objects-do-not-reproduce-the-windows-output)). Times are wall-clock seconds per megapixel on 4 cores with
rayon; only the ratios between variants carry over to a Windows machine. The Linux ISPC 1.31.0 binary cannot
emit Windows COFF objects (`--target-os` offers only linux, custom_linux, freebsd and android), so the vendored
objects were not rebuilt here.

## Results

Raw rows are on branch `bench`, under `docs/research/bc7e-tuning/`: `headroom.csv` and `headroom-attribution.csv` (step 2),
`normal-mode-transitions.csv`, `sweep-parameters.csv` (step 3, parameters only) and `sweep-patch0002.csv` (step 3,
with the patch). Δ is the per-image difference against `base` (Tessalith's `veryslow`), mean with a 95% t
interval; for angles the sign is flipped so + is better. Time is the summed encode time against `base`.

### 1. Baseline

bc7e `veryslow`, linear weights, over the corpus (n = 35, 35, 15, 15):

| Class | RGB PSNR (dB) | Luma PSNR | Alpha PSNR | Premultiplied | Angle (°) | s/Mpix (4 cores, Linux) |
|---|---|---|---|---|---|---|
| diffuse | 48.32 | 51.49 | — | — | — | 0.83 |
| normal | 40.56 | 42.90 | — | — | 1.587 | 0.79 |
| cutout | 45.94 | 49.03 | 55.66 | 62.57 | — | 0.55 |
| smooth | 46.31 | 49.48 | 51.46 | 46.84 | — | 0.65 |

### 2. Headroom against the reference (10 images)

| Class (n) | Metric | bc7e `veryslow` | bc7e `slowest` | Reference | Reference Δ vs base [95% CI] | Reference time |
|---|---|---|---|---|---|---|
| diffuse (3) | RGB dB | 49.307 | 49.292 | 49.289 | −0.018 [−0.088, +0.051] | 49× |
| normal (3) | angle ° | 2.296 | 2.289 | 2.226 | +0.070 [+0.021, +0.120] | 50× |
| normal (3) | RGB dB | 36.418 | 36.484 | 36.669 | +0.252 [+0.131, +0.373] | |
| cutout (2) | RGB dB | 45.811 | 45.817 | 45.913 | +0.103 [−0.146, +0.352] | 59× |
| smooth (2) | premul dB | 48.785 | 48.712 | 48.861 | +0.076 [−1.213, +1.366] | 57× |

- On colour, `veryslow` is already at the reference: within 0.02 dB, and the reference loses on two of the
  three diffuse images (bc7e's single-colour tables and selector-perturbation passes find solutions the
  reference's local search does not). By the study's rule there was nothing to gain on colour from search
  alone.
- On normal maps the reference is better on every image, by 0.07° and 0.25 dB RGB. The attribution puts
  roughly a third of the squared-error gain on blocks where the reference keeps bc7e's mode and partition
  (endpoint quality) and two thirds on blocks where it picks a different structure; of the structural gain,
  48% is mode 4 to mode 4 with a different rotation or index selector, 17% is mode 4 to a three-subset mode
  (0 or 2), 7% mode 5 to mode 4. Mode 4 at 5-bit endpoints is where rounding each channel on its own costs
  most, which pointed at the endpoint fit rather than at the mode decision.
- The alpha classes show about 0.1 dB with intervals that include 0 at n = 2.

### 3. Targeted changes

**Parameters only** (the sweep over all 100 images; no source change):

| Change | diffuse Δ RGB | normal Δ angle | normal Δ RGB | cutout Δ RGB | smooth Δ premul | Time |
|---|---|---|---|---|---|---|
| `slowest` (4 partitions, uber 4) | −0.023 [±0.009] | +0.009 [±0.005] | +0.078 | +0.012 [±0.007] | −0.028 [±0.033] | 1.3 to 1.7× |
| partitions tried 4 | −0.044 [±0.008] | +0.005 [±0.004] | +0.054 | −0.008 [±0.004] | −0.051 [±0.030] | 0.9 to 1.2× |
| partitions tried 16 | −0.018 [±0.012] | +0.014 [±0.010] | +0.142 | −0.006 [±0.004] | −0.020 [±0.025] | 1.4 to 2.8× |
| partitions tried 64 | −0.006 [±0.020] | +0.015 [±0.010] | +0.165 | −0.006 [±0.004] | −0.008 [±0.031] | 1.7 to 4.0× |
| uber 4 | +0.025 [±0.005] | +0.004 [±0.001] | +0.022 | +0.020 [±0.006] | +0.026 [±0.010] | 1.4× |
| 2 least-squares passes | +0.004 [±0.001] | +0.001 [±0.001] | +0.009 | +0.010 [±0.005] | +0.006 [±0.002] | 1.0 to 1.1× |
| p-bit search off | −0.165 [±0.037] | −0.002 [±0.001] | −0.018 | −0.099 [±0.050] | −0.105 [±0.054] | 0.85× |
| weights (2, 2, 1) for normals | −0.129 [±0.018] | +0.033 [±0.008] | −0.096 | (colour only) | | 1.0× |
| weights (4, 4, 1) for normals | −0.421 [±0.065] | +0.038 [±0.010] | −0.337 | | | 1.0× |
| partitions 64 + uber 4 | +0.014 [±0.020] | +0.019 [±0.010] | +0.189 | +0.014 [±0.007] | +0.014 [±0.031] | 2.4 to 5.8× |

- **Partition breadth as bc7e implements it does not pay on colour.** Once more than two partitions are
  tried, bc7e ranks them without refinement and refines only the winner (`disable_faster_part_selection`),
  so the ranking itself gets worse; 4 partitions lose 0.044 dB on diffuse. 64 partitions recover to the
  baseline at 3.4× the time. On normals breadth helps (+0.165 dB at 64), but at a high price.
- **Uber 4 is the one parameter worth having**: +0.02 to +0.03 dB on every colour class at 1.4×, 35 of 35
  diffuse images.
- **The p-bit search is already pulling its weight**: turning it off costs 0.10 to 0.17 dB.
- **Weighting X and Y over Z lowers the angular error of normals** (+0.033° at ×2, +0.038° at ×4) but lowers
  their RGB PSNR by 0.10 to 0.34 dB, that is, it moves error into Z. Skyrim reads the stored Z directly and
  does not rebuild it, so this trades the channel the shader uses for the metric; not adopted.
- **Endpoint refinement passes** add almost nothing: the least-squares fit converges in one pass.

**Patch 0002** (source changes behind new flags, measured over all 100 images):

| Change | diffuse Δ RGB | normal Δ angle | normal Δ RGB | cutout Δ RGB | smooth Δ premul | Time |
|---|---|---|---|---|---|---|
| endpoint search (`ep`) | +0.051 [±0.015] | +0.065 [±0.016] | +0.337 [±0.143] | +0.140 [±0.070] | +0.055 [±0.015] | 1.4 to 1.5× |
| refine partitions, 2 tried (`rp`) | 0 | 0 | 0 | 0 | 0 | 1.0× |
| refine partitions, 4 tried | +0.040 [±0.007] | +0.005 [±0.004] | +0.061 | +0.004 [±0.003] | +0.043 [±0.020] | 1.3 to 1.5× |
| refine partitions, 8 tried | +0.067 [±0.013] | +0.010 [±0.007] | +0.111 | +0.006 [±0.005] | +0.070 [±0.034] | 1.8 to 2.5× |
| `ep` + uber 4 | +0.073 [±0.014] | +0.064 [±0.015] | +0.339 | +0.155 [±0.068] | +0.077 [±0.011] | 1.8 to 1.9× |
| **`max`** = `ep` + refined 4 partitions + uber 4 + 2 passes | **+0.115 [±0.018]** | **+0.069 [±0.015]** | **+0.398 [±0.130]** | **+0.162 [±0.068]** | **+0.121 [±0.014]** | **2.4 to 2.9×** |
| `ep` + refined 8 partitions + uber 4 + 2 passes | +0.140 [±0.023] | +0.073 [±0.015] | +0.446 | +0.164 [±0.068] | +0.148 [±0.026] | 3.4 to 4.9× |

Every row above `rp` wins on 35 of 35 (normal, diffuse) and 15 of 15 (cutout, smooth) images; every interval
excludes 0.

- **The quantised endpoint search is the finding.** bc7e fits endpoints by least squares and rounds each
  channel on its own; a greedy ±1 walk over the quantised codes with the selectors re-assigned at each step
  (what the reference encoder does) gives nearly the whole reference gap on normals (+0.065° of the 0.070°
  measured on the subset, +0.34 dB RGB) and +0.05 to +0.14 dB on colour, for 40 to 50% more time. It also
  lifts the alpha PSNR of opaque normal maps from 81 to 87 dB, because more blocks land on p-bit choices that
  decode alpha as exactly 255.
- **Refining every ranked partition fixes the breadth lever.** With refinement, 4 partitions tried gains
  +0.040 dB on diffuse instead of losing 0.044; 8 tried gains +0.067 at 2.4×.
- **`max`** is the composition that stays under 3× `veryslow`: +0.12 dB RGB on diffuse (luma +0.09),
  +0.069° and +0.40 dB on normals, +0.16 dB on cutouts (alpha +0.23), +0.12 dB premultiplied on smooth alpha
  (alpha +0.27). The 8-partition composition adds a further 0.02 to 0.05 dB for 1.6× more time; not taken.
- For comparison, the encoder benchmark put NVTT Production at −0.27 dB mean / −0.03 dB median against bc7e
  on colour at about 40 to 60× the time, and ahead by 0.3 to 0.4 dB on linear data. `max` reaches a similar
  gain on data-like content (normals) at 2.7×.

### 4. Recommendation

| Option | Verdict |
|---|---|
| Adopt `max` as a new, off-by-default level | **Yes.** Done in this branch: `encoder.effort = "max"`, bc7e patch 0002, `Bc7Level::Max`. Default output is byte-identical (the level-identity test pins every shipped level; the goldens do not move). |
| Adopt the endpoint search in the default (`veryslow`) | **Owner ruling needed.** It is the cheapest of the gains: +0.05 dB colour, +0.065° / +0.34 dB normals, +0.14 dB cutouts at 1.4× the time, 100 of 100 images. It changes every BC7 output, so the goldens, the version and an `if-changed` re-encode of every recorded output follow. Recorded here, not done. |
| Partition breadth without refinement (the `slowest` level's approach) | Drop as a lever; keep `slowest` as it is for compatibility. |
| Per-channel weights for normals | Drop: lower angular error but worse Z, which Skyrim reads. |
| More least-squares passes | Drop: converges in one. |

## What Tessalith does because of this

- **A new `max` effort level** (`encoder.effort = "max"`, DECISIONS Note BZ) maps to bc7e's
  `bc7e_compress_block_params_init_sdc_max`: `veryslow`'s search with 4 partitions tried and fully refined,
  uber 4, two least-squares passes and the quantised endpoint search. It is off by default and about 2.7× the
  time of `veryslow`. BC6H under `max` uses the peak-preserving ISPC `veryslow`, as `slowest` does.
- **The default is unchanged.** `crates/encoders-sys/tests/level_identity.rs` pins the output of every shipped
  level for both perceptual and both exact-opaque settings; the patch leaves all of them byte-identical, and
  `cargo test -p engine --test golden` passes.
- **The patch is `crates/encoders-sys/vendor/bc7e/patches/0002-sdc-max-level.patch`**, described in
  [`README-VENDOR.md`](../../crates/encoders-sys/vendor/README-VENDOR.md). The vendored Windows objects were
  rebuilt with it on 2026-10-18 (`tools/regen-ispc.ps1 -Update -Bc7eOnly`, then `tools/hash-vendor.ps1`).
  `level_identity.rs` pins every level per ISPC target (see the Windows re-check below).
- **The measurement path** (`sdc-bench bc7-tune` and the reference encoder) lives on branch `bench` (Note FM8).
- **Per-channel weights** are opt-in as `encoder.bc7_weights` (the GUI's *BC7 weighting*, DECISIONS Note CP),
  from the [alpha-weighting study](#bc7-alpha-weights-against-bc3-2026-10-21) on branch `research/bc7-alpha-vs-bc3`.

## Windows re-check on Skyrim textures (2026-10-18)

Question: does the endpoint search (`ep`) hold up as a default-`veryslow` change on real Skyrim textures,
with the Windows objects? Answer under the owner's acceptance rule: **no, it does not hold**. It wins on
nearly every image, but two of the four criteria fail. The default stays as it is.

### Setup

- **Objects.** The vendored Windows objects with patch 0002 (commit `79dcbb2`), ISPC dispatch on this
  machine's CPU, then the avx2 path (the objects had four targets).
  `sdc-bench bc7-tune --variants base,ep --repeats 5`: the fastest of 5 runs is kept, rayon on all cores, on a
  machine with other builds running. `base` and `ep` run back to back per image.
- **Sample.** Drawn from the owner's set of 10,000 PNGs, taken from installed mods and not redistributable.
  The set is not in the repo, and paths are relative to its root. Every PNG is listed in sorted order, and
  the list is split into two pools:
  - **normal:** the stem ends in `_n` or `_normal` (2,515 files);
  - **colour:** no non-colour suffix (`_n`, `_normal`, `_nrm`, `_norm`, `_msn`, `_rmaos`, `_p`, `_s`, `_f`,
    `_g`, `_sk`, `_mask`, `_m`, `_e`, `_em`, `_h`, `_coat`, `_b`, `_i`, `_cnr`), 2,823 files.

  Each pool is shuffled with Python's `random.Random(20261018)`, and files are taken in that order. Sizes that
  are not multiples of 4 are skipped.
  - **Normal:** the first 60.
  - **Colour, opaque:** fewer than 1% of texels have alpha below 255. The first 60 go to `diffuse`.
  - **Colour, alpha-bearing:** the first 30. These go to `cutout` when at least 90% of texels have alpha
    exactly 0 or 255, and to `smooth` otherwise. That gives 16 cutout and 14 smooth.

  The list is `docs/research/bc7e-tuning/skyrim-sample.tsv` on branch `bench`: class, corpus name, relative path, SHA-256 and alpha
  statistics. The raw rows are in `.superpowers/bc7e-ep-verify/skyrim-base-ep.csv`, which is gitignored.
- **Metrics:** as in the study (Metrics above). Δ = `ep` − `base` per image, averaged per class, with a 95%
  Student-t interval. For angles the sign is flipped, so + is better. Time is the summed `ep` time over the
  summed `base` time.
- **Differences from the study's corpus.**
  - Skyrim `_n` maps keep specular in alpha, while the study's normals had alpha 255. Part of each block's
    effort goes to alpha.
  - Many Skyrim diffuse alphas are masks, not decals.
  - A few textures are near-flat, with a base PSNR above 60 dB. On those a tiny absolute error change shows
    up as a very large PSNR change. Two diffuse images gain +10.9 and +21.1 dB, and one normal map gains
    +32.0 dB RGB.

### Results

| Class | n | Primary Δ [95% CI] | Wins | Second Δ [95% CI] | Time | Study (Linux, Poly Haven) |
|---|---|---|---|---|---|---|
| diffuse | 60 | RGB +0.645 [±0.784] (−0.139 to +1.429) | 60/60 | luma +0.892 [±1.128] | 1.37× | RGB +0.051 [±0.015] |
| normal | 60 | angle +0.0285° [±0.0048] (+0.0236 to +0.0333) | 59/60, 1 tie | RGB +1.081 [±1.075] | 1.45× | +0.065° [±0.016], RGB +0.337 [±0.143] |
| cutout | 16 | RGB +0.133 [±0.142] (−0.009 to +0.275) | 16/16 | alpha +0.102 [±0.064] | 1.39× | RGB +0.140 [±0.070] |
| smooth | 14 | premul +0.204 [±0.088] | 12/14, 2 ties | alpha +0.216 [±0.144] | 1.50× | premul +0.055 [±0.015] |

No image got worse on its class's primary metric. All four classes together take 1.41× the time.

### Verdict under the owner's rule

| Criterion | Result | |
|---|---|---|
| Diffuse Δ RGB > 0, CI excludes 0, at least +0.025 dB | +0.645, CI −0.139 to +1.429 | **fails** (CI includes 0) |
| Normal Δ angle > 0, CI excludes 0, at least +0.03° | +0.0285°, CI +0.0236 to +0.0333 | **fails** (below +0.03°) |
| No class with mean Δ < 0 and CI excluding 0 | all means positive | passes |
| Time ratio at most 1.7× | at most 1.50× | passes |

**The claim does not hold on this sample, so `veryslow` is unchanged.**

Context for a re-ruling (not part of the rule):
- **The diffuse failure comes from the two near-flat images.** The diffuse median Δ is +0.056 dB and the 10%
  trimmed mean is +0.083 dB. With images over 60 dB base excluded (n = 54), the mean is +0.079 [±0.019] dB.
- **The normal shortfall is 0.0015°.** The median is +0.026° and the trimmed mean +0.027°. With images over
  60 dB base excluded (n = 53), the mean is +0.0313° [±0.0049]. It is about half the study's +0.065°, which
  fits the alpha (specular) channel taking part of the effort.
- **Cutout and smooth.** Without the over-60 dB images, cutout gives +0.048 [±0.011] and smooth +0.238
  [±0.079].

### Correction: the Linux objects do not reproduce the Windows output

The study first assumed its Linux objects reproduce the vendored objects' output byte for byte. On Windows
they do not.
- None of the 32 level hashes recorded on Linux matches any of the four Windows ISPC code paths. The level
  test therefore failed on `main` with the shipped objects, while the goldens passed.
- On this machine the sse2, sse4 and avx paths encode alike, and the avx2 path (FMA) encodes differently. So
  bc7e output depends on the CPU the encode runs on.
- `crates/encoders-sys/tests/level_identity.rs` now pins every level per target (commit `6ef3bbf`).
  Patch 0002 leaves levels 0 to 6 byte-identical on all four targets (commit `79dcbb2`).
- A fifth code path, avx512, was added on 2026-11-13. It encodes differently from the avx2 path too, and
  `level_identity.rs` pins it in a row of its own: [The AVX-512 ISPC path](avx512-ispc.md).
- The study's quality deltas are unaffected, since each one compares two variants built from the same
  objects.

### `max` against `veryslow` and `slowest` on the same sample

Same objects, sample and metrics as above, avx2 path, rayon on all 16 threads. Two timing methods:
- **Alternating:** `--variants base,max,slowest` repeated three times with `--repeats 1`, so each image runs
  base, max, slowest, base, max, slowest, and so on. The fastest of the three is kept.
- **Separate:** one run per variant over the whole sample, `--repeats 3`, fastest kept, run back to back.

Per-image rows: `docs/research/bc7e-tuning/skyrim-max.csv` on branch `bench` (scores, fastest alternating time, fastest separate time).
Δ is `max` − `base` on the class's primary metric (angle sign flipped), 95% t interval.

| Class | n | Δ [95% CI] | Wins | `max` s/Mpix (alt / sep) | Ratio to `veryslow` (alt / sep) | `slowest` ratio (alt / sep) |
|---|---|---|---|---|---|---|
| diffuse | 60 | RGB +0.762 [±0.783]; +0.178 [±0.026] without images over 60 dB | 60/60 | 0.421 / 0.379 | 2.68× / 2.70× | 1.61× / 1.61× |
| normal | 60 | angle +0.0338° [±0.0050] | 59/60, 1 tie | 0.517 / 0.476 | 2.84× / 3.18× | 1.64× / 1.62× |
| cutout | 16 | RGB +0.287 [±0.268] | 16/16 | 0.399 / 0.464 | 2.54× / 3.03× | 1.50× / 1.51× |
| smooth | 14 | premul +0.227 [±0.093] | 12/14, 2 ties | 0.212 / 0.243 | 2.43× / 2.81× | 1.33× / 1.38× |
| all | 150 | | | 0.438 / 0.414 | **2.73× / 2.95×** | 1.59× / 1.59× |

- `veryslow` ran at 0.161 s/Mpix alternating and 0.141 s/Mpix separate. These are kernel times on all
  cores, not the file-to-file times of the [encoder benchmark](encoder-benchmark.md).
- **Load.** The machine was in use. A cargo build ran during about half of the alternating run (up to 6
  rustc processes; CPU 14% busy just before the run). The separate runs had no cargo or rustc running
  (CPU 2 to 10% busy before each). Under that build both `base` and `max` slowed, and the ratio came out
  lower. Both ratios are within 15% of the expected ~3×, and the alternating one is inside the study's 2.4 to
  2.9×. The separate ratio is just above it. Expect roughly 3× the `veryslow` time; yours may differ under load.
- On this sample `slowest` is worse than `veryslow` on diffuse (RGB −0.175 [±0.098], 7/60 wins) and slightly
  better on normals (+0.0072°). `max` beats `slowest` on every class: diffuse RGB +0.323 [±0.122] without
  images over 60 dB, normals +0.0266° [±0.0043], alpha PSNR +0.320 [±0.125] over the 30 alpha textures.

## BC7 alpha weights against BC3 (2026-10-21)

Measured before patch 0005, with weights 1, 1, 1, w; [x16 on colour and the alpha weights again](#x16-on-colour-and-the-alpha-weights-again-2026-11-24-note-lr) supersedes it. Its alpha SSIM and premultiplied columns were not measured again.

Written 2026-10-21 (commit dbf54808).

Source: local study, 2026-10-21, and DECISIONS Note CP. It asks whether BC7 with a higher alpha error weight can replace BC3 for textures with delicate alpha, and keep BC7's better RGB. BC7 is the built-in bc7e at its default level (veryslow), with linear weights 1,1,1,w for w = 1, 2, 4, 8 and 16, and in a follow-up on 2026-10-22, 32 and 64. BC3 is the bundled rgbcx (level 18, `-r`). w1 is today's Linear. The dropdown's options, and the chart, are in the [GUI guide](../wiki/GUI-Texture-Panel.md#bc7-weighting-against-bc3).

These tables copy the study's values. They leave out the 95% confidence intervals and the per-image worst cases that the study also reports. PSNR is in dB, alpha SSIM is 0 to 1, alpha max error is in codes, and exact-0 and exact-255 are the share of source texels at 0 or 255 that decode exactly. Each *Means* table is the mean over the class. Each *Where BC7 beats BC3* table counts the images where the BC7 setting beats BC3.

### Answer in one table

| Class | n | BC3 alternative to offer | What it gives against BC3 | Ranked options |
|---|---|---|---|---|
| `alpha_real` (colour + smooth alpha) | 96 | **BC7 w8** | Alpha PSNR better on 85/96 (mean +4.2 dB), alpha SSIM better on 77/96, max alpha error 3.5 codes lower, RGB +5.6 dB (better on 88/96), premultiplied +3.7 dB | 1. BC7 w8. 2. BC7 w4 (milder: SSIM 62/96, RGB +6.1 dB on 94/96). 3. BC3 kept as a manual choice for faint, low-contrast alpha such as fingerprint decals: no weight up to 16 closes them. bc7e perceptual is beaten by w8 on every mean. Where alpha must decode to exactly 0 and 255, NVTT-style BC3 still wins on every image. |
| `n_spec` (`_n`, specular in alpha), mip 0 and mip 2 | 30 + 30 | **BC7 w16**, a compromise | Normal error still 0.28° lower than BC3 (24/30). Specular is 1.1 dB below BC3 (BC7 wins 10/30) and alpha SSIM 0.993 against 0.997. Mip 2 gives the same picture. | 1. BC7 w16. 2. BC3, when specular fidelity matters more than the normal: +8.8 dB alpha over today's BC7, for 1.1° more normal error (+67%). 3. BC7 as today: the best normals, but specular at 37 dB with a mean worst-texel error of 102 codes. |
| `rmaos_spec` (`_rmaos`, specular in alpha) | 24 | **BC7 w8**, not BC3 | Against today's BC7: +4.8 dB specular and a mean worst-texel error of 35 → 18 codes, for −1.0 dB RGB. It is still +5.6 dB RGB over BC3. | 1. BC7 w8. 2. BC7 as today. 3. BC3 only if exact specular 255 matters: BC3 keeps 91% of 255 texels exact, BC7 65 to 73%. BC3 costs 6.5 dB on roughness and 6.2 dB on AO. |
| `m_height` (complex `_m`, height in alpha) | 30 (proxy) | **BC3 still** (see Follow-up) | Measured on a proxy: real `_rmaos` RGB plus real `_p` height. BC7 w16 is still −2.25 dB height (9/30 wins) and loses exact 0/255 on nearly every image, for +3.4 dB RGB. | 1. BC3. 2. BC7 w16. 3. BC7 today (−8.5 dB height). |

**Weights ×32 and ×64 (follow-up, 2026-10-22).** No weight up to 64 beats BC3 on alpha for every texture in any class. Alpha keeps rising with each doubling and never plateaus, and each doubling costs 0.25 to 0.55 dB of RGB or about 0.17° of normal angle.

- On `n_spec`, w64 passes BC3's mean specular, but its normals are by then worse than BC3's.
- On `rmaos_spec` and `m_height`, BC3 still leads on alpha SSIM on 23/24 and 27/30 textures.
- On `alpha_real`, w32 and w64 add only +0.3 and +0.45 dB of alpha over w16.

Since 2026-11-22 (owner) the dropdown also offers ×4, ×32 and ×64; the measured gains above stand as written.

Follow-up on `m_height`:

| Item | Headline | Options, ranked |
|---|---|---|
| `m_height` (real `_rmaos` RGB + real `_p` height in alpha, n = 30) | **BC3 still.** No BC7 weight up to 16 reaches BC3 on height. w16 is −2.25 dB alpha PSNR (9/30 wins), loses alpha SSIM on 28/30 and loses exact-0/255 on all but one image. BC3 keeps 98 to 100% of the 0 and 255 texels exact; BC7 keeps 75 to 97%. BC3 costs 6.1 dB RGB. | 1. BC3 (rgbcx or NVTT: rgbcx has +0.7 dB alpha PSNR, NVTT pins 100% of extremes). 2. BC7 w16 as a compromise: −2.3 dB height, +3.4 dB RGB over BC3. 3. BC7 today: −8.5 dB height, +6.1 dB RGB. Exact-opaque adds almost nothing here (+4.6 pt exact-255, mean). |

### Colour with smooth alpha (`alpha_real`, 96 textures)

Means:

| Setting | RGB PSNR | premul PSNR | alpha PSNR | alpha SSIM | alpha max err | exact-0 share (%) | exact-255 share (%) |
|---|---|---|---|---|---|---|---|
| BC3 rgbcx L18 -r | 41.00 | 49.17 | 47.24 | 0.9978 | 17.35 | 89.35 | 55.87 |
| BC3 round-3 rgbcx -r | 41.00 | 49.04 | 46.43 | 0.9967 | 17.59 | 89.72 | 56.84 |
| BC3 round-3 NVTT -highest | 40.96 | 49.29 | 47.38 | 0.9976 | 17.90 | 100.00 | 100.00 |
| BC7 w1 (today) | 48.40 | 53.04 | 47.93 | 0.9951 | 22.27 | 88.35 | 54.71 |
| BC7 w2 | 47.74 | 53.29 | 49.39 | 0.9970 | 18.26 | 90.95 | 61.87 |
| BC7 w4 | 47.13 | 53.17 | 50.56 | 0.9979 | 15.96 | 92.13 | 63.69 |
| BC7 w8 | 46.60 | 52.87 | 51.40 | 0.9984 | 13.91 | 92.69 | 67.00 |
| BC7 w16 | 46.17 | 52.53 | 51.93 | 0.9987 | 13.02 | 93.06 | 66.01 |
| BC7 w32 | 45.84 | 52.23 | 52.23 | 0.9989 | 12.42 | 93.06 | 66.39 |
| BC7 w64 | 45.58 | 52.00 | 52.38 | 0.9990 | 12.24 | 93.08 | 66.37 |
| BC7 perceptual | 46.54 | 52.81 | 51.17 | 0.9983 | 14.39 | 92.70 | 65.27 |

Where BC7 beats BC3:

| Setting | alpha PSNR wins | alpha SSIM wins | both | RGB PSNR Δ vs BC3 | RGB PSNR wins | alpha PSNR Δ | alpha SSIM Δ (×1000) |
|---|---|---|---|---|---|---|---|
| BC7 w1 (today) | 61/96 | 30/96 | 30/96 | +7.40 dB | 96/96 | +0.69 | -2.65 |
| BC7 w2 | 73/96 | 46/96 | 46/96 | +6.74 dB | 95/96 | +2.14 | -0.77 |
| BC7 w4 | 77/96 | 62/96 | 62/96 | +6.13 dB | 94/96 | +3.32 | +0.17 |
| BC7 w8 | 85/96 | 77/96 | 77/96 | +5.61 dB | 88/96 | +4.16 | +0.67 |
| BC7 w16 | 86/96 | 80/96 | 80/96 | +5.18 dB | 84/96 | +4.69 | +0.97 |
| BC7 w32 | 87/96 | 82/96 | 82/96 | +4.84 dB | 82/96 | +4.98 | +1.14 |
| BC7 w64 | 87/96 | 82/96 | 82/96 | +4.59 dB | 79/96 | +5.14 | +1.24 |
| BC7 perceptual | 82/96 | 71/96 | 71/96 | +5.54 dB | 91/96 | +3.93 | +0.54 |

### Normal map with specular in alpha (`n_spec`, 30 textures, mip 0)

Means:

| Setting | angle | RGB PSNR | alpha PSNR | alpha SSIM | alpha max err | exact-0 share (%) | exact-255 share (%) |
|---|---|---|---|---|---|---|---|
| BC3 rgbcx L18 -r | 2.806 | 33.47 | 46.06 | 0.9966 | 16.73 | 47.08 | 47.97 |
| BC7 w1 (today) | 1.682 | 37.54 | 37.24 | 0.9537 | 102.47 | 83.07 | 37.14 |
| BC7 w2 | 1.868 | 36.79 | 39.28 | 0.9695 | 75.47 | 83.09 | 37.91 |
| BC7 w4 | 2.081 | 36.00 | 41.39 | 0.9807 | 55.90 | 81.60 | 43.31 |
| BC7 w8 | 2.308 | 35.22 | 43.36 | 0.9883 | 41.10 | 80.19 | 47.44 |
| BC7 w16 | 2.523 | 34.53 | 44.96 | 0.9929 | 32.17 | 76.90 | 51.26 |
| BC7 w32 | 2.706 | 33.95 | 46.02 | 0.9953 | 24.87 | 75.62 | 54.68 |
| BC7 w64 | 2.851 | 33.49 | 46.64 | 0.9963 | 19.47 | 74.28 | 53.82 |

Where BC7 beats BC3:

| Setting | alpha PSNR wins | alpha SSIM wins | both | angle Δ vs BC3 | angle wins | alpha PSNR Δ | alpha SSIM Δ (×1000) |
|---|---|---|---|---|---|---|---|
| BC7 w1 (today) | 0/30 | 0/30 | 0/30 | +1.124 ° | 30/30 | -8.83 | -42.94 |
| BC7 w2 | 0/30 | 0/30 | 0/30 | +0.938 ° | 29/30 | -6.78 | -27.09 |
| BC7 w4 | 1/30 | 0/30 | 0/30 | +0.725 ° | 29/30 | -4.67 | -15.95 |
| BC7 w8 | 3/30 | 1/30 | 1/30 | +0.498 ° | 27/30 | -2.70 | -8.30 |
| BC7 w16 | 10/30 | 3/30 | 3/30 | +0.283 ° | 24/30 | -1.11 | -3.70 |
| BC7 w32 | 17/30 | 6/30 | 6/30 | +0.100 ° | 16/30 | -0.04 | -1.37 |
| BC7 w64 | 21/30 | 13/30 | 13/30 | -0.045 ° | 11/30 | +0.58 | -0.34 |

### The same normal maps at mip 2 (`n_spec_mip2`, 30 textures)

Means:

| Setting | angle | RGB PSNR | alpha PSNR | alpha SSIM | alpha max err | exact-0 share (%) | exact-255 share (%) |
|---|---|---|---|---|---|---|---|
| BC3 rgbcx L18 -r | 3.560 | 31.62 | 44.29 | 0.9962 | 13.23 | 83.60 | 31.58 |
| BC7 w1 (today) | 2.212 | 35.40 | 34.79 | 0.9423 | 64.37 | 86.63 | 30.27 |
| BC7 w2 | 2.496 | 34.49 | 37.18 | 0.9664 | 49.47 | 89.06 | 32.62 |
| BC7 w4 | 2.788 | 33.67 | 39.58 | 0.9808 | 37.60 | 92.39 | 59.13 |
| BC7 w8 | 3.052 | 33.00 | 41.59 | 0.9886 | 25.70 | 92.56 | 54.53 |
| BC7 w16 | 3.270 | 32.44 | 42.95 | 0.9925 | 20.23 | 94.48 | 55.51 |
| BC7 w32 | 3.460 | 31.94 | 43.80 | 0.9946 | 15.87 | 95.81 | 57.35 |
| BC7 w64 | 3.640 | 31.45 | 44.38 | 0.9956 | 14.40 | 94.86 | 57.27 |

Where BC7 beats BC3:

| Setting | alpha PSNR wins | alpha SSIM wins | both | angle Δ vs BC3 | angle wins | alpha PSNR Δ | alpha SSIM Δ (×1000) |
|---|---|---|---|---|---|---|---|
| BC7 w1 (today) | 0/30 | 0/30 | 0/30 | +1.347 ° | 30/30 | -9.51 | -53.87 |
| BC7 w2 | 0/30 | 0/30 | 0/30 | +1.064 ° | 30/30 | -7.11 | -29.79 |
| BC7 w4 | 1/30 | 0/30 | 0/30 | +0.772 ° | 29/30 | -4.71 | -15.38 |
| BC7 w8 | 1/30 | 1/30 | 1/30 | +0.508 ° | 25/30 | -2.70 | -7.62 |
| BC7 w16 | 8/30 | 5/30 | 4/30 | +0.290 ° | 22/30 | -1.35 | -3.63 |
| BC7 w32 | 13/30 | 9/30 | 9/30 | +0.100 ° | 18/30 | -0.49 | -1.59 |
| BC7 w64 | 19/30 | 12/30 | 12/30 | -0.080 ° | 13/30 | +0.09 | -0.59 |

### Packed `_rmaos` with specular in alpha (`rmaos_spec`, 24 textures)

Means:

| Setting | R PSNR | G PSNR | B PSNR | RGB PSNR | alpha PSNR | alpha SSIM | alpha max err | exact-0 share (%) | exact-255 share (%) |
|---|---|---|---|---|---|---|---|---|---|
| BC3 rgbcx L18 -r | 37.54 | 51.51 | 38.49 | 38.64 | 58.98 | 0.9995 | 11.42 | 65.82 | 91.20 |
| BC7 w1 (today) | 44.00 | 53.12 | 44.64 | 45.27 | 49.92 | 0.9908 | 35.46 | 97.09 | 64.80 |
| BC7 w2 | 43.71 | 52.50 | 44.36 | 44.95 | 51.79 | 0.9939 | 28.58 | 96.44 | 67.65 |
| BC7 w4 | 43.39 | 51.72 | 44.08 | 44.62 | 53.36 | 0.9959 | 23.33 | 95.69 | 70.44 |
| BC7 w8 | 43.02 | 51.05 | 43.78 | 44.25 | 54.74 | 0.9972 | 17.67 | 95.14 | 72.16 |
| BC7 w16 | 42.58 | 50.50 | 43.42 | 43.82 | 56.07 | 0.9981 | 15.33 | 78.66 | 73.05 |
| BC7 w32 | 42.05 | 50.09 | 43.01 | 43.32 | 57.31 | 0.9987 | 13.79 | 78.52 | 73.43 |
| BC7 w64 | 41.47 | 49.70 | 42.57 | 42.77 | 58.51 | 0.9991 | 13.12 | 78.14 | 74.13 |
| BC7 w8 + exact opaque | | | | 44.25 | 55.01 | 0.9973 | 17.67 | 95.14 | 91.64 |
| BC7 w16 + exact opaque | | | | 43.82 | 56.43 | 0.9982 | 15.33 | 78.66 | 92.53 |
| BC7 w64 + exact opaque | | | | 42.77 | 59.10 | 0.9992 | 13.12 | 78.14 | 93.60 |

Where BC7 beats BC3:

| Setting | alpha PSNR wins | alpha SSIM wins | both | RGB PSNR Δ vs BC3 | RGB PSNR wins | alpha PSNR Δ | alpha SSIM Δ (×1000) |
|---|---|---|---|---|---|---|---|
| BC7 w1 (today) | 0/24 | 0/24 | 0/24 | +6.63 dB | 24/24 | -9.06 | -8.75 |
| BC7 w2 | 2/24 | 0/24 | 0/24 | +6.31 dB | 24/24 | -7.19 | -5.59 |
| BC7 w4 | 2/24 | 1/24 | 1/24 | +5.98 dB | 23/24 | -5.62 | -3.61 |
| BC7 w8 | 4/24 | 1/24 | 1/24 | +5.61 dB | 23/24 | -4.24 | -2.36 |
| BC7 w16 | 5/24 | 1/24 | 1/24 | +5.18 dB | 22/24 | -2.91 | -1.46 |
| BC7 w32 | 6/24 | 1/24 | 1/24 | +4.68 dB | 22/24 | -1.67 | -0.83 |
| BC7 w64 | 11/24 | 1/24 | 1/24 | +4.13 dB | 22/24 | -0.47 | -0.42 |

Why BC3 keeps the lead on this class, even at w64:

- **BC3 gives alpha its own half of the block.** Its alpha is a separate 64-bit block with two 8-bit endpoints and 8 levels between them, so it never competes with colour.
- **BC7 fits all four channels into one 128-bit block.** Its alpha either shares the colour's indices (mode 6) or gets its own indices with only 4 levels (mode 5). The weight only changes which error the encoder prefers; it adds no bits.
- **The RGB here is three unrelated maps** (roughness, metal, AO), so colour already needs most of BC7's bits. That is also why BC7's RGB lead is so large (+6.6 dB at w1).
- **The specular is smooth and sits near 255.** BC4-style alpha fits that well: BC3's mean is 58.98 dB, and it keeps 91% of 255 texels exact against BC7's 65 to 74% at every weight.

BC3 pays 4 to 7 dB of RGB for it, mostly in roughness and AO. The mode explanation follows from the formats; the study did not count which modes bc7e chose.

**With Exact opaque alpha** (follow-up, 2026-10-22): the flag pins blocks whose alpha is 255 everywhere, which is 9% of blocks in this class.
- **Alpha ×8 + exact opaque** keeps 91.6% of specular-255 texels exact against BC3's 91.2%, and keeps +5.6 dB of RGB.
- At w64 the flag lifts mean alpha to 59.10 dB against BC3's 58.98, winning 12/24 images.
- BC3 still wins alpha SSIM on 22/24.
- The flag leaves RGB unchanged here (R/G/B columns left blank: same as the row without it).

Over all classes: no weight with the flag beats BC3 on every image. Exact-255 reaches BC3's level on `m_height` from w32 (99.6%). On `n_spec` the flag does nothing, because there are almost no all-255 blocks. On opaque `_rmaos` it costs 0.20 dB RGB.

### Complex `_m` with height in alpha (`m_height`, 30 textures, proxy)

Means:

| Setting | R PSNR | G PSNR | B PSNR | RGB PSNR | alpha PSNR | alpha SSIM | alpha max err | exact-0 share (%) | exact-255 share (%) |
|---|---|---|---|---|---|---|---|---|---|
| BC3 rgbcx L18 -r | 37.94 | 65.98 | 37.51 | 38.67 | 53.81 | 0.9990 | 9.90 | 98.07 | 98.99 |
| BC3 NVTT -highest | 37.93 | 65.98 | 37.50 | 38.66 | 53.14 | 0.9988 | 10.67 | 100.00 | 100.00 |
| BC7 w1 (today) | 43.77 | 52.71 | 43.65 | 44.75 | 45.27 | 0.9850 | 36.13 | 74.98 | 86.43 |
| BC7 w2 | 43.24 | 51.38 | 43.03 | 44.11 | 47.02 | 0.9897 | 29.53 | 83.66 | 88.66 |
| BC7 w4 | 42.64 | 50.32 | 42.34 | 43.41 | 48.73 | 0.9929 | 21.77 | 93.91 | 90.89 |
| BC7 w8 | 42.03 | 49.61 | 41.62 | 42.70 | 50.27 | 0.9952 | 18.13 | 95.69 | 90.63 |
| BC7 w16 | 41.44 | 49.13 | 40.94 | 42.02 | 51.56 | 0.9969 | 14.27 | 97.13 | 93.80 |
| BC7 w32 | 40.94 | 48.82 | 40.33 | 41.44 | 52.46 | 0.9980 | 12.43 | 98.15 | 95.05 |
| BC7 w64 | 40.52 | 48.41 | 39.84 | 40.95 | 52.99 | 0.9984 | 11.70 | 98.76 | 95.27 |
| BC7 w1 + exact-opaque | 43.77 | 52.71 | 43.65 | 44.75 | 45.29 | 0.9851 | 36.13 | 74.98 | 90.99 |
| BC7 [4,1,2,1] | 46.09 | 48.83 | 43.49 | 45.10 | 43.14 | 0.9773 | 48.03 | 67.90 | 83.16 |

Where BC7 beats BC3:

| Setting | alpha PSNR wins (rgbcx / NVTT) | alpha SSIM wins | exact-0 wins | exact-255 wins | RGB Δ vs rgbcx | RGB wins |
|---|---|---|---|---|---|---|
| BC7 w1 (today) | 0/30 / 0/30 | 0/30 / 0/30 | 0/14 / 0/14 | 0/15 / 0/15 | +6.08 dB | 30/30 |
| BC7 w2 | 0/30 / 1/30 | 0/30 / 0/30 | 0/14 / 0/14 | 0/15 / 0/15 | +5.44 dB | 30/30 |
| BC7 w4 | 2/30 / 1/30 | 1/30 / 1/30 | 1/14 / 0/14 | 0/15 / 0/15 | +4.75 dB | 29/30 |
| BC7 w8 | 4/30 / 3/30 | 1/30 / 2/30 | 1/14 / 0/14 | 0/15 / 0/15 | +4.04 dB | 29/30 |
| BC7 w16 | 9/30 / 11/30 | 2/30 / 3/30 | 1/14 / 0/14 | 1/15 / 0/15 | +3.35 dB | 27/30 |
| BC7 w32 (rgbcx only) | 13/30 | 2/30 | — | — | +2.77 dB | 26/30 |
| BC7 w64 (rgbcx only) | 16/30 | 3/30 | 11/14 | — | +2.28 dB | 24/30 |
| BC7 w1 + exact-opaque | 0/30 / 0/30 | 0/30 / 0/30 | 0/14 / 0/14 | 0/15 / 0/15 | +6.08 dB | 30/30 |
| BC7 [4,1,2,1] | 0/30 / 0/30 | 0/30 / 0/30 | 0/14 / 0/14 | 0/15 / 0/15 | +6.43 dB | 30/30 |

The `m_height` rows are a proxy: a real `_rmaos` for RGB plus a real `_p` height written into alpha. Its Means table also lists NVTT BC3, BC7 w1 with exact-opaque, and a `[4,1,2,1]` weight mode that the owner dropped (Note CP12). In the `m_height` wins table, BC3 means rgbcx / NVTT.

## Kernel fixes (patch 0005, 2026-11-25, Note LR)

Measured 2026-11-23 to 2026-11-25 on Windows, on the vendored objects built from `bc7e.ispc` with patches 0001, 0002 and 0005 (ISPC 1.31.0; the AVX-512 object with 1.32.0), on an AMD CPU. Every section above this one measured bc7e before patch 0005.

Patch 0005 fixes the defects an audit of `bc7e.ispc` found, adds the kernel side of the channel guard, and makes every linear BC7 encode rank its errors as the same encode with every weight ×16 would. It changes the bytes of every level. [README-VENDOR](../../crates/encoders-sys/vendor/README-VENDOR.md) describes each change with its proof test; DECISIONS Note LR holds the rulings.

### What changed

| Item | What it does | Which bytes change |
|---|---|---|
| S1 | Mode 4 writes the end state of an alpha slot that is 255 on every texel without running its search. | None |
| Q2t | A refit spreads a channel's endpoints only when they differ by more than a flat tolerance, set per call ([its values](#q2ts-threshold)). Without it, float noise spread constant channels, and a rotated opaque block's alpha decoded 239. | Every level |
| Q1, Q4 | Mode 7 scores its one-colour candidate on the values the decoder reads, and its table keeps each p-bit pair's own best entry. | Alpha blocks, `veryfast` and above |
| XC | The channel guard: a subset whose guarded channel is 0 or 255 on every texel keeps that value wherever the search finds such a solution. Guarded channels that cannot all be kept keep the higher one: A, then G, then B, then R. A is exact opaque, bit 3 of the guard's mask. | Only with a channel guarded (A included) |
| Q3 | With unequal linear weights (`alphaN`), mode 6 picks an alpha block's indices in the weighted metric over all 16 palette entries (the window `full`). | `alphaN` only |
| DG | Four divisions by a flat subset's zero spread are guarded; the result is the value the clamp already gave. | None |
| WP | With unequal linear weights, the mode 6 and 7 one-colour packers choose their p-bits by the weighted error. | `alphaN` only |
| KS | Mode 4 tries the exact code pair for a scalar that is one value across the block. Without it, an alpha of 252 decoded 255 at `ultrafast` to `fast`. | Blocks with a constant scalar other than 0 and 255 |
| M5 | Mode 5 gets a one-colour candidate, and the one-colour test ignores alpha where alpha is not scored. | Flat colour beside a varying scalar |
| SF | Modes 4 and 5 search the scalar slot first and skip colour work that cannot win. | None |
| L16 | Outcome D, the direct form: a linear call compares the uber threshold at 1/16 and casts the partition estimators' float error ×16. That writes exactly the bytes of the same call with every weight ×16. The weights stay as passed, and perceptual calls are unchanged. | Linear encodes, `veryfast` and above |

### What was dropped

- **Q5** (rotated opaque modes 4 and 5 pay for the alpha the engine rewrites): under 0.005 dB everywhere in the spike. The owner dropped it.
- **Q6** (opaque mode 6's average candidate is charged alpha): it reached 314 of 23.6 million blocks. The owner dropped it.
- **PA** (perceptual one-colour packers sum each texel's error as a float): the keep rule dropped it under one reading of its rule (2). At the witness's setting, perceptual `basic`, it changed no decode. At `veryslow` 55 synthetic blocks decoded closer and 8 farther, so a `veryslow` reading would have kept it. The owner took the rule's verdict (BC7-O29 (g)).
- **Left for a later plan**, none of them in patch 0005: refinement of `slowest`'s partition candidates, the perceptual estimator's weights, anti-correlated colours in the estimators, the p-bit choice outside the search under weights, the RGBA principal axis, the checkerboard partitions, the mode 4/5 alpha grid recentring, WP under perceptual, a scale-free form of L16, perceptual mode 7's estimator truncation, and byte-neutral speed items.

### Method

- **Corpus.** Real textures from installed mods, not redistributable, so the lists stay outside the repo:
  - `opaque` (60): the diffuse rows of the [Windows re-check's sample](#setup);
  - the [alpha weights study's](#bc7-alpha-weights-against-bc3-2026-10-21) classes: `alpha_real` (96), `n_spec` (30, and the same maps at mip 2 as `n_spec_mip2`), `rmaos_spec` (24) and `m_height` (30, a proxy for a complex `_m`: a real `_rmaos`'s RGB with a real `_p` height in alpha);
  - `n_opaque` (60): the Windows re-check's normal rows with alpha forced to 255;
  - `rmaos_opaque` (24), and `msn` (24: 18 owner-supplied sources plus 6 from the mod set);
  - `m_zero`, an alpha-0 complex `_m`: the 24 `rmaos_opaque` sources with alpha 0.

  Synthetic classes (16 linear ramps, and each fix's witness blocks) come from seeded generators.
- **Harness.** A measurement tool outside the repo builds each kernel variant from `bc7e.ispc` with a byte script and encodes every image through the engine's entry points, with the engine's rotated-block post-fix. `crates/encoders-sys/examples/bc7_quality.rs` writes the same bytes from the committed objects: each committed fix was checked against its variant byte for byte on AVX2 and AVX-512.
- **Metrics.** PSNR per channel of the 8-bit decode against the source, mip 0 unless named. The objective is the PSNR of the weighted error the encoder minimises (RGB plus w times alpha). Normal maps add the mean angle in degrees. Per class, the per-image difference is averaged with a 95 % paired bootstrap interval (10,000 resamples, seed 20261122). A change is significant when the interval excludes 0.
- **Time.** Kernel time on 8 threads, three to five interleaved rounds per ISA with the job order rotated, the fastest round per image summed. A machine-claim log showed no other session's work during each timing.

### Q2t's threshold

The sweep ran T = 0 (today), float noise only (`q2r`), 0.25, 0.5, 1 and 2 /255 on every class at `basic`, `veryslow` and `max`, ×1 and ×16, exact opaque on, on both ISAs (AVX-512 within 0.01 dB):

| T | Summed ×1 gain, dB | Share of 2/255's | Keep rule | Ramp blocks flattened against today (cells with more) |
|---|---|---|---|---|
| float noise only | +0.074 | 6 % | keep | +17 (2 of 5) |
| 0.25/255 | +0.427 | 34 % | keep | +49 (2) |
| 0.5/255 | +0.641 | 51 % | keep | +51 (2); AVX-512 +30 (1) |
| 1/255 | +0.950 | 76 % | keep | +295 (4) |
| 2/255 | +1.252 | 100 % | keep | +353 (5) |

- The gain still rises at 2/255 on the data classes (`rmaos_spec` `veryslow`: +0.17 dB at 0.5, +0.25 at 1, +0.32 at 2), so the sweep did not bracket the best T. Opaque colour levels off near 1/255.
- No T raises the ramps' largest error (1, 1 and 2 levels in R, G and B). A flattened ramp block is a one-level step lost inside the block.
- The owner picked 0.5/255 for every type: it costs `alpha_real`'s alpha 0.002 to 0.008 dB and `n_spec`'s Z at `max` 0.004 to 0.0065 dB, and every other channel gains (BC7-O12). The data and normal types take 2/255 ([below](#per-type-tuning-flat-tolerance-and-the-channel-guard-2026-11-23-note-lr)).

### Before and after, per level

The final kernel against the vendored bc7e before patch 0005, AVX2, on the spike's cells (exact opaque on, weights 1, linear unless named). Objective delta in dB with its 95 % interval; in brackets, the images whose bytes changed.

| Class | `basic` | `veryslow` | `max` | `veryslow`, alpha ×16 | `veryslow`, perceptual |
|---|---|---|---|---|---|
| `alpha_real` | -0.032 [-0.046, -0.019] (85/96) | +0.114 [+0.088, +0.142] (96/96) | +0.127 [+0.098, +0.160] (96/96) | +0.175 [+0.138, +0.216] (96/96) | +0.013 [+0.008, +0.019] (73/96) |
| `opaque` | +0.424 [+0.014, +1.232] (59/60) | +0.549 [+0.107, +1.385] (60/60) | +0.186 [+0.113, +0.273] (60/60) | +0.549 (60/60) | +0.006 [+0.002, +0.011] (57/60) |
| `n_spec` | +0.025 [+0.020, +0.031] (30/30) | +0.029 [+0.022, +0.037] (30/30) | +0.012 [+0.008, +0.016] (30/30) | +0.046 [+0.033, +0.062] (30/30) | |
| `n_spec_mip2` | +0.028 (30/30) | +0.031 (30/30) | +0.010 (30/30) | +0.044 (30/30) | |
| `rmaos_spec` | +0.184 [+0.138, +0.237] (24/24) | +0.202 [+0.161, +0.245] (24/24) | +0.066 [+0.048, +0.089] (24/24) | +0.185 [+0.157, +0.214] (24/24) | |
| `m_height` | +0.040 [+0.032, +0.050] (30/30) | +0.084 [+0.062, +0.108] (30/30) | +0.039 [+0.024, +0.056] (30/30) | +0.178 [+0.111, +0.258] (30/30) | |

- **`opaque` rests on one image.** At `veryslow` the mean is +0.549 dB, the median +0.053, and the mean without the near-flat `023_gold01silver` (+24.1 dB on its own) +0.150. Every one of the 60 images moved by more than 0.001 dB.
- **The one cell below 0 is A's cost.** `alpha_real` at `basic` with exact opaque on is -0.032 dB: exact alpha 0 and 255 cost that much colour there ([x16 and the alpha weights again](#x16-on-colour-and-the-alpha-weights-again-2026-11-24-note-lr)).
- **The shipped cells** (`veryslow`, exact opaque as shipped): opaque colour with exact opaque off, the default for colour, +0.088 dB [+0.063, +0.119] (59 of 60 changed). With each type's shipped settings: `rmaos_opaque` +0.128 [+0.076, +0.199], `rmaos_spec` +0.336 [+0.256, +0.426], `m_height` +0.098, `n_spec` +0.061 [+0.047, +0.078], `n_opaque` +0.661 [+0.352, +1.032], `msn` +0.138 [+0.110, +0.168]. Nothing is worse.
- **Three paths.** None of the 46 settings has a significant objective of the opposite sign on AVX-512 or SSE2. Their means differ from AVX2's by up to 0.043 dB (`n_opaque` on AVX-512). `opaque` and `n_opaque` differ mostly through one image each, and `msn` reads about 0.007 dB lower on AVX-512, spread over all 24 images.

### What each kept fix gains, and its limits

The chain on AVX2 at `veryslow`, each step against the one before it:
- **Q2t** carries most of the gain on data and normals: `rmaos_spec` +0.171 dB, `m_height` +0.046, `n_spec` +0.026, `opaque` +0.012, `alpha_real` +0.006.
- **L16** (×16 for every linear encode): `alpha_real` +0.119, `opaque` +0.134, `m_height` +0.040, `rmaos_spec` +0.025, `n_spec` +0.004.
- **A as bit 3** (exact opaque on): `alpha_real` -0.010 [-0.015, -0.006], `m_height` -0.009. Exact opaque off, no image changed.
- **Q1, Q4, XC, Q3, WP** leave the default cells within 0.001 dB; Q3 and WP act only at alpha weights above 1 ([below](#x16-on-colour-and-the-alpha-weights-again-2026-11-24-note-lr)).
- **WP, KS and M5 alone** (third measurement, each against its base kernel): WP +0.0003 to +0.0009 dB on colour with alpha at alpha ×64 and +0.11 to +0.19 dB on its synthetic class; KS up to +0.025 dB on `m_height` (perceptual `fast`) and +0.22 dB on its class; M5 +0.35 to +0.40 dB on opaque colour.

Their evidence is narrow in two places:
- **M5's opaque gain is one image.** The near-flat `023_gold01silver` goes from an R squared error of 8,128 to 0; the other 59 opaque images move by about 0.000 dB.
- **The A3 and M5 witnesses are synthetic.** Each generator was shaped after one corpus block seen in a scratch run; no corpus texel is in them.

### Time

**S1 alone** (exact off, weights 1, fastest of three rounds per image, AVX2): `basic` -3.5 %, `veryslow` -2.1 %, `max` -1.1 %, over `opaque` and `rmaos_opaque` together, with the same bytes on all 252 image-settings.

**The final kernel against before patch 0005** (exact off, weights 1, linear; each class with its shipped settings unless named):

| Class | `fast` AVX2 / AVX-512 | `basic` | `veryslow` |
|---|---|---|---|
| `opaque` | +2.3 % / +0.3 % | -4.8 % / -3.8 % | +4.9 % / +5.1 % |
| `alpha_real` | +2.7 % / +1.5 % | -1.8 % / -1.6 % | +7.2 % / +6.1 % |
| `rmaos_opaque`, no guard or tolerance | +2.3 % / +1.8 % | -4.2 % / -4.3 % | +5.2 % / +6.3 % |
| `rmaos_opaque`, shipped (G guarded, `data`) | +8.4 % / +6.1 % | +7.0 % / +8.0 % | +22.0 % / +22.1 % |

- BC7 encodes faster only at `basic`, and only on colour and on data maps without their settings. That is SF's gain.
- At `veryslow` the ×16 ranking costs about 9 to 10 % on colour (+9.4 % on `opaque`, +10.0 % on `alpha_real`, measured alone), and SF takes part of it back.
- On `rmaos_opaque` the guard and the tolerance add the rest of the shipped +22 %, which is no more than the +23 to +26 % the owner accepted for these settings ([below](#per-type-tuning-flat-tolerance-and-the-channel-guard-2026-11-23-note-lr)).

## Per-type tuning: flat tolerance and the channel guard (2026-11-23, Note LR)

Measured 2026-11-23 on the kernel of the time with patch 0005's S1, Q2t, Q1 and Q4, then on the committed kernel at each step, and re-checked 2026-11-24 on the final kernel (D4 and G4 pass on every shipped cell, both ISAs). Corpus, harness and metrics as in [Kernel fixes](#method-1).

### The rule per class

Each class is a set of sub-types with its main channel, the channels that must stay similar and its flat controls (a real image with one channel replaced by 0 or 255 on every texel):

| Class | Sub-types | Main (G1) | Similar (G2) | Flat controls (G3) |
|---|---|---|---|---|
| `rmaos_opaque` | `rmaos` opaque; `skin_rfaos` rides | R (roughness) | B (AO) | G 0 and 255, alpha 0 and 255 |
| `rmaos_spec` | `rmaos` with specular in alpha; `skin_rfaos` rides | R | B | G 0 and 255, alpha 0 and 255 |
| `n_spec` | `normal` with specular; `landscape_normal`, `coat_normal`, `detail_normal`, `skin_wet` and `skin_detail` with alpha ride | RGB (the vector) | B (Z, which Skyrim reads), A | alpha 0 and 255 |
| `n_opaque` | `pbr_normal`; the same riders without alpha | RGB | B (Z) | none |
| `m_height` | `complex_material` (exact opaque on) | R | B, A (height) | G 0 and 255, alpha 0 and 255 |
| `msn` | `msn` | RGB | B (Z) | none |

A candidate ships for a class when, against the fixed kernel at weights ×1 (B):
- **G1:** the main channel is not significantly worse;
- **G2:** each similar channel loses at most 0.25 dB on average and is not significantly worse;
- **G3:** no flat control is more than 4 levels worse than B's on any image;
- **G4** (the G channel of `rmaos_opaque`, `rmaos_spec` and `m_height`): on every image, no more texels whose source G is 0 or 255 decode off it than the encoder before patch 0005 writes (BC7-O4). Texels rolled into 0 or 255 are flagged, not failed;
- **Benefit:** the main channel, a similar channel or the total is significantly better, or a flat control B misses by more than 4 levels is kept.

### What the classes measured

`veryslow`, AVX2; AVX-512 gave the same verdict on every class.
- **Mode 0 off with ×16 (C16), the planned setting, is a measured option not taken** (BC7-O5, O13). Mode 0 off alone costs quality on every class (main channel -0.0001 dB on `n_spec` to -0.587 on `n_opaque`, against ×16 with mode 0 on), and ×16 alone gains on every class (`rmaos_opaque` +0.096 dB R, `rmaos_spec` +0.026, `n_spec` +0.004, `n_opaque` +0.268, `m_height` +0.045, `msn` +0.094). C16 failed G4 on all three G classes and G1 and G2 on `n_opaque`, and had no Benefit on `msn`. What mode 0 off buys is the flat controls and speed.
- **×16 per type became ×16 for every linear encode** (BC7-O27, L16 [above](#what-changed)), so the types below set no weights.
- **T_data, the bracket** (0.5 to 4 /255 on the three G classes, BC7-O9): 2/255 is the largest T at which G1 to G4 hold per image on all three classes and both ISAs; the G gain from 1 to 2 is significant everywhere. 3/255 lost 3 more metal absolutes than before on one `rmaos_opaque` image (BC7-O11).
- **Exact G against C16 and ×16** (on the 2/255 kernel, weights ×16): exact G keeps every flat G control exact (worst error 8 to 0) and loses 41 to 69 % fewer metal absolutes than C16 or ×16 (`rmaos_opaque` 516,809 before the patch, 361,072 with ×16, 178,752 with exact G). Against ×16 alone it costs R 0.024 to 0.031 dB, AO 0.019 to 0.037 and the total 0.010 to 0.014 (all significant), and 3 to 6 % of blocks change.
- **Exact B on normals is a measured option not taken** (BC7-O17). It brought `n_spec`'s Z back to before the patch (from -0.003 to -0.0065 dB to within ±0.002, never significant), but by keeping the all-255 subsets the old encoder already missed: none of 0.5/255's Z loss sits in all-255 subsets. It sits in near-255 noise and mixed subsets. Exact B costs X and Y 0.001 to 0.002 dB (`n_opaque` 0.01 to 0.02 dB RGB) and 10 to 27 % kernel time (an upper bound: that measurement scanned for constants once per call).
- **T_data on the normal types** (BC7-O21, O22): against before the patch, 2/255 improves the angle and pooled XYZ on every `n_opaque` and `msn` cell, with no image worse. Against 0.5/255 the angle still improves on every cell; 1 to 9 images per cell read worse by at most 0.00032 degrees, below the AVX2 to AVX-512 spread on the same image. On `n_opaque` it gives back 0.011 to 0.021 dB of Z against 0.5/255, while Z stays 0.54 to 0.86 dB above before the patch.

### What shipped

| Sub-types | `encoder.bc7_flat_tolerance` | Channel guard |
|---|---|---|
| `rmaos`, `skin_rfaos` | `data` (2/255) | G |
| `complex_material` | `data` | A (exact opaque) and G |
| the [normal types](../wiki/Texture-Types.md#bc7-per-type-settings) | `data` | none |
| every other type | default (0.5/255) | as before (`envmask`: A) |

On the final kernel (third measurement, against B and against the encoder before patch 0005): `rmaos_opaque` main +0.065 dB, total +0.075, metal absolutes lost 516,809 to 178,756 (fewer on 24 of 24 images); `rmaos_spec` total +0.019, 735,026 to 282,812 (24 of 24); `m_height` main +0.023, total +0.021, 905,887 to 219,546 (30 of 30); `n_spec` +0.004, `n_opaque` +0.268, `msn` +0.095. On the committed kernel before the last fixes, `data` against the default (both ×16) improved the angle by 0.00659 degrees on `n_opaque`, 0.00155 on `msn` and 0.01036 on `n_spec`.

### Time per type

Kernel time at `veryslow` against the encoder before these settings, fastest of five rounds, AVX2 / AVX-512 (BC7-O19): `rmaos_opaque` +26.4 / +23.2 %, `rmaos_spec` +17.7 / +18.0 %, `m_height` +15.1 / +16.5 %, `n_spec` +1.6 / +2.4 %, `n_opaque` +5.4 / +3.6 %, `msn` +10.0 / +8.3 %. The tolerance costs nothing; the rest is ×16 and, on the data maps, the G guard. The owner accepted exact G's +20 to +59 % spike bound (BC7-O13). Base to final on the shipped `rmaos_opaque` is +22 % ([Time](#time)).

## x16 on colour and the alpha weights again (2026-11-24, Note LR)

Measured 2026-11-24 (second and third measurements), on the kernel with the fixes of the time and then on the final kernel; numbers moved by at most 0.02 dB between the two. Corpus, harness and metrics as in [Kernel fixes](#method-1).

### Is ×16 only positive on colour?

×16 against ×1 on the same kernel (0.5/255, no guard, exact opaque off), RGB pooled:

| Class | `fast` | `basic` | `veryslow` | `max` | Time, `veryslow` AVX2 / AVX-512 |
|---|---|---|---|---|---|
| `opaque` (60) | +0.006 [+0.002, +0.014] | +0.006 [+0.001, +0.013] | +0.124 [+0.080, +0.176], 57 of 60 better | +0.223 [+0.127, +0.336], 58 of 60 | +9.4 % / +8.9 % |
| `alpha_real` (96), RGB / alpha | ±0.000, not significant | ±0.000, not significant | +0.113 / +0.162, 96 of 96 | +0.124 / +0.179 | +10.0 % / +8.0 % |

No cell is worse on any class, level or ISA; AVX-512 matches AVX2 within 0.002 dB. At `fast` ×16 costs +3.6 to +3.7 % on AVX2. So ×16 became the ranking of every linear encode (BC7-O27; L16 [above](#what-changed)).

### The alpha weights on the fixed kernel

`veryslow`, each class at its shipped tolerance and guard; second measurement, with Q3's planned two-index window. "What Alpha ×N becomes" is weights 1, 1, 1, N with ×16 ranking and Q3, against the same weights ranked at ×1 without Q3. "Q3's own" is the same weights at ×16 with and without Q3. Objective = the weighted error's PSNR.

| Class | N | What Alpha ×N becomes: RGB / alpha / objective | Q3's own: RGB / alpha / objective |
|---|---|---|---|
| `alpha_real` | 8 | +0.027 / +0.239 / +0.122 | -0.067 / +0.213 / +0.048 |
| `alpha_real` | 16 | -0.049 / +0.390 / +0.164 | -0.129 / +0.379 / +0.112 |
| `alpha_real` | 64 | -0.245 / +0.684 / +0.367 | -0.316 / +0.683 / +0.343 |
| `n_spec` | 16 | -0.035 / +0.112 / +0.018 | -0.036 / +0.112 / +0.018 (angle +0.014 degrees) |
| `rmaos_spec` | 16 | -0.044 / +0.234 / +0.048 | -0.061 / +0.248 / +0.038 |
| `m_height` | 16 | -0.176 / +0.545 / +0.121 | -0.194 / +0.544 / +0.110 |

- Q3 with the window `full` against the planned two-index window: +0.008 to +0.017 dB on the objective in 6 of 6 cells, -2.8 to +0.8 % time; mode-6 texels off their weighted-nearest entry fell from 285,562 to 55,711, all in opaque blocks, where Q3 does not run.
- With Q3 `full` (final kernel), colour with alpha gives -0.05 / +0.21 dB RGB / alpha at ×8, -0.11 / +0.38 at ×16 and -0.30 / +0.68 at ×64.
- Against BC3 (rgbcx level 18): at ×8 `alpha_real` still beats BC3 on alpha in 85 of 96 images and on RGB in 88 of 96. At ×16, `n_spec`'s normals beat BC3's on 24 of 30 (2.512 against 2.806 degrees), and its specular loses 0.96 dB to BC3 (12 of 30 wins).
- ×32 and ×64 gain +0.46 and +0.75 dB of alpha over ×16 on `alpha_real`, for 0.43 and 0.78 dB of RGB (the texture panel's text rounds these to 0.05).
- On colour, ×8 costs about 1 dB of RGB per 1.9 dB of alpha gained against ×1; on packed and normal maps 1 per 2.4 to 4.2.
- Time of Q3 on its own path: +0.3 to +1.9 %; with equal weights, within the run-to-run spread.

### Channel guard A against the old exact-opaque flag

On fully opaque blocks A writes the old flag's bytes. On blocks with alpha, it keeps alpha 0 and 255 exact on more texels and costs colour. Final kernel, ×16, weights 1, AVX2 at every level (AVX-512 at `veryslow`), against the flag on:

| Content | Alpha | RGB | Pooled objective |
|---|---|---|---|
| complex `_m` with height (`m_height`) | +0.039 to +0.061 dB (`ultrafast` +0.089); alpha-0 texels exact 84 %, against 72 to 77 % (66 % at `ultrafast`) | -0.009 to -0.018 dB (`ultrafast` +0.007, not significant) | -0.006 to -0.011 dB (`ultrafast` +0.009, not significant) |
| colour with alpha, A ticked (`alpha_real`) | +0.060 to +0.466 dB | -0.014 (`max`) to -0.341 dB (`fast`) | -0.004 (`max`) to -0.214 dB (`fast`), every cell significantly worse; 0 to 2 image wins against 55 to 57 losses |

- What A buys is exact alpha 0 and 255 on more texels; the pooled objective pays for it.
- **Alpha-0 complex `_m` files that had A off** are encoded with A. Alpha stays exactly 0, and RGB moves -0.021 to -0.072 dB at `ultrafast` to `basic` and +0.013 and +0.020 dB at `veryslow` and `max`, against A off plus the engine's old exact-0 step.
- A block whose alpha is 0 throughout decodes 0 at every level, linear and perceptual, by the kernel's own rules: 0 failures on 84,162 A-on image rows.
- A's time against the flag: `m_height` within the noise; colour with alpha about +3 % (+2.7 % at `fast`, +3.5 % at `veryslow`).

## Estimator precision (2026-11-23, Note LR)

Measured 2026-11-23 with Task 3's harness, `veryslow`, both ISAs. Corpus and metrics as in [Kernel fixes](#method-1).

**The mechanism.** Scaling every integer weight by 16 multiplies every weighted error by exactly 16. The partition estimators then cast a float error to an integer, and the truncation loses less at ×16, so ×16 ranks partitions more finely than ×1.

**The question.** Does a kernel fix to those casts (E), with weights left at 1, give ×16's gain on its own?

| Class | E-off's blocks equal to C16's (AVX2 / AVX-512) | E against B, main | E against ×16, main |
|---|---|---|---|
| `rmaos_opaque` | 84.8 % / 84.9 % | +0.0006 | -0.096 [-0.146, -0.052] |
| `rmaos_spec` | 89.7 % / 89.8 % | +0.0000 | -0.025 [-0.042, -0.013] |
| `n_spec` | 91.5 % / 91.9 % | -0.0000 | -0.004 [-0.008, -0.002] |
| `n_opaque` | 94.1 % / 94.3 % | +0.006 [+0.001, +0.011] | -0.262 [-0.411, -0.134] |
| `m_height` | 85.3 % / 85.6 % | +0.0000 | -0.045 [-0.070, -0.023] |
| `msn` | 86.3 % / 86.9 % | +0.003 [+0.000, +0.008] | -0.091 [-0.122, -0.063] |

- E writes ×16's bytes only at `basic` (all 96 `rmaos_opaque` image-controls). At `veryslow` it writes them on no image but 2 of 60 `n_opaque`, and at `veryslow` and `max` it recovers at most 0.006 dB of ×16's 0.004 to 0.27 dB.
- E-off (E with mode 0 off) matches C16 on barely more blocks than mode 0 off at ×1 does (0.2 to 1.3 points).
- So the casts are not the whole of ×16 at the slower levels. The third measurement later found the other site, the uber threshold, which L16 scales too ([What changed](#what-changed)).
- **The owner chose P2** (BC7-O6): ship the per-type settings and leave the casts. The kernel fix alone is a measured option not taken; L16 superseded the per-type ×16 with the direct form for every linear encode.
