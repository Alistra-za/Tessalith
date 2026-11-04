# BC7: bc7e tuning, the `max` level and alpha weights

Two BC7 studies of Tessalith's encoder settings: whether bc7e has quality left above `veryslow` (2026-10-18, commit 65c6db00), which became the `max` level, and [whether BC7 with a higher alpha weight can replace BC3](#bc7-alpha-weights-against-bc3-2026-10-21) for delicate alpha (2026-10-21, commit dbf54808), which became the *BC7 weighting* options. The encoder comparison they build on is [BCn encoders by format](encoder-benchmark.md#bc7).

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
  machine's CPU (the avx2 path). `sdc-bench bc7-tune --variants base,ep --repeats 5`: the fastest of 5 runs
  is kept, rayon on all cores, on a machine with other builds running. `base` and `ep` run back to back per
  image.
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

Written 2026-10-21 (commit dbf54808).

Source: local study, 2026-10-21, and DECISIONS Note CP. It asks whether BC7 with a higher alpha error weight can replace BC3 for textures with delicate alpha, and keep BC7's better RGB. BC7 is the built-in bc7e at its default level (veryslow), with linear weights 1,1,1,w for w = 1, 2, 4, 8 and 16, and in a follow-up on 2026-10-22, 32 and 64. BC3 is the bundled rgbcx (level 18, `-r`). w1 is today's Linear. The dropdown offers Alpha ×2, ×8 and ×16, and Perceptual. The chart is in the [GUI guide](../wiki/GUI-Texture-Panel.md#bc7-weighting-against-bc3).

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

The dropdown stays at ×16. w32 and w64 are in the bench only.

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
| BC7 w32 (study only) | 45.84 | 52.23 | 52.23 | 0.9989 | 12.42 | 93.06 | 66.39 |
| BC7 w64 (study only) | 45.58 | 52.00 | 52.38 | 0.9990 | 12.24 | 93.08 | 66.37 |
| BC7 perceptual | 46.54 | 52.81 | 51.17 | 0.9983 | 14.39 | 92.70 | 65.27 |

Where BC7 beats BC3:

| Setting | alpha PSNR wins | alpha SSIM wins | both | RGB PSNR Δ vs BC3 | RGB PSNR wins | alpha PSNR Δ | alpha SSIM Δ (×1000) |
|---|---|---|---|---|---|---|---|
| BC7 w1 (today) | 61/96 | 30/96 | 30/96 | +7.40 dB | 96/96 | +0.69 | -2.65 |
| BC7 w2 | 73/96 | 46/96 | 46/96 | +6.74 dB | 95/96 | +2.14 | -0.77 |
| BC7 w4 | 77/96 | 62/96 | 62/96 | +6.13 dB | 94/96 | +3.32 | +0.17 |
| BC7 w8 | 85/96 | 77/96 | 77/96 | +5.61 dB | 88/96 | +4.16 | +0.67 |
| BC7 w16 | 86/96 | 80/96 | 80/96 | +5.18 dB | 84/96 | +4.69 | +0.97 |
| BC7 w32 (study only) | 87/96 | 82/96 | 82/96 | +4.84 dB | 82/96 | +4.98 | +1.14 |
| BC7 w64 (study only) | 87/96 | 82/96 | 82/96 | +4.59 dB | 79/96 | +5.14 | +1.24 |
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
| BC7 w32 (study only) | 2.706 | 33.95 | 46.02 | 0.9953 | 24.87 | 75.62 | 54.68 |
| BC7 w64 (study only) | 2.851 | 33.49 | 46.64 | 0.9963 | 19.47 | 74.28 | 53.82 |

Where BC7 beats BC3:

| Setting | alpha PSNR wins | alpha SSIM wins | both | angle Δ vs BC3 | angle wins | alpha PSNR Δ | alpha SSIM Δ (×1000) |
|---|---|---|---|---|---|---|---|
| BC7 w1 (today) | 0/30 | 0/30 | 0/30 | +1.124 ° | 30/30 | -8.83 | -42.94 |
| BC7 w2 | 0/30 | 0/30 | 0/30 | +0.938 ° | 29/30 | -6.78 | -27.09 |
| BC7 w4 | 1/30 | 0/30 | 0/30 | +0.725 ° | 29/30 | -4.67 | -15.95 |
| BC7 w8 | 3/30 | 1/30 | 1/30 | +0.498 ° | 27/30 | -2.70 | -8.30 |
| BC7 w16 | 10/30 | 3/30 | 3/30 | +0.283 ° | 24/30 | -1.11 | -3.70 |
| BC7 w32 (study only) | 17/30 | 6/30 | 6/30 | +0.100 ° | 16/30 | -0.04 | -1.37 |
| BC7 w64 (study only) | 21/30 | 13/30 | 13/30 | -0.045 ° | 11/30 | +0.58 | -0.34 |

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
| BC7 w32 (study only) | 3.460 | 31.94 | 43.80 | 0.9946 | 15.87 | 95.81 | 57.35 |
| BC7 w64 (study only) | 3.640 | 31.45 | 44.38 | 0.9956 | 14.40 | 94.86 | 57.27 |

Where BC7 beats BC3:

| Setting | alpha PSNR wins | alpha SSIM wins | both | angle Δ vs BC3 | angle wins | alpha PSNR Δ | alpha SSIM Δ (×1000) |
|---|---|---|---|---|---|---|---|
| BC7 w1 (today) | 0/30 | 0/30 | 0/30 | +1.347 ° | 30/30 | -9.51 | -53.87 |
| BC7 w2 | 0/30 | 0/30 | 0/30 | +1.064 ° | 30/30 | -7.11 | -29.79 |
| BC7 w4 | 1/30 | 0/30 | 0/30 | +0.772 ° | 29/30 | -4.71 | -15.38 |
| BC7 w8 | 1/30 | 1/30 | 1/30 | +0.508 ° | 25/30 | -2.70 | -7.62 |
| BC7 w16 | 8/30 | 5/30 | 4/30 | +0.290 ° | 22/30 | -1.35 | -3.63 |
| BC7 w32 (study only) | 13/30 | 9/30 | 9/30 | +0.100 ° | 18/30 | -0.49 | -1.59 |
| BC7 w64 (study only) | 19/30 | 12/30 | 12/30 | -0.080 ° | 13/30 | +0.09 | -0.59 |

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
| BC7 w32 (study only) | 42.05 | 50.09 | 43.01 | 43.32 | 57.31 | 0.9987 | 13.79 | 78.52 | 73.43 |
| BC7 w64 (study only) | 41.47 | 49.70 | 42.57 | 42.77 | 58.51 | 0.9991 | 13.12 | 78.14 | 74.13 |
| BC7 w8 + exact opaque | | | | 44.25 | 55.01 | 0.9973 | 17.67 | 95.14 | 91.64 |
| BC7 w16 + exact opaque | | | | 43.82 | 56.43 | 0.9982 | 15.33 | 78.66 | 92.53 |
| BC7 w64 + exact opaque (study only) | | | | 42.77 | 59.10 | 0.9992 | 13.12 | 78.14 | 93.60 |

Where BC7 beats BC3:

| Setting | alpha PSNR wins | alpha SSIM wins | both | RGB PSNR Δ vs BC3 | RGB PSNR wins | alpha PSNR Δ | alpha SSIM Δ (×1000) |
|---|---|---|---|---|---|---|---|
| BC7 w1 (today) | 0/24 | 0/24 | 0/24 | +6.63 dB | 24/24 | -9.06 | -8.75 |
| BC7 w2 | 2/24 | 0/24 | 0/24 | +6.31 dB | 24/24 | -7.19 | -5.59 |
| BC7 w4 | 2/24 | 1/24 | 1/24 | +5.98 dB | 23/24 | -5.62 | -3.61 |
| BC7 w8 | 4/24 | 1/24 | 1/24 | +5.61 dB | 23/24 | -4.24 | -2.36 |
| BC7 w16 | 5/24 | 1/24 | 1/24 | +5.18 dB | 22/24 | -2.91 | -1.46 |
| BC7 w32 (study only) | 6/24 | 1/24 | 1/24 | +4.68 dB | 22/24 | -1.67 | -0.83 |
| BC7 w64 (study only) | 11/24 | 1/24 | 1/24 | +4.13 dB | 22/24 | -0.47 | -0.42 |

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
| BC7 w32 (study only) | 40.94 | 48.82 | 40.33 | 41.44 | 52.46 | 0.9980 | 12.43 | 98.15 | 95.05 |
| BC7 w64 (study only) | 40.52 | 48.41 | 39.84 | 40.95 | 52.99 | 0.9984 | 11.70 | 98.76 | 95.27 |
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
| BC7 w32 (study only, rgbcx only) | 13/30 | 2/30 | — | — | +2.77 dB | 26/30 |
| BC7 w64 (study only, rgbcx only) | 16/30 | 3/30 | 11/14 | — | +2.28 dB | 24/30 |
| BC7 w1 + exact-opaque | 0/30 / 0/30 | 0/30 / 0/30 | 0/14 / 0/14 | 0/15 / 0/15 | +6.08 dB | 30/30 |
| BC7 [4,1,2,1] | 0/30 / 0/30 | 0/30 / 0/30 | 0/14 / 0/14 | 0/15 / 0/15 | +6.43 dB | 30/30 |

The `m_height` rows are a proxy: a real `_rmaos` for RGB plus a real `_p` height written into alpha. Its Means table also lists NVTT BC3, BC7 w1 with exact-opaque, and a `[4,1,2,1]` weight mode that the owner dropped (Note CP12). In the `m_height` wins table, BC3 means rgbcx / NVTT.
