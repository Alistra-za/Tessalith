# Diagnostic codes

For anyone who saw a code in the Problems list or in `sdc`'s output: what it means and what to do, for every code. A symptom with no code is on [Troubleshooting](Troubleshooting.md).

## Severities

| Severity | Effect |
|---|---|
| info | Recorded. Not shown in the CLI log (except `CFG-FILTER-IGNORED`). |
| warning | Shown; the file is converted. |
| blocking | The file isn't converted until you acknowledge the code: `--allow <CODE>` for the batch, `allow = ["<CODE>"]` in the file's `[files]` entry, or the acknowledge button in the GUI. |
| error | The file fails. |

## Input

| Code | Severity | Meaning | Fix |
|---|---|---|---|
| `IN-DUPLICATE` | blocking | Two inputs would write one output (two raw maps of one name write nothing, so they don't count), or two outputs land on one path (for example two packed outputs after a raw map moved to another set). The first keeps the path. | Exclude or rename one, or change an output path. |
| `IN-DDS-SKIPPED` | info | A DDS input isn't re-encoded by default. | Use the source, or set `reencode = true`. |
| `IN-PACKING-INPUT` | warning | A raw map isn't packed because its recipe is off (`packing.enabled = false`). | Enable packing, or give the file a sub-type. |
| `IN-GREEN-UNSTATED` | info | A plain `_normal`/`_nrm`/`_norm` doesn't state its green convention; it was converted as DirectX. | Set `channels.green_convention = "opengl"` if it is an OpenGL map. |
| `IN-DECODE` | error | The source can't be read. | Fix or re-save the file. |
| `IN-NONFINITE` | warning | NaN or infinite samples were replaced. | Fix the source. |
| `IN-UNREADABLE` | warning | A file or folder under an input folder couldn't be read (access denied, broken junction, cloud placeholder) and was skipped. | Fix permissions, or ignore. |
| `IN-ALIAS-INVALID` | warning | An `[inputs]` alias that breaks the [rules](Project-Config.md#inputs) is ignored. | Fix the `as` path. |
| `IN-ALIAS-STALE` | info | An `[inputs]` key matches no file. | Remove the entry. |

## Loss

| Code | Severity | Meaning | Fix |
|---|---|---|---|
| `LOSS-ALPHA-DATA` | blocking | Varying alpha would be discarded by `remove` or a format without alpha. Not raised for a `pbr_normal` or an explicit `alpha.policy = "remove"`. | Keep the alpha, choose `remove` explicitly, or acknowledge. |
| `LOSS-CLAMP-HDR` | warning | Float values outside 0 to 1 written to an LDR format are clamped. | Use an HDR format under `cs`, or accept. |
| `LOSS-REENCODE` | warning | A compressed DDS is being re-encoded. | Use the source image. |

## Channels

| Code | Severity | Meaning | Fix |
|---|---|---|---|
| `CHAN-NOT-GRAY` | warning | A grayscale sub-type's or raw map's R, G and B differ by more than 4/255 under the default `r` or `luma`; only the channel `channels.grayscale_source` picks is kept. A `g`, `b` or `a` pick is yours, so it's reported only when that channel holds one value (an `a` pick on a source with no alpha reads 255). The message names the pick and, when the picked channel is flat while another varies, suggests the first varying one. | Pick the channel or the sub-type. |
| `CHAN-NORMAL-LENGTH` | warning | A normal source is far from unit length, or has no Z. | **Rebuild Z** on its Problems row (sets `channels.reconstruct_z`), or check the sub-type. |

## Format and tag

| Code | Severity | Meaning | Fix |
|---|---|---|---|
| `FMT-BC5-NORMAL` | blocking | BC4, BC5, R8 or RG8 on a normal map ([why](Texture-Types.md#sub-types-what-each-file-becomes)). | Use BC7. |
| `FMT-LANDSCAPE-BC1` | blocking | BC1 on `landscape_diffuse` destroys the snow mask. | Use BC7 or BC3. |
| `FMT-CS-SKIN-LOADER` | blocking | Community Shaders' loader reads only RGBA8, BGRA8, BC1, BC2, BC3 and BC7 on a skin map (BC5 on `skin_detail`); any other format loads as nothing. `--allow` can't lift it. | Pick a format the loader reads. |
| `FMT-BC1-ALPHA` | blocking | BC1 with alpha kept, without `encoder.bc1_alpha` or with `data` alpha that isn't empty ([BC1 alpha](Texture-Types.md#alpha)). | Turn on `encoder.bc1_alpha`, use BC7 or BC3, or `remove`. |
| `FMT-RGB8-ALPHA` | blocking | RGB8 with alpha kept. | Use BC7 or RGBA8, or `remove` the alpha. |
| `FMT-PERCEPTUAL-DATA` | error | Perceptual encoding on a data map. | Turn `encoder.perceptual` off. |
| `FMT-HDR-SOURCE` | info | Why a `pbr_emissive` is BC6H (float source above 1.0) or BC7 sRGB. | Set `format` on the file to choose. |
| `TAG-SRGB-VANILLA` | error | `_SRGB` under `vanilla`. | UNORM ([rule](Texture-Types.md#srgb-or-unorm)). |
| `TAG-SRGB-NONPBR` | blocking | `_SRGB` on a non-PBR sub-type under `cs`; renders darker. | Remove the `color.tag = "srgb"` override, or set the PBR sub-type ([rule](Texture-Types.md#srgb-or-unorm)). |
| `TAG-UNORM-PBR` | blocking | UNORM on PBR colour; looks washed out with linear lighting off. | Remove the `color.tag = "unorm"` override ([rule](Texture-Types.md#srgb-or-unorm)). |
| `TAG-SRGB-LOD` | blocking | `_SRGB` under a `lod` folder; LOD shaders reject it. | UNORM. |
| `TAG-CONTENT-MISMATCH` | warning | Content and tag disagree: linear content tagged `_SRGB`, a colour sub-type with linear content, or a roughness member with sRGB content (then not variance-baked). | Check `color.content` and `color.tag`. |

## Alpha

| Code | Severity | Meaning | Fix |
|---|---|---|---|
| `ALPHA-EXACT-OFF-M` | blocking | `exact_opaque` turned off on a BC7 `_m`; decoded 254s may trip PGPatcher's 50% rule or ENB checks. | Keep it on, or acknowledge. |
| `ALPHA-PARALLAX-NO-ALPHA` | warning | A complex `_m` has [`alpha.parallax`](Project-Config.md#alpha) on and no source alpha. It's written with alpha 0: complex material on, parallax off. | Give the source a height alpha, or set `alpha.parallax = false`. |
| `ALPHA-BC1-ONE-BIT` | warning | BC1 1-bit alpha on a source whose soft alpha would turn hard ([BC1 alpha](Texture-Types.md#alpha)). | Use BC7 or BC3 to keep the soft edges, or accept. |

## Configuration

| Code | Severity | Meaning | Fix |
|---|---|---|---|
| `CFG-UNAVAILABLE` | blocking | The sub-type or format isn't available under the target. | Accept the fallback with `--allow CFG-UNAVAILABLE`, or change the sub-type or target. |
| `CFG-INVALID` | error | A value outside its range, or a combination that can't be honoured (`legacy` header on BC7 or `_SRGB`, a bad swizzle, a bad filter scope). In a config, preset, rule or `--set` the run stops with exit 2; reached after layering, that file fails. | Fix the value. |
| `CFG-FILE-UNMATCHED` | info | A `[files]` key matches no file. | Fix or remove the entry. |
| `CFG-FILTER-IGNORED` | info | An unscoped [`mips.filter`](Project-Config.md#mipmaps) this texture can't use; it uses box. | Scope the filter to colour maps, or ignore. |
| `CFG-PRESET-SKIPPED` | warning | A user preset file failed to load ([presets](Presets-and-Targets.md#checking-and-installing)). | Fix the file. When another preset already declares its suffix aliases, the Problems row has **Remove aliases from {id}**. |
| `CFG-GREEN-CONFLICT` | warning | An explicit `flip_green` contradicts the [green convention](Project-Config.md#channels). | Remove one. |

## Sizes

| Code | Severity | Meaning | Fix |
|---|---|---|---|
| `DIM-NPOT` | warning | A texture with `mips.levels = "none"` keeps a side that isn't a power of two (textures with mips are [resized to one](Project-Config.md#format-and-size)). | |
| `DIM-SMALL` | blocking | A side is under 4 pixels. A cube face under 4 is also `CFG-INVALID`, which can't be acknowledged. | Resize, or acknowledge (not for a cube). |
| `DIM-FLOW-SMALL` | warning | A `_flow` map is 32 px or smaller; Community Shaders ignores it. | Use 64 px or more. |
| `DIM-RESIZED` | info | The input was resized, for example `1000x600 → 1024x512`. | |
| `DIM-TOO-LARGE` | error | A side is over the [limit](Texture-Types.md#which-files-tessalith-reads). | Downscale the source. |
| `RESIZE-NO-REFERENCE` | info | [`resize_relative_to`](Project-Config.md#format-and-size) names a slot the set has no map in, or more than one, so `resize` was applied to the texture's own size. The message names the slot. | Add the map, pick a slot the set fills, or set `self`. |

## Cubemaps

| Code | Severity | Meaning | Fix |
|---|---|---|---|
| `CUBE-FACES` | error | A cube can't be built; the message names the fault and nothing is written ([causes](Cubemaps.md#ways-to-make-one)). | Fix the files or the layout. |
| `CUBE-NO-MIPS` | warning | A cube without mips ([why](Cubemaps.md#checks)). | Leave `mips.levels` at `full`. |

## Sets and suggestions

| Code | Severity | Meaning | Fix |
|---|---|---|---|
| `SET-MISSING-COMPANION` | info | A PBR set lacks `_n` or a roughness member, an `_msn` lacks `_s`, or a `_flow` lacks `_n`. | Add it, or ignore. |
| `SET-COMPANION-DDS` | warning | The companion exists only as DDS, so there is no bake. | Supply the source, or opt the DDS in. |
| `SET-EDIT-STALE` | info | A set edit or `set:` override names a set or file that doesn't exist. | Fix or remove it. |
| `SUG-LOW-CONFIDENCE` | blocking | A sub-type or alpha meaning is unconfirmed. | Confirm it, or `--accept-suggestions high` for High ones. A lone file with an unknown suffix has no suggestion: [assign a type](Texture-Types.md#how-files-form-a-set). `--allow SUG-LOW-CONFIDENCE` converts nothing that has no suggestion. |
| `SUG-CONTRADICTION` | warning | The content contradicts the suffix. | Confirm or change the sub-type. |

## Validation

The rules and thresholds are on [Validation and output](Validation-and-Output.md#what-is-checked).

| Code | Severity | Meaning | Fix |
|---|---|---|---|
| `VAL-HEADER` | error | The written header fails the header rules. On `sdc validate`, also a cube with no mips, a 1×1 cube with a non-black +Y, a cube in an sRGB format, or a 2D file named `_e` or under `textures\cubemaps\`. | Report it as a bug, or fix the cube. |
| `VAL-MIP-ERROR` | warning | A mip's PSNR is below 30 dB. | Check the source, raise the effort, or accept. |
| `VAL-ALPHA-GUARANTEE` | error | Decoded opaque alpha is below the encoder's guarantee. | Report it as a bug. |
| `VAL-COVERAGE` | warning | Cutout coverage drifted more than 2 points on a mip. | Adjust the threshold. |
| `VAL-M-COMPLEX-MARGIN` | warning | A complex `_m`'s smallest-mip alpha is 247/255 or more: within the margin below Community Shaders' 251/255 gate, or past it (complex off). | Adjust the height range. |
| `VAL-M-GRAYSCALE` | warning | A complex `_m`'s smallest mip is grey, so it reads as a plain env mask. | Check the channels. |
| `VAL-M-HEIGHT-CLIP` | warning | Large `_m` alpha areas with no parallax. | Remap the height. |
| `VAL-PGPATCHER-50` | warning | A plain `_m` with alpha is 50% or more non-white; PGPatcher would treat it as complex. | Use BC4, or turn `exact_opaque` on. |
| `VAL-BC6H-PEAK` | warning | A BC6H texture's peak ratio is below 0.8. | Remove the `encoder.effort` set for that texture ([the BC6H default](Project-Config.md#encoder)). |
| `VAL-GRASS-DETECT` | error | The encoded complex-grass corner texel still fails detection after a slowest re-encode. The file is written. | Fix the source corner. |

## Encoders and output

| Code | Severity | Meaning | Fix |
|---|---|---|---|
| `ENC-WEIGHTS-IGNORED` | warning | `encoder.bc7_weights` has no effect: the format isn't BC7, or `encoder.perceptual` is on. | Remove `encoder.bc7_weights`, or turn perceptual off. |
| `OUT-UNKNOWN-EXISTING` | warning | An existing output with no manifest record was kept ([re-runs](Validation-and-Output.md#re-runs-and-if-changed)). | `--on-existing overwrite`. |

## Packing

| Code | Severity | Meaning | Fix |
|---|---|---|---|
| `PACK-DEFAULTED` | warning | A packed channel has no source and uses the neutral default. | Supply the map, set a fill, or accept. |
| `PACK-SIZE-MISMATCH` | error | Sources of one recipe, or an assigned helper and its target, have different aspect ratios. | Crop or resize the sources to one aspect ratio. |
| `PACK-SOURCE-RESIZED` | info | Sources of one aspect ratio and different sizes: each smaller one was enlarged to the largest. | Nothing; supply maps of one size for exact control. |
| `PACK-CONFLICT` | warning | A pre-packed file and raw maps target one output; the pre-packed file wins by default. | Remove one side, or set `packing.prefer`. |
| `PACK-UNUSED-INPUT` | warning | A raw map is used by no recipe under the target. | Change the target, set a sub-type, or remove it. |
| `PACK-RECIPE-CONFLICT` | warning | The `_rfaos` and `_rmaos` keys of one set disagree on `packing.recipe`; the `_rfaos` entry wins. | Keep the recipe on one key. |
| `PACK-RECIPE-UNBUILDABLE` | warning | An `rfaos` recipe the set can't build; `_rmaos` was built ([recipes](Texture-Packing.md#packing-loose-maps)). | Add roughness, glossiness or AO, or drop the setting. |

## Shaders, skin and grass

| Code | Severity | Meaning | Fix |
|---|---|---|---|
| `SHADER-AUTO` | info | The set's shader was picked automatically, with the rule. | |
| `SHADER-UNAVAILABLE` | warning | The shader isn't available under the target; it fell back. | Change the shader or target. |
| `SHADER-EXTRA-MAP` | warning | A member the shader doesn't use; it still converts ([set shaders](Set-Shaders.md)). | |
| `SHADER-MISSING-MAP` | warning | A map the shader requires is missing. | Add it. |
| `SKIN-WET-GREY` | warning | A `skin_wet` source is grey. Community Shaders ignores a grey wet map, so that file isn't written; the set's other files are. | Supply a tangent normal with the wet mask in alpha. |
| `GRASS-SHAPE` | error | A `complex_grass` source isn't a power-of-two 1:2 image. | Supply e.g. 2048x4096. |
| `GRASS-TOP-OPAQUE` | warning | The top half is fully opaque: no cutout. | Check the alpha. |
| `GRASS-NORMALS` | warning | Under 99% of the bottom half is unit-length normals. | Check the bottom half is a normal map. |
| `GRASS-DETECT-SOURCE` | warning | The source's bottom-left texel already fails Community Shaders' detection test. | Fix the corner texel. |
| `PBR-GRASS-DETECT` | warning | A PBR grass albedo's bottom-left texel looks like a complex-grass normal, so Community Shaders' grass shadow pass would read it as complex grass and shadows won't match. | Change that texel. |

## Also export

Raised only by Also export ([GUI or `[export] also`](Set-Shaders.md#also-export-as-vanilla-and-cm)).

| Code | Severity | Meaning | Fix |
|---|---|---|---|
| `DERIVE-DDS-SOURCE` | warning | A source the Vanilla or CM folder needs is a DDS, which is never re-encoded, so the set is skipped. | Supply the source image. |
| `DERIVE-NO-ALBEDO` | warning | The set has no albedo, so it gets no Vanilla diffuse and no CM diffuse. | Add the albedo. |
| `DERIVE-PATH-CONFLICT` | error | A derived output lands on another job's output. The other job is kept. | Rename or exclude one of the two. |
| `DERIVE-NOT-CONVERTED` | info | A set member has no role in the Vanilla or CM recipe, so it isn't converted there. | Nothing, if the file feeds no map. Otherwise fix the set's own skip or error row (`DERIVE-DDS-SOURCE`, `DERIVE-NO-ALBEDO`, a recipe error) or put the set back in the export. |
| `DERIVE-NIF-SUGGEST` | info | Suggested NIF glossiness, specular strength and whether to use the Environment Map shader. | Enter them in the NIF if you want them. |
| `ALSO-PHASE-FAILED` | error | The PBR, Vanilla or CM phase of an Also export stopped with an engine error. The phases after it did not run. | Read the message; fix and re-run. |

## Artist checks

The GUI raises these ([what each flags](Texture-Types.md#artist-checks)). They only warn. The CLI raises none of them; the engine measures `CHK-LOW-DETAIL` only when the GUI asks.

| Code | Severity | Meaning | Fix |
|---|---|---|---|
| `CHK-GREEN-GUESS` | info | The normal map's slopes look like OpenGL green. | **Treat as OpenGL (flip green)**, or ignore. |
| `CHK-Z-SIGN` | warning | A normal map's blue channel points the wrong way. | Check the source. |
| `CHK-PARALLAX` | warning | A height map, judged against the set's neutral height (0.5, or 1.0 with a multilayer coat), uses little range, clips, is mostly white or is too steep. | Check the height map. |
| `CHK-PARALLAX-SCALE` | warning | A landscape set's displacement scale is more than 2x from the others. | Make the scales match. |
| `CHK-LOW-DETAIL` | warning | Full size adds little detail. | Set `resize = "half"` on the texture, or ignore. |
| `CHK-UV-GUTTER` | warning | UV islands closer than 8 texels for more than 2 % of used texels. | Space the islands out. |

## Generated maps

These concern `*_generated.png` files you supply; Tessalith reads them as ordinary sources ([rules](Texture-Types.md#files-named-_generated)).

| Code | Severity | Meaning | Fix |
|---|---|---|---|
| `GEN-REPLACED` | info | A generated file was skipped on purpose because a real map for the same slot is in the set. | Nothing; the real map wins. Remove the generated file if you no longer want it. |
| `GEN-AO` | info | A packed `_rmaos` takes its blue channel from an `_ao_generated` file (replaces `PACK-DEFAULTED`). | Nothing; a baked AO is better if you have one. |
