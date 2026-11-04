# Project config fields

Every `sdc.toml` field with its values, default and meaning. A lookup page: how settings layer, rules and `[files]` are on [Project config](Project-Config.md).

Every field can be set in `[defaults]`, `[subtypes.<id>]`, a rule's `overrides`, a `[files]` entry, or with `--set`, except `subtype`, `target`, `category` and `resize_reference` (see `subtype` below). Defaults are the built-in base preset's; a sub-type or preset may change them, as noted.

## Type and colour

| Field | Values | Default | Notes |
|---|---|---|---|
| `subtype` | a sub-type id | from the file name | Refused in a `subtype:` scoped `--set`. In `[defaults]` and `[subtypes]` it is a load error, as are `target`, `category` and `resize_reference` there. |
| `color.content` | `srgb`, `linear` | `linear`; `srgb` for colour sub-types | What the pixels mean. Affects mip filtering. On a `cubemap` it stays `srgb` but means the game's `pow(c, 1.6)` curve, not the sRGB curve. |
| `color.tag` | `srgb`, `unorm`, `n/a` | `unorm`; `srgb` for PBR colour under `cs`; `n/a` for HDR | The DDS format's `_SRGB` flag. `n/a` for BC6H and RGBA16F. See [Presets and targets](Presets-and-Targets.md#srgb-tagging). |

## Alpha

