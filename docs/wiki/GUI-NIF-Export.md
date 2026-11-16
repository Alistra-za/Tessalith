# Exporting an edited NIF

For mod authors with an edited NIF: how the export writes it, what happens when a file is already in the way, and how PGPatcher's copy and the **In game** view relate to your edit. Editing itself is on [Editing a NIF](GUI-NIF-Edit.md).

## Export

The bottom of the edit panel stays under the tabs however long a tab is. Its first line is a status: a green dot and **Ready**, a grey dot and "No edits to export." while nothing is edited, a grey dot and "Skipped until an export folder is chosen" once there are edits and no folder (Review lists the same), or a red dot and **Blocked:** with the reason: an edit cannot be applied, conflicts are open, or an edit introduced an error. When a pending row is red, **Show** opens the pending list. When a file Tessalith did not write is already at the export path, **Diff…** opens a list of the blocks that differ between that file and what the export writes, read at the click; **Close** shuts it. Then "Writes meshes\…\x.nif to <export folder>\meshes\…\x.nif" (hover for the full path) and "The source file is never changed. Change the folder with Export to ▾ in the header."

**Review…** opens Review in the bottom panel: NIF edits export from there, with the textures, when you press Review's Export. <!-- D:GD2 --> The button itself never writes. In Review each NIF with edits is a row: its mod, how many unused blocks the export leaves out, and the result after the export (written, or skipped and why). A blocked NIF row is skipped and never blocks the texture export. Each write puts "Exported <path> to <mod>." in Messages, with a placement line when the mod order is wrong: when the export folder is a mod folder in the MO2 instance's `mods`, the export checks the mod order and says whether that mod must be enabled, or dragged below the highest source or above the lowest PGPatcher output; any other folder says "The export folder is not an MO2 mod here: load order not checked." and still exports. With no folder open (no project), Review still exports NIF edits: its button reads "Export 1 NIF edit", it needs an export folder (**Choose the export folder…**; the `gui.toml` one), writes the NIFs there and starts no texture run.

### Export this NIF (right-click)

While an edit is open, a right-click on the 3D view (a click, not a drag) opens a menu with **Export this NIF**. It writes the open NIF to the export folder at once, with no Review and no texture run, and reports as Review's export does (Messages, a toast, the per-file lines). It is greyed with the reason on hover: "No edits to export.", the NIF's own blocked reason, "An export is running", or "No export folder chosen", in which case **Choose export folder…** appears under it and opens the header's picker. A file Tessalith did not write at the destination is never overwritten this way: the click opens Review on that row, and once you have answered Overwrite there the next **Export this NIF** writes. Right-drag still orbits and Shift+right-drag still turns the sun; the set preview's 3D view has no menu.

### Changing the export folder

Open edits stay in memory and the next save goes to the new folder. If that folder already holds saved edits for the NIF, your open edit wins: the old saved edits are moved aside to `<name>.<seconds>.toml` beside them, never overwritten, and Messages names the file. If the new folder's edit document can't be opened, the edit stays blocked and saves go to the autosave in `%LOCALAPPDATA%\sdc\nif-edits` until you pick a folder that works. Once the NIF is edited, a folder that holds the mesh the edit applies to is refused, so an export never replaces its source: the folder is not taken, and the NIF's row is blocked with the reason until you pick another one. <!-- D:HS12 -->

### After an export

After an export, the worklist has a Warning row "Re-run PGPatcher so its output includes this edit." when the NIF has a PGPatcher copy. The exported file leaves out the blocks your edits added that nothing uses any more (a removed controller), so it can have fewer blocks than the edit shows. When that cleanup cannot read the file, an Info row says "Exports without cleanup: the file has a block Tessalith does not understand", and the export writes the file as edited.

### A file Tessalith did not write

If the file at the destination is not the one Tessalith last wrote, its Review row waits for an answer, keep the file on disk or overwrite it, with neither picked. Until you answer, the export skips that NIF. An answer counts for one export, and only while the file on disk is the one you answered for: a file that changed since says "changed on disk since you answered" and asks again. The row's **Diff…** switches to the Meshes view and opens the edit panel's list of the blocks that differ.

## PGPatcher and the game

The toggle reads **In game** and **Source** with or without an edit open; while editing, In game is simulated and Source is your edited NIF, as their hovers say ([below](#in-game-and-source-while-editing)). Once you have exported, the amber line "PGPatcher output predates your edit: re-run PGPatcher." shows: the game still loads PGPatcher's copy, which was made from the unedited NIF, until you re-run PGPatcher. If the edit preview cannot be shown, the line "Your edit could not be shown. Switch to In game or Source to retry." says so; edits you make meanwhile still count.

### In game and Source while editing

In game runs Tessalith's PGPatcher simulation on your edited NIF and shows still materials (the clock pauses). It follows PGPatcher 2.3.1: loose JSON only, no modrules priority, read at the last scan. The JSON is read in the background when the edit opens, and a folder scan has no MO2 instance, so In game says no JSON was read. PGPatcher's real output, made from the unedited NIF, is not shown while you edit. For the selected shape a line says what happened:

- No JSON matches it, it is already PBR, or a file is not applied (amber, with the reason; the next file can still match).
- PGPatcher skips it (`PG_IGNORE`, amber).
- The shape carries a `PG_STOCK` block (amber): PGPatcher reverts such a shape to its recorded stock state first, and the preview shows it as loaded.
- **IMPORTANT** A match key such as `match_diffuse` or `nif_filter` that isn't a string, in any JSON the instance loads, crashes PGPatcher 2.3: the run stops, so every shape shows one red line per such file, even shapes PGPatcher would leave alone. The set preview in the Textures view shows the same lines.
- PGPatcher makes the shape PBR while it has an Env map scale controller (red): that crashes the game.

The simulation leaves out:

- JSON inside BSAs, and PGPatcher's own mod priority (`modrules.json`; the MO2 order stands in);
- plugin alternate textures (TXST), and the plugin and mesh-blocklist rules that decide which meshes PGPatcher patches;
- complex-material and parallax competition, and shapes with no texture set;
- JSON changed on disk since it was read (rescan), and the `PG_STOCK` revert.
