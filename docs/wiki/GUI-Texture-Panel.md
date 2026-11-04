# Texture settings panel

The centre column with one texture selected: its settings, the questions it still needs answered, and how edits are written to `sdc.toml`. Several selected textures share one panel; see [Bulk editing](GUI-Bulk-and-Rename.md#editing-them-together).

## Centre

The settings of the selected texture, in groups: Type, Colour, Alpha, Mipmaps, Compression. Each value shows where it came from: the preset, the config, the analysis, or your `[files]` entry (for example "from sdc.toml", "suggested", or "from the meshes that use this texture"). Selecting a set shows the set editor instead.

| Row | Choices |
|---|---|
| Sub-type | Grouped as PBR (Community Shaders), Advanced skin (Community Shaders), Colour maps, Normal maps, Grayscale maps, Packed and data, HDR maps, Other. Only sub-types the current Mode allows are offered, plus the texture's current sub-type. The list opens under the field with a search box and the suggested type on top, the same picker as the Problems tab's **Set type…**. |
| Content | sRGB-encoded colour, or Linear data. |
| DDS tag | _SRGB or UNORM (linear), limited to what the format and Mode allow. |
| Alpha | The alpha policy ([values](Project-Config-Fields.md#alpha)). |
| Threshold, Comparison | Slider 1 to 254; Greater than, or Greater than or equal. |
| Coverage, 1-bit alpha, Exact opaque alpha | Auto / On / Off; on or off (BC1 only, [BC1 alpha](Texture-Types.md#alpha-policies)); on or off. |
| Normal handling, Normal base level | Renormalise or Renormalise and bake; Renormalise to unit length or Keep as authored. |
| Mipmaps: Levels, Filter, Edge | Full chain or None (base level only); Box, Tent, Mitchell, Kaiser; Wrap or Clamp. |
| Format, Effort, BC7 weighting | The block formats plus RGBA8, RGB8, R8, RG8 and (under `cs`) RGBA16F; bc7e effort; Linear, Perceptual, Alpha ×2, ×8 or ×16. |

Options a texture cannot use are greyed out, with the reason on hover.

**Output size.** The settings start with a line such as *Output: BC7 · 2048×2048 · 12 mips · 5.3 MiB ≈ VRAM*: the format, the size after any resize, the mip count and the bytes of the whole chain (hover for the exact bytes, the header and the file size). The status line and the bulk panel add *≈ VRAM: selection … · folder …*, the total over the selected textures and over every texture that is, or will be once decided, written. A source whose size cannot be read cheaply (a PSD) has no estimate and counts as "unsized", as does a texture whose size is still being read. A failed size read is tried again once, at the next plan; after that the file stays unsized for the session, and the Messages log says so.

**Compression.** For an emissive it shows the `FMT-HDR-SOURCE` reason (why it is BC6H or BC7 sRGB). Effort is locked for the uncompressed formats.

**Mipmaps: Filter and Edge.** The default filter is *Box*; the rules are under `mips.filter` in [Project config fields](Project-Config-Fields.md#mipmaps). *Edge* (*Wrap* by default) is locked while the filter is Box, because only Tent, Mitchell and Kaiser read past the edge.

**Normal base level** (normal maps only) stores mip 0 at unit length by default; choose *Keep as authored* for a normal map whose short normals are deliberate. A PBR set's normal map also bakes its detail into the set's `_rmaos`/`_rfaos` roughness (a normal map shared into another set bakes into that set's own roughness); its Normal handling row then reads *decided by the texture set*. A `_cnr` bakes its detail into its own coat roughness by default, so its row reads *built-in default*.

**Enable parallax** (complex material `_m` only) sits in the Alpha group.

- Ticked, the `_m` alpha is kept as the parallax height. Unticked, the alpha is written as 0 at every mip: complex material stays on and parallax is off.
- It starts ticked when the source alpha varies, and unticked when the alpha is constant (solid 255, mid grey, 0) or missing.
- A source without an alpha channel cannot turn it on; the box is greyed out, with the reason on hover. **↺** clears a value set on the texture itself.
- The row is hidden when the alpha is removed (the `_m` alpha is then 255).
- The same box is under the slot rows in the set editor, on the `_m`'s entry, and the bulk panel lists it for a selection of complex `_m` textures.

**DDS files.** A DDS in the folder reads "Already a DDS file: it is left as it is and not written to the export." **Re-encode this file** decodes it and encodes it again with the project's settings (block-compressed files lose some quality). It writes `reencode = true` to the file's `[files]` entry in `sdc.toml` as one undo step; the file then has settings like any other texture, with **Stop re-encoding** at the top of its panel to go back. The button is off while `sdc.toml` is read-only, and for a DDS another input already writes (an image twin such as `rock.png` beside `rock.dds`, which would be a blocking duplicate); the hover names that file. Re-encoding into the folder the DDS comes from fails that file with a red row ("re-encoding would overwrite the source DDS: choose another output folder"). With several textures selected, the bulk panel's **Re-encode *n* DDS files** does the same for the selected DDS files, leaving out the ones another file already writes.

**Used by meshes.** When a mod scan exists, the settings end with a collapsed **Used by meshes (N)** list: each mesh that uses the texture, with its mod and MO2 priority, the winner first (loose files before BSAs, then the highest priority).

- When the meshes decided the texture's alpha meaning, the meshes whose alpha property gave that answer are marked **decided alpha** (for blend, every blending mesh; for cutout, the meshes testing at the threshold used).
- The list is read in the background, starts once the selection has stayed on the texture for 150 ms, shows 200 meshes then "and N more", and is read again after a scan. A failed read shows its error in the list.
- **Show in Meshes**, under the list, switches to the Meshes view on **Any mod** with the search and chips cleared and a **Uses <texture> ✕** chip. The chip's **✕** clears it, and the selected mesh stays selected.

### BC7 weighting against BC3

*BC7 weighting* (Compression, and in the Advanced drawer) sets how much the built-in BC7 encoder weighs alpha against colour. It shows only for BC7 (otherwise hidden, with "BC7 only"). Each option has one line of hover text:

| Option | Use | Writes |
|---|---|---|
| **Linear** (the default) | Every channel weighs the same. | Clears `encoder.perceptual` and `encoder.bc7_weights`; over an inherited value from `sdc.toml` or a rule, writes `perceptual = false` and `bc7_weights = "linear"`. |
| **Perceptual** | bc7e's own weights. Greyed out on anything but colour without data alpha. | `perceptual = true` |
| **Alpha ×2**, **Alpha ×8**, **Alpha ×16** | Alpha error counts 2, 8 or 16 times. ×8 is an alternative to BC3 for delicate alpha; ×16 suits a `_n` with specular in alpha. | `bc7_weights` (and `perceptual = false` over an inherited perceptual) |

One pick is enough even when the texture's own value hid an inherited one: once the plan shows what was underneath, the override is written in the same undo step. The reset clears both keys. Changing the format away from BC7 drops the texture's own weighting. In the Advanced drawer the row is greyed out, with the same reasons, where the panel hides it. The bulk editor has the same row: Perceptual skips the textures it is locked for, and *Copy settings* carries a Perceptual pick.

![Grouped bar chart of mean RGB and alpha PSNR for BC3 and the BC7 weightings on three texture classes. On colour with smooth alpha, Alpha ×8 gives 51.40 dB alpha against 47.24 for BC3 and keeps 46.60 dB RGB against 41.00. On a normal map or packed rmaos with specular in alpha, BC3 keeps the highest alpha, and BC7 alpha rises with the weight.](../img/bc7-alpha-weights.svg)

For colour with smooth alpha, Alpha ×8 beats BC3 on alpha and keeps 5.6 dB more RGB. For specular in the alpha of a normal map or an `_rmaos`, BC3 still has the highest alpha; BC7 closes the gap as the weight rises, and keeps the better RGB or normals. The "w4" row was measured but is not a dropdown option. Full results: [BC7 alpha weights](../research/bc7e-tuning.md#bc7-alpha-weights-against-bc3-2026-10-21).

## Deciding

A texture whose name follows the conventions decides itself: a High-confidence suggestion is accepted and written to the project ([what the GUI writes on its own](GUI-Problems.md#what-the-gui-writes-on-its-own)). Until then its sub-type reads **(auto)**, with the reason on hover. High suggestions include a known suffix such as `_n`, `_rmaos` or `_rfaos`; an exporter term such as `_albedo`; a file with no suffix, as its set's colour map; and `_n` in a PBR set. The naming rules are in [Texture types](Texture-Types.md). Naming that breaks the conventions (a normal map saved without `_n`) is read as a colour map, so rename the file ([Renaming](GUI-Bulk-and-Rename.md#renaming)) or choose its sub-type.

A texture that still needs an answer has an **amber border**: a Medium or Low sub-type suggestion, the meaning of its alpha, or a blocking warning you must acknowledge. The centre column then shows an *Undecided* box with the question:

| Question | Controls |
|---|---|
| What kind of texture is this? | A sub-type dropdown and **Confirm**. |
| What does the alpha channel mean? | One line, "Alpha: Cutout (proposed) — confirm", and an **Alpha type** dropdown already set to the proposed type. Pick the type you want, or pick the shown one to confirm it. <!-- D:FM35 --> |
| An info line "IN-GREEN-UNSTATED" on a plain `_normal` | The map is converted as DirectX. Press *Treat as OpenGL (flip green)* if it is an OpenGL map; an OpenGL file then shows *Green: OpenGL (flipped)* with a *Treat as DirectX* button that undoes it. |
| A blocking warning | **Acknowledge for this texture**. |

Each answer is written to `sdc.toml`, as a field value or as an `allow` code on the file's `[files]` entry. A choice you make always wins over a suggestion.

### Alpha from the meshes

After a mod scan ([Mod scanning](Mod-Scanning.md)), a colour map whose alpha varies takes its alpha meaning from the meshes that use it, and the alpha question is not asked:

| Meshes using the texture | Result |
|---|---|
| Any mesh with alpha blending | Blend |
| Alpha testing | Cutout, at the strictest mesh threshold |
| Both | Cutout + Blend |
| No mesh with alpha | Remove |

The Alpha row then reads *from the meshes that use this texture*. A texture with no alpha, or a fully opaque one, keeps *Remove*. Your own choice still wins. Right after a folder opens, the question shows until the scan has been read.

## Editing

Any field in the centre column can be changed. The change goes into `sdc.toml` as a `[files."<path>"]` entry, keyed by the file's path relative to the folder holding `sdc.toml`. Packed outputs are keyed without an extension, for example `landscape/rock01_rmaos`.

**After an edit** the folder is planned in two steps: a quick plan of the sets the edit touches shows the change, then the whole plan lands behind it. Until the quick plan lands, the centre column's edit controls and the fix buttons in Problems and Review are greyed out ("Wait for the plan to finish"). These wait for the whole plan instead: set edits, the preset, the Mode, settings, per-file changes that move files between sets or change a set's outputs (a helper role, a PBR member's sub-type, a packing recipe, re-encoding a DDS), and any edit made before such an edit's whole plan has landed. <!-- D:FL6 --> Export blocks come from the whole plan, so their fix buttons stay greyed ("Being rechecked") until it lands. While it runs, the status line reads "· planning…" after the counts, and **Export** waits for it, then checks again ([Review and export](GUI-Review-and-Export.md#review)).

The GUI writes only `target`, `preset`, the `[files.*]` tables and `[inputs.*]` aliases. Comments, ordering, rules and every other table in your `sdc.toml` stay byte for byte as they were. Per-texture `[files]` entries sit above preset and config rules and analysis, and below any `--set` on the command line, so `sdc convert <folder>` honours them ([Project config](Project-Config.md#files)).

Unsaved edits put a dot in the window title until you save with **Save** or **Ctrl+S**; Export saves first. If `sdc.toml` changes on disk while you have unsaved edits, saving asks whether to reload it (**Reload from disk**) or overwrite it (**Overwrite with my changes**). Without unsaved edits, the GUI reloads it when the window regains focus or when you save or export. The other unsaved-changes dialogs are under [Dialogs](GUI-Dialogs.md#dialogs).

**Encoding follows type.** When a texture's sub-type changes (a set shader, a slot command, a drag, or a sub-type edit), a format and tag you set on it that the new type does not allow are reset, so the new type's default applies. An info row in Problems says which, with **Undo this change**. Settings that are still valid are kept, and a texture in a pinned slot is left alone and named in that row. One undo restores both the change and the reset.

**Resize** and **Size floor** are in the Advanced drawer ([Advanced texture fields](GUI-Advanced-Drawer.md#advanced-drawer)). How a size is chosen, including the automatic power of two: [Format and size](Project-Config-Fields.md#format-and-size). A resized texture shows a `DIM-RESIZED` note.
