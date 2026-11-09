# Texture types and naming

For anyone naming source files: this page says what each file becomes (set, sub-type, format, colour tag, alpha) and what the artist checks are telling you. Packing raw maps is on [Texture packing](Texture-Packing.md); shaders and the extra exports are on [Set shaders](Set-Shaders.md).

## Which files Tessalith reads

| Extension | How it is read |
|---|---|
| `.png`, `.tga`, `.tif`, `.tiff`, `.bmp`, `.jpg`, `.jpeg`, `.webp` | As stored. |
| `.exr`, `.hdr` | Float. A TruePBR emissive needs one (peaking above 1.0) to become BC6H; `hdr_sky` is BC6H whatever the source. An `.exr` alpha is read as premultiplied. For straight alpha set `alpha.exr_straight = true` (GUI: the Problems row's **Straight alpha**, or the tick box in the texture's Alpha group). |
| `.psd`, `.psb` | The merged image only, never layers. RGB or Grayscale, 8, 16 or 32 bits. Save with Maximize Compatibility on, or the file is refused. Colour Photoshop matted against white is un-matted. |
| `.kra`, `.ora` | Refused with `IN-DECODE`: export the image as PNG. |
| `.dds` | Skipped (`IN-DDS-SKIPPED`). Set `reencode = true` on the file to convert it anyway, at a quality cost (`LOSS-REENCODE`). |

