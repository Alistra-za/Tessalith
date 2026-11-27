# Mipmap filters, normal maps and alpha coverage

How mip levels should be built for each kind of texture, and how alpha-tested textures keep their shape at a distance: a review of published papers, engine documentation and tool source code, plus local tests made while building Tessalith. Written 2026-10-16 (commit 1bae037b); Result 6 is the 2026-11-03 alpha survey behind Note FM34; Result 7 is the 2026-11-26 measurement of mip quality on a corpus (build e315ac28).

## The questions

- Which filter should build mip levels: box, tent, Mitchell, Kaiser or something else?
- What has to happen around the filter (gamma, normal length, alpha, data channels) for the result to be right?
- How do alpha-tested textures such as foliage avoid thinning out in the smaller mips?

## How it was done

- Read the classic mipmapping studies (Jonathan Blow 2001, Toksvig 2004, Castaño 2010, Wyman and McGuire 2017), engine docs (Unity HDRP, Unreal, The Order: 1886, Call of Duty: WWII) and the source of NVTT 2 and DirectXTex.
- Checked NVTT 3.2.5's command-line help on the benchmark machine.
- **No one has published a PSNR, SSIM or FLIP comparison of mip filters on game textures.** The filter rankings below rest on qualitative studies and source reading.

## Result 1: the steps around the filter matter more than the filter

Getting these wrong gives errors that are systematic and visible: darkened detail, specular shimmer, dissolving foliage, zeroed data channels. The kernel choice is a smaller trade.

1. Decode sRGB colour to linear light before averaging.
2. Renormalise normal mips, and turn the lost length into extra roughness where the shader can use it.
3. Filter data channels independently: no sRGB, no premultiplying, no sharpening.
4. Fill the colour under fully transparent texels, so it does not bleed into visible ones.
5. Rescale alpha-tested mips so each level keeps mip 0's coverage.

## Result 2: box is the safe default; sharper filters only help LDR colour

| Filter | Taps per axis at 2:1 | Behaviour | Good for |
|---|---|---|---|
| Box | 2 | Blurs a little and still aliases a little. Never overshoots. Exact when repeated at power-of-two sizes. | Data, normals, HDR, alpha, foliage |
| Tent | 4 | Softer than box, aliases less. No overshoot. | Data where shimmer matters |
| Mitchell-Netravali (B = C = 1/3) | 8 | Small negative lobes. Between tent and windowed sinc. | LDR colour where Kaiser halos show |
| Kaiser-windowed sinc (width 3, α 4) | 12 | Sharpest practical choice. Rings at hard edges. | LDR colour that must stay legible |
| Lanczos-3 | 12 | Looks the same as Kaiser. | Same as Kaiser |
| Min / max | 4 texels | Non-linear by design. Never rings. | Conservative height pyramids. Never colour. |

- Blow found that text on a billboard vanished three levels down with box and stayed visible with Kaiser. He could widen Kaiser to only about 14 samples before ringing showed, and NVTT's wiki puts the practical limit at radius 3.
- NVTT's wiki concludes that box "does a great job in most cases" and works best for HDR.
- Sinc filters should not be used on HDR. A negative lobe next to a very bright texel produces negative output, which clips to black rings in BC6H.

### Averaging sRGB values darkens thin bright detail

Averaging gamma-encoded values is not averaging light. With a 50/50 box under gamma 2, a bright line comes out at half the brightness it should. The fix is to decode sRGB, filter, then encode again. This is right for albedo, emissive and UI colour. It is wrong for normals, roughness, metalness, AO, masks, height and packed data, which are already linear.

### Building every level from the one before

Almost every shipping tool builds level N from level N−1. For box at power-of-two sizes that is exact. For other kernels the error compounds: a repeated tent drifts toward a Gaussian, and a repeated Kaiser re-rings the previous level's overshoot. Local tests during development measured the cost of the recursive build against a direct Lanczos-8 reference at **1.1 to 2.7 dB for tent and 0.8 to 1.8 dB for Mitchell** ([DECISIONS](../../.claude/DECISIONS.md), M6 Track C ruling S-mips-3).

