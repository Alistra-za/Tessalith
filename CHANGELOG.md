# Changelog

## 3.1.0

### Main
- Faster encoding: BC1, BC3, BC4 and BC5 more than four times as fast with the same bytes, and BC7 and BC6H about 1.5 times as fast on CPUs with AVX-512.
- BC6H gets four encoder fixes, and BC3, BC4 and BC5 get a new **Max** search (3.0.0's `max` is `slowest`).
- Outputs written by an older encode version are kept until you ask for them with `--ev`, and validation after encoding is off by default.
- `_fuzz` and `_sheen` maps and, in the GUI, normals follow their set's shader; the GUI opens Review before an export, Problems can reach zero, and the Advanced drawer's settings move to a **Settings** tab, the texture panel and the set editor.

### Engine
- BC1, BC3, BC4 and BC5 textures encode more than four times as fast, and the files are byte for byte what 3.0.0 writes.
  - Converting 300 test textures as BC1 or BC3 took about 14 seconds where 3.0.0 took 63 to 68, and as BC4 or BC5 about 5 to 6 seconds where it took 20 to 28. Tessalith runs its own Rust port of the rgbcx encoder (`bcenc`), which skips work the original repeats; the original stays in the build as the reference the port is tested against. Numbers: docs/research/encoder-benchmark.md#encode-time-of-the-rust-port-of-rgbcx-2026-11-13-note-hy.
  - On CPUs with AVX2 the four formats encode faster again, with the same output and nothing to set: per block, BC1 takes 0.88 to 0.90 of the time and BC3 about 0.91 of it (on a Ryzen 7 9800X3D); the BC4 and BC5 gain is smaller and depends on the CPU. Numbers: docs/research/encoder-benchmark.md#speed-per-path-on-cpus-with-avx2-2026-11-22-note-hy.
  - BC3 at the default `encoder.bc45_search_radius`, and BC4 and BC5 on CPUs with AVX2, skip endpoint pairs the encoder can prove are no better than the best it has, with the same output: BC3 takes about 0.95 of its old time and BC4 and BC5 0.97 to 0.98 (an Intel Core i9-13900K, measured in a VM). Numbers: docs/research/bc4-family.md#max-near-exhaustive.
- BC7 and BC6H textures encode faster on CPUs with AVX-512: about 1.5 times as fast on the one CPU measured, a Ryzen 7 9800X3D, before the BC6H kernel fixes (Intel CPUs were not measured). On such a CPU their bytes differ from what an AVX2 CPU writes, at equal quality. Existing outputs are not re-encoded because of it. Numbers: docs/research/avx512-ispc.md.
  - On CPUs without AVX2, BC7 and BC6H use ISPC's SSE2 code, with the same bytes; CPUs with AVX2 are not affected. Measured before the BC6H kernel fixes, a CPU with AVX but no AVX2 takes 1.76 times as long as before for BC7 and 1.50 times for BC6H, and one with SSE4.2 but no AVX 1.29 and 1.09 times. Numbers: docs/research/avx512-ispc.md#without-avx2-the-sse2-path.
- BC6H HDR textures (`hdr_sky` and TruePBR emissive) encode with four fixes in the ISPC encoder, so their bytes change at every effort.
  - A black texel beside a very bright one (brighter than about 1344.0) no longer turns the whole block grey at the slower efforts. Endpoint colours round to the value the GPU decodes, where they were sometimes one step off near black and near the top of the range. A block encoded with one pair of endpoints keeps its best refinement pass instead of the last one. At efforts `fast` and `basic`, a block whose widest colour range sits just inside what mode 2 allows can use that mode.
  - On 18 real HDR images at BC6H's default effort (`slowest`) the picture gets closer to the source by 0.069 dB on average (95 % interval ±0.018 dB, better on all 18). Efforts `veryfast` and `ultrafast` are the exception: they lose 0.003 dB on average (interval ±0.002 dB) and 0.10 dB on one test image with a sun, and I kept the fix anyway because every slower effort gains about 0.07 dB. BC6H encoding takes 2 to 5 % longer than before at every effort (5 % at `slowest`), timed on one Intel CPU. Numbers: docs/research/encoder-benchmark.md#kernel-fixes-patch-0004-2026-11-22-note-lo.
  - A BC7 `pbr_emissive` is counted as written by an older encode version too, but its bytes are the same, so the command below leaves it. Existing BC6H outputs are kept by `if-changed` and counted as older. To encode them again, run `sdc convert <project> --on-existing if-changed --ev --property bc6h_uf16`.
- `--on-existing if-changed`, the default and what every GUI export uses, keeps two kinds of output that 3.0.0 encoded again, and counts them on every run:
  - an output written by an older encode version (a Tessalith update that changed how its texture type is encoded);
  - an output encoded at a higher quality target than the settings now ask for (`encoder.effort`, `encoder.bc45_search_radius` or `encoder.bc1_search` set lower);
  - the CLI adds the counts to its `done` line, the GUI to the export's Notes, and `--json` gets the `OUT-KEPT-OLDER-ENCODE` and `OUT-KEPT-LOWER-TARGET` events. A raised target, a changed source or any other changed setting still encodes the output again. See docs/wiki/Validation-and-Output.md#re-runs-and-if-changed.
- On the `cs` target, a `_fuzz` or `_sheen` map no longer makes a set Advanced skin by itself: the set's other maps pick the shader, and with nothing else to go on the set is PBR. To keep a set skin, choose **Advanced Skin** as its shader.
  - The map is read by the set's shader. In a PBR set it is the PBR fuzz texture, colour and weight, written as `<base>_f.dds` (it turns PGPatcher's fuzz on), and the set packs `_rmaos`, not `_rfaos`; if the set has its own `_f`, that `_f` is the fuzz texture and the map is listed as unused. In an Advanced skin set the map is the greyscale mask in the `_rfaos` G channel, as before. Under any other shader it isn't converted, and Problems lists it under Notes as unused. `sdc convert` can't see a shader chosen in the GUI, so there it still converts the map.
  - Where an earlier version recorded a set's diffuse as `subtype = "skin_albedo"` in `sdc.toml` only because of a `_fuzz` or `_sheen` map, the set leaves Advanced skin when you open or reload the project (not when `sdc.toml` is read-only). A Problems row lists the changed files on hover, and **Undo this change** puts them back until you make another change; Save writes the switch. If you undo the switch and save, it runs again the next time the project opens: choose **Advanced Skin** as the set's shader to keep the set skin. `sdc convert` keeps building such a project as Advanced skin until the GUI has saved it once.
  - Where no such `skin_albedo` was recorded, the set leaves Advanced skin at once. `sdc convert` agrees, except where the scanned meshes or a shader you chose in the GUI pick the shader (docs/wiki/CLI-Reference.md#does-the-cli-use-my-gui-edits).
  - Output names change with this. On `cs`, a `_fuzz` map typed `pbr_fuzz` is written as `<base>_f.dds` when its output has the default name (no output path or input alias set) and the set has no `_f`; under High it was written as `<base>_fuzz.dds`. A set that left Advanced skin exports a PGPatcher JSON entry and `_rmaos` in place of its `.csskin.json` and `_rfaos`. Tessalith doesn't delete old outputs, so the old files stay in the export folder, the set's old `.csskin.json` beside the diffuse included: delete them yourself.
  - A project that exports into its own source folder: a set whose earlier `_rfaos.dds` is still in that folder stays Advanced skin until you delete that file. A PBR set's fuzz map keeps converting there: the `<base>_f.dds` it wrote last time is its own output, not the set's `_f`. A `<base>_f.dds` you made yourself at that path is kept and the run says so (`OUT-UNKNOWN-EXISTING`); only `sdc convert --on-existing overwrite` replaces it.
- On the next encode with `mips.normal_base = "keep"`, standalone XYZ base normals and coat normals keep their average authored length in smaller mips, up to what XYZ can hold. Baked `_rmaos` and `_rfaos` roughness, opt-in complex `_m` gloss and self-baked coat roughness respond only to variation in direction, so a flat patch of short normals no longer gets rougher at mip 1. Mip 0 stays as authored.
  - With BC5 acknowledged, a base normal set to Keep fails at Resolve, even under `if-changed`. The error is `CFG-INVALID`. Its existing DDS and record stay in place. If it was the only eligible normal for a roughness member, the member reports `SET-MISSING-COMPANION` (Info) and loses the bake, and a member baked before rebuilds unbaked. Choose XYZ storage such as BC7, or set `mips.normal_base = "renormalize"`.
  - Projects without `mips.normal_base = "keep"` gain nothing from re-encoding for this change. By default, `if-changed` keeps otherwise matching older outputs. To update an affected project, run this command; the properties select whole texture types, including their default profiles:

    ```text
    sdc convert src --recursive --on-existing if-changed --ev --property normal --property landscape_normal --property pbr_normal --property coat_normal --property rmaos --property skin_rfaos --property complex_material
    ```
- Advanced Skin detail normals start from unit length even with `mips.normal_base = "keep"`. The encode version is 3.1.0. To rebuild otherwise matching older outputs, run `sdc convert src --recursive --on-existing if-changed --ev --property skin_detail`.
- Tangent normal outputs in formats that drop Z start from unit length even with Keep, and the texture panel locks **Normal base level** for those outputs and for Advanced Skin detail. To rebuild otherwise matching older outputs, run `sdc convert src --recursive --on-existing if-changed --ev --property coat_normal --property detail_normal`.
- Shared roughness maps bake once when every set uses the same normal; Problems explains when the sets cannot share a bake.
- Advanced Skin skips an authored `landscape_normal` for its roughness bake ([how the bake picks its normal](docs/wiki/Texture-Packing.md#variance-and-gloss-bake)).
- A TruePBR set whose albedo has an input name matches the meshes that use that name: the PGPatcher match key, the set preview's mesh list and the outputs under `textures\pbr\` follow the input name. Such a project writes its outputs under the new name on the next export; the old ones stay on disk.

### GUI
- New **Settings…** button in the header, which opens a **Settings** tab beside Presets. **This project** holds the Mode (moved from the Presets tab), the Preset, the export folder and **Export without review**. **App settings** holds Live encode, the three checks, the analysis cache limit and the **Textures** and **Sets** column ticks, which replace the header's eye buttons.
- The **Advanced** drawer at the window's right edge is gone; its settings are in the **Settings** tab, the texture panel, the set editor's **Also export** tab and a Problems row for unmatched `[files]` entries. An older `gui.toml` still loads.
- The texture panel has the fields the Advanced drawer held: **Levels** under Mipmaps, a **Size** group (Resize, Relative to, Size floor), **DDS header** under Compression, and a folded **Where these settings come from** section. The bulk panel edits Size and DDS header too.
- The set editor's **Advanced** tab is now **Also export**: this set's **Skip vanilla/CM for this set** tick (moved from the Material tab), the project's two **Also export as** ticks and the Vanilla recipe numbers. Review's **Recipe settings…** opens it.
- **Effort** sets the encoder search on BC1 (Fast, Default), BC3, BC4 and BC5 (Fast, Default, Slowest, Max); on those formats it used to write `encoder.effort`, which they ignore. The separate search rows are gone.
- The set editor's RMAOS/RFAOS dropdown is gone: the set's shader and maps pick `_rmaos` or `_rfaos`, and the packed row's title shows which. `packing.recipe` still works in `sdc.toml`, presets and `--set`.
- **Rename inputs…** is no longer in the Sets view; rename inputs from the Textures view. F2 on a set that is not a PGPatcher PBR set says so.
- The header's **Review n files…** opens Review for the whole project, and exporting a set, group or selection opens Review for that selection. Press **Export** in Review to write the files.
- Tick **Export without review next time** to skip Review when the project is ready and has only its regular outputs to write; the header button then reads **Export n files now**.
- Review lists the DDS and JSON outputs, and **Also updated:** shows the other files the export touches (project settings, records, borrowed copies, edited NIFs), with why on hover.
- Review's file count is the number of DDS files the export writes; with **Also export** on it is the PBR folder's, and the button reads **Export 24 files + Vanilla folder**.
- With a project open, **Tools ▾** holds **PGPatcher JSON editor…**, **Make marker cube…** and the new **Re-encode every output**.
- **Re-encode every output** makes the next export encode every recorded output again; files Tessalith has no record of are still kept. The tick lasts one export.
- Problems can reach zero: rows that only say what a default did are Notes, behind a **Notes** pill unless they have a button, and the tab reads `Problems (2) · Notes (67)`.
- Warning and check rows have **Mark as intended**, saved in `sdc-project.toml`: the row moves to Notes and leaves the count in **Files**, and **Undo mark** brings it back.
- A set is Ready when it and its textures have no problem row; notes, rows marked as intended and checks don't count. The set editor's first line says Ready, or Not ready with up to three reasons as buttons that take you to the fix.
- **Go to set** closes an expanded preview, scrolls to the file or slot the row is about and outlines it for two seconds.
- The editor for a set with no matching file shows its missing-file rows.
- A set's **⚠ n** and **+k more in Problems** open the [set filter](docs/wiki/GUI-Problems.md#narrowing-the-list).
- A set with no files of its own shows "Not ready: this set has no files yet." if it is empty or you marked all its problems as intended.
- In the Sets column, Ctrl+A and Shift+click ranges skip folded groups.
- More Problems rows have a fix button: **Change shader…**, **Set type…** (also on `CFG-UNAVAILABLE`, in place of an **Acknowledge** that changed nothing), **Turn parallax off** and **Rebuild Z**.
- Fixes that remove or switch something off (**Remove from set**, **Delete set**, **Remove match**, **Turn off {feature}**, **Turn parallax off**) run on a mouse click only.
- Hidden kinds show in the tab (`Problems (12 · 60 hidden)`), and the list says what is hidden.
- After **Rebuild Z** the exported row says it is queued until the next export, and a row's buttons sit right after its sentence.
- New **Clear recipe** in Problems on `cs`, for a recipe other than `auto` written on the set's own `_rmaos` or `_rfaos` that the set can't build, that its two packed files disagree on, or that names a map its shader doesn't read; it removes that recipe from both files in one undo step.
- A packed map the set's shader doesn't read offers **Change shader…**, not **Remove from set**; with such a recipe on `cs` it offers **Clear recipe** first.
- Problems warns when a PGPatcher entry or skin material uses a borrowed slot and none of them uses the set's own DDS. Remove a share, drop the borrow, or choose **Mark as intended**.
- Saved `[files]` entries that name no file in the current mode are one Problems row, with **Entries ▾** to remove them one at a time and **Remove all**.
- **Export details** in **Files** and in the Markdown report lists what the export did besides converting textures.
- PGPatcher JSON links on a set that keeps its entry:
  - a linked slot shows the file the JSON names, the card marks the link, the set's 3D view shows it, and Problems drops the missing-map warning for it;
  - a file a link displaces still converts and is listed under **Output files**; **Unlink** and **Use this file** keep any share underneath, one Undo step each;
  - a link to a local map can turn on Emissive, Parallax, Coat, Fuzz or Subsurface by its destination slot, and clearing the slot removes the feature it supplied; an explicit off still wins;
  - `CHK-PARALLAX-SCALE` compares keyed Landscape and PBR Landscape sets with an effective height;
  - export accepts a required map a link supplies, and a partial export brings in linked lenders as it does shared files; an excluded lender's DDS must exist at the exact output path or the entry is held.
- Set cards:
  - empty shader slots are hidden at rest: hover the status pill to list them, or drag a texture or files over the card;
  - a slot shared from another set is outlined in the text colour with two linked rings, and its chip can be dragged onto another set to share the file there too;
  - a slot shows at most two helper thumbnails, with a **+** that lists the rest;
  - a click on a thumbnail opens the set, and Shift+click opens the texture's settings;
  - a title keeps at least the last two parts of the set's id (`…/stone/banner`), and takes its own line when it doesn't fit beside the status pill.
- The set-drop question highlights your last accepted answer (Move, Share or Duplicate); hold Shift at release to use it. Dragging a helper onto a set no longer offers **Duplicate**.
- New in a set's right-click menu: **Copy JSON settings from <set> to N selected…**, and **Copy JSON settings** with **Paste JSON settings from <set> to N selected…**.
- **Edit match…** on a set card opens the set editor at its Matching group.
- In the set editor, 2D **Output files** lists each planned DDS of the set, and **Show in Explorer** appears for files already on disk.
- The slot rows put Slot, File, Format and the diffuse Alpha label above the controls, and the channel source pickers under a packed map line up in one column.
- **Match on this** on the Diffuse and Normal rows is the small link mark the cards use.
- Coat, Emissive, Glint and Fuzz keep their on/off state in **Features**, and closed Subsurface and XPMF headers show their mode or state.
- The diffuse row has an alpha dropdown in place of the **Opaque** toggle, with **Confirm** while the alpha is a proposal and ↺ to go back to the inherited alpha.
- Choosing **Advanced Skin** for a set that opened as PBR makes it an Advanced skin set: the albedo becomes `skin_albedo`, an `_f` is packed into the `_rfaos` as the fuzz mask, and the export writes a `.csskin.json` and no PGPatcher JSON entry. A type you picked yourself stays.
- Choosing **PBR**, **PBR grass** or **PBR Landscape** for a set with a `_fuzz` or `_sheen` map writes the types `sdc convert` needs. Auto or a shader outside the PBR family removes the ones you haven't changed since, and one other than **Advanced Skin** turns the recorded `pbr_albedo`, `pbr_normal` and `pbr_emissive` back into `diffuse`, `normal` and `glow`.
- In the GUI a grey map (envmask, height, model-space specular, skin `_sk`, projected noise, gray) is BC4 or R8 only, and a format a texture's type doesn't allow resets itself and says so in Problems (`FMT-CS-SKIN-LOADER` included). The CLI still writes what `sdc.toml` says.
- Normals follow their set's shader:
  - an untouched `_n` the GUI types `pbr_normal` (in a PBR Landscape set, the landscape normal too) drops its alpha silently by default and re-encodes once without it or a warning; leaving PBR can clear a `pbr_normal` an older project lists in `auto_pbr`; an `_rmaos` that starts baking re-encodes once with `sdc convert … --on-existing if-changed`;
  - a plain `normal` in a PBR, PBR Grass, PBR Landscape or Advanced Skin set can supply its roughness bake and keep its specular alpha;
  - sharing a PBR-family normal with a regular set can write `alpha.policy = "data"`, and if shared specular would be lost Problems offers **Keep alpha** ([when Share writes Data](docs/wiki/GUI-Sets-Column.md#sharing-specular-alpha));
  - Tessalith changes a following normal's settings only where its entry controls that one input, and **Keep alpha**, a shader pick, **Switch**, **Revert** and **Reset all** use the same check;
  - a failed **Save** keeps the recorded normal list out of the unsaved-edits prompt.
- A complex-grass source whose corner normal isn't unit length is a note while Tessalith renormalises the written file, unless the normal base level is kept as authored.
- A single undecided texture offers up to three likely types as buttons, and the bulk panel asks "What kind are these {n} textures?" for an undecided selection, with **Apply to {n}**.
- The bulk panel's option lists count how many selected textures each option writes ("BC7 ({k} of {n})").
- A packed tile's hover lists its source files, and a source's hover the outputs it is packed into.
- Every texture type has a plain name and a one-line description, shown on hover and found by the type menu's search.
- Normal handling, Effort, the DDS tag's UNORM entry and Settings' Mode (**Community Shaders (cs)**, **Base game (vanilla)**) use plain words too. No stored value changes.
- The window and the wiki say "type" where they said "sub-type". The `subtype` key in `sdc.toml`, `--subtype` and `--set subtype=` keep their names.
- When a type you choose makes a new set, Messages says so, with a **Show** button that selects the set.
- The automatic recording row in Messages names the first three files it typed, and its hover lists each file.
- Opening a folder with automatic type or alpha picks leaves **Save** off; in a new project **Write project files now** saves them at once.
- The status line counts source files, planned textures and sets, and export progress counts each extra export separately.
- Launch Tessalith without a project to see **Open folder…** and recent folders; `tessalith.exe <folder>` opens a project directly.
- The export folder dialog starts at the project's destination, then the last export folder you picked, then the project folder.
- The header stays on one row down to 1055 px wide; **Tools ▾**, **MO2…** and **Preset** move into **⋯** as space runs out.
- **Settings…**, Presets and the PGPatcher JSON editor open in Meshes, and **Back to meshes** returns to the mesh you were editing.
- The **Preset** picker lists the target's default once, at the top, tagged `· default`.
- The Textures and Sets status pills start off every run and are not saved; which set editor tab is open is not saved either.
- "Used by N meshes" counts each copy of a NIF in the scan, and each shape link opens that copy in Meshes.
- The plan bar says "Re-checking after your change…" after your own edit and "Re-checking…" for other re-plans.
- The 3D preview renders a frame in less than half the time on the one PC measured (an RTX 4090), with the same pixels. Numbers: docs/research/render3d-backends.md#after-the-gpu-conversion.
- NIF edits are saved with the project, and export writes the edited NIF and its record to the export folder. An unexported draft from an older version moves out of that folder on your next edit.
- The NIF edit footer shows where the draft is saved, and **Edit** is a button beside the NIF's name. Switching or closing projects ends the NIF edit session once its changes are saved.
- The Meshes list names the scan it shows and when it was last written. The NIF editor uses **Texture wrap**, **Shader flags 1** and **Shader flags 2**.
- The GUI runs on egui 0.37. Screen readers hear a name for every box, list and number field, and Undo and Redo say which step they undo.
- Fixed in menus and lists:
  - a list or menu opened with a click no longer risks closing on the same click;
  - once a list inside a panel reaches its end, the wheel scrolls the panel;
  - in a menu, Up, Down and Enter move between and pick the entries;
  - while you drag a set, the mouse wheel scrolls the Sets list;
  - a scroll bar only takes clicks along its own length, and a panel inside another no longer draws past its parent's edge.
- Fixed in sharing:
  - a shared slot follows its texture when the texture moves to another set or another texture is shared into the slot;
  - deleting a set or removing a texture from it gives the texture to the first set that shared it and can take it;
  - a share of a share finds the texture, and a set that shares a slot from itself keeps its own file;
  - if your project already has a slot such a share left empty, the next export fills it: its PGPatcher JSON names the texture, a vanilla set gets its copy of it, and on Community Shaders a shared normal feeds its roughness bake. If that set is a PBR set listed before the texture's other PBR sets in the Sets column, the texture's own Community Shaders PBR output moves to that set's prefix: the export writes it at the new path, leaves the old file on disk, and points the other sets' JSON lines at the new path;
  - a set that borrows its normal or packed map no longer says the shader needs that map;
  - Replace waits for a successful plan after a type change, and Undo keeps a file that replaced a duplicate with different contents while the copy was finishing.
- Fixed in the window and panels:
  - after an edit that needs a full re-plan, the texture panel waits for that plan instead of taking edits early;
  - behind the Presets, JSON and Settings tabs, an open set editor or preview no longer keeps the Textures and Sets columns narrow;
  - the Textures and Sets columns show their scroll bar, and the 3D view's **Mesh** dropdown wraps instead of running past the window's edge;
  - a packed tile, such as an eye `_rmaos`, draws opaque instead of almost black;
  - on a high-DPI display, the starting size is at most 8192 physical pixels per side and fits the GPU's limit once known;
  - texture slots scroll above the NIF edit footer when the bottom panel is open;
  - counts of one read in the singular ("1 file, 1 problem");
  - Tessalith starts when Segoe UI is missing: it uses Arial or another common font, else its own built-in fonts, which Messages reports, and CJK file names use a Windows font for their script.
- Fixed in Problems:
  - a set's Problems filter follows it when it loses its last file or its export root changes;
  - the **Kinds ▾** hover no longer says Review lists every warning;
  - when an `_f` takes a fuzz map's output name, the unused-map Note names the file's resolved type.
- **Assign…** on an Advanced skin set's empty **Skin wet** or **Skin detail** row works: it lists that kind of file from the set's folder, and **From another set** borrows another set's map.
- Renaming a texture's input from the rename dialog keeps its set's Also export opt-out, and moving an opt-out keeps the comments in `[export]`.

### CLI
- New `sdc convert` flags `--ev` (alias `--encoder-version`) and `--property`. `--ev` encodes the outputs of an older encode version again; `--property <words>` limits that to outputs with those properties: a format, `alpha`, `cutout`, `mips` or a texture type, comma for all of them, the flag repeated for any. From this version on, a release entry that changes how a type is encoded ends with the command that picks it up, in the form `sdc convert … --on-existing if-changed --ev --property <words>`. `--ev` with `--on-existing skip` or `overwrite` is an error. See docs/wiki/CLI-Reference.md#re-encoding-after-an-update.

### Common
- Validation after encoding is off by default. `sdc convert` and GUI exports no longer decode and check every DDS after encoding, so they finish sooner, and the files are byte for byte the same either way.
  - Complex material `_m` and complex grass textures are still always checked. `sdc convert --validate` and the new **Validate exported DDS** in Settings (App settings, Export) turn the check on for every type. With it on, outputs an earlier run left unchecked are checked where they are, without encoding them again, and the `done` line counts them.
  - Without it, `sdc convert` no longer exits 4 for most files that fail a check: add `--validate` to a script that relies on exit 4. See docs/wiki/Validation-and-Output.md#what-is-checked.
- **Max** on the **Effort** row of BC3, BC4 and BC5 (`encoder.bc45_search_radius = "max"`) is a new, slower search: after the radius-10 search it tries every endpoint pair within 16 of the block's lowest and highest value, and never leaves a block with more error than **Slowest**. **Slowest** (`"slowest"`) is the radius-10 search that `max` meant in 3.0.0.
  - A project or preset that already says `max` gets the new search, so its BC3, BC4 and BC5 files may change and take longer to write; write `slowest` to keep the radius-10 search. A config that sets `slowest` can't be read by 3.0.0.
  - BC3, BC4 and BC5 files that 3.0.0 or older wrote at `max`, or whose record names no engine version, are encoded again on the next `if-changed` run; if you change `max` to `slowest` first, they are kept, as long as nothing else about them changed. Numbers: docs/research/bc4-family.md#max-near-exhaustive.
- New setting `encoder.bc1_search` (`default` or `fast`, set by the texture panel's **Effort** row on BC1 and BC3). `fast` searches BC1 endpoints at rgbcx level 12 instead of 18, in about a third of the time for about 0.1 dB of colour PSNR. `fast-preview` sets it. `default` writes the same bytes as before and is not saved to `sdc.toml`; a config that sets `fast` can't be read by 3.0.0.
  - Outputs `fast-preview` wrote with 3.0.0 are kept on the next run, and a run over such BC1 or BC3 outputs reports them as kept at a higher quality target every time; that is expected, not a fault. Numbers: docs/research/encoder-benchmark.md#fast-bc1-search-2026-11-14.
- The BC7 weighting dropdown gains Alpha ×4, ×32 and ×64 (`encoder.bc7_weights` = `alpha4`, `alpha32`, `alpha64`), for the built-in BC7 encoder only. No existing output changes; an `sdc.toml` that uses them is refused by older versions.
- The engine's and the CLI's messages say "type" where they said "sub-type", for example `unknown type 'nope'`, and `sdc inspect` prints each planned file's line as `  type pbr_albedo  format …`. Messages no longer cite spec sections or Note letters. Anything that matches the old message prose needs updating, event-stream warning text included (`--json`). The JSON schemas' descriptions are unchanged. Exported files are byte-identical.
- `PACK-UNUSED-INPUT` tells you to remove the file, or change the set's shader or the file's type, not to change the recipe.
- New crate `texcore` (DDS I/O, decoders, colour curves, cube maths, image loading; no C or C++): `render3d` depends on it, not `engine`, which re-exports every moved item at its old path. No output byte changes.
- `sdc.exe` and `tessalith.exe` are each 463 KB smaller: the ISPC Texture Compressor code holds only its BC6H encoder, with no BC6H byte changed.

## 3.0.0

### Main
- Sets gain groups, partial exports, shared helpers, a *no set* state and mapping to texture set records.
- Advanced skin sets write a `.csskin.json` on every export, and the 3D preview has Community Shaders 1.9.0 built in.
- Removed: panorama, ball and photo cubemap sources, RNM normal layers, **Generate from normal map** and the GUI's UV checker.
- The engine is GPL-3.0-or-later, every export has the `textures` folder, and a lone file with an underscore and no known suffix waits for a type.

### Engine
- Cubemaps are made from six faces, a cross, a strip or a cube DDS. Panorama (`equirect`), `mirror_ball`, `angular` and single-photo (`photo`) sources, `cubemap.ground_fill` and `cubemap.horizon` are removed, with the GUI's Set layout menu and Ground group. A project or preset that names one is refused with "removed in 3.0" and a hint. Convert a panorama, ball or photo to six faces, a cross or a strip first. A square or 2:1 image typed as a cube is Unresolved with "not a cube source"; change its sub-type. A cube's lower half is the source's own pixels.
- RNM normal layers are removed: Tessalith no longer blends `_bump`, `_bump2`, `_detailnorm`, `_detailnorm2` and `_bent` files (or their masks, or the `n_` and `cnr_` prefixes) into a normal, so **a set that had layer files exports a different base normal on its next export**. Blend detail into a normal in your image editor before you export.
  - `_bump`, `_bump2`, `_detail2`, `_detailnorm2`, `_bent`, the mask terms and the two prefixes are not recognised: a layer file such as `rock_bump.png` is a set of its own, and alone it waits for a type (`sdc convert` exits 1), so delete it or assign a type.
  - Every `rnm.*` key, the `RNM-*` codes in an `allow` list and a `helper.role` that names a layer are refused with "removed in 3.0" and a hint; delete them. A file called `rnm` still loads.
  - The GUI loses the RNM section, the blended-normal preview, the normal card's teal outline and the **Blends into** row; `sdc inspect` prints no `rnm` lines.
- The export always has the `textures` folder. A project opened at a `textures` folder, or at a folder inside one (`MyMod\textures\armor`), used to write `<export>\armor\x.dds`; it now writes `<export>\textures\armor\x.dds`, the path the game, the PGPatcher JSON and the Community Shaders JSON name. **Such a project writes to new paths on its next export, with its `_sdc` manifest in `<export>\textures\...\_sdc`; the files and the manifest at the old paths stay on disk**: delete them yourself. Set names and your set settings are unchanged. A project opened at the mod folder, or at a folder with no `textures` in its path, writes where it did. See docs/wiki/Validation-and-Output.md#where-the-files-go.
- Community Shaders Advanced skin material files: every export writes `<diffuse>.csskin.json` for each Advanced skin set, with the values set on the skin card, the paths of the set's `_rfaos`, `_wet` and `_detail_n` (a borrowed slot points at the host set's file) and the diffuse's size and hash. There is no button and no opt-out; the card's **Export JSON** placeholder is gone. An edited file is kept, a stale one is left in place and listed in the export notes. A set with no diffuse or a borrowed diffuse writes no file, and a value outside the Community Shaders range blocks the export. See docs/wiki/GUI-Set-Materials.md#advanced-skin-card.
- `skin_wet` is always BC7; the BC4 height form is gone. A grey `_wet` is not written (new warning `SKIN-WET-GREY`) and the rest of the batch still writes. `skin_rfaos`, `skin_wet` and `skin_detail` accept only the formats the Community Shaders loader reads (BC1, BC3, BC7, RGBA8, plus BC5 for `skin_detail`); another format is `FMT-CS-SKIN-LOADER`, which `--allow` cannot lift. Existing `skin_wet` outputs re-encode once.
- New sub-type `skin_detail`, suffix `_detail_n`, recognised only in an Advanced skin set on `cs`: BC5 when its source is opaque and BC7 otherwise. A set's `_wet` and `_detail_n` have slots 9 and 10 in the GUI, so another Advanced skin set can borrow them; they never reach a PGPatcher JSON or a texture set.
- The detail normal has three names: `_detail`, `_detailnorm` and `_detail_n`. In a landscape or PBR landscape set each is the `detail_normal` sub-type, in a `cs` Advanced skin set `skin_detail`, and in any other set they stay undecided. 2.x knew `_detail_n` only on a skin set and the other two only on a landscape set. The output keeps your spelling, two of them in one set is `IN-DUPLICATE`, and the GUI refuses an output name that ends in one of them.
- A file alone in its set with an underscore and no recognised suffix (`rock_bump.png`, or an ordinary `barrel_01.png`) no longer becomes a colour map: it waits for a type. A CLI project that relied on that default now exits 1 for those files, with `unknown suffix: assign a type with --set file:<path>.subtype=<type> or a [files] entry`. A set shader no longer types such a file either: `--set set:<id>.shader=advanced_skin` on a lone `x_y.png` used to export a `skin_albedo` and now exits 1. `--allow SUG-LOW-CONFIDENCE` no longer converts a job that has no suggestion (it used to write it as `generic_data`), waiting or not. In the GUI a waiting file is an Undecided row with no set card until you pick a type; one a scanned mesh uses as its diffuse, or a GRAS model's diffuse, does not wait. See docs/wiki/Texture-Types.md#texture-sets.
- A set of only `_n`, `_sk` and `_wet` maps with the Advanced Skin shader is a shared skin set: its maps are skin maps and it warns of no missing diffuse or `_rfaos`. Its `_wet` and `_detail_n` join it; for every other set the join still follows the set's maps, not its chosen shader.
- An explicit `alpha.policy = "remove"` on a colour map whose alpha varies is opaque colour to the preset's rules, as a mesh `remove` is: under cs-compact it is BC1, not BC7.
- A cutout texture that is resized (a `resize` mode, or a size that is not a power of two) keeps its source's alpha-test coverage on every mip, the largest included; it used to keep the coverage of its resized largest mip, so thin cutouts could lose texels. Under `if-changed`, every diffuse, PBR albedo, UI/sky and skin albedo output is written again once; only resized cutouts change.
- A helper shared into another set, or joined under a hand-set role, gives that set's packed output its own card and `[files]` key (`x_rmaos`), so fills and recipes set on it reach it and a `[files]` entry on it is no longer `CFG-FILE-UNMATCHED`, and an output name or texture-set record on that set puts it in the set's own folder; settings under the old key are not moved.
- An `_rmaos`, `_rfaos` or complex `_m` of another shape or a non-power-of-two size than its `_n` gets the variance bake instead of `SET-BAKE-RATIO`; a member with one mip level is not baked.
- A renamed PGPatcher set can have more than one match or a path contains again: its entry then keeps its matches and names the renamed files in slot commands instead of writing `rename`; a set with one match writes `match_diffuse` + `rename` as before. The "not written" markers are gone.
- PGPatcher JSON: every PBR set gets its entry, also with no PBR albedo, no mesh or no record; with no albedo the mesh keeps its own diffuse.
- Also export leaves out a PBR set whose recipe failed, with the note "not derived: N map(s) failed in the PBR export". A set with `packing.prefer = raw` and mismatched raw maps is left out too, although its pre-packed `_rmaos` converts.
- A texture whose alpha a helper fills is checked on the filled image, so its warnings and readouts see the filled alpha.
- Fixed in packing and the bake:
  - a packed output whose source feeds some channels inverted and others not re-encodes when that source's invert is toggled; under `if-changed` every such output is written again once;
  - a pre-packed `_rmaos` or `_rfaos` with a flat alpha takes a separate specular into that alpha; an alpha with content, or another loose map beside the image, still keeps the file;
  - two raw maps of one name (`mouth_ao.psd` and `mouth_ao.png`) are no longer `IN-DUPLICATE`: the first by path feeds the channel and the other is `PACK-UNUSED-INPUT`. Only a raw map, a skin `_f` or a normal layer you set by hand counts; two normal layers found by name stay duplicates;
  - not fully fixed: a twin removed from its set with **Remove from set** still makes a set of its own, and writes the same output as the first, until the groups work;
  - the variance bake no longer changes mip 0 of an `_rmaos`, `_rfaos` or complex `_m`, which matters when it is smaller than its `_n`. These types re-encode once.
- Fixed in inputs and outputs:
  - an export folder inside the opened folder no longer moves the output root; before, the second export wrote a flat copy without the `textures` folder. A project that already flipped goes back to the `textures` paths on its next export; the flat copy and its `_sdc` manifest stay on disk, so delete them yourself;
  - a set mapped to a texture set record, or to a target file, with an `_s` backlight map no longer blocks the export with a collision against itself;
  - a file link (symlink) whose target is in the output folder is no longer read as an input, unless the run is in place.

### GUI
- The 3D preview uses Community Shaders 1.9.0 (AIO build 2026-10-03) built into Tessalith, with its default settings, and works with no Community Shaders installed. It no longer looks for your Community Shaders folder, so **Shader folder…**, the "Community Shaders not found" line and the "moved/updated" prompt are gone, and your own CS settings, enabled features and Game Gamma no longer change the preview. An old `shaders` key in `gui.toml` is ignored. See docs/wiki/GUI-Set-Preview-3D.md.
- Set groups: the Sets column shows each group as a header with its name, its set count and the worst status of its sets, with its sets indented under it.
  - Click the header to fold it (kept per project in `gui.toml`). **Move to group** is in a set's and a selection's right-click menu, and a set dropped on a header joins it.
  - A group sits where its first set sits. Deleting a set takes it out of its group, and Undo puts it back.
  - Each group is a `[[groups]]` table in `sdc-project.toml` with a `name`, the `sets` in it and `excluded`. See docs/wiki/GUI-Sets-Column.md#groups.
- **Export set**, **Export n sets** and **Export group** export only those sets, plus any set they borrow a slot from; the rest of the export folder, its per-set JSON and its PGPatcher JSON entries stay as they are. **Exclude from export** on a group header leaves its sets out of every whole-project export, Also export's Vanilla and CM folders included, until **Include in export**. Also export runs only on a whole-project export. `sdc convert` exports everything as before. See docs/wiki/GUI-Review-and-Export.md#exporting-part-of-the-project.
- **Delete set** works on every set: its textures go to *no set* and the automatic set doesn't come back, even after a new plan. **Remove member** puts the texture in no set too, also the only one in a set and one you moved in. A texture in no set is on no card, isn't written, shows **No set** in the Textures column, counts for no warning, question or Undecided total, and doesn't make its same-named twin `IN-DUPLICATE`. Drag it onto a card, or use **Back to its automatic set**. The list is `no_set` under `[project]` in `sdc-project.toml`. See docs/wiki/GUI-Sets-Column.md#textures-in-no-set.
- A set can share another set's helper (an AO, a roughness, a mask) with no copy:
  - the set it is shared with packs or fills from it as if it were its own, writing in that set's folder under that set's name; a share writes no file of its own;
  - a shared roughness makes a `cs` set PBR, and a helper in no set can be shared too;
  - in the GUI, drag a helper onto another set's card and pick **Share (use this file here too, no copy)**; a helper in no set is dragged from its row in the Textures column;
  - the set editor (**Stop sharing**), the helper panel (**Shared with**) and the **Source** picker list shared helpers; one only another set uses reads `used by <set>`;
  - the line is `share` in the set's `sdc-project.toml` entry. See docs/wiki/GUI-Set-Editor.md#shared-helpers.
- A set can be mapped to a texture set record:
  - **Map to target…** opens a picker over a read-only target mod's records, mesh paths and texture file bases (the scan's winning records when no target mod is set), with **Change mapping…** and **Remove mapping** once mapped;
  - the card shows `Record: EDID (0x814, plugin)`, a slot the record supplies reads "From record: file", and after a scan a changed record raises a Problems row with **Use new paths**;
  - a mapped set writes each file in a slot the record names to the record's path, the `.csskin.json` follows the diffuse, and other files keep theirs;
  - the mapping is `[sets.txst]` in `sdc-project.toml`;
  - an empty slot borrows from the set that writes the record's path (one `_n` written), or counts as supplied when no set writes it;
  - a skin set that takes its normal this way also takes that set's `_wet` and `_detail_n`, and a shared-map set of a skin set becomes Advanced skin itself;
  - its own file, an explicit borrow and a pin win; **Clear** turns a fill off (`no_fill`) and **From <set>** turns it back on.
- **Rename outputs** appears only on PGPatcher PBR sets on Community Shaders; with no such set selected it leaves the menus and the dialog's mode switch, and a vanilla set renames through Map to target. A set that already has an `output_name` keeps writing under it, and the set editor can clear it.
- **Rename outputs** can move a PGPatcher PBR set's outputs to another folder (*Whole path*, or a path such as `armor\steel\plate` in **Rename**); Outputs rows show game paths.
- The texture panel's rows line up in one label and one control column. The Colour group is gone: **Input content** (was Content) and **DDS tag** are in Compression, after Format. Picking another format keeps the DDS tag when that format allows it.
- In the texture panel, **Helper** is a tick under Sub-type; unticking a helper writes `helper.role = "none"`, as **Not a helper** did.
- Setting rows of the texture and bulk panels and the Helper tick have a **?** after the label; hovering it or the label says what the row does.
- The alpha question has a **Confirm** button beside its dropdown, in the texture panel, Problems and Review, that keeps the proposed type.
- NIF Edit no longer edits clips (add, remove, rename, timing, Convert to clips). Existing clips still play, and the keys of a clip-driven controller stay editable. A saved edit drops its clip edits when it opens.
- The skin card edits the 22 parameters of the new Community Shaders contract in six groups. Values saved under the old names (`SkinMainRoughness`, `F0`, `UseSSS`, `sssWidth` and the rest) are not translated: the next edit of the set removes them and the card starts from not set.
- A shared skin set reads "shared" on its card, shows only its maps, writes no `.csskin.json` and shows no skin warning.
- Set cards:
  - each helper is a badge on the corner of the slot chip it fills, with the file and its fill on hover ("fills RMAOS R"); a click opens the helper panel;
  - empty chips and trailing tiles carry their letter or tag bottom-left, and 5 chips fit a 260-point Sets column (minimum 133 points);
  - a click anywhere free on a card opens the set, and the shader combo has its own row at the card's right.
- Sets column:
  - the search box and shader filter have their own names for screen readers, and the shader list shows how many sets use each shader;
  - filter counts are what a click shows, and the list starts at the top when a filter changes;
  - opening a set takes its room from Textures before Sets;
  - a search keeps the matched folder in a cut title;
  - status text is readable in both themes, and the texture settings checkboxes have names.
- A set line (`add` or `remove`) that names a file deleted from disk clears itself, with an Info line in Messages, instead of leaving a red **?** chip. It is not an undo step. Nothing is cleared while the plan has no files or the project folder is missing, a line is kept when the file's own folder is missing, and borrows keep their chip.
- A set no longer warns `SHADER-MISSING-MAP` for a map a scanned mesh or a winning texture set record already supplies beside its diffuse (GUI only).
- Problems: a set no mesh or record uses yet, or with no albedo, shows an info row and is still exported; collisions and mismatches stay warnings.
- Problems has a **Kinds ▾** menu that hides kinds of warning and info row in a project. Review still lists every warning.
- The analysis cache's limit is 500 MB (was 200 MB) and thumbnails are stored compressed. Set it with **Analysis cache limit** under Advanced → Cache (`analysis_cache_mb` in `gui.toml`); a new limit applies the next time Tessalith starts. Entries an older version wrote still read.
- A cube DDS opened as a source previews its whole cross within the preview limit instead of decoding several GB first; one with no small enough mip is not previewed and says so.
- Fixed in the Sets column:
  - opening a set in a narrow window no longer leaves its card out of view;
  - the "hidden by the Sets filter" line wraps, with **Clear filter** below it;
  - long card titles are cut in the middle so the leaf stays;
  - the "Drop here to start a new set" hint is grey instead of amber;
  - dragging a group's first set down onto a card drops the group where the insertion line shows, and dropping it on a set of its own group does nothing;
  - a set with a PBR map but no PBR albedo (a lone `_f`) says "PBR → JSON" on Community Shaders and "PBR, no JSON" elsewhere, not PBR over a "vanilla" pill;
  - an Advanced skin set's packed roughness slot says RFAOS, not RMAOS.
- Fixed in the set editor and renaming:
  - changing a set's input name also renames it in the project order and in other sets' borrows when it has no entry in `sdc-project.toml`;
  - **Rename inputs** and the input name box read a preset's suffix aliases, so a member named by an alias keeps its suffix;
  - renaming a set to the name of an automatic set the engine still makes is refused with the reason; before, the set lost its settings;
  - renaming an Advanced skin set's outputs, or giving it an input name, keeps the `_wet` and `_detail_n` suffixes, and Rename refuses two outputs of one set on one name;
  - the input name box shows `body` for `body_albedo`, so typed names no longer snap back or double the suffix;
  - **Clear** and **From this set** on a borrowed slot drop the borrow.
- Fixed in the texture and helper panels:
  - changing a texture's sub-type clears a format and DDS tag the new type does not allow, also on the quick re-plan;
  - Copy settings also writes a field another ticked field unlocks on the target, such as the alpha threshold with a cutout policy;
  - clicking a centre tab commits the text field being edited; before, the typed value was lost;
  - the helper panel's Fills rows line up, one line per target, and a packed target is named by its output (`bake_1001_rmaos R`).
- Fixed in the previews:
  - in the set preview, a member that stops being blocked is simulated again, so its "will be skipped" row clears and its material is patched;
  - a failed preview shows the error as plain text instead of Rust debug output;
  - CS hair self-shadow no longer leaves hair lit by ambient light only.
- Fixed in Problems and the path pickers:
  - rows about a packed channel or a shader's map say which map and channel, not "add the missing map";
  - textures named only in a texture set record (TXST) are offered by the path pickers;
  - the path pickers and the missing-map check read a record's slots by lighting slot, so its `_rmaos` no longer shows as slot 3 or its `_sk` as a height.
- **Export set**, **Export n sets** and **Export group** keep their sets when you choose or change the export folder while their Review is open or the export waits for a plan; before, the export wrote nothing.
- **Export set**, **Export n sets** and **Export group** with an Also tick check the PBR folder they write, not the export folder.
- In NIF Edit, a texture path typed without Enter is kept when you click a tab or another control, and the click's own edit is kept too; a **Fix** on that same slot is dropped.
- NIF Edit no longer takes an export folder that holds the mesh it edits: the NIF's row is blocked with the reason until you choose another folder.
- The analysis cache's size prune keeps the entries you use: opening a folder refreshes its entry's time when it is over a day old.
- The line shown when the toast's Undo comes too late says: press Ctrl+Z to undo or Ctrl+Y to redo.

### CLI
- New `sdc uv <PATTERN>` writes an image of a texture's UV islands, from the GUI's scan database or `--meshes`; it replaces the GUI's UV coverage checker.
- New `sdc dilate <PATTERN>...` fills the texels outside a `_matid` mask or the `_uvmask` image `sdc uv` writes, keeps the masked texels, and writes to a `dilated` folder.
- `sdc dilate` never replaces an input of the same run, or the `--mask` file, with or without `--overwrite`; that input writes nothing and the run exits 1.
- `sdc validate` reads the alpha of all six faces of a cube DDS, so a marker from `sdc cubemap-marker` no longer fails `VAL-ALPHA-GUARANTEE`.

### Common
- Tessalith 3.0.0 also carries the 2.1 work, which was never released on its own.
- The engine (`engine`, `encoders-sys`) is now GPL-3.0-or-later like the rest of Tessalith; it was MPL-2.0. The vendored encoders keep their own licences.
- **Generate from normal map** is removed: the dialog, **Regenerate…**, **Delete generated file…**, the "generated" marker and the stale-map row (`GEN-STALE`) are gone, with the engine solver behind them. Files named `_generated` are still read as ordinary sources (`GEN-REPLACED`, `GEN-AO`, and `_curves_generated` for Also export), so make a height, AO or curvature map in another tool and name it that way. An `allow` list or `--allow` that names `GEN-STALE` is refused with "removed in 3.0". An old `[generated]` table in `sdc-project.toml` loads without a message and is left as it is.
- The UV coverage checker is removed from the GUI: the preview's **UV coverage** toggle, **Check all UV coverage** and **Stop checking UV coverage**, the UV switch under Advanced → Checks, and the `CHK-UV-GUTTER` warning. An `allow` list or `--allow` that names `CHK-UV-GUTTER` is refused with "removed in 3.0" and a hint to use `sdc uv`. An old `uv` key under `[checks]` in `gui.toml` is ignored.
- The GUI records an alpha the scanned meshes drop (`remove` on a colour map whose alpha varies) in `sdc.toml`, so `sdc convert` plans it the same.
- New wiki page, Specular reference: the F0 of 81 non-metal materials and the albedo colour of 39 metals, for setting **Specular level** on a True PBR set.

## 2.0.0

### Main
- Every output re-encodes once, and records written by 1.x are stale.
- NVTT and many keys, codes and commands are removed; a config, preset or allow list that names a removed key or code is an error that says so.
- BC6H keeps small bright highlights, BC1 can keep 1-bit alpha, and every output with mips is a power of two.
- XPMF profiles for Landscape sets, a flat **Meshes** list, and a Review that only holds what blocks or needs an answer.

### Engine
- 2.0.0 re-encodes every output once (one encode version for every texture type). Records written by 1.x are stale, so the next export writes them again. The version change alone changes no bytes; bytes change only where a line below says so.
- BC1 and BC3 colour encode for NVIDIA and AMD together (rgbcx `cBC1Dual`); every BC1 and BC3 output re-encodes once. `--bc1-decoder` and the GUI's BC1 decoder setting are gone (a `bc1_model` key in a config is an error that says so; an old `gui.toml` with `bc1_decoder` still loads). Validation, previews and readouts decode BC1 as NVIDIA does; a BC1 DDS read as an input keeps the exact decode.
- BC6H (HDR) at encoder effort `slowest` and `max` uses a peak-preserving mode: small, very bright highlights such as a sun keep their brightness (venice_sunset's peak goes from 0.22 to 0.92 of the source). Other blocks encode as `veryslow`. BC6H now uses `slowest` unless you set the effort yourself (a config, preset, rule, `[files]` entry or `--set`; `veryslow` still gives the plain mode), including a texture of another type set to BC6H by hand. An 8-bit emissive stays BC7 at `veryslow`. `fast-preview` keeps `fast`.
- Every output with mips is a power of two: the longer side rounds up when it is within 5 % of the next power of two and down otherwise, and the shorter side keeps the shape, rounding up within 15 % (1000x1000 becomes 1024x1024, 1800x1800 becomes 1024x1024, 1024x768 becomes 1024x512; the rule is in docs/wiki/Project-Config-Fields.md#format-and-size). A side that grows is enlarged with linear interpolation. Cube faces round the same way.
  - Such sources no longer stop with `DIM-NPOT`; it is a warning, raised only for a texture with `mips.levels = "none"`, which keeps its size.
  - `resize = "pot-down"` and `drop_top_mip` (the Advanced **Half size** tick and the **Resize to a power of two** button) are removed, and a config, preset or `--set` naming either is an error that says so.
  - `resize` gains `quarter` and `eighth`, and each halving stops at `resize_floor` (512, the GUI's **Size floor**); height maps under `half` go down to 256, so under `cs-compact` and `vanilla-compact` a 512 height map with no diffuse in its set shrinks to 256 (one beside a diffuse is measured from it, see `resize_relative_to`).
- BC3 alpha, BC4 and BC5 use a better block search (cutout alpha about +3 dB); existing BC3, BC4 and BC5 outputs re-encode once. BC4 and BC5 take about 1.7 to 1.9 times as long and BC3 under 10 % longer, the `fast` presets included.
- The BC3 alpha guard uses each texture's alpha-test value (`alpha.threshold`, `alpha.compare` or its meshes') instead of 128; at the default threshold the output is unchanged.
- New output format `rgb8` (24-bit B8G8R8 with a legacy header, as vanilla tint masks) refuses kept alpha (`FMT-RGB8-ALPHA`), `_SRGB` and `header = "dx10"`; existing outputs do not change.
- DDS inputs also read 24-bit R8G8B8, A8L8 and L16 (from the high byte), and the 3D view loads them as body textures; A8L8 and L16 are never written.
- BC1 and RGB8 are accepted on a type with `data` alpha when that alpha is empty (none, or 255 everywhere). Any other `data` alpha still raises `FMT-BC1-ALPHA` or `FMT-RGB8-ALPHA`, and a `complex_material` stays refused for BC1.
- Alpha counts as opaque when only stray texels are below 251 (under 0.1 % of the image, no connected group over 16). Such a texture no longer asks about its alpha or raises `LOSS-ALPHA-DATA`, and one blocked or asked only because of them converts with its alpha removed by default (a complex `_m` becomes an env mask with parallax off); an explicit `alpha.policy` is honoured as before. Bytes for files that were not affected do not change, cached image analysis is recomputed once, and `sdc validate` reads a stored DDS the same way.
- `channels.grayscale_source` also takes `g`, `b` and `a` (it had `r` and `luma`); the default stays `r` and no existing output changes. `CHAN-NOT-GRAY` names the pick, and for `g`, `b` or `a` fires only when that channel is flat. `helper.read` on a raw map still picks for that map alone.
- New sub-type `gray` in the type picker's Grayscale maps group: a plain grey map written as BC4 (the default) or R8, that you pick on the file.
- Krita and OpenRaster files (`.kra`, `.ora`) are not read: they are reported as `IN-DECODE`. Export the image as PNG from the editor.
- Mip filter: Tent is allowed on `cutout`, `blend` and `cutout_blend` colour and weights each tap by the source alpha, so transparent colour does not bleed into the opaque edge; alpha stays the area-exact box. Box is the default and its output does not change; a texture that set `mips.filter = "tent"` on such colour used box in 1.x and now writes the tent.
- Packed, derived and alpha-fill sources of different sizes but one aspect ratio are enlarged to the largest (`PACK-SOURCE-RESIZED`) instead of refused; another ratio still fails (`PACK-SIZE-MISMATCH`). No existing output changes.
- `vanilla-quality` (and `vanilla-compact`, which inherits it) encodes `diffuse` and `glow` perceptually in BC1, BC3 and BC7, so those outputs re-encode once; where the alpha is `data` the preset's value drops out instead of raising `FMT-PERCEPTUAL-DATA`. `cs-quality`, `cs-compact` and `fast-preview` are unchanged. `VAL-MIP-ERROR` scores a perceptual BC1, BC3 or BC7 texture by Rec.709 luma PSNR, and `sdc presets show` and the preset editor show a BC6H texture's effective effort.

### GUI
- Meshes view: one flat list of NIF files titled **Meshes**, with a search over path or mod, an **Any mod** picker and chips (Wins, All, Overridden, Conflicted, Problems). Mod chips sit on the rows. **Show in Meshes** from a texture's Used by meshes adds a "Uses <texture>" chip.
  - Upgrading from 1.1: the scan database is emptied once; scan again in the Meshes view.
- Review only holds what blocks or needs an answer: blockers, undecided textures and the Keep or Overwrite questions.
  - Its Warnings column (the ticks and **Acknowledge all**), the half-size question and the **Stale outputs** list with **Delete** are gone. Tessalith no longer offers to delete old outputs; they stay in the export folder.
  - Export pitfalls are ordinary Warning rows in Problems and never hold back Export.
  - Review opens at 40 % of the window height, waits for overwrite answers and offers hand remedies as row buttons. Enter never saves, deletes or exports; Esc only closes.
- Removed from the Meshes view and the 3D preview:
  - **Plugin file references…** (the Missing and Unused window) and the set editor's **Used by plugins**;
  - **Show inert**: **Plugin overrides** lists every winning record that uses the mesh;
  - the **Stale patch** chip, the "PGPatcher output is out of date" banner with **Show list**, and the "PGPatcher check" line;
  - **Scan without PGPatcher output**: with PGPatcher running, the banner says to close it and press **Retry**;
  - the mod-level **Generated** chip, and the shape card's Details lines other than which mod each texture comes from;
  - the **UV checker** debug view;
  - the 3D frame-time footer line and the "CS settings: N fields unset" line;
  - the preview timing log (`SDC_PREVIEW_LOG`), the Details **Timings** section and the scan's per-step timings and "N ms"; "Scanned in N s" stays;
  - `set_preview_max` in `gui.toml` (ignored if present).
- Meshes view, shape cards:
  - a **Texture set** dropdown previews the alternate texture sets that plugin records assign to the shape, in the 3D view only;
  - an **Advanced Skin preview** card on Face Tint and Skin Tint shapes picks the RFAOS and wetness maps and a **Wet amount** from 0 to 1, for the preview only.
- NIF edits in the worklist: NIF warnings are worklist rows, **Export NIF** is a Review row with **Diff…**, and NIF edits need an export folder you chose.
  - An empty RMAOS slot is an amber row, and cancelling Review writes no NIF edits.
  - Upgrading from 1.1: NIF edits made without an export folder are in `mods\sdc NIF Edits`. Choose that folder in **Export to ▾** to reopen them.
- Header and editor:
  - **Export** stays neutral until the project is clean, and there is no inherited export folder;
  - **Mode** moved to the **Presets** tab (This project), **Export to** reads "not chosen" until a folder is chosen (**More ▾** only with no project open);
  - Review's button is **Choose the export folder…**, and the header's Review/Export text counts NIF edits, also with no folder open;
  - the set editor is one scrolling column, with helpers in a collapsible group;
  - Presets and the TruePBR JSON editor are centre-column tabs with **Pop out**, and a built-in preset offers only Duplicate;
  - one type picker, Problems rows that accept their suggestion in one click, diagnostic codes on the hover and in Messages, and plainer labels.
- Renamed: *Pin this slot (always use this file)*, *Keep the original texture* / *Let PGPatcher replace the texture*, the **PGPatcher JSON** editor (was TruePBR JSON) and its **+ Add a delete section**.
- GUI settings removed (their `gui.toml` keys are ignored and dropped at the next save):
  - **Worker threads** and **Memory budget** (Advanced → Batch): the GUI runs the engine with its defaults, and the copied CLI command no longer carries `--jobs` (`sdc convert --jobs` and `--mem-budget` stay);
  - **Largest previewed side** and **Allow 16384 previews**: previews go up to 4096 pixels on a side, or the GPU's limit when that is lower;
  - **Reduce motion**, with all the motion it governed: toasts, progress bars, fades, flashes and the scroll to a moved set show their end state at once.
- The **Measure whether full size adds anything** switch in Advanced is gone and Export never asks about half size; **Low detail** warns in Problems. An old `[checks] drop_top_mip` line in `gui.toml` is ignored.
- Preview: **Pin this result**, the Compare controls, the `P` key and the **Split** layout are removed, with the **Side by side** button and the `preview_layout` setting; the preview shows the source beside the encode. An old `gui.toml` with `preview_layout` loads, and the key drops on the next save.
- The alpha question is one **Alpha type** dropdown set to the proposed type, with one Problems line; the `Decide…` menu is gone and Enter on the row confirms the type shown. Choices are written as before.
- The 3D preview's studio HDRI is Poly Haven's 1k file, and its reflection cube is GGX-prefiltered as Community Shaders does it, so rough surfaces don't show sharp softbox streaks, and every material's studio reflection changes.
- The 3D **In game** preview simulates PGPatcher 2.3.1: a JSON that would crash the game stops the run, and `disabled` in a complex material JSON is honoured.
- Unsaved edits survive a crash: the next open restores them, with a toast that can undo it. After a lost graphics device the app restarts.
- A DDS input can be re-encoded (**Re-encode this file**, or **Re-encode *n* DDS files** in the bulk panel) and shows its stored image; an all-DDS set is a set.
- The input name box edits the whole path by segment (double-click, Ctrl+Left/Right, Ctrl+Up/Down lists); the path lock rail is gone.
- The command palette's Enter acts only after you type or move the highlight, and automatic types alone no longer ask on close.
- In the Meshes view with a NIF selected, the bottom panel leaves room for the 3D preview.
- The TruePBR editors list a feature's group (Coat, Emissive, Glint, Fuzz) only while the feature is on, and mark values of an off feature "kept, not used". Nothing in the JSON changes.
- A PGPatcher feature clash in Problems offers **Turn off X** and **Turn off Y**, where 1.x offered one button for the later feature.
- A `DERIVE-NIF-SUGGEST` row in Problems has no **Show in Messages** button: its values are on the row's hover.
- The GUI's JSON Lines export report leaves out the per-mip progress events.
- Fixed in the texture panel and previews:
  - typing a value into a pack fill box is one edit (one undo step), applied when you leave the box;
  - the 2D and set previews update when a helper's Invert or Read, a bake, a cube face or a derive source changes;
  - dragging the Alpha threshold slider on a shared alpha property keeps the split;
  - an edit on a large project no longer blocks the texture panel for a whole re-plan: on 10,000 textures it takes the next edit after 0.13 s (was 10.4 s);
  - the scan-database read after a re-plan no longer greys the panel;
  - the tile of a stored HDR DDS (BC6H, RGBA16F, RGBA32F) is tone-mapped like the preview, and the preview no longer turns a negative value bright;
  - Up and Down move through the helper rows under the tiles, and a re-plan of unchanged files no longer reloads each target an alpha helper fills.
- Fixed elsewhere in the GUI:
  - a greyed-out set control says why it is read-only (the `sdc-project.toml` error);
  - NIF validate reads non-ASCII texture paths and BSX names as Windows-1252, the same as apply;
  - renaming a large selection no longer slows down quadratically;
  - the Sets column: Tab and screen readers reach a card scrolled out of view, card heights follow a new width at once, and a far Go to set lands the set at the top;
  - the Sets column's first card in view stays put while the heights above it are learned;
  - the 3D settings window keeps its saved size.

### CLI
- Removed from `sdc`: `presets diff`, `presets export`, `presets validate`, `convert --fill` (use `--set packing.fill.<role>=<value>`) and `encoders` (`sdc --version` prints the encoder versions after the engine version).
- In `--json` output a `set_resolved` event's pack jobs lose `hand_placed`, and every packed source's `fill` names its channels (`ch:<rgba>`); the event schema version stays 2.

### Common
- XPMF profiles: a Landscape or PBR Landscape set can write `SKSE\Plugins\XPMF\tessalith_<set>.json` on Export, from an **XPMF profile** card in the set editor. Landscape sets get two optional maps, **Projected noise** (`_noise`, written as BC4) and **Detail normal**, that the profile points at, and the 3D preview's Material object layer shows the profile's textures and coverage. An Also export with Vanilla ticked writes those two maps and a `pbr = false` copy of the profile into the Vanilla folder too. See docs/wiki/XPMF-Profiles.md.
  - Upgrading from 1.1: in a landscape set, `_detail` and `_detailnorm` are the set's detail normal and no longer blend into `_n`; a landscape set's `_noise` is written as BC4 with no action by you. A set that had a `_detail` or `_noise` file now writes those outputs, and its `_n` changes (the detail blend is gone).
  - Upgrading from 1.1: `noise` is a file suffix, so every `*_noise.png` joins the set of its base name (`perlin_noise.png` joins set `perlin`; a `[sets."../perlin_noise"]` entry no longer matches). A user preset suffix alias named `noise` is refused on load (`CFG-PRESET-SKIPPED`; `--preset-file` exits 2).
- BC1 can keep 1-bit alpha: turn on `encoder.bc1_alpha` (GUI: **1-bit alpha** in the Alpha group, BC1 only) for a `cutout`, `blend` or `cutout_blend` texture. Texels below the alpha-test cutoff become transparent black, and a new warning, `ALPHA-BC1-ONE-BIT`, flags sources with soft alpha. See docs/wiki/Texture-Types.md#alpha-policies.
  - Upgrading from 1.1: `encoder.bc1_transparent_black` is removed; a config or preset that still has the line, even `= false`, is an error that names `encoder.bc1_alpha`. Delete the line.
- NVTT (the optional BC7/BC6H encoder, not bundled) removed: `encoder.backend = "nvtt"` and `--nvtt` now fail (as does an allow list naming `ENC-NVTT-MISSING`, `ENC-NVTT-VERSION` or `ENC-NVTT-FAILED`); re-encode with the built-in encoders. The GUI loses the NVTT section in Advanced, the Encoder row in the texture panel and the Textures column's Encoder filter, and `sdc encoders` stops looking for NVTT. `VAL-BC6H-PEAK` advice is now only `encoder.effort = "slowest"`.
- The format alone picks the encoder: `encoder.backend` and `encoder.rgbcx_level` are removed (BC1 and BC3 colour always use rgbcx level 18, the value every built-in preset set), and a config, preset or `--set` that names either is an error that says so. The GUI loses the rgbcx level row in Advanced. Output bytes do not change.
- Diagnostic codes removed; an allow list naming one is an error that says so:
  - `IN-FLOAT-LDR`, which was never emitted;
  - fifteen info and low-value warning codes that gave the user nothing to act on: `LOSS-BITDEPTH`, `CHAN-CONSTANT-ALPHA-DATA`, `CHAN-NORMAL-SHORT`, `ALPHA-254`, `ALPHA-NEAR-OPAQUE`, `ALPHA-PBR-NORMAL-DROPPED`, `IN-ICC-IGNORED`, `SKIN-WET-FORM`, `PACK-ORM-OVERRIDDEN`, `RNM-AO-MISSING`, `DERIVE-NO-SCAN`, `DERIVE-NO-ENVMAP`, `CHAN-AO-INVERTED` and `CHAN-GLOSS-AS-ROUGH` and `SET-COMPANION-OUTSIDE`. What each reported is no longer reported; decisions and output bytes do not change.
- The re-encode report is removed: `sdc convert` no longer prints what the new version re-encodes, and the GUI's Review no longer lists it. A record written by 1.x is still stale and re-encodes. Cached image analysis is recomputed once.
- CLI and GUI extras removed:
  - `.dds.json` sidecars: `sdc convert --sidecar` and the `OUT-SIDECAR-KEPT` code are gone, and Tessalith neither writes nor deletes `.dds.json` files. One left by 1.x stays on disk; delete it by hand.
  - `sdc patch-header`, the `DDS-HEADER-PATCHED` and `DDS-HEADER-UNPATCHABLE` codes, and the GUI's header check, **Tag on patch** and **Patch header…**. Re-encode the file from its source image instead.
  - An allow list naming `OUT-SIDECAR-KEPT`, `DDS-HEADER-PATCHED` or `DDS-HEADER-UNPATCHABLE` is an error that says so.
- `rfaos` is no longer read as a sub-type: use `skin_rfaos`. A key or value that 2.0 removed is an error that names every such key (ten, then a count) and its fix, in `sdc.toml`, presets and `--set`; the GUI opens such a file read-only. A user preset skipped for one is a Problems row with the reason, and a project that names it says it was skipped instead of "not installed".
- The five `validation.*` thresholds (`min_mip_psnr_db`, `coverage_tolerance_pts`, `complex_m_warn_alpha`, `bc6h_min_peak_ratio`, `pgpatcher_non_white_share`) are fixed at the values every preset used; naming one is an error. `validation.mips` stays. Output bytes do not change.
  - The project-wide `[defaults] packing.fill` is removed the same way and its **Clear** line leaves the Advanced drawer: delete the `[defaults]` fill lines by hand and set fills per set on the packed output.
- A preset or `sdc.toml` that sets `subtype`, `target`, `category` or `resize_reference` in `[defaults]` or `[subtypes.X]` is now a load error that names the table and the key and says it has no effect there. Until now the three profile keys were ignored and `resize_reference` skewed the size checks. Delete the line.
- The Also export writes no zip, no `SHA256SUMS.txt` and no `conversion-report.md`. Output bytes do not change.
  - `[export] zip` is removed (a config, preset or `--set` that names it is an error that says so), and so are the warning codes `ALSO-ZIP-FAILED`, `ALSO-REPORT-FAILED` and `ALSO-OWNED-UNREADABLE` (an allow list naming one is an error).
  - The GUI loses the Review checkbox **Also write a zip of each folder** and the Problems row **Reset also.json**.
  - An Also export keeps no `_sdc/also.json`; one a 1.x export left is never read, rewritten or deleted. Files an earlier export left in the three folders are still listed in the export notes as "not written by this export".
  - After an Also export the command line prints those files, and the PBR sets left out of Vanilla and CM, as plain lines on stderr.
- `encoder.perceptual` also applies to BC1 and BC3 colour (Rec.709 luma); it stays off by default and `FMT-PERCEPTUAL-DATA` still refuses it on data maps. The GUI shows a **Perceptual** tick in Compression for BC1 and BC3, and the preset editor one for BC1, BC3 and BC7.
- New setting `resize_relative_to` (GUI: **Relative to**, next to Resize): `slot1` to `slot8` measure `resize` from the source size of the set's map in that texture-set slot instead of the texture's own size, and never enlarge. `cs-compact` and `vanilla-compact` use `slot1` for every map but the diffuse, so a normal or mask already exported smaller than its diffuse is no longer halved again. A map whose set has none or several maps in that slot is resized from its own size and gets the info code `RESIZE-NO-REFERENCE`.
- Automatic decisions (the ones Tessalith makes without asking) are recorded in `sdc.toml`, so the GUI and CLI agree on them.
- The **Value range** and **Plastic look** artist checks are removed (`CHK-RANGE`, `CHK-PLASTIC`), with the preview's **Range** overlay and their two switches in Advanced → Checks. An allow list naming either code is an error that says so, and an old `[checks]` line for either in `gui.toml` is ignored.
- The bc7e objects must carry both sdc patches: a tree whose `bc7e_ispc.h` lacks either marker no longer builds, so the mode-6 byte-patch fallback for `alpha.exact_opaque` and the `CFG-INVALID` refusal of `encoder.effort = "max"` are gone.
- The host plugins (chaiNNer, GIMP, Paint.NET, Photoshop) moved to branch `host-plugins` (bug fixes only, not kept in parity with the engine), with their pages. `main` no longer builds them.
- The measurement tool `sdc-bench` and the raw BC7 tuning data (CSV, TSV) moved from `main` to a local branch `bench`; the BC4 reference test lives in `encoders-sys` and `engine`.
- `docs/research/` is organised by format, with `docs/research/README.md` as the index, and the Known issues page is a section of Troubleshooting.

## 1.1.0

### Main
- Cubemaps: a new `cubemap` texture type, with a Cubemap section in the GUI and a new `sdc cubemap-marker`.
- Helper textures fill a channel of another output of their set, so some existing outputs change on the next export.

### Engine
- Cubemaps: a new `cubemap` texture type for `_e` environment maps and files under `textures\cubemaps\`. It reads six face files (`_posx` to `_negz`), crosses and strips, and existing cube DDS files, and writes a cube DDS with a full mip chain. See docs/wiki/Cubemaps.md.
  - `cubemap.*` settings in `sdc.toml` give four styles (`vanilla_metal`, `room`, `eye`, `hd`), face turns and flips, yaw and mirror, blur and colour adjustments. Colour uses Community Shaders' game curve (`pow(c, 1.6)`), not sRGB.
  - Files under `textures\cubemaps\` that 1.0.0 wrote as 2D are cubemaps: one shaped like a cross or strip is written as a cube, and any other shape (a square one, say) is skipped as unclear until you set `cubemap.layout`. Other texture types are unchanged.
- A set whose shader is Advanced Skin (saved in the GUI project, or `--set set:<id>.shader=advanced_skin`) is an Advanced skin set: its diffuse (inferred, or written as plain `diffuse`) becomes `skin_albedo`, and it packs `_rfaos` instead of `_rmaos` when it can build one. Existing Advanced Skin projects switch `_rmaos` to `_rfaos` on the next export; the old `_rmaos.dds` stays in the export folder until you delete it.
- `--on-existing if-changed` re-encodes only the texture types whose output changed in a new version. Upgrading from 1.0.0 re-encodes nothing.

### GUI
- Cubemaps in the GUI: a Cubemap section in the settings panel, a six-face cross preview, and Export to > Make marker cube... Settings are written to `sdc.toml`, so the GUI and CLI make the same bytes.
- The GUI keeps its image analyses in `%LOCALAPPDATA%\sdc\analysis-cache` (pruned to 200 MB at start, safe to delete), so reopening a folder reuses the analyses of unchanged files.

### CLI
- New `sdc cubemap-marker` writes the 1×1 dynamic-cubemap marker.
- New codes `CUBE-FACES` and `CUBE-NO-MIPS`. `sdc inspect` and `sdc validate` flag cubes wider than 1 pixel with one mip, a lit 1×1 marker, an sRGB cube, and 2D files named `_e` or under `textures\cubemaps\`. **`sdc validate` now fails those 2D files**, because the game reads them as cubes.

### Common
- Helper textures: raw maps and masks are *helpers* that fill a channel of another output of their set. An `_opacity` map fills the colour texture's alpha, and a `_specular` map fills the alpha of a non-PBR `_n` as well as its packed channel when the set packs one. See docs/wiki/Texture-Types.md#helper-textures.
  - You can set a helper's role (or make it *Not a helper* to convert it alone), the channels it fills (including the `_cnr` coat roughness, `_f` fuzz weight and `_s` subsurface weight alphas), the channel it is read from and Invert.
  - Set them in the GUI on the helper's panel and on a packed card's new Source picker, in `sdc.toml` with `helper.role`, `helper.fills`, `helper.read` and `helper.invert` on the helper file's `[files]` entry, and with `--set file:<glob>.helper.<key>=`.
  - In the GUI every helper is a row in a new Helpers group under the tiles (pill on by default).
- **Existing projects: some outputs change on the next export.** A set with a helper that 1.0.0 ignored, beside a target whose alpha is empty (none, fully opaque or flat), now fills that alpha: a colour texture beside an `_opacity` file, and a non-PBR `_n` beside a `_specular` file. That output gains an alpha and may change format (a BC1 diffuse moves to an alpha format). A target with a real alpha keeps it. `--on-existing if-changed` re-exports only those outputs (the helper is now recorded as a source of the output); nothing else re-encodes. To keep the old output, set `helper.role = "none"` on the helper (it is then converted on its own) or `helper.fills = []` (it is then not used).

## 1.0.0

### Main
- First official release: it covers everything since 0.8.0-alpha.2 (0.9.0 was never released).
- Edit NIF files on the Meshes page: shader and material fields, extra data, alpha properties and animation keys, written with **Export NIF**.
- New **Also export as Vanilla / Complex Material**, **Artist checks** and **BC7 weighting**.
- Every recorded output re-encodes once on the first `if-changed` run after you upgrade.

### Engine
- **Upgrading: one re-encode.** The version change makes every `--on-existing if-changed` run re-encode each recorded output once after you upgrade. The bump itself changes no pixels and no presets.
- Derived outputs (Also export) no longer re-encode when you move a project folder. The first `if-changed` run after upgrading re-encodes them once.
- Fixed in the engine:
  - followed links: recursive discovery follows folder links, and a link loop or a dead link is reported instead of hanging or being read back as input;
  - a NaN alpha pixel no longer turns an opaque float image almost opaque, and a corrupt PSD with no pixel data is refused instead of crashing;
  - the derive env-map gate finds the colour map by its own key (`_d` kept).

### GUI
- Meshes page: edit NIF files in the GUI. Change shader and material fields, edit extra data, add and remove alpha properties, and see the result in the 3D preview. **Export NIF** writes the edited mesh as a loose file into your Textures export folder (or an `sdc NIF Edits` mod in MO2). See docs/gui.md.
- Meshes page redesign:
  - shape cards update as you edit, and gain a Clips section, heel height, a known-extra-data list and controllers;
  - **Export NIF** is a footer that stays in view and names its work ("Export NIF · 3 edits", "Export NIF again"); Enter or Space on it never exports or overwrites.
- Animation: a clock with play, pause and scrub, a key editor with colour pickers, and controller editing (add, remove, retarget).
- The In game preview animates UV scroll, emissive, glossiness, specular, alpha and alpha threshold the way the game does.
- Export NIF drops the orphan blocks that your edits left behind; a mesh with no orphans exports byte-identical to before.
- Shader-flag checks: Skin Tint and FaceGen detail need their shader type, and a mesh that would crash the game or render black is an error that blocks export.
- Edit history: undo, redo, revert and Keep mine / Take source behave the same everywhere, and a refusal message goes away once it no longer applies.
- The 3D preview matches Community Shaders' deferred path more closely: **Accurate (deferred)** in the *View* menu, on by default. Missing textures show a fill instead of white.
- **Artist checks**: advice for banding, very dark or bright areas, normal-map curl and stepped height or metalness maps, under **Checks**; they never block an export.
- Fixed in the mod scan and Meshes:
  - MO2 files are read one line at a time, so a byte-order mark or a non-UTF-8 mod name no longer breaks the scan;
  - the loose-file walk skips only the actor facegen folder, so other folders are no longer missed;
  - a scan that is stopped, or one started while a mesh read is in flight, no longer leaves "Used by meshes" empty;
  - the Meshes path filter folds ASCII case only, and the NIF edit source ignores path-separator spelling (`/` or `\`);
  - a new 3D load no longer waits behind a replaced load's queued compiles.
- Fixed in renaming and the JSON editor:
  - Rename Inputs refuses two rows that would write the same output, and the JSON rename box refuses device and map-suffix names;
  - Fill from names checks slot clashes inside the batch, and a file dropped from Explorer onto a slot card gets the same slot 3 and 7 check as a drag inside the app;
  - the JSON editor's pending match pills stay visible, follow a mode change and shift when you remove a row, and Shift+F8 goes to the previous error.
- Fixed in keyboard shortcuts:
  - Ctrl+Shift+Z with nothing to redo no longer undoes, and Shift+Tab in the input-name box no longer takes the ghost suggestion;
  - a disabled shortcut yields to an enabled one on the same keys.
- Fixed elsewhere in the GUI:
  - moving a file back to its set, or a set that shares an extension-less key, splits and restores correctly;
  - the Also gate with no project open says to open a folder;
  - the presets' Delete confirmation is per preset;
  - Encode now pressed during an export runs when the export ends;
  - a non-Unicode path is left out of `gui.toml` instead of failing the settings save;
  - an edit that changes no text leaves the project clean, and a `[files]` lookup also finds a key without an extension.

### Common
- First official release. It covers everything since 0.8.0-alpha.2 (0.9.0 was never released).
- **Also export as Vanilla / Complex Material**: from a PBR set, write a vanilla or Complex Material version beside the PBR one. `_curves`, `_curvature` and `_cavity` files join their set as packing inputs instead of making their own set, and write nothing by themselves.
- **BC7 weighting** dropdown replaces the Perceptual check box: Linear (default), Perceptual, Alpha ×2, ×8 or ×16, for the built-in BC7 encoder only; other formats warn that it is ignored. The `encoder.bc7_weights` setting is also available from `--set`, `[files]` and rules. Output with the default setting is unchanged.
- A quoted dotted `[files]` key is a config error that names the field; before, the value was silently dropped.

## 0.8.0-alpha.2

### Main
- GUI layout v2: a rebuilt header, one dialog frame, a split-out Review tab and a Sets column with a filter.
- New bc7e quality level `encoder.effort = "max"`, off by default.

### Engine
- New bc7e quality level `encoder.effort = "max"` (sdc patch 0002, a wider search on top of `veryslow`), off by default, so the default output is unchanged.

### GUI
- GUI layout v2. The header is rebuilt, and dialogs share one frame with a fixed button order. The export Review tab is split out, the Sets column has a filter, and the Presets window and the 3D settings window have moved. See docs/gui.md.
- Set cards show their position number. **Move to…** in a set's menu takes a position or a set name; while a filter is on, it is the only way to reorder.
- The Sets column draws only the cards on screen, so it stays fast with hundreds of sets; Tab and screen readers reach only those, Ctrl+K and Move to… every set.
- Motion: toasts rise and fade, Export fades to the accent colour, progress bars glide, dragged textures show a ghost, and reordered sets and changed slots flash.
- Toasts stay while the pointer is on them, the columns fade in when a folder opens, and Ctrl+K, F1 and path autocomplete open at once.
- Rename dialog: a **Whole path** toggle (Inputs) runs Find, Replace, Prefix and Suffix over the folders too; the clipped *New* column is gone and *Output* shows the result.
- Fewer per-frame costs in the GUI; background-only reads (output sizes, *used by*, report save) may appear up to 250 ms later.

## 0.8.0-alpha.1 (first public alpha)

### Main
- Works out what each file is, packs loose maps into Skyrim's layouts, builds the mips and checks every output against Skyrim's rules.
- Upgrading from 0.7: the `cpm` target is gone, and some defaults change.
- This is an alpha: back up your mod folders first.

### Engine
- Works out what each file is from its name and folder, groups files into material sets, packs loose maps into Skyrim's layouts (`_rmaos`, `_rfaos`, complex `_m`), builds the mips and picks the BC format and the `_SRGB`/UNORM tag.
- Targets `cs` (Community Shaders with TruePBR, the default) and `vanilla` (which also packs the complex-material `_m` and complex grass).
- Inputs: PNG, TGA, TIFF, EXR, PSD/PSB. Output: BC1, BC3, BC4, BC5, BC6H, BC7.
- Coverage-preserving mips for cutout alpha, renormalised normal mips, normal-mip variance baked into TruePBR roughness.
- TruePBR: PGPatcher JSON written per set. Complex grass. Mod scan of MO2 meshes and plugins to decide each diffuse's alpha meaning and shader.

### GUI
- GUI with encoded-mip preview, pixel readout and a 3D preview.

### Common
- Tessalith converts a folder of source images to DDS for Skyrim SE/AE. The GUI (`tessalith.exe`) and the CLI (`sdc.exe`) share one engine and one per-project `sdc.toml`. This is an alpha: back up your mod folders first.
- Every output is decoded and checked against Skyrim's rules; `sdc validate` checks DDS files from other tools.

### Changes that matter when upgrading from 0.7

Breaking:

- **The `cpm` target is gone.** `cpm` in `--target`, `sdc.toml`, the user config or a preset is an error (exit 2), `--preset cpm-quality` no longer exists, and a user preset with `target = "cpm"` is skipped with `CFG-PRESET-SKIPPED`. Use `vanilla` with the set's shader on complex material instead.
- **Vanilla gains complex grass and complex material.** Under `vanilla` a set packs the complex `_m` only when its shader is complex material; every other vanilla set keeps the env-mask `_m`, so existing vanilla output is unchanged.
- **Plain `_normal`, `_nrm` and `_norm` default to DirectX green.** They no longer block or ask; an info line (`IN-GREEN-UNSTATED`) says DirectX was assumed. `_normalgl` still flips. For an OpenGL map set `channels.green_convention = "opengl"` or use the GUI's *Treat as OpenGL* button. JSON event output no longer has `needs: ["green_convention"]`.
- **Hair diffuse defaults to cutout + blend** (was cutout), with coverage mips on. Mesh evidence still wins.
- **A complex `_m` with no source alpha now writes alpha 0** (was 255, which Community Shaders reads as a plain env mask).
- **One PGPatcher match key per entry.** A renamed set writes `match_diffuse` + `rename` only, with no `matchN`.
- **Rules layer.** Every matching rule of a source (each preset, the user config, the project config) applies in file order, field by field, where before only the first matching rule of each source applied.
- **Skin sets write no PGPatcher JSON** (Advanced skin).
- **Re-encode once.** The version bump re-encodes each recorded output once under `--on-existing if-changed`. Pixel output and presets are otherwise unchanged.

### Known issues

See [docs/wiki/Troubleshooting.md](docs/wiki/Troubleshooting.md#known-issues).
