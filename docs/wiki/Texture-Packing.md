# Texture packing and helper textures

How raw maps such as roughness, metallic and AO pack into the files each shader reads and how helper textures feed other outputs. For modders whose sources come as separate maps.

## Packing raw maps

Raw single-channel maps (from the exporter names above) are not converted on their own. They are packed into the layout the target reads. One source per role is used; if a set has two roughness maps, the first by path wins and the other is reported as `PACK-UNUSED-INPUT`. A raw map is read from its red channel; for a map that keeps its value in another channel, set [`channels.grayscale_source`](Project-Config-Fields.md#channels).

An `_orm` file counts as three raw maps at once: AO from its R channel, roughness from G and metallic from B. Every recipe below that reads roughness, AO or metallic takes them from it; a separate map for the same slot (an `_roughness` next to the `_orm`) wins that slot. An `_rfaos` has no metallic channel, so it leaves the `_orm` metallic unused. Slots an `_orm` does not hold (specular, height, fuzz) come from their own maps, the fill, or the default, so an `_rmaos` built from an `_orm` alone has `packing.fill.specular` in alpha, or 1 when no fill is set.

### Community Shaders (`cs`)

`_rmaos` is built when the set has a roughness, glossiness, AO or metallic map:

| Channel | Source | Default when missing |
|---|---|---|
| R | roughness, or 1 minus glossiness | `packing.roughness_default` |
| G | metallic | 0 |
| B | AO | 1 |
| A | specular | 1 |

`_rfaos` (Advanced skin) replaces it when a fuzz map exists or the set's shader is set to Advanced Skin (`packing.recipe = "auto"`), or always with `packing.recipe = "rfaos"`:

| Channel | Source | Default when missing |
|---|---|---|
| R | roughness, or 1 minus glossiness | `packing.roughness_default` |
| G | fuzz | 0 |
| B | AO | 1 |
| A | specular | 1 |

`packing.recipe = "rfaos"` on a set that only has a metallic map builds an `_rmaos` instead (`PACK-RECIPE-UNBUILDABLE`).

`_p` is built from a raw height map.

### Vanilla, complex material set

Under `vanilla`, a set whose shader is complex material (picked in the GUI, `--set set:<id>.shader=complex_material`, or chosen automatically because the set already has a complex-material `_m` file) builds the complex `_m` when it has a roughness, glossiness, metallic, height or `_orm` map. If the set already has an `_m` file, that file still wins unless `packing.prefer = "raw"`.

| Channel | Source | Default when missing |
|---|---|---|
| R | environment: metallic, else specular | 0 |
| G | glossiness, or 1 minus roughness | 1 minus `roughness_default` |
| B | metallic | 0 |
| A | height | 0 |

### Vanilla, any other set

`_m` is a BC4 environment mask, built from metallic (else specular) only when one of them exists. Other raw maps are not used (`PACK-UNUSED-INPUT`).

The complex `_m` recipe id in events and manifests is `cpm_m`.

### Fills and defaults

A channel with no source gets the neutral default and a `PACK-DEFAULTED` warning that names the output and the channel. To choose the value yourself, set a fill (0 to 255) on the packed output:

```toml
[files."textures/armor/iron/iron_rmaos"]
packing.fill.metallic = 255     # fully metallic
packing.fill.specular = 128
```

or for a whole run with `--set packing.fill.metallic=255`. In the GUI, set it on the packed output's card in the set editor.

If a set has both a ready-made packed file (`iron_rmaos.png`) and raw maps for it, the packed file wins and the raw maps are reported (`PACK-CONFLICT`). Set `packing.prefer = "raw"` to build from the raw maps instead. The sources of one recipe may differ in size if they share an aspect ratio: each smaller one is enlarged to the largest (`PACK-SOURCE-RESIZED` names it), and the packed image is then sized like any output. Sources with different aspect ratios fail the recipe (`PACK-SIZE-MISMATCH`). Set `packing.enabled = false` to leave the raw maps alone (`IN-PACKING-INPUT`).

## Helper textures

A **helper** is a source file that fills one channel of another output of its own set and is never written as its own DDS: the raw maps above, an opacity map, and fuzz or subsurface weight maps you mark by hand. The GUI lists every helper in a Helpers group under the tiles (see [Helper textures in the GUI](GUI-Helpers-and-Cubemaps.md#helper-textures)); the CLI reads the same settings from `sdc.toml` ([Project config](Project-Config.md#files)), so both write the same bytes.

| Role (`helper.role`) | Suggested target | Also offered |
|---|---|---|
| Opacity (`opacity`) | the colour texture's alpha (`diffuse.a`) | packed channels |
| Specular (`specular`) | the `_n` alpha of a non-PBR set (`n.a`), plus its packed channel as above (`_rmaos` A, or `_m` R under vanilla) | other packed channels |
| Fuzz weight, PBR (`fuzzweight`) | the PBR `_f` alpha (`f.a`) | packed channels |
| Subsurface weight, PBR (`subsurfaceweight`) | the PBR `_s` alpha (`s.a`) | packed channels |
| Roughness, glossiness, metallic, AO, height, fuzz mask (skin), `_orm`, curvature, cavity | the packed channels of the tables above, unchanged | any planned alpha target; the `_cnr` alpha (`cnr.a`, coat roughness) is offered but never suggested |

Fuzz weight and subsurface weight have no file-name term: a file is one only when you set its role.

**Alpha targets.** The colour texture's alpha is a `diffuse`, `pbr_albedo` or `skin_albedo` file of the set (never complex grass, a UI/sky texture or a landscape diffuse); `n.a` is a plain `normal` (so never a PBR `_n`); `f.a` is `pbr_fuzz`, `s.a` `pbr_subsurface`, `cnr.a` `coat_normal`. A target must be planned under the active target: under `vanilla` there is no `_f`, `_s`, `_cnr` or `_rmaos`, so a fuzz-weight helper reads "not used: no _f planned under vanilla". Cubemaps and the outputs of Also export never take a fill. A helper never creates an output, except the packed ones (`_rmaos`, `_rfaos`, `_m`, `_p`).

**Which data wins.**
- A **suggested** helper (its role and targets come from its name) fills an alpha only when that alpha is empty in the source: no alpha, fully opaque, or one flat value. A colour PNG with a real alpha keeps it, and the helper reads "not used: x.png already has alpha".
- An **assigned** helper (you set its role or its targets) always replaces what the channel holds, a PNG's own alpha included.
- Two helpers on one channel: an assigned one beats a suggested one; otherwise the first by path wins, and the other reads "`<target>` is filled by `<winner>`".
- A value fill (`packing.fill.*`) applies only when no helper fills the channel.

**Sizes.** A suggested alpha fill whose helper is not the target's size is skipped ("512x512, target 1024x1024: sizes differ"). An assigned one of the same aspect ratio is resized like a packed source, so a helper larger than its colour texture makes that texture larger; one with another aspect ratio fails the target with `PACK-SIZE-MISMATCH`, naming both sizes.

**The fill comes first.** The helper's channel is written into the target before the target is analysed, so alpha policy, format choice, coverage mips and cutout handling see the filled alpha as if the source had carried it. A diffuse that would be BC1 can therefore move to an alpha format. The target keeps its own bit depth.

**Read from and Invert.** `helper.read` picks the helper's channel: `r`, `g`, `b`, `a` or `luma` (Rec. 709 luminance). Unset reads R (or luminance where a recipe already does); A is never picked by itself. `helper.invert = true` flips whatever the channel would otherwise read (one minus the value). In a packed channel it flips on top of the recipe's own conversion: a glossiness source feeding roughness is already inverted, so a gloss map needs `helper.role = "glossiness"` and no Invert. An alpha fill has no automatic conversion, so Invert is its only flip. A helper with an explicit read is not reported for colour in a greyscale slot (`CHAN-NOT-GRAY`).

**Not a helper.** `helper.role = "none"` makes the file an ordinary texture again, converted on its own. Any other `helper.role` makes a file a helper even when its name or sub-type says otherwise. Roles apply to image sources, not DDS files. A file you give a role that has no known suffix (`iron_mask.png`) joins the set of its name without the last `_part` (`iron`) when that set exists.

A helper that fills nothing is reported, never a failure: `PACK-UNUSED-INPUT` (or `IN-PACKING-INPUT` with packing off), and "not used: <reason>" on its GUI row. `packing.enabled = false` stops packed outputs only: alpha fills still apply. `--on-existing if-changed` re-encodes an output when its helper, or the helper's read or invert, changes.

## Variance bake

In a `cs` PBR set with an `_n` and an `_rmaos` or `_rfaos`, the detail that the normal map loses in each smaller mip is added to the roughness mips, so distant surfaces do not look too shiny. Mip 0 stays as authored. The amount is limited by `mips.bake_clamp`. A `_cnr` bakes its own detail into its coat roughness. <!-- D:HD1 -->

The bake needs both files in the batch; if not, it is skipped and reported: `SET-MISSING-COMPANION` or `SET-COMPANION-DDS`. Sizes need not match: a smaller, larger or differently shaped map bakes from the `_n` at the matching detail, and its mip chain does not change. A map with no mips (`mips.levels = "none"`) is not baked. <!-- D:HD2,HD5 --> A companion that is only on disk, outside the batch, skips the bake without a message. A shared `_rmaos` (borrowed by several sets in the GUI) is never baked.