### Edges

A texture that tiles should be filtered with wrap at the edges; atlases, UI and decals with clamp. A 2:1 box never reads outside its 2×2 footprint, so the edge mode does not matter for it. Kaiser at width 3 reaches 6 source texels in.

## Result 3: normal maps need renormalising and a roughness bake

Averaging unit normals gives a shorter vector whenever the normals differ, and the lost length measures exactly the detail the mip throws away. Without help, that detail comes back as sparkling specular highlights. Unity HDRP, The Order: 1886 and Call of Duty: WWII all handle it the same way:

1. Average the full-resolution normals over each mip's footprint.
2. Turn the average length L into variance and add it to the GGX roughness.
3. Renormalise the stored normal.

Unity's form, following The Order: 1886's von Mises–Fisher fit:

- κ = (3L − L³) / (1 − L²)
- variance = 0.25 / κ
- α′² = saturate(α² + min(2 · variance, t²)), where α is perceptual roughness squared and t is a clamp (Unity's `NORMALMAP_HIGHEST_VARIANCE` stores 0.03125, which is 0.25² / 2).

Other points:

- Sharpening filters damage normal mips: overshoot can push Z to zero or below, and the extra detail returns without the matching roughness.
- 8-bit components carry about 0.0068 of error in the length, so the length must be read from full-precision data.
- In local tests, plain 8-bit quantisation noise on a flat normal map gave 1 − L of 1e-5 to 1e-4 and lifted zero roughness to 0.04 to 0.10 at lower mips. Such a map should add no roughness.

## Result 4: height and data maps

- A height map used as a value (parallax, displacement) wants an averaged chain: box, linear, never sharpened.
- Data maps are independent linear values. Filter each channel without sRGB, without premultiplying and without alpha weighting.
- Roughness combines with normal variance in α = r² space, so the bake works there.
- Metalness should be averaged, not point-sampled. AO should never be sharpened (clipped overshoot brightens it).
- IDs and indices must never be interpolated.

## Result 5: alpha-tested mips lose coverage unless rescaled

A box filter pulls alpha toward the texture's mean. If the mean is below the alpha-test threshold, distant foliage dissolves. Wyman and McGuire measured a hair texture with mean alpha 0.32.

The standard fix is Ignacio Castaño's (2010): measure mip 0's coverage at the shader's threshold, then for each mip find the alpha value that the same share of texels exceeds and scale alpha so that value lands on the threshold. Shipped versions differ from the blog post:

| Implementation | Coverage estimate | Solve |
|---|---|---|
| Castaño 2010 | per texel | bisection on the reference value |
| NVTT 2.x | 4×4 bilinear supersample | bisection on the scale in [0, 4], best kept |
| DirectXTex (`--keep-coverage`) | 8×8 bilinear supersample | bisection on the scale in [0, 4], last midpoint |

The ×4 cap matters: a sparse or heavily padded texture whose small mips need more than ×4 cannot recover.

Further points:

- **Coverage scaling suits alpha-tested content only.** For blended alpha (smoke, glass, soft decals), the box-averaged alpha already is the right opacity, and scaling makes distant objects too opaque.
- **Colour under transparent texels must be handled.** Averaging opaque green with transparent black gives a darkened colour under straight-alpha blending. Either premultiply, or weight colour by alpha while filtering and also fill (dilate) the colour into the alpha-0 area, because bilinear sampling and the BC encoder still see those texels.
- **Coverage drift after BC7 encoding is small.** The [benchmark](encoder-benchmark.md#bc7-with-alpha) measured 0.05 to 0.13% of texels flipping across alpha 128 at mip 0 for every BC7 encoder.

Tessalith's coverage step rebuilds the idea of AlphaBloom's Auto Coverage (by tkRiverGamer) and Castaño's method. No code is copied; see [CREDITS](../CREDITS.md).

<a id="coverage-remap"></a>The owner kept a monotone piecewise-linear remap (0 → 0, a → T, 1 → 1) over Castaño's uniform t/a scale after comparing the two on synthetic and real 4096² sources at mips 1024 and 128 (2026-10-09, RULINGS #50).

## Result 6: a few low alpha texels are noise, a cutout is not

A survey of 708 colour textures that asked about their alpha, checked against the meshes that use them, looked for a rule that clears stray low texels from re-encoded sources without silencing a real cutout. The smallest real cutout covers 0.31 % of its image (`weapons/elven/bow/bow`), so a limit of 0.1 % of the texels clears none of the 457 mesh-confirmed real alpha textures. Limits of 0.5 % and 1 % silence 4 and 20 real cutouts. On the other side, 3 glow maps lose a false blocking warning and 135 normals whose minimum alpha is 249 or 250 become opaque. 0.1 % of a 4K texture is 16 777 texels, so a second condition caps any connected group of low texels at 16; a small real cutout on a large texture still asks (`sprigganburnt2`, 12 alpha-test shapes).

## Result 7: measured mip quality (2026-11-26)

How much of a decoded mip's error comes from our filter, from rounding to 8 bits and from the encoder. The test takes 300 of the 9,228 outputs the 10,000-image corpus's reference run wrote (a fixed seed picks them; the base build a1eabd9c wrote them), re-builds each mip with build e315ac28 and scores three things against a reference built from the source's level 0 by a direct area average in f64, the same filter in exact arithmetic. Errors are mean squared error in 8-bit code units, pooled over texels, on the RGB channels. The run is described on [the time page](engine-time.md).

- **e** is our float mip (before rounding) against the reference.
- **q** is the 8-bit image handed to the encoder against the reference.
- **d** is the decoded DDS against the reference. "d against q" is the encoder's own error.

Levels are counted from the full size (top0, top1, top2) and from the smallest (bottom0, bottom1).

| Format | Level | q | d | d against q |
|---|---|---|---|---|
| BC7 | top0 | 0.0168 | 4.2484 | 4.2455 |
| BC7 | top1 | 0.0591 | 5.1662 | 5.1516 |
| BC7 | top2 | 0.0596 | 5.4074 | 5.3965 |
| BC7 | bottom1 | 0.0677 | 0.3184 | 0.2506 |
| BC7 | bottom0 | 0.0000 | 0.5823 | 0.5823 |
| BC4 | top0 | 0.0000 | 0.1684 | 0.1684 |
| BC4 | top1 | 0.0739 | 0.6458 | 0.5745 |
| BC4 | top2 | 0.0720 | 1.2096 | 1.1408 |
| BC4 | bottom1 | 0.0737 | 0.1159 | 0.0385 |
| BC4 | bottom0 | 0.0804 | 0.0804 | 0.0000 |

- e is about exact. It is at most 5.1e-8 in every format and level row (121 dB or better, float rounding). Our filter does what the reference does.
- q is small. At most 0.080, which is 59 dB or better. The 8-bit step costs little at every level.
- The encoder owns the decoded loss at the top levels. On BC7, "d against q" is 99.7 to 99.9 % of d at top0 to top2, and 89 to 100 % on BC4. At the smallest levels the totals are under 0.6 and the 8-bit step is a larger part of them: on BC7 bottom1 q is 21 % of d, and on BC4 bottom0 q is all of d.
- BC7 alpha follows the same pattern. At top2, e is 0.0000, q is 0.0298 and d is 6.0597.

Every job used the defaults: the box filter with the wrap edge (box never reads the edge), no `--set` and no mip key in the corpus's `sdc.toml`. Counts, all scored and none skipped: 300 outputs, of which 274 are BC7 and 26 are BC4 (the `height` sub-type). By sub-type, BC7 has `diffuse`, `landscape_diffuse`, `landscape_normal`, `normal`, `pbr_albedo`, `pbr_emissive`, `pbr_fuzz`, `pbr_normal`, `pbr_subsurface` and `rmaos` at 26 each, `glow` 8, `ui_sky` 4 and `coat_normal` 2. The 4 `ui_sky` outputs have one level, the other 296 have 11. A rebuild check re-encoded level 1 and the last level of every output and found 0 byte mismatches against the files, so the two builds agree on these outputs (AVX-512 for BC7). Cubemaps, complex grass and HDR outputs are not scored.

The mixed corpus has no mip quality run. Only the 10,000-image corpus was scored.

## What Tessalith does because of this

The mip chain is in [`crates/engine/src/mips.rs`](../../crates/engine/src/mips.rs), the kernels in [`filter.rs`](../../crates/engine/src/filter.rs), coverage in [`coverage.rs`](../../crates/engine/src/coverage.rs), dilation in [`dilate.rs`](../../crates/engine/src/dilate.rs) and the normal bake in [`bake/mod.rs`](../../crates/engine/src/bake/mod.rs).

- **Box is the default filter for every texture type** ([DECISIONS](../../.claude/DECISIONS.md), S-mips-1).
- **Tent, Mitchell and Kaiser are allowed only for sRGB colour.** On colour whose alpha weights the mips (cutout and blend) only box and tent are allowed, because Mitchell's and Kaiser's negative lobes ring at cutout edges (Note FM28). Kaiser is fixed at width 3, α 4. These filters build each level directly from mip 0, with the kernel widened per level; box stays recursive, and alpha always uses the box (S-mips-2, S-mips-3, S-mips-4).
- **Colour is filtered in linear light**, decoded with the exact sRGB curve (not a 2.2 power) in float32 ([`crates/engine/src/ops.rs`](../../crates/engine/src/ops.rs)). This depends on the content, not the DDS tag: a vanilla diffuse is filtered in linear light and still written UNORM.
- **Edge mode defaults to wrap.** Only the non-box kernels read it.
- **Every normal mip is renormalised.** In a TruePBR set, the `_n`'s variance goes into the `_rmaos` (or `_rfaos`) roughness with the vMF formula above, clamped at t = 0.25. A coat normal `_cnr` bakes into its own alpha. A footprint whose average length is 0.999 or more (about a 2° tilt) adds no variance, so quantisation noise adds no roughness ([DECISIONS](../../.claude/DECISIONS.md) #6, Note E, M4 rulings R1 to R14 and M4-final-2). Vanilla `_n`, landscape `_n` and `_msn` are renormalised but never baked.
- **Data maps use plain per-channel box mips.** Normals, `_msn`, `_flow` and roughness chains are never alpha-weighted (R11).
- **Colour under alpha 0 is filled with a push-pull dilation before any filtering.** Colour with cutout or blend alpha uses an alpha-weighted box, or an alpha-weighted tent when `mips.filter` asks for it. Tessalith never writes premultiplied DDS files, because Skyrim blends straight alpha.
- **Coverage preservation is on by default for `cutout` and `cutout_blend`**, with a per-texture threshold (default 127) and `>` as the test ([DECISIONS](../../.claude/DECISIONS.md) Notes E and R13; [`presets/sdc-base.toml`](../../presets/sdc-base.toml)). Tessalith's solver differs from Castaño's uniform scale in three ways:
  - it has no ×4 cap;
  - every level passes exactly `round(p · n)` texels, where p is mip 0's share; ties are split in a Bayer order of position, so thin strands neither overshoot nor vanish;
  - texels are remapped with a monotone curve (opaque stays opaque) and kept at least 2 codes clear of the threshold, so BC7's one- or two-code error cannot flip them.
- **No mips under `interface` and `sky` folders** (built-in rules in `sdc-base`).
