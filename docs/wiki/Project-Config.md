# Project config (sdc.toml)

For anyone writing `sdc.toml` by hand or checking what the GUI saved: every key and every field, with its values and default. What Also export's folders hold is on [Set shaders](Set-Shaders.md#also-export-as-vanilla-and-cm); what each texture type is, on [Texture types](Texture-Types.md).

## Where the files are

- **Project config:** `sdc.toml` in the folder you convert. The CLI reads the one in the first input folder, or `--config <file>`. The GUI reads and writes the one in the folder you open.
- **User config:** `%APPDATA%\sdc\config.toml`, for personal defaults such as a preferred target. Same format, but `[files]`, `[inputs]` and `[export]` belong to one project: each is refused here with `[files] is only valid in a project sdc.toml, not in <path>`.
- **Errors:** an unknown key, an unknown value or a wrong type stops the CLI with exit 2. The GUI refuses the file too ([When `sdc.toml` is rejected](GUI-Main-Window.md#when-sdctoml-is-rejected)).
- **Checking a file:** `sdc inspect <folder> --recursive > $null` stops with the error and names the problem. `sdc presets schema config` prints the JSON Schema; the repository has it as `schema/config.schema.json` for editor completion.

## A complete example

```toml
schema_version = 1
preset = "cs-quality"
target = "cs"

# Applies to every texture, before sub-type tables
[defaults]
encoder.effort = "veryslow"
mips.filter = "box"

# Applies to one sub-type
[subtypes.diffuse]
mips.filter = "kaiser"

[subtypes.normal]
channels.green_convention = "directx"

# Rules: every rule that matches applies, in order; a later rule overrides only the fields it sets
[[rules]]
match = { glob = "**/interface/**" }
overrides = { mips = { levels = "none" } }

[[rules]]
match = { glob = "**/foliage/**", suffix = "" , alpha = "varying" }
overrides = { alpha = { policy = "cutout", threshold = 100 }, mips = { coverage = "on" } }

[[rules]]
match = { glob = "**/*_mask.png" }
subtype = "generic_data"

# Per-file settings, keyed by path relative to this file's folder
[files."textures/armor/iron/iron.png"]
alpha.policy = "blend"

[files."textures/armor/iron/iron_rmaos"]
packing.fill.metallic = 255

[files."textures/armor/iron/iron_d.png"]
resize = "half"

# Read a misnamed file under another name; the file on disk is not renamed
[inputs."textures/armor/iron/iron_normalmap.png"]
as = "textures/armor/iron/iron_n.png"
```

## Top-level keys

| Key | Type | Default | Meaning |
|---|---|---|---|
| `schema_version` | integer, 1 or more | 1 | Format version of this file. A newer version than the build supports is an error: `<file>: config schema_version N is newer than this sdc supports (1)`. |
| `preset` | string | the target's default | The preset, by id: built-in or a user preset ([Presets and targets](Presets-and-Targets.md)). |
| `target` | `"cs"`, `"vanilla"` | from the preset, else `"cs"` | The target. The GUI's **Mode** row (Presets tab) writes it. |
| `[defaults]` | partial profile | none | Field values for every texture. |
| `[subtypes.<id>]` | partial profile | none | Field values for one sub-type. |
| `[[rules]]` | array of tables | none | Conditional settings. |
| `[files."<path>"]` | partial profile plus extra keys | none | Settings for one file. Project config only. |
| `[inputs."<path>"]` | table with `as` | none | Read a file under another name. Project config only. |
| `[export]` | table | none | Also export as Vanilla / CM. Project config only. |

## How settings layer

A texture's final settings (its profile) are built from layers. Later layers override earlier ones:

1. Built-in defaults, from the hidden `sdc-base` preset.
2. The selected preset and its base chain: all `[defaults]` tables from base to child, then all `[subtypes]` tables from base to child.
3. The user config: `[defaults]`, then `[subtypes.<id>]`.
4. The project config: `[defaults]`, then `[subtypes.<id>]`.
5. Rules: from the preset chain, then the user config, then the project config. In each source, every rule that matches the file applies, in file order: a later rule overrides only the fields it sets, and the earlier rules' other fields stay. A field's provenance names the rule that last set it.
6. Facts from analysing the image, such as a High-confidence alpha policy (no alpha channel means `remove`).
7. The set shader's member settings, and alpha from scanned meshes (GUI only).
8. Facts from the file's suffix.
9. Accepted suggestions (`--accept-suggestions high`, or always in the GUI). A suggestion only picks the sub-type, never a field value. In that choice it ranks below `--set`, `[files]`, rules and a shader you chose for the set.
10. The file's `[files]` entry.
11. `--set` on the command line: unscoped first, then `subtype:` scoped, then `file:` scoped.

`sdc presets show`, `sdc inspect --json` and the GUI show each field's provenance, so you can see which layer set it.

## [defaults] and [subtypes]

Both take a partial profile: any of the [fields](#fields) below, as nested tables or dotted keys (`alpha = { policy = "cutout" }` is the same as `alpha.policy = "cutout"`). Sub-type ids are on [Texture types](Texture-Types.md#sub-types-what-each-file-becomes). `subtype`, `target`, `category` and `resize_reference` in either are a load error: the file is rejected and the message tells you to remove the key.

## [[rules]]

```toml
[[rules]]
match = { glob = "...", suffix = "...", set_has = "...", alpha = "...", subtypes = ["..."] }
subtype = "<sub-type id>"     # optional
overrides = { ... }           # optional partial profile
```

| Match key | Meaning |
|---|---|
| `glob` | Glob over the file's path, case-insensitive: the game path (from the last `textures` folder), the path relative to the input folder, or, for a glob without `/`, the file name. `*` doesn't cross `/`; `**` does. |
| `suffix` | The file's recognised suffix without the underscore, such as `"n"` or `"rmaos"`. `""` means no suffix (the colour map). |
| `set_has` | The file's texture set contains a member with this suffix. |
| `alpha` | The source's alpha content: `"none"` (no alpha channel, or varying alpha that scanned meshes decided to remove), `"constant255"`, `"varying"`. |
| `subtypes` | The file's sub-type is one of these. A rule with this key never decides a sub-type; it only adds settings once the sub-type is known. |

- All given match keys must match. A rule with no match keys matches everything.
- `subtype` names the sub-type for matching files. It decides the file's sub-type ahead of suggestions, but below a `--set subtype` or a `[files]` entry. When something else decided the file's sub-type, a rule naming a different `subtype` doesn't apply to it.
- `overrides` is a partial profile applied to matching files.

```toml
# Files ending _ids.png are data
[[rules]]
match = { glob = "*_ids.png" }
subtype = "generic_data"

# In sets that have a parallax map, keep the diffuse at full effort
[[rules]]
match = { set_has = "p", suffix = "" }
overrides = { encoder = { effort = "slowest" } }
```

## [files]

Each key is a path relative to the folder that holds `sdc.toml`, with forward slashes. Matching ignores case and works with or without the file extension. A packed output has no source file of its own, so it's keyed by its output path without an extension:

```toml
[files."landscape/rock01_rmaos"]
packing.fill.specular = 64
```

An entry takes any [field](#fields), `subtype` included, plus:

| Key | Meaning |
|---|---|
| `allow` | Blocking codes to acknowledge for this file only: `allow = ["DIM-SMALL", "TAG-UNORM-PBR"]`. An unknown code is an error. |
| `reencode` | `true` converts a DDS input instead of skipping it (`IN-DDS-SKIPPED`). Per file only (`--set reencode=true` or a `file:` scope). Re-encoding a DDS loses quality (`LOSS-REENCODE`). |
| `helper.role` | For an image: what it is as a [helper texture](Texture-Packing.md#helper-textures). `"none"` (not a helper: convert it on its own), `"roughness"`, `"metallic"`, `"ao"`, `"specular"`, `"glossiness"`, `"height"`, `"opacity"`, `"fuzz"` (skin fuzz mask), `"orm"`, `"curves"`, `"cavity"`, `"fuzzweight"` (PBR `_f` weight), `"subsurfaceweight"` (PBR `_s` weight). Overrides the name and any `subtype`. Ignored on a DDS. |
| `helper.fills` | The channels the helper fills, replacing the suggestion. Alpha: `"diffuse.a"`, `"n.a"`, `"f.a"`, `"s.a"`, `"cnr.a"`. Packed: `"rmaos.r"` to `"rmaos.a"`, `"rfaos.r"` to `"rfaos.a"`, `"m.r"` to `"m.a"`, `"p.r"`. `[]` = not used. |
| `helper.read` | The helper's channel to read: `"r"`, `"g"`, `"b"`, `"a"` or `"luma"`. Unset reads R (or luminance where the recipe does). |
| `helper.invert` | `true` flips what the helper feeds (one minus the value), for an inverted mask. It flips on top of the recipe's own reading, so leave it off for a gloss map in a roughness channel: `helper.role = "glossiness"` already inverts it there. |

The `helper.*` keys go on the helper file's own entry, never on the output it fills. `--set` takes them only with a `file:` scope.

```toml
# A mask whose name says nothing: it is the diffuse's opacity, stored in green
[files."textures/armor/iron/iron_mask.png"]
helper.role = "opacity"
helper.read = "g"

# A roughness map that is also the coat roughness (the _cnr alpha), beside its usual _rmaos channel
[files."textures/armor/iron/iron_roughness.png"]
helper.fills = ["rmaos.r", "cnr.a"]

# Convert this specular map on its own instead
[files."textures/armor/iron/iron_specular.png"]
helper.role = "none"
```

Setting `helper.role` or `helper.fills` makes the helper *assigned*: it replaces what its channels already hold. Without them a helper fills only empty channels ([which data wins](Texture-Packing.md#helper-textures)).

An entry that names no file under the current target is reported as `CFG-FILE-UNMATCHED` (info), and the GUI lists it under "Unmatched [files] entries" in the Advanced drawer so you can remove it.

## [inputs]

```toml
[inputs."textures/armor/iron/iron_normalmap.png"]
as = "textures/armor/iron/iron_n.png"
```

- The file is grouped, typed, named and written as if it were at the `as` path. The file on disk isn't renamed, and it keeps its own `[files]` entry under its real name.
- `as` is relative to the folder holding `sdc.toml`, and keeps the file's extension (you can leave it out).
- An alias must stay under the project folder (no absolute path, no `..`), keep the extension, not be empty, and not equal another file's real or aliased path (compared case-insensitively). One that breaks these is ignored and reported as `IN-ALIAS-INVALID`; the file is planned under its real name.
- A key that matches no file is reported as `IN-ALIAS-STALE`.
- The rename dialog's Inputs mode writes these entries ([Bulk editing and renaming](GUI-Bulk-and-Rename.md)).

## [export]

```toml
[export]
also = ["vanilla", "cm"]
skip = ["textures/pbr/armor/iron/iron"]

[export.names]
vanilla = "Iron Armour SE"

[export.legacy]
brightness = 0.9
```

Also export needs the `cs` target. A value out of range is an error only when an Also export runs, and it names the key (`export.legacy.brightness = 2: must be between 0 and 1.5`). Unknown keys are always an error.

| Key | Default | Meaning |
|---|---|---|
| `also` | `[]` | `"vanilla"`, `"cm"` or both. Empty is a plain export. `"cm"` alone writes Vanilla too: CM installs on top of it. |
| `skip` | `[]` | Set ids (as `--set set:<id>` names them) left out of Vanilla and CM. |
| `names.pbr`, `names.vanilla`, `names.cm` | `<project> PBR`, `<project> Vanilla`, `<project> CM` | The folder names. `<project>` is the opened folder's name (the CLI: the first input folder's). They must be usable folder names, not a Windows device name, and all three different. |

`[export.legacy]` is the recipe. All numbers are fractions of the stored value, and gloss is 1 − roughness.

| Key | Default | Range | Meaning |
|---|---|---|---|
| `brightness` | 0.9 | 0–1.5 | Diffuse = brightness × albedo × lerp(1, AO, AO strength). |
| `ao_strength` | 0.5 | 0–1 | How much AO darkens an object's diffuse. |
| `ao_strength_landscape` | 0.3 | 0–1 | The same for a PBR Landscape set. |
| `curves_opacity` | 0.4 | 0–1 | Opacity of a `_curves` map's soft light and a `_cavity` map's multiply on the diffuse. |
| `metal_darken` | 0.3 | 0–1 | The vanilla diffuse's factor at full metal (lerp(1, this, metal)). Landscape has no metal. |
| `spec_scale` | 0.66 | 0–2 | `_n` alpha = spec_scale × gloss × AO × the metal boost, clamped to 1. |
| `spec_metal_boost` | 2.0 | 1–3 | The `_n` alpha boost at full metal. |
| `landscape_spec` | 2.5 | 0–5 | A landscape `_n` alpha is multiplied by this before the clamp (no metal boost). |
| `cm_dielectric_env` | 0 | 0–1 | CM `_m` R = gloss × max(metal, this). 0 = no env on non-metals; 0.35 is the spec's option. |
| `keep_metal_brightness` | `false` | | The CM folder gets its own diffuses without the metal darkening. |
| `keep_metal_strength` | 1.0 | 0–1 | How much of the metal darkening **Keep metal brightness** undoes. |

## Fields

Every field works in `[defaults]`, `[subtypes.<id>]`, a rule's `overrides`, a `[files]` entry and `--set`, except `subtype`, `target`, `category` and `resize_reference`: the sub-type is decided per file (a `subtype:` scoped `--set` refuses it), and the planner alone writes the other three. Defaults are the base preset's; a sub-type or preset may change them, as noted.

### Type and colour

| Field | Values | Default | Notes |
|---|---|---|---|
| `subtype` | a sub-type id | from the file name | Per file: `[files]`, a rule's `subtype`, or `--set file:…`. |
| `color.content` | `srgb`, `linear` | `linear`; `srgb` for colour sub-types | What the pixels mean. Affects mip filtering. On a `cubemap` it stays `srgb` but means the game's `pow(c, 1.6)` curve, not the sRGB curve. |
| `color.tag` | `srgb`, `unorm`, `n/a` | `unorm`; `srgb` for PBR colour under `cs`; `n/a` for HDR | The DDS format's `_SRGB` flag. `n/a` for BC6H and RGBA16F. Which sub-types take which tag: [sRGB or UNORM](Texture-Types.md#srgb-or-unorm). |

### Alpha

| Field | Values | Default | Notes |
|---|---|---|---|
| `alpha.policy` | `remove`, `cutout`, `blend`, `cutout_blend`, `data` | `remove`; `data` for packed and data maps | `remove` stores opaque alpha. `cutout` is alpha testing, `blend` alpha blending, `cutout_blend` both. `data` keeps alpha as numbers with no visual meaning. |
| `alpha.threshold` | 1 to 254 | 127 | The alpha-test threshold the mesh uses. Used for coverage preservation. |
| `alpha.compare` | `gt`, `ge` | `gt` | Greater than, or greater than or equal, as the mesh tests. |
| `alpha.exact_opaque` | `true`, `false` | `false`; `true` for `envmask` and `complex_material` | Opaque alpha decodes as exactly 255 in BC7. Avoid turning it off on a BC7 `_m`: `ALPHA-EXACT-OFF-M`. |
| `alpha.parallax` | `true`, `false` | unset | `complex_material` only (GUI: **Enable parallax**). `true` keeps the source alpha as the height map. `false` writes alpha 0: complex material on, parallax off. Unset is decided from the source: on only when its alpha varies, off for a constant alpha or none. `true` with no source alpha writes alpha 0 and raises `ALPHA-PARALLAX-NO-ALPHA`. |
| `alpha.exr_straight` | `true`, `false` | `false` | An OpenEXR source with alpha is read as premultiplied (the OpenEXR standard) and its colour divided by alpha on load. `true` reads it as straight alpha, as stored. |

### Channels

| Field | Values | Default | Notes |
|---|---|---|---|
| `channels.swizzle` | four characters from `r g b a 0 1` | none | Reorder channels before encoding, for example `"bgra"` or `"rrr1"`. Anything else is `CFG-INVALID`. |
| `channels.flip_green` | `true`, `false` | `false` | Invert the green channel of a normal map. |
| `channels.green_convention` | `directx`, `opengl` | unset | Which way the source normal map's green points. `opengl` turns on `flip_green`. A plain `_normal` file with this unset is converted as DirectX and notes `IN-GREEN-UNSTATED`. An explicit `flip_green` that contradicts the convention gives `CFG-GREEN-CONFLICT`, and `flip_green` wins. |
| `channels.msn_source` | `skyrim`, `swap_gb`, `invert_b`, `invert_gb` | `skyrim` | `msn` only; other sub-types ignore it. The baker a model-space normal came from, which Tessalith changes to Skyrim's layout when it encodes. `swap_gb` for a Blender object-space bake, `invert_b` for Marmoset right-handed or Substance, `invert_gb` for Marmoset left-handed. The GUI row is **Baked in** (`Skyrim MSN (no change)`, `Blender object-space (swap G and B)`, `Marmoset right-handed or Substance (invert B)`, `Marmoset left-handed (invert G and B)`). <!-- D:IA1 --> |
| `channels.reconstruct_z` | `true`, `false` | `false` | Rebuild the normal's blue channel from red and green. |
| `channels.grayscale_source` | `r`, `g`, `b`, `a`, `luma` | `r` | The channel a grayscale value is read from in a colour source: red, green, blue, alpha or Rec. 709 luminance. It applies to grayscale sub-types and to the raw maps that [pack](Texture-Packing.md#packing-loose-maps) into a recipe. With `a` the alpha is the data, so it isn't reported as discarded; a source with no alpha channel reads 255 (`CHAN-NOT-GRAY`). To pick for one raw map only, set `helper.read` on that file; it beats this value. |

### Mipmaps

| Field | Values | Default | Notes |
|---|---|---|---|
| `mips.levels` | `full`, `none`, or a count | `full`; `none` for `ui_sky` and under `interface` and `sky` folders | Number of mip levels. |
| `mips.filter` | `box`, `tent`, `mitchell`, `kaiser` | `box` | Downsampling filter. Tent, Mitchell and Kaiser are for sRGB colour content. With `cutout`, `blend` or `cutout_blend` only `tent` is allowed: it weights each tap by alpha, and Mitchell and Kaiser ring at cutout edges. HDR textures without alpha weighting may use `box` or `tent`. An unscoped filter a texture can't use falls back to box with `CFG-FILTER-IGNORED`; a scoped one is `CFG-INVALID`. |
| `mips.edge` | `wrap`, `clamp` | `wrap` | How tent, Mitchell and Kaiser read past the texture edge. Ignored by box. |
| `mips.normal` | `renormalize`, `renormalize_bake` | `renormalize`; `renormalize_bake` for `coat_normal` | How normal-map mips are built. `renormalize_bake` also bakes the normal detail lost in each mip into the roughness stored in the same texture (the coat roughness of a `_cnr`). |
| `mips.normal_base` | `renormalize`, `keep` | `renormalize` | Mip 0 of a normal map: stored at unit length, or as authored. |
| `mips.bake_clamp` | greater than 0, up to 1 | 0.25 | Limits how much roughness the variance bake can add (at most the square of this value). |
| `mips.coverage` | `auto`, `on`, `off` | `auto` | Alpha-coverage preservation. Applies to `cutout`, `cutout_blend`, and `blend` with `encoder.bc1_alpha`; there `auto` and `on` both enable it. |

### Format and size

| Field | Values | Default | Notes |
|---|---|---|---|
| `format` | `bc1`, `bc3`, `bc4`, `bc5`, `bc6h_uf16`, `bc7`, `rgba8`, `rgb8`, `r8`, `rg8`, `rgba16f` | `bc7` (per sub-type: [Texture types](Texture-Types.md#sub-types-what-each-file-becomes)) | `bc6h_uf16` and `rgba16f` only under `cs`. BC1 keeps alpha only with `encoder.bc1_alpha`. `rgb8` is 24-bit: no alpha (`FMT-RGB8-ALPHA`), linear tag only, legacy header only. <!-- D:FM38 --> Which encoder each format uses: [Research](Research.md). |
| `header` | `auto`, `dx10`, `legacy` | `auto` | DDS header form. `auto` writes a legacy header for BC1, BC3 and RGBA8 without the `_SRGB` tag, and the DX10 header for everything else. `legacy` is valid only for formats that have a legacy form (BC1, BC3, BC4, BC5) and no `_SRGB` tag; otherwise it is `CFG-INVALID`. |
| `resize` | `none`, `half`, `quarter`, `eighth` | `none` | Halves both sides once, twice or three times. Each halving happens only while the longer side stays at or above `resize_floor`: with the 512 floor, 2048x1024 under `quarter` becomes 512x256, and 1024x1024 under `eighth` stops at 512x512. A changed size is noted as `DIM-RESIZED`. A `cubemap` ignores it (its size is [`cubemap.face_size`](#cubemap)). Textures with mips are first rounded to a power of two (below the table). <!-- D:FM29 --> |
| `resize_relative_to` | `self`, `slot1` to `slot8` | `self` | What `resize` is measured from. `self` halves the texture's own size. A slot measures from the source size of the set's map in that texture-set slot (1 diffuse, 2 normal, 3 glow or skin, 4 height, 5 environment, 6 environment mask, complex material or RMAOS, 7 multilayer or coat, 8 back-light, specular or PBR subsurface): the texture halves while its longer side is above that map's size after the same `resize` and power-of-two step, and is never enlarged. With a 2048 diffuse and `half`, a 2048 normal becomes 1024 and a 1024 or 512 normal keeps its size. It does nothing under `resize = "none"`, and the texture's own slot works as `self`. When the set has no map in the slot, or more than one, `resize` applies to the texture's own size and the texture gets `RESIZE-NO-REFERENCE`. <!-- D:FM44 --> |
| `resize_floor` | pixels, 1 to 16384 | 512; 256 for `height` | The longer side `resize` never halves below. Outside 1 to 16384 it is `CFG-INVALID`. <!-- D:FM29 --> |

Every texture with mips is written at a power-of-two size, before `resize` applies. The longer side rounds up to the next power of two when it is at least 95 % of it, otherwise down: 1000 becomes 1024, 900 becomes 512. The shorter side is first scaled by the same factor, then rounds up when it is at least 85 % of the next power of two, so 1024x768 becomes 1024x512 and 1000x600 becomes 1024x512. A side that grows is enlarged with linear interpolation. A texture with `mips.levels = "none"` keeps its own size, gets the `DIM-NPOT` warning, and still takes `resize`. A `cubemap` ignores `resize` and `resize_floor`. <!-- D:FM29 -->

### Encoder

| Field | Values | Default | Notes |
|---|---|---|---|
| `encoder.effort` | `ultrafast`, `veryfast`, `fast`, `basic`, `veryslow`, `slowest`, `max` | `veryslow` | bc7e quality level; `max` costs several times `veryslow`'s time ([timings](Research.md)). For BC6H, `ultrafast` and `veryfast` map to veryfast and `veryslow` to veryslow; `slowest` and `max` both use a peak-preserving veryslow that keeps small, very bright highlights (a sun) near their source brightness. A BC6H texture with no effort set anywhere but the built-in defaults uses `slowest`; set `veryslow` for the plain mode. <!-- D:FM43 --> |
| `encoder.perceptual` | `true`, `false` | `false` | Perceptual error metric for BC1, BC3 and BC7: BC7 uses bc7e's own weights; BC1 and BC3's colour block weight red, green and blue error by Rec.709 luma (54, 183, 19 of 256). It keeps luma detail at some cost in colour ([measurements](Research.md)). Other formats ignore it. Colour sub-types whose alpha policy isn't `data` only: anywhere else it is `FMT-PERCEPTUAL-DATA`. Which presets turn it on: [Presets and targets](Presets-and-Targets.md). <!-- D:FM43 --> |
| `encoder.bc7_weights` | `linear`, `alpha2`, `alpha8`, `alpha16` | unset | BC7 linear error weights for bc7e (R, G, B, A): `linear` 1,1,1,1 (the same encode as unset; use it to override an inherited weighting for one file); `alpha2` 1,1,1,2; `alpha8` 1,1,1,8 (an alternative to BC3 for delicate alpha); `alpha16` 1,1,1,16 (a `_n` with specular in alpha). `encoder.perceptual = true` wins over it. On another format, or with perceptual on, it has no effect and raises `ENC-WEIGHTS-IGNORED`. The GUI's **BC7 weighting** row edits both fields. |
| `encoder.bc1_alpha` | `true`, `false` | `false` | BC1 with 1-bit alpha for `cutout`, `blend` and `cutout_blend` ([BC1 alpha](Texture-Types.md#alpha)). Ignored by other formats. |
| `encoder.bc45_search_radius` | `default`, `max`, `fast` | `default` | BC4/BC5 endpoint search. `max` searches a radius of 10, `fast` a radius of 4. |

### Validation

| Field | Values | Default | Notes |
|---|---|---|---|
| `validation.mips` | `all`, `smallest` | `all` | `smallest` runs the PSNR check on the smallest mip only (`fast-preview` uses it). Coverage is always checked on every level. |

### Packing

| Field | Values | Default | Notes |
|---|---|---|---|
| `packing.enabled` | `true`, `false` | `true` | Build this packed output. `false` leaves its raw maps as packing inputs (`IN-PACKING-INPUT`). |
| `packing.roughness_default` | 0 to 1 | 0.5 | Roughness used when there is no roughness or glossiness source. |
| `packing.prefer` | `prepacked`, `raw` | `prepacked` | When a set has both a packed file (such as `_rmaos.png`) and raw maps: keep the packed file, or build from the raw maps (`PACK-CONFLICT`). A lone specular beside a packed image with a flat alpha fills that alpha under either value. |
| `packing.recipe` | `auto`, `rmaos`, `rfaos` | `auto` | Which `cs` recipe a set packs. `auto` builds `_rfaos` when a fuzz source exists, otherwise `_rmaos`. |
| `packing.fill.<role>` | 0 to 255 | unset | Value for a channel with no source map. Roles: `roughness`, `metallic`, `ao`, `specular`, `glossiness`, `height`, `opacity`, `fuzz`. Unset means the neutral default (AO 1, metallic 0, specular 1, roughness `roughness_default`, height 0). `[defaults]` takes no fills: set them per file or sub-type. |

Packing fields go on the packed output's key, for example `[files."textures/armor/iron/iron_rmaos"]`.

### Cubemap

These apply to the `cubemap` sub-type (`_e` files and files under a `cubemaps` folder) and are ignored by every other. A cube's formats are on [Cubemaps](Cubemaps.md#checks): any other `format`, or an sRGB tag, is `CFG-INVALID`; `mips.levels = "none"` raises `CUBE-NO-MIPS`.
| Field | Values | Default | Notes |
|---|---|---|---|
| `cubemap.style` | `vanilla_metal`, `room`, `eye`, `hd` | unset | Fills the keys below that nothing else set: `vanilla_metal` 32 px, BC1, blur 0.6, saturation 0.8; `room` 128 px, BC1, blur 0.35; `eye` 256 px, BC7, blur 0.35; `hd` 512 px, BC7, blur 0.1. |
| `cubemap.layout` | `faces`, `hcross`, `vcross`, `hstrip`, `vstrip` | from the image shape | `faces` is six separate files. A cube DDS must leave it unset. How each is detected: [Ways to make one](Cubemaps.md#ways-to-make-one). |
| `cubemap.face_size` | a power of two, 4 to 4096 | by layout | Face edge in pixels ([source cell per layout](Cubemaps.md#settings)). |
| `cubemap.yaw` | −360 to 360 | 0 | Turns the cube about world up, in degrees. Positive is clockwise seen from above, the way Skyrim headings turn: 90 moves east to south. |
| `cubemap.mirror` | `true`, `false` | `false` | Swaps world east and west. |
| `cubemap.face_rotate` | six of 0, 90, 180, 270 | `[0, 0, 0, 0, 0, 0]` | Clockwise turn per face, in the order +X, −X, +Y, −Y, +Z, −Z. Applied before the flip. Works on every layout. |
| `cubemap.face_flip` | six of `"none"`, `"h"`, `"v"` | all `"none"` | `h` mirrors left-right, `v` flips top-bottom. |
| `cubemap.vcross_negz_rotate` | `true`, `false` | `true` | Turn a vertical cross's bottom cell (−Z) by 180°. Other layouts ignore it. |
| `cubemap.blur` | 0 to 1 | none (0) or the style's | Cone blur across the faces. |
| `cubemap.exposure` | −10 to 10 | 0 | Stops, in linear light. 1 doubles the light. |
| `cubemap.brightness` | −1 to 1 | 0 | Added to each channel, on the game curve. |
| `cubemap.contrast` | −1 to 1 | 0 | Scales around mid-grey by (1 + contrast), on the game curve. |
| `cubemap.midtones` | 0.1 to 10 | 1 | A power curve on the game curve: above 1 brightens the mid-tones, below 1 darkens them, 1 is off. |
| `cubemap.saturation` | 0 to 2 | 1 | 0 is grey. |
| `cubemap.tint` | `[R, G, B]`, each 0 to 255 | unset | Each channel is multiplied by its value / 255, on the game curve (255, 255, 255 changes nothing). |
| `cubemap.highlight_clamp` | 1 or more | unset | Soft-compresses brightness so that this linear level becomes full white; lower values are also pulled down a little, and hue is kept. For HDR sources. |