Other extensions in a folder (`.xcf`, `.pdn`) are ignored: export to PNG or TGA first. Images over 16384 px on a side fail with `DIM-TOO-LARGE`. Any smaller size is accepted; [Format and size](Project-Config.md#format-and-size) says what size gets written.

## How files form a set

Files in the same output folder with the same base name (the file name minus its suffix) form a set:

```text
textures\armor\iron\
  iron.png          set "iron", colour map (no suffix)
  iron_n.png        set "iron", suffix _n
  iron_m.png        set "iron", suffix _m
  iron_trim.png     set "iron_trim", colour map (_trim is not a suffix)
  iron_trim_n.png   set "iron_trim", suffix _n
```

A file **waits for a type** instead of becoming a colour map when all three hold: no `[files]` entry or rule gives it a sub-type, its name has an underscore but no known suffix (aliases count), and it is alone in its set. `rock_bump.png` alone waits, because nothing says whether it is a bump map or a colour map. `iron.png` has no underscore, so it stays a colour map, and `iron_ore.png` stays one while `iron_ore_n.png` sits beside it.

Never waiting: files under `textures\cubemaps`, `landscape`, `sky` and `interface`, a kept `.dds`, and (in the GUI) a file that a scanned mesh or a GRAS model uses as its diffuse, because the mesh says what it is. A set's shader doesn't give the type. <!-- D:GY1 -->

To give the type, set `subtype` in the file's `[files]` entry, keyed by the path under the input folder, or pick it in the GUI:

```toml
[files."textures/x/rock_bump.png"]
subtype = "diffuse"
```

- **GUI:** a waiting file is Undecided, with a Problems row and a Review row, and no set card until you pick a type. High-confidence suggestions are applied for you; Medium and Low always need you.
- **Command line:** [CLI reference](CLI-Reference.md#why-you-usually-want---accept-suggestions-high).

## Suffixes

Matched case-insensitively, longest first, so `_msn` wins over `_n` and `_envmask` over `_m`.

| Suffix | Suggested sub-type | Confidence |
|---|---|---|
| none, or `_d` | `diffuse`; `pbr_albedo` in a PBR set; `skin_albedo` in an Advanced skin set; `landscape_diffuse` when a folder in the path is named `landscape`; `ui_sky` under `interface` or `sky` folders; `hdr_sky` for a float source under `sky` | High (`hdr_sky` Medium) |
| `_n` | `normal`; `pbr_normal` in a PBR set; `landscape_normal` under a `landscape` folder | High |
| `_msn` | `msn` (model-space normal) | High |
| `_s` | `msn_specular` when the set has `_msn`; `pbr_subsurface` in a PBR set; otherwise `subsurface_tint` | High, High, Low |
| `_m`, `_em`, `_envmask` | `envmask`; `complex_material` when the alpha minimum is below 251 | High for both |
| `_g` | `glow`; `pbr_emissive` in a PBR set | High |
| `_p` | `height` (parallax) | High |
| `_rmaos` | `rmaos` | High |
| `_rfaos` | `skin_rfaos` (Advanced skin) | High |
| `_cnr`, `_crn` | `coat_normal` | High |
| `_f` | `pbr_fuzz` | High |
| `_flow` | `flow` (hair flow) | High |
| `_noise` | `projected_noise` (a landscape set decides it itself; elsewhere you confirm it) | Medium |
| `_b`, `_bl` | `backlight_tint` | High |
| `_sk` | `skin_sk` in an Advanced skin set; otherwise `backlight_tint` | High, Low |
| `_i` | `diffuse` (multilayer inner layer) | Medium |
| `_wet` | `skin_wet`, only in a `cs` Advanced skin set; elsewhere `_wet` is part of the base name | High |
| `_detail`, `_detailnorm`, `_detail_n` | `detail_normal` or `skin_detail`, by the set: see [Detail names](#detail-names) | High in a skin set; in a landscape set the shader decides |
| `_e`, or a file under `textures\cubemaps\` | `cubemap`; the face suffixes `_posx` to `_negz` (or `_px` to `_nz`) are cut off first | High |

- **A PBR set** (under `cs`) has a `pbr` folder in its path, or an `_rmaos`, `_rfaos`, `_cnr` or `_f` member, or a raw roughness, glossiness, metallic or AO map.
- **An Advanced skin set** has or builds an `_rfaos`. Setting the set's shader to Advanced Skin builds one (unless `packing.recipe` says otherwise or only a metallic map exists), and its diffuse is `skin_albedo` either way. A pinned Diffuse slot turns it off ([Skin](Skin.md)). In such a set a plain `_f` is the skin fuzz mask: greyscale, packed into the `_rfaos` G channel, not converted on its own. A `_f` never makes a set Advanced skin by itself.
- **A name that fits none of the above** is read as a colour map when the set has other files (`rock_nm.png` becomes a diffuse); alone, it [waits for a type](#how-files-form-a-set). Rename it, add an `[inputs]` alias, or set `subtype`.

### Detail names

The three detail spellings all mean the set's detail normal. What the file becomes depends on the set:

| Set | The detail file becomes |
|---|---|
| Landscape or PBR Landscape (a `landscape` folder in the path, or the set's shader) | `detail_normal` |
| Advanced skin (`cs` only) | `skin_detail` |

- **The output keeps your spelling:** `iron_detailnorm.png` writes `iron_detailnorm.dds`, because the shader or [XPMF profile](PBR-Landscape.md#projected-noise-and-detail-normal) looks for that name.
- **Any other set** leaves `iron_detail.png` and `iron_detailnorm.png` as undecided members of set `iron`: pick a sub-type on the file. `iron_detail_n.png` there keeps its own set stem, as a normal in set `iron_detail`.

### Exporter names

Names that texturing tools commonly export are understood too:

| Term | Read as | Written as |
|---|---|---|
| `_colormap`, `_basecolor`, `_base_color`, `_albedo`, `_diffuse`, `_color`, `_col` | Colour map | `<base>.dds` |
| `_normal`, `_nrm`, `_norm` | Normal map, green direction not stated | `<base>_n.dds` |
| `_normalgl` | Normal map, OpenGL green (flipped on output) | `<base>_n.dds` |
| `_normaldx` | Normal map, DirectX green | `<base>_n.dds` |
| `_emissive`, `_emission` | Emissive | `<base>_g.dds` |
| `_coat` | PBR coat colour (RGB) and strength (A) | `<base>_s.dds` |
| `_roughness`, `_rough` | Raw roughness | packed |
| `_metallic`, `_metalness`, `_metal` | Raw metallic | packed |
| `_ao`, `_ambientocclusion`, `_occlusion` | Raw AO | packed |
| `_orm` | AO in R, roughness in G, metallic in B ([how it packs](Texture-Packing.md#packing-loose-maps)) | packed |
| `_specular`, `_spec` | Raw specular | packed |
| `_glossiness`, `_gloss` | Raw glossiness | packed (inverted into roughness) |
| `_height`, `_displacement`, `_disp` | Raw height | packed |
| `_opacity` | Raw opacity | the colour texture's alpha ([helper](Texture-Packing.md#helper-textures)) |
| `_fuzz`, `_sheen` | Raw fuzz | packed |
| `_curves`, `_curvature` | Raw curvature (0.5 is flat) | read only by [Also export](Set-Shaders.md#also-export-as-vanilla-and-cm) |
| `_cavity` | Raw cavity | read only by Also export |

**Green direction.** Skyrim reads DirectX normals. A file named only `_normal`, `_nrm` or `_norm` doesn't say which way its green points, so Tessalith assumes DirectX, doesn't flip, and notes `IN-GREEN-UNSTATED`. <!-- D:AW1 --> A map from Substance or Blender is often OpenGL: name it `_normalgl`, or set `channels.green_convention = "opengl"` on the file (the GUI's note has a **Treat as OpenGL (flip green)** button for it). A file named `_n` is taken as DirectX.

Your own suffixes go in a preset's `suffix_aliases`: see [Suffix aliases](Presets-and-Targets.md#suffix-aliases).

## Sub-types: what each file becomes

One row per sub-type. **vanilla** says whether the sub-type exists under the Vanilla target; when it doesn't, the file is blocked with `CFG-UNAVAILABLE`, and the GUI's acknowledge button converts it as the fallback named there (on the command line: [CLI reference](CLI-Reference.md#acknowledging-blocking-codes)). With no fallback, change the sub-type or the target.

| Sub-type | Category | Default format | Tag (`cs`) | Default alpha | vanilla | Notes |
|---|---|---|---|---|---|---|
| `diffuse` | Color | BC7 | UNORM | remove | yes | The mesh decides the alpha. |
| `landscape_diffuse` | Color | BC7 | UNORM | data | yes | Alpha is the snow/height mask. BC1 refused (`FMT-LANDSCAPE-BC1`). |
| `pbr_albedo` | Color | BC7 | sRGB | remove | `diffuse` | `cs` only. |
| `pbr_emissive` | Color | BC6H for a float source above 1.0, else BC7 | n/a or sRGB | remove | `glow` | `cs` only. `FMT-HDR-SOURCE`. |
| `glow` | Color | BC7 | UNORM | remove | yes | |
| `pbr_subsurface` | Color | BC7 | sRGB | data | no | `cs` only. Also the coat colour map. |
| `pbr_fuzz` | Color | BC7 | sRGB | data | no | `cs` only. |
| `subsurface_tint` | Color | BC7 | UNORM | remove | yes | |
| `backlight_tint` | Color | BC7 | UNORM | remove | yes | |
| `ui_sky` | Color | BC7 | UNORM | remove | yes | No mips. |
| `skin_albedo` | Color | BC7 | UNORM | remove | no | Advanced skin. UNORM because the CS skin shader reads it that way. |
| `cubemap` | Color | BC7 (BC1 for `vanilla_metal` and `room`) | UNORM | remove | yes | See [Cubemaps](Cubemaps.md). BC1, BC3, BC7, RGBA8 or RGB8 only. |
| `normal` | Normal | BC7 | UNORM | data | yes | Alpha often holds specular. |
| `landscape_normal` | Normal | BC7 | UNORM | data | yes | |
| `pbr_normal` | Normal | BC7 | UNORM | remove | `normal` | `cs` only. Vanilla specular in alpha is dropped. |
| `detail_normal` | Normal | BC7 | UNORM | data | yes | A landscape set's detail normal ([Detail names](#detail-names)). |
| `msn` | Normal | BC7 | UNORM | remove | yes | Model-space normal. Pick the baker with [`channels.msn_source`](Project-Config.md#channels) (*Baked in* in the GUI). |
| `coat_normal` | Normal | BC7 | UNORM | data | no | `cs` only. Coat roughness in alpha. |
| `skin_wet` | Normal | BC7 | UNORM | data | no | A grey source isn't written (`SKIN-WET-GREY`). CS reads only some formats here: see `FMT-CS-SKIN-LOADER`. <!-- D:HB3,HB4 --> |
| `skin_detail` | Normal | BC5 when the source is opaque, else BC7 | UNORM | data | no | A tangent normal with a strength mask in alpha. BC5 is allowed on it. <!-- D:HB4 --> |
| `envmask` | Grayscale | BC4 | UNORM | remove, exact opaque | yes | `_m`. |
| `height` | Grayscale | BC4 | UNORM | remove | yes | `_p`. |
| `projected_noise` | Grayscale | BC4 | UNORM | remove | yes | `_noise`, for an XPMF profile. |
| `msn_specular` | Grayscale | BC4 | UNORM | remove | yes | `_s` beside an `_msn`. |
| `skin_sk` | Grayscale | BC4 | UNORM | remove | no | Red channel only. |
| `gray` | Grayscale | BC4 | UNORM | remove | yes | A plain grey map you pick on the file. BC4 or R8 only. Reads red unless [`channels.grayscale_source`](Project-Config.md#channels) says otherwise. <!-- D:FM37 --> |
| `complex_material` | MixedPacked | BC7 | UNORM | data, exact opaque | yes | Complex `_m`. |
| `rmaos` | MixedPacked | BC7 | UNORM | data | no | `cs` only. |
| `skin_rfaos` | MixedPacked | BC7 | UNORM | data | no | |
| `flow` | MixedPacked | BC5 | UNORM | remove | no | Hair flow. At 32 px or less CS ignores it (`DIM-FLOW-SMALL`). |
| `generic_two_channel` | MixedPacked | BC5 | UNORM | remove | yes | |
| `generic_data` | MixedPacked | BC7 | UNORM | data | yes | Any data map Tessalith has no type for. |
| `complex_grass` | MixedPacked | BC7 only | UNORM only | cutout (when alpha varies) | yes | Own size. Both targets accept it; only CS and ENB read it. See [Complex grass](PBR-Landscape.md#complex-grass). |
| `hdr_sky` | Hdr | BC6H | n/a | | no | `cs` only. |

- **Default alpha is a start.** Analysis decides: no alpha channel resolves to `remove`; alpha is opaque when its minimum is 251 or more, or when fewer than 0.1 % of texels are below 251 and no connected group of them is larger than 16 texels (stray noise from a re-encode). <!-- D:FM34 --> A colour map whose alpha really varies is undecided unless scanned meshes answer.
- **Exact opaque** means opaque alpha decodes as exactly 255 in BC7, not 254. PGPatcher and ENB read the alpha of `_m`, so it is on for those, and turning it off on a BC7 `_m` is blocked (`ALPHA-EXACT-OFF-M`).
- **Normal maps are never BC4 or BC5:** the game would decode Z as −1 and lose the alpha (`FMT-BC5-NORMAL`, blocking).
- **HDR formats** (BC6H, RGBA16F) exist only under `cs`; an emissive becomes BC6H only from a float source above 1.0.

### sRGB or UNORM

Under `cs`, PBR colour maps (`pbr_albedo`, `pbr_emissive` as BC7, `pbr_subsurface`, `pbr_fuzz`) are tagged `_SRGB` (`TAG-UNORM-PBR` if not) and everything else is UNORM, `diffuse` and `skin_albedo` included (`TAG-SRGB-NONPBR`). Vanilla tags nothing `_SRGB` (`TAG-SRGB-VANILLA`), and neither does anything under a `lod` folder, because LOD shaders reject it (`TAG-SRGB-LOD`). A wrong tag makes the texture look too dark or washed out.

## Alpha

| Policy | Meaning | Mips |
|---|---|---|
| `remove` | No alpha; stored opaque. | |
| `cutout` | Alpha test against `alpha.threshold` with `alpha.compare`. | Coverage preserved per mip (unless `mips.coverage = "off"`), so foliage doesn't thin out at a distance. |
| `blend` | Alpha blending. | Box-averaged opacity. |
| `cutout_blend` | Both. | Coverage preserved. |
| `data` | Alpha is data (specular, height, a mask). | A plain channel. |

Throwing away varying alpha by accident is blocked (`LOSS-ALPHA-DATA`), unless you chose `remove` yourself or the file is a `pbr_normal`.

### BC1 and 1-bit alpha

- BC1 is opaque unless you turn on `encoder.bc1_alpha` (GUI: **1-bit alpha**); nothing turns it on for you. Without it, BC1 with alpha is `FMT-BC1-ALPHA` (blocking), and so is `data` alpha, unless that alpha is empty (no channel, or 255 everywhere). A `complex_material` stays refused: its empty alpha is written as 0, which BC1 can't hold with colour. <!-- D:FM20,FM39 -->
- With it on, a `cutout`, `blend` or `cutout_blend` texture keeps 1-bit alpha: texels below the cutoff become transparent black, the rest opaque. The cutoff is `alpha.threshold` + 1 with `gt` and `alpha.threshold` with `ge` (128 by default). `ALPHA-BC1-ONE-BIT` warns when more than 1 % of the source texels have alpha between 1 and 254.

## Artist checks

Four checks read the numbers Tessalith already measures while it plans, and warn about maps that will probably look wrong in game. They only warn: none blocks an export, asks you anything or changes "ready", and none reaches Review or the Files rows. The Problems tab lists them with a grey **check** dot and a **Checks** pill ([Problems](GUI-Problems.md)). Each has a switch in **Advanced → Checks** (all on, saved in `gui.toml`); a switch hides its results at once, with no re-plan.

| Check | What it flags | Code |
|---|---|---|
| **Low detail** | A top mip that adds almost nothing over mip 1 (an upscale, a 2K made from a 1K). | `CHK-LOW-DETAIL` |
| **Green direction** | A normal that curls like OpenGL when read as DirectX (an Info hint; it never flips by itself, and **Treat as OpenGL (flip green)** is one undo step). A blue channel that points the wrong way. | `CHK-GREEN-GUESS`, `CHK-Z-SIGN` |
| **Parallax** | A height map or complex `_m` alpha that uses too little range, clips, is mostly white, or is steep enough to tear. Landscape sets whose `displacementScale` differs by more than 2×. | `CHK-PARALLAX`, `CHK-PARALLAX-SCALE` |
| **UV coverage** | UV islands closer than 8 texels for more than 2 % of the used texels, so colours bleed across them at a distance (mip 3 and up). | `CHK-UV-GUTTER` |

The green-direction test is weak on straight ridges: look at the map.

### UV coverage

It reads the Meshes scan, so scan your MO2 instance first ([Mod scanning](Mod-Scanning.md)). Switch on the preview's **UV coverage** toggle for one texture ([Previews](Previews.md)), or press **Check all UV coverage** in Advanced → Checks for every colour texture of the folder (a bar shows *done / total*; **Stop checking UV coverage** ends it). The toggle is greyed out, with the reason, when:

- no scan has finished (an empty scan counts as none);
- a scan is running, or was stopped ("The Meshes scan was stopped: scan again to check UV coverage");
- the sub-type is anything but diffuse, PBR albedo or skin albedo. Landscape diffuses are left out: terrain doesn't use NIF shapes.

### Low detail

With **Low detail** on, Export measures each texture it writes and writes it at its planned size; a texture that adds little detail gets the warning in Problems. Nothing is asked and nothing is held back. To write it at half size, set **Advanced → Resize** to *Half* ([`resize`](Project-Config.md#format-and-size)). Only textures that *Half* would shrink, with no resize set yet, are measured. <!-- D:FM29 -->

## Height maps

The Parallax check judges a height map against the neutral height (the value that shifts nothing) for the set's shader. White is raised and black is deepest on every path.

| Path | Neutral | Where the height lives |
|---|---|---|
| Vanilla parallax | 0.5 | `_p`, red |
| Complex material | 0.5 | `_m` alpha. Parallax is on only where alpha is strictly between 4/255 and 251/255; 4/255 or less keeps complex material on with no parallax |
| PBR parallax | 0.5 | `_p`, red |
| PBR multilayer with inner-layer (coat) parallax | 1.0 (white); black is the deepest recess and nothing is raised | `_p`, red |
| Landscape, vanilla and PBR | 0.5 per layer | The layer alpha or displacement map |
| Vanilla multilayer | Not a height map: the layer texture's alpha is a depth, so the height checks are skipped | The layer texture's alpha |

A mostly white `_p` is normal on the coat path and a warning on the 0.5 paths.

## Files named _generated

A file named `<base>_<role>_generated.png` is read as an ordinary source. Make them in any tool.

| File | Used as |
|---|---|
| `<base>_p_generated.png` | The set's `_p` |
| `<base>_height_generated.png` | The raw height packed into a complex material `_m` alpha (vanilla target) |
| `<base>_ao_generated.png` | The AO packed into the `_rmaos` blue channel (PBR sets, `cs` only) |
| `<base>_curves_generated.png` | The set's `_curves` (read only by Also export) |

- `_generated` after one of those roles is stripped when grouping, so the file joins its set by base name. A real map for the same slot wins and the generated file is skipped (`GEN-REPLACED`, Info).
- `_ao_generated` is read as visibility (1 = open). It fills B of a pre-packed `_rmaos`/`_rfaos` whose blue is flat white, keeping R, G and A (`GEN-AO`, Info).
- `_curves_generated` is read only by Also export's soft light, as a 0.5-centred map with peaks bright. A plain export doesn't use it.

## Where next

- Raw maps and helper textures: [Texture packing](Texture-Packing.md).
- Shaders and the Vanilla/CM export: [Set shaders](Set-Shaders.md).
- Targets, presets and your own suffixes: [Presets and targets](Presets-and-Targets.md).
- Which encoder each format uses: [Research](Research.md).