| Field | Values | Default | Notes |
|---|---|---|---|
| `alpha.policy` | `remove`, `cutout`, `blend`, `cutout_blend`, `data` | `remove`; `data` for packed and data maps | `remove` stores opaque alpha. `cutout` is alpha testing, `blend` alpha blending, `cutout_blend` both. `data` keeps alpha as numbers with no visual meaning. |
| `alpha.threshold` | 1 to 254 | 127 | The alpha-test threshold the mesh uses. Used for coverage preservation. |
| `alpha.compare` | `gt`, `ge` | `gt` | Greater than, or greater than or equal, as the mesh tests. |
| `alpha.exact_opaque` | `true`, `false` | `false`; `true` for `envmask` and `complex_material` | Opaque alpha decodes as exactly 255 in BC7 ([Texture types](Texture-Types.md#sub-types)). |
| `alpha.exr_straight` | `true`, `false` | `false` | An OpenEXR source with alpha is read as premultiplied (the OpenEXR standard) and its colour divided by alpha on load. `true` reads it as straight alpha, as stored. |

## Channels

| Field | Values | Default | Notes |
|---|---|---|---|
| `channels.swizzle` | four characters from `r g b a 0 1` | none | Reorder channels before encoding, for example `"bgra"` or `"rrr1"`. Anything else is `CFG-INVALID`. |
| `channels.flip_green` | `true`, `false` | `false` | Invert the green channel of a normal map. |
| `channels.green_convention` | `directx`, `opengl` | unset | Which way the source normal map's green points. `opengl` turns on `flip_green`. A plain `_normal` file with this unset is converted as DirectX and notes `IN-GREEN-UNSTATED` (info). An explicit `flip_green` that contradicts the convention gives `CFG-GREEN-CONFLICT`, and the explicit `flip_green` wins. |
| `channels.reconstruct_z` | `true`, `false` | `false` | Rebuild the normal's blue channel from red and green. |
| `channels.grayscale_source` | `r`, `g`, `b`, `a`, `luma` | `r` | The channel a grayscale value is read from in a colour source: red, green, blue, alpha or Rec. 709 luminance. It applies to grayscale sub-types and to the raw maps that [pack](Texture-Packing.md#packing-raw-maps) into a recipe. With `a` the alpha is the data, so it is not reported as discarded; a source with no alpha channel reads 255 (`CHAN-NOT-GRAY`). To pick for one raw map only, set [`helper.read`](Project-Config.md) on that file; it beats this value. |

## Mipmaps

| Field | Values | Default | Notes |
|---|---|---|---|
| `mips.levels` | `full`, `none`, or a count | `full`; `none` for `ui_sky` and under `interface` and `sky` folders | Number of mip levels. |
| `mips.filter` | `box`, `tent`, `mitchell`, `kaiser` | `box` | Downsampling filter. Tent, Mitchell and Kaiser are for sRGB colour content. With `cutout`, `blend` or `cutout_blend` only `tent` is allowed: it weights each tap by alpha, and Mitchell and Kaiser ring at cutout edges. HDR textures without alpha weighting may use `box` or `tent`. An unscoped filter a texture cannot use falls back to box with `CFG-FILTER-IGNORED` (info); a scoped one is `CFG-INVALID`. |
| `mips.edge` | `wrap`, `clamp` | `wrap` | How tent, Mitchell and Kaiser read past the texture edge. Ignored by box. |
| `mips.normal` | `renormalize`, `renormalize_bake` | `renormalize`; `renormalize_bake` for `coat_normal` | How normal-map mips are built. `renormalize_bake` also bakes the normal detail lost in each mip into the roughness stored in the same texture (the coat roughness of a `_cnr`). |
| `mips.normal_base` | `renormalize`, `keep` | `renormalize` | Mip 0 of a normal map: stored at unit length, or as authored. |
| `mips.bake_clamp` | greater than 0, up to 1 | 0.25 | Limits how much roughness the variance bake can add (at most the square of this value). |
| `mips.coverage` | `auto`, `on`, `off` | `auto` | Alpha-coverage preservation. Applies to `cutout`, `cutout_blend`, and `blend` with `encoder.bc1_alpha`; there `auto` and `on` both enable it. |

## Format and size

| Field | Values | Default | Notes |
|---|---|---|---|
| `format` | `bc1`, `bc3`, `bc4`, `bc5`, `bc6h_uf16`, `bc7`, `rgba8`, `rgb8`, `r8`, `rg8`, `rgba16f` | `bc7`; see [Texture types](Texture-Types.md#formats) | `bc6h_uf16` and `rgba16f` only under `cs`. BC1 keeps alpha only with `encoder.bc1_alpha`. `rgb8` is 24-bit (no alpha, linear only). |
| `header` | `auto`, `dx10`, `legacy` | `auto` | DDS header form. `auto` writes a legacy header for BC1, BC3 and RGBA8 without the `_SRGB` tag, and the DX10 header for everything else. `legacy` is valid only for formats that have a legacy form (such as BC1, BC3, BC4, BC5) and no `_SRGB` tag; otherwise it is `CFG-INVALID`. |
| `resize` | `none`, `half`, `quarter`, `eighth` | `none` | Halves both sides once, twice or three times. Each halving happens only while the longer side stays at or above `resize_floor`: with the 512 floor, 2048x1024 under `quarter` becomes 512x256, and 1024x1024 under `eighth` stops at 512x512. <!-- D:FM29 --> |
| `resize_relative_to` | `self`, `slot1` to `slot8` | `self` | What `resize` is measured from. `self` halves the texture's own size. A slot measures from the source size of the set's map in that texture-set slot (1 diffuse, 2 normal, 3 glow or skin, 4 height, 5 environment, 6 environment mask, complex material or RMAOS, 7 multilayer or coat, 8 back-light, specular or PBR subsurface): the texture halves while its longer side is above that map's size after the same `resize` and power-of-two step, and is never enlarged. With a 2048 diffuse and `half`, a 2048 normal becomes 1024 and a 1024 or 512 normal keeps its size. It does nothing under `resize = "none"`, and the texture's own slot works as `self`. When the set has no map in the slot, or more than one, `resize` applies to the texture's own size, as with `self`, and the texture gets `RESIZE-NO-REFERENCE`. <!-- D:FM44 --> |
| `resize_floor` | pixels, 1 to 16384 | 512; 256 for `height` | The longer side `resize` never halves below. Outside 1 to 16384 it is `CFG-INVALID`. <!-- D:FM29 --> |

Every texture with mips is written at a power-of-two size, before `resize` applies. The longer side rounds up to the next power of two when it is at least 95 % of it, otherwise down: 1000 becomes 1024, 900 becomes 512. The shorter side is first scaled by the same factor, then rounds up when it is at least 85 % of the next power of two, so 1024x768 becomes 1024x512 and 1000x600 becomes 1024x512. A side that grows is enlarged with linear interpolation. A texture with `mips.levels = "none"` keeps its own size, gets the `DIM-NPOT` warning, and still takes `resize`. A changed size is noted as `DIM-RESIZED`, with the source and output sizes. A `cubemap` ignores `resize` and `resize_floor`: its size is [`cubemap.face_size`](#cubemap). <!-- D:FM29 -->

## Encoder

| Field | Values | Default | Notes |
|---|---|---|---|
| `encoder.effort` | `ultrafast`, `veryfast`, `fast`, `basic`, `veryslow`, `slowest`, `max` | `veryslow` | bc7e quality level. `max` is the patch-0002 level from the [bc7e tuning study](../research/bc7e-tuning.md), at 2.7 to 2.9× the time of `veryslow`. For BC6H, `ultrafast` and `veryfast` map to veryfast and `veryslow` to veryslow; `slowest` and `max` both use a peak-preserving veryslow that keeps small, very bright highlights (a sun) near their source brightness. A BC6H texture with no effort set anywhere but the built-in defaults uses `slowest`; set `veryslow` for the plain mode. <!-- D:FM43 --> |
| `encoder.perceptual` | `true`, `false` | `false` | Perceptual error metric for BC1, BC3 and BC7: BC7 uses bc7e's own weights; BC1 and BC3's colour block weight red, green and blue error by Rec.709 luma (54, 183, 19 of 256), which keeps luma detail at some cost in colour (on 93 real textures about +0.7 dB luma PSNR and −0.3 dB RGB PSNR, [measurements](../research/encoder-benchmark.md#luma-weighted-bc1-error-2026-10-31)). Other formats ignore it. Allowed only on colour sub-types whose alpha policy is not `data`; anywhere else it is an error (`FMT-PERCEPTUAL-DATA`). `vanilla-quality` turns it on for `diffuse` and `glow` (BC1, BC3 and BC7; it drops out where the alpha is `data`), and `vanilla-compact` also for its BC1 colour maps. <!-- D:FM43 --> |
| `encoder.bc7_weights` | `linear`, `alpha2`, `alpha8`, `alpha16` | unset | BC7 linear error weights for bc7e (R, G, B, A): `linear` 1,1,1,1, the same encode as unset (use it to override an inherited weighting for one file); `alpha2` 1,1,1,2; `alpha8` 1,1,1,8 (an alternative to BC3 for delicate alpha); `alpha16` 1,1,1,16 (a `_n` with specular in alpha). Unset encodes as `linear` and is not written to profiles or manifests. `encoder.perceptual = true` wins over it. On another format, or with perceptual on, it has no effect and raises `ENC-WEIGHTS-IGNORED`. The GUI's *BC7 weighting* row edits both fields. |
| `encoder.bc1_alpha` | `true`, `false` | `false` | BC1 with 1-bit alpha for `cutout`, `blend` and `cutout_blend` ([BC1 alpha](Texture-Types.md#alpha-policies)). Ignored by other formats. |
| `encoder.bc45_search_radius` | `default`, `max`, `fast` | `default` | BC4/BC5 endpoint search. `max` searches a radius of 10, `fast` a radius of 4. |

## Validation

| Field | Values | Default | Notes |
|---|---|---|---|
| `validation.mips` | `all`, `smallest` | `all` | `smallest` runs the PSNR check on the smallest mip only (used by `fast-preview`). Coverage is always checked on every level. |

## Packing

| Field | Values | Default | Notes |
|---|---|---|---|
| `packing.enabled` | `true`, `false` | `true` | Build this packed output. `false` leaves its raw maps as packing inputs (`IN-PACKING-INPUT`). |
| `packing.roughness_default` | 0 to 1 | 0.5 | Roughness used when there is no roughness or glossiness source. |
| `packing.prefer` | `prepacked`, `raw` | `prepacked` | When a set has both a packed file (such as `_rmaos.png`) and raw maps: keep the packed file, or build from the raw maps (`PACK-CONFLICT`). |
| `packing.recipe` | `auto`, `rmaos`, `rfaos` | `auto` | Which `cs` recipe a set packs. `auto` builds `_rfaos` when a fuzz source exists, otherwise `_rmaos`. |
| `packing.fill.<role>` | 0 to 255 | unset | Value for a channel with no source map. Roles: `roughness`, `metallic`, `ao`, `specular`, `glossiness`, `height`, `opacity`, `fuzz`. Unset means the neutral default (AO 1, metallic 0, specular 1, roughness `roughness_default`, height 0). Set it per file or sub-type; `[defaults]` takes no fills. |

Packing fields belong on the packed output's key, for example `[files."textures/armor/iron/iron_rmaos"]`.

## RNM normal layers

These apply to a base normal (`_n`, `pbr_normal`, `landscape_normal`, or `_cnr`) that has layer files. See [Texture types](Texture-Packing.md#rnm-normal-layers).

| Field | Values | Default | Notes |
|---|---|---|---|
| `rnm.enabled` | `true`, `false` | `true` | Blend the layers into the base normal. |
| `rnm.order` | list of `bump`, `bump2`, `detail`, `detail2`, `bent` | `["bent", "bump", "bump2", "detail", "detail2"]` | Blend order. Missing ids are appended in the default order. |
| `rnm.<layer>.strength` | 0 to 2 | bump 0.4, bump2 0.4, detail 0.6, detail2 0.3, bent 0.35 | Layer strength. `<layer>` is `bump`, `bump2`, `detail`, `detail2` or `bent`. |
| `rnm.<layer>.flip_green` | `true`, `false` | the base's `channels.flip_green` | Invert this layer's green channel. |
| `rnm.<layer>.mask` | `true`, `false` | `true` | Use the layer's mask file when one exists. |
| `rnm.bent.mode` | `ao_blend`, `detail` | `ao_blend` | Blend the bent normal weighted by occlusion, or like a detail layer. |
| `rnm.bent.ao_power` | 0.1 to 8 | 1.5 | Power on the occlusion (1 minus AO). |
| `rnm.bent.ao` | `true`, `false` | `true` | Read the AO source (a raw `_ao` map, or the B channel of `_rmaos`/`_rfaos`); `false` uses a uniform weight. Supply normal AO (white = open); Tessalith inverts it. |
| `rnm.target` | `n`, `cnr`, `both` | from the layer's name | Per layer file (`[files]` or `file:` `--set`), or an unscoped `--set` for every layer file; refused in a `subtype:` scope. Which base normal it feeds when a set has both `_n` and `_cnr`. |

## Cubemap

These apply to the `cubemap` sub-type (`_e` files and files under a `cubemaps` folder) and are ignored by every other. See [Cubemaps](Cubemaps.md).

| Field | Values | Default | Notes |
|---|---|---|---|
| `cubemap.style` | `vanilla_metal`, `room`, `eye`, `hd` | unset | Fills the keys below that nothing else set: `vanilla_metal` 32 px, BC1, blur 0.6, saturation 0.8; `room` 128 px, BC1, blur 0.35; `eye` 256 px, BC7, blur 0.35; `hd` 512 px, BC7, blur 0.1. |
| `cubemap.layout` | `faces`, `hcross`, `vcross`, `hstrip`, `vstrip`, `equirect`, `mirror_ball`, `angular`, `photo` | from the image shape | How each layout is detected: [Cubemap sources](Cubemap-Sources.md#ways-to-make-one). `faces` is six separate files. A cube DDS must leave it unset. |
| `cubemap.face_size` | a power of two, 4 to 4096 | one source cell rounded to a power of two, at most 4096 | Face edge in pixels ([source cell per layout](Cubemap-Sources.md#face-size)). |
| `cubemap.yaw` | −360 to 360 | 0 | Turns the cube about world up, in degrees. Positive is clockwise seen from above, the way Skyrim headings turn: 90 moves east to south. |
| `cubemap.mirror` | `true`, `false` | `false` | Swaps world east and west. |
| `cubemap.face_rotate` | six of 0, 90, 180, 270 | `[0, 0, 0, 0, 0, 0]` | Clockwise turn per face, in the order +X, −X, +Y, −Y, +Z, −Z. Applied before the flip. Works on every layout. |
| `cubemap.face_flip` | six of `"none"`, `"h"`, `"v"` | all `"none"` | `h` mirrors left-right, `v` flips top-bottom. |
| `cubemap.vcross_negz_rotate` | `true`, `false` | `true` | Turn a vertical cross's bottom cell (−Z) by 180°. Other layouts ignore it. |
| `cubemap.blur` | 0 to 1 | none (0), the style's, or 0.6 for `photo` | Cone blur across the faces. A `photo` layout blurs at 0.6 unless you set this. |
| `cubemap.exposure` | −10 to 10 | 0 | Stops, in linear light. 1 doubles the light. |
| `cubemap.brightness` | −1 to 1 | 0 | Added to each channel, on the game curve. |
| `cubemap.contrast` | −1 to 1 | 0 | Scales around mid-grey by (1 + contrast), on the game curve. |
| `cubemap.midtones` | 0.1 to 10 | 1 | A power curve on the game curve: above 1 brightens the mid-tones, below 1 darkens them, 1 is off. |
| `cubemap.saturation` | 0 to 2 | 1 | 0 is grey. |
| `cubemap.tint` | `[R, G, B]`, each 0 to 255 | unset | Each channel is multiplied by its value / 255, on the game curve (255, 255, 255 changes nothing). |
| `cubemap.highlight_clamp` | 1 or more | unset | Soft-compresses brightness so that this linear level becomes full white; lower values are also pulled down a little, and hue is kept. For HDR sources. |
| `cubemap.ground_fill` | `"gradient"` or `[R, G, B]`, each 0 to 255 | unset (no fill) | Replaces everything below `cubemap.horizon`. `"gradient"` starts from the colour at the horizon and darkens it to half at straight down. A colour is read as code values on the game curve (`pow(c, 1.6)`), not sRGB. Unset means no fill, and `horizon` alone does nothing. |
| `cubemap.horizon` | −45 to 45 | 0 | Horizon elevation in degrees: 0 is level, positive is above it. Read only with `ground_fill`. |

The order the cube is built in is in [Cubemaps](Cubemaps.md#order-of-work).

A cube's formats are listed in [Cubemaps](Cubemaps.md#inside-the-file); any other `format` or an sRGB tag is `CFG-INVALID`. `mips.levels = "none"` raises `CUBE-NO-MIPS`.

## Re-encoding DDS inputs

| Field | Values | Default | Notes |
|---|---|---|---|
| `reencode` | `true`, `false` | `false` | Per file only (`[files]`, unscoped or `file:` `--set`). DDS inputs are skipped unless this is `true`. |
