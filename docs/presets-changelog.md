# Built-in presets changelog

One entry per built-in preset version. A version changes when anything the preset resolves to changes; the workspace
version changes with it so `--on-existing if-changed` re-encodes (spec §3.6).

## 2026-11-03 — sdc 2.0.0 (every preset 2.0.0)

- All six built-in presets move to 2.0.0 and every output re-encodes once under `if-changed` (engine version 2.0.0).
- `sdc-base` loses the keys 2.0 removed: `encoder.backend`, `encoder.rgbcx_level`, `encoder.bc1_transparent_black`
  and the five `validation` thresholds. The format picks the encoder, BC1 and BC3 colour use rgbcx level 18 (the value
  every preset set), and the thresholds are fixed at the values the presets used. A user preset that still names one is
  refused with a message that names the key.
- New sub-types in `sdc-base`: `gray` (BC4, read from the red channel), `projected_noise` (BC4) and `detail_normal`
  (alpha `data`). `height` gets `resize_floor = 256`, so a `half` height map shrinks down to 256 px.
- `cs-compact` and `vanilla-compact` halve `projected_noise`, `detail_normal` and `gray` like the other data maps (larger
  side above 512).
- `cs-compact` and `vanilla-compact` set `resize_relative_to = "slot1"` on every halved sub-type except `diffuse`,
  `landscape_diffuse`, `pbr_albedo` and `skin_albedo`: such a map halves only while it is larger than the set's
  halved slot-1 map, so a normal already exported smaller than its diffuse keeps its size; a map in a set with no
  slot-1 map halves its own size as before (`RESIZE-NO-REFERENCE`). Those 25 sub-types' profile hashes change.
- `vanilla-compact` sets `encoder.perceptual = true` in its two BC1 colour rules (luma-weighted BC1), so those outputs
  change; its data maps do not.
- `vanilla-quality` sets `encoder.perceptual = true` in its `diffuse`, `glow` and `skin_albedo` tables, and
  `vanilla-compact` inherits them. Those sub-types are perceptual in BC1, BC3 and BC7, so a vanilla diffuse or glow
  re-encodes once; a file whose alpha is `data` is not perceptual (the preset's value drops out). `skin_albedo` is
  CS-only, so the table has no effect under `vanilla`. `cs-quality`, `cs-compact` and `fast-preview` do not change.
- No preset sets the BC6H effort. The engine encodes a BC6H file at `slowest` (the peak-preserving mode) when no layer
  above `sdc-base` chose the effort, so every preset's BC6H output changes and `fast-preview` keeps `fast`.
- `vanilla-compact`'s description says its BC1 colour is perceptual.

## 2026-10-28 — sdc 1.1.0 (sdc-base 1.2.0)

- New sub-type `cubemap` (`_e`, or any file under a `cubemaps` folder): sRGB content, UNORM tag; format BC7, alpha
  remove and full mips come from `[defaults]`. Cubes are written as BC1, BC3, BC7 or RGBA8, UNORM only.
- New `cubemap.*` keys, read only by `cubemap` and skipped at their defaults, so no 2D profile hash changes.
- `cubemap.style` fills the keys you did not set yourself:

  | style | face_size | format | blur | saturation |
  |---|---|---|---|---|
  | `vanilla_metal` | 32 | bc1 | 0.6 | 0.8 |
  | `room` | 128 | bc1 | 0.35 | — |
  | `eye` | 256 | bc7 | 0.35 | — |
  | `hd` | 512 | bc7 | 0.1 | — |

- No 2D output changes, and `if-changed` re-encodes nothing that is not a cube (per-type encode versions, Note DM).

## 2026-10-22 — sdc 1.0.0

- Version bump only; no preset or output change. Every output re-encodes once under `if-changed` (engine version 1.0.0).

## 2026-10-19 — sdc 0.9.0

- Version bump only; no preset or output change. Every output re-encodes once under `if-changed` (engine version 0.9.0).

## 2026-10-15 — sdc 0.8.0 (public alpha)

- Version bump only; no preset or output change. Every output re-encodes once under `if-changed` (engine version 0.8.0).
- **cpm-quality removed** with the `cpm` target (DECISIONS Note AZ). `base = "cpm-quality"` and `target = "cpm"` are
  errors. A vanilla set with the complex material shader writes the same complex `_m` the cpm target did.

## 2026-10-12 — sdc 0.7.0 (M6 Track F2)

- RNM normal layers: `_bump`, `_bump2`, `_detailnorm`, `_detailnorm2`, `_bent` and their masks (plus rnm.py's aliases and
  `_n_`/`_cnr_` prefixes) blend into the set's `_n`/`_cnr` before mips. New `rnm.*` profile keys (defaults: bump 0.4,
  detail 0.6, detail2 0.3, bent 0.35, AO power 1.5) and the per-file `rnm.target`.
- These files were previously their own sets and could be written as a diffuse (`foo_bump.dds`). They are no longer
  written; sdc deletes nothing, so remove any stale `*_bump.dds`/`*_detailnorm.dds` from an earlier export yourself.
- Every output re-encodes once under `if-changed` (engine version 0.7.0).
- `sdc convert body_n.png` on a single file does not blend layers (it discovers only that file); run it on the folder.

## 2026-10-12 — sdc 0.5.0 (M6 Track D)

- **Quality change, every preset:** the vendored rgbcx encoder now rounds BC4 interpolants to nearest, as bcdec and the
  GPU do (DECISIONS T13). Every BC4, BC5 and BC3-alpha output improves (about +3.07 dB BC4 and +2.61 dB BC5 at the
  default search radius, on the bench corpus), so outputs from earlier versions re-encode under `if-changed`.
- **cs-compact 1.0.0** (new; base cs-quality, target cs): BC1 for opaque colour maps (no alpha, or alpha constant
  255 where the policy resolves to remove); colour maps that stay BC7 and data maps (normals, masks, packed and
  single-channel maps) at half size, both sides, when the larger side is above 512; opaque colour that becomes BC1,
  emissive and sky stay full size. BC4/BC5 search radius `fast` (4: −0.06 dB BC4 and −0.04 dB BC5 against 5, at
  about 0.7× the time).
- **vanilla-compact 1.0.0** (new; base vanilla-quality, target vanilla): the same for Vanilla/ENB.
- **fast-preview 1.0.0** (new; base sdc-base, any target): bc7e `fast`, PSNR validation on the
  smallest mip only (coverage on every level). BC4/BC5 search radius `fast` (4). For checking a mod quickly, not for release.
- **sdc-base**: unchanged (1.1.0). The rule that would raise the default radius to `max` did not fire (+0.12 dB BC4,
  +0.06 dB BC5, at 2.8× and 3.0× the time). cs-quality, vanilla-quality and cpm-quality inherit it.

## Earlier versions

- **sdc-base 1.1.0**, **cs-quality 1.0.0**, **vanilla-quality 1.0.0**, **cpm-quality 1.0.0**: as shipped before M6
  Track D (no changelog was kept).
