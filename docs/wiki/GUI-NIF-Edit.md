# Editing a NIF

For mod authors tuning a mesh, the Meshes view lets you edit its shader, alpha and extra data and export a copy. For controllers and clip playback, see [NIF animation](GUI-NIF-Animation.md).

## Before you start

- Scan your mods first, so the Meshes view lists your meshes ([Mod scanning](Mod-Scanning.md)).
- To export the edited copy alongside your textures, choose a folder with **Export to ▾** in the header. You can edit before choosing an [export folder](GUI-Main-Window.md#export-to).

## Editing a NIF

Click the **Edit** button beside the selected NIF's name. Click it again to close the panel. If a rescan drops the row, **Edit** stays beside the name so you can retry the save and close. <!-- D:LD8 --> The panel shows the NIF's data path and source. In the mesh list, **Edit pending** marks edits to export, **Edited** marks an unchanged export and **Edit conflict** marks a conflict. Tessalith skips copies from an `sdc NIF Edits` folder or a folder that holds the export record for that NIF, so you edit the source mesh. <!-- D:GQ3 -->

**Cards.** In edit mode the shape cards get a **Root** card first, then one card per shape. Click a card's header to select it; the tabs below edit it, and a line under the tab strip names it (“Editing: BodyMesh”). Hover the name for its NIF block type. <!-- D:LD10 --> **Show only this shape** hides the other shapes in the 3D view. A change to the shader type, a flag, the alpha or a texture path shows on its card at once.

## Tabs

- **Shader** edits the selected shape's texture slots, shader type, **Texture wrap**, per-type fields and **Shader flags 1** and **Shader flags 2**. Hover the flag headers for their NifSkope names. Slots count from 1. True PBR uses the set editor's names for slots 3 and 7. A flag list with a warning starts open.
- **Alpha** edits the shape's alpha settings. A blend pair the engine mapping does not cover is drawn as stored and marked **Not verified in game**.
- **Controllers** lists the shape's controllers and edits their keys: [NIF animation](GUI-NIF-Animation.md#the-controllers-tab).
- **Extra data** lists the extra data of the root or the selected shape: BSXFlags bits, string, integer, boolean and float values, BSInvMarker. **Add to root**, **Add to shape** and **Remove** change the list. Each row shows its own warnings under it, for example "BSXFlags must be named BSX". A **Known** list above the type and name fills in an entry the game or a plugin reads, with its type, name and a starting value. On a shape: **AnisotropicAlphaMaterial** (Community Shaders see-through look; starts at 3, woven fabric). On the root: **BSX**, **INV**, **HDT Skinned Mesh Physics Object** (type the physics xml path in **Value**; Add stays off until you do) and **Prn (unconfirmed)** (the skeleton node the item attaches to; not checked against a source yet). An entry that goes on the other owner is greyed ("Goes on the root", "Goes on a shape"), and so is one this owner already has ("Already on this root"). **Custom…** keeps free typing for anything else. BODYTRI is not offered: BodySlide and Outfit Studio write it. **Heel height (RaceMenu)** (shapes only; on the root: "The game ignores heel height here. Put it on a shape.") raises the actor so heels stand on the ground. It starts at the shape's stored value, **Calculate** fills it from the lowest point of the shape in bind pose (rounded to 0.1, still editable), and **Set heel height** writes it: the shape's HH_OFFSET is changed in place or added, and an SDTA the shape already has gets the same height in its NPC entry (a new SDTA only when **Also write SDTA** is ticked). It says when an earlier shape's heel height wins, and for `_0`/`_1` files it reminds you to apply the same value to the other weight file. Heel data anywhere but on a shape, and on a shape after the first shape in the scene that has some, warns under its row. A negative height says it lowers the actor. When the shape's HH_OFFSET is not a number the editor can set (another type), Set heel height stays off until that row is removed.

## Shared blocks

The panel says when a block has more than one user.
- A shared alpha: "Shared by N shapes: editing will split." The first edit splits it, so only this shape changes. The Controllers tab splits it by a button: [NIF animation](GUI-NIF-Animation.md#the-controllers-tab).
- A shared texture set: "Shared by N shapes: editing makes a copy for this shape."
- A shared shader: "Shared by N shapes: shader edits change all of them." A texture path edit on a shared shader is refused, because it would change the other shapes.
- A shared interpolator: a value edit is a conflict ("This interpolator is shared by N controllers"), and the value row is greyed. A key edit copies it first: [NIF animation](GUI-NIF-Animation.md#the-controllers-tab).

## Warnings

On a PBR shape, the flags under **Shader flags 2** that CS reads differently (Fit_Slope, Multi_Layer_Parallax, Soft, Back, Rim and Effect lighting) get a dimmer "PBR: <meaning>" label, and the same text in their hover; a non-PBR shape shows only the vanilla names.

Values are never clamped. A value outside the range the game expects, or a flag combination that looks wrong, shows an amber warning under the field (red for an error), sometimes with a **Fix** button. A red error that your edit introduced blocks the NIF's export. An error that was already in the source only warns. Three flag warnings come from the corpus census: shader type 11 without the Multi_Layer_Parallax flag (Shader flags 2, bit 24) and without PBR; Shader flags 2, bit 24 on a shader type other than 11; Fit_Slope (Shader flags 2, bit 12) without PBR. On a True PBR shape an empty RMAOS slot is amber: "Slot 6 (RMAOS) is empty: in game the mesh renders fully metallic".

**Flags that need a shader type.** The skin tint flag (Shader flags 1, bit 21) works only on the Skin Tint type and the FaceGen detail flag (Shader flags 1, bit 10) only on Face Tint (which needs it) or Skin Tint: a mismatch is a red error that always blocks the NIF's export, even in an untouched file, with a **Clear the flag** or **Set the flag** button; a type change fixes the flag in the same undo step and says so in a line under the type, and a flag that cannot apply to the current type is greyed with the reason on hover.

## Pending edits, undo and source changes

**Pending edits (N).** Every edit is a row in this list under the file name, named in words ("BodyMesh: Shader flags 1"). A slider drag is one row. **Revert** removes that edit. A row in red did not apply, with the reason.

**Undo.** With the NIF editor visible, **Undo NIF edit** and **Redo NIF edit** act on that NIF. Hover either for the step's full name. With no NIF edit open, or when a docked tool covers Meshes, the header, command palette and **Ctrl+Z** / **Ctrl+Y** use the shared project history. If your last project edit changed a texture, **Undo** in Settings reverses that texture edit. The status line names the step while the tool covers Meshes. <!-- D:JY13,JY15,JY20 -->

**Source changed.** If the source NIF changes on disk (a mod update), the edits are re-applied by name and role, not by position. The panel says "Source changed: N applied, M conflicts". A conflicting edit has **Take source** (drop your edit) and **Keep mine** (keep it on top of the new source).

## Export

The footer stays visible while you scroll the texture slots. <!-- D:LD6 --> Green means **Ready**. Grey reads "No edits to export." or "Skipped until an export folder is chosen". Red **Blocked:** explains an unapplied edit, open conflict or new error. **Show** opens the pending list for a red row. For a foreign file at the export path, **Diff…** compares its blocks with your edit at the click. **Close** shuts the list. The **Writes** line shows the NIF's data path and export destination. Hover it for both full paths and the reminder that the source file stays untouched.

**Review…** opens Review in the bottom panel: NIF edits export from there, with the textures, when you press Review's Export. <!-- D:GD2 --> The button itself never writes. In Review each NIF with edits is a row: its mod, how many unused blocks the export leaves out, and the result after the export (written, or skipped and why). A blocked NIF row is skipped and never blocks the texture export. Each write puts "Exported <path> to <mod>." in Messages, with a placement line when the mod order is wrong: when the export folder is a mod folder in the MO2 instance's `mods`, the export checks the mod order and says whether that mod must be enabled, or dragged below the highest source or above the lowest PGPatcher output; any other folder says "The export folder is not an MO2 mod here: load order not checked." and still exports. With no folder open (no project), Review still exports NIF edits: its button reads "Export 1 NIF edit", it needs an export folder (**Choose the export folder…**; the `gui.toml` one), writes the NIFs there and starts no texture run.

### Export this NIF (right-click)

While an edit is open, a right-click on the 3D view (a click, not a drag) opens a menu with **Export this NIF**. It writes the open NIF to the export folder at once, with no Review and no texture run, and reports as Review's export does (Messages, a toast, the per-file lines). It is greyed with the reason on hover: "No edits to export.", the NIF's own blocked reason, "An export is running", or "No export folder chosen", in which case **Choose export folder…** appears under it and opens the header's picker. A file Tessalith did not write at the destination is never overwritten this way: the click opens Review on that row, and once you have answered Overwrite there the next **Export this NIF** writes. Right-drag still orbits and Shift+right-drag still turns the sun; the set preview's 3D view has no menu.

### Where your edits are saved

Tessalith saves each change in `_sdc\nif-edits` inside your project folder. A slider drag saves when you release it. With no project open, the draft goes in `%LOCALAPPDATA%\sdc\nif-edits`. The export folder gets the edited NIF and its record when you export. <!-- D:LD1,LD2 -->

Look for **Draft saved in** at the bottom of the edit panel. **Show in Explorer** selects that draft in its folder. If the button is disabled, hover it for the reason. <!-- D:LD5 -->

When you switch or close projects, Tessalith saves any outstanding edits with their project and ends that edit session. The selected NIF can reopen with the edits for the project you switched to. If the project save fails, Tessalith tries to keep an autosave and lists its path in **Messages**. If the edits can't be recovered, the session stays open and the panel tells you why. Edits made without a project carry into the next project you open. If it has edits for the same mesh, the newer edits win. <!-- D:LD4,LD12 -->

If Tessalith finds an unexported draft in the export folder, your next change moves it to the current draft location. An exported record stays where it is. <!-- D:LD3 --> Choosing a different export folder leaves your draft in place. If that folder holds the source mesh itself, the row is blocked and tells you why. <!-- D:HS12 -->

### After an export

If the NIF was written but its record wasn’t saved, export again. If you undid the last edit, use **Redo NIF edit** first. If Redo is disabled, reapply the edit first. <!-- D:LD13 --> After an export, the worklist has a Warning row "Re-run PGPatcher so its output includes this edit." when the NIF has a PGPatcher copy. The exported file leaves out the blocks your edits added that nothing uses any more (a removed controller), so it can have fewer blocks than the edit shows. When that cleanup cannot read the file, an Info row says "Exports without cleanup: the file has a block Tessalith does not understand", and the export writes the file as edited.

### A file Tessalith did not write

If Tessalith has no record of the installed NIF, another project owns it, or its bytes differ from the recorded export, Review asks whether to keep the file or overwrite it. Until you answer, Export skips that NIF. With no edits, it refuses an unrecorded file and explains why. An answer counts for one export, and only while the file on disk is the one you answered for: a file that changed since says "changed on disk since you answered" and asks again. The row's **Diff…** switches to the Meshes view and opens the edit panel's list of the blocks that differ.

## PGPatcher and the game

The toggle reads **In game** and **Source** with or without an edit open; while editing, In game is simulated and Source is your edited NIF, as their hovers say ([below](#in-game-and-source-while-editing)). Once you have exported, the amber line "PGPatcher output predates your edit: re-run PGPatcher." shows: the game still loads PGPatcher's copy, which was made from the unedited NIF, until you re-run PGPatcher. If the edit preview cannot be shown, the line "Your edit could not be shown. Switch to In game or Source to retry." says so; edits you make meanwhile still count.

### In game and Source while editing

In game runs Tessalith's PGPatcher simulation on your edited NIF and shows still materials. It follows PGPatcher 2.3.1: loose JSON only, no modrules priority, read at the last scan. The JSON is read in the background when the edit opens, and a folder scan has no MO2 instance, so In game says no JSON was read. PGPatcher's real output, made from the unedited NIF, is not shown while you edit. For the selected shape a line says what happened:

- No JSON matches it, it is already PBR, or a file is not applied (amber, with the reason; the next file can still match).
- PGPatcher skips it (`PG_IGNORE`, amber).
- The shape carries a `PG_STOCK` block (amber): PGPatcher reverts such a shape to its recorded stock state first, and the preview shows it as loaded.
- **IMPORTANT** A match key such as `match_diffuse` or `nif_filter` that isn't a string, in any JSON the instance loads, crashes PGPatcher 2.3: the run stops, so every shape shows one red line per such file, even shapes PGPatcher would leave alone. The set preview in the Textures view shows the same lines.
- PGPatcher makes the shape PBR while it has an Env map scale controller (red): that crashes the game ([why](GUI-NIF-Animation.md#warnings)).

The simulation leaves out:

- JSON inside BSAs, and PGPatcher's own mod priority (`modrules.json`; the MO2 order stands in);
- plugin alternate textures (TXST), and the plugin and mesh-blocklist rules that decide which meshes PGPatcher patches;
- complex-material and parallax competition, and shapes with no texture set;
- JSON changed on disk since it was read (rescan), and the `PG_STOCK` revert.
## Not supported

- A Skyrim LE NIF opens without a 3D preview ("LE mesh: no 3D preview").
- Splitting a shared shader, or a shared interpolator for a value edit (the edit is refused or is a conflict); an outline for the selected shape.
- Effect-shader controllers: the editor does not read them; only the alpha controllers of such a shape are listed.
