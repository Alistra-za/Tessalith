# Editing a NIF

How to edit a mesh's shader, alpha, controllers and extra data in the Meshes view, and export an edited copy. For mod authors fixing or tuning meshes. Your source file is never changed. Controllers are on [NIF animation](GUI-NIF-Animation.md); existing clips play there. Exporting the edit, and how PGPatcher's copy relates to it, is on [Exporting an edited NIF](GUI-NIF-Export.md).

## Before you start

- Scan your mods first, so the Meshes view lists your meshes ([Mod scanning](Mod-Scanning.md)).
- Pick an export folder with **Export to ▾** in the header ([Export to](GUI-Main-Window.md#export-to)). Edited NIFs go there, next to your textures. <!-- D:GD5 -->

## Editing a NIF

**Edit** (next to the NIF's name in the Meshes view) opens the edit panel for the selected NIF; click it again to close the panel. A NIF with a saved edit gets a chip in the mesh list: **Edit pending**, **Edited** (exported and unchanged) or **Edit conflict**. The panel shows the file's data path and `Source: <where it came from>`. Edits never change the source file. They are kept in a small edit document in `.sdc\nif-edits` under the export folder, and the export (from Review) writes a copy there. The export folder is the Textures one, once you have chosen it for the project (**Export to ▾** in the header, or **Choose the export folder…** in Review); there is no default. Until you choose one the edit works, the edit panel says "Skipped until an export folder is chosen", Review's export says "No export folder chosen", and the edit document is kept in `%LOCALAPPDATA%\sdc\nif-edits` (one file per NIF); the first save into a chosen folder moves it there. Tessalith never edits a NIF from an `sdc NIF Edits` folder, or from any folder that holds its edit document for that NIF. <!-- D:GQ3 -->

**Cards.** In edit mode the shape cards get a **Root** card first, then one card per shape. Click a card's header to select it; the tabs below edit it, and a line under the tab strip names it ("Editing: BodyMesh (BSTriShape)"). **Show only this shape** hides the other shapes in the 3D view. A change to the shader type, a flag, the alpha or a texture path shows on its card at once.

## Tabs

- **Shader** edits the BSLightingShaderProperty of the selected shape: texture slots (counted from 1 like the set editor; a True PBR set uses the set editor's names for slots 3 and 7), shader type, the per-type fields and the SLSF1 and SLSF2 flag lists. A list with a flag warning starts open.
- **Alpha** edits the NiAlphaProperty. A blend pair the engine mapping does not cover is drawn as stored and marked **Not verified in game**.
- **Controllers** lists the shape's controllers (alpha, shader float and colour, texture transform) with their start, stop, frequency, phase, cycle and the animated value, and edits their keys ([NIF animation](GUI-NIF-Animation.md#the-controllers-tab)). Controllers of other classes are read-only.
- **Extra data** lists the extra data of the root or the selected shape: BSXFlags bits, string, integer, boolean and float values, BSInvMarker. **Add to root**, **Add to shape** and **Remove** change the list. Each row shows its own warnings under it, for example "BSXFlags must be named BSX". A **Known** list above the type and name fills in an entry the game or a plugin reads, with its type, name and a starting value. On a shape: **AnisotropicAlphaMaterial** (Community Shaders see-through look; starts at 3, woven fabric). On the root: **BSX**, **INV**, **HDT Skinned Mesh Physics Object** (type the physics xml path in **Value**; Add stays off until you do) and **Prn (unconfirmed)** (the skeleton node the item attaches to; not checked against a source yet). An entry that goes on the other owner is greyed ("Goes on the root", "Goes on a shape"), and so is one this owner already has ("Already on this root"). **Custom…** keeps free typing for anything else. BODYTRI is not offered: BodySlide and Outfit Studio write it. **Heel height (RaceMenu)** (shapes only; on the root: "The game ignores heel height here. Put it on a shape.") raises the actor so heels stand on the ground. It starts at the shape's stored value, **Calculate** fills it from the lowest point of the shape in bind pose (rounded to 0.1, still editable), and **Set heel height** writes it: the shape's HH_OFFSET is changed in place or added, and an SDTA the shape already has gets the same height in its NPC entry (a new SDTA only when **Also write SDTA** is ticked). It says when an earlier shape's heel height wins, and for `_0`/`_1` files it reminds you to apply the same value to the other weight file. Heel data anywhere but on a shape, and on a shape after the first shape in the scene that has some, warns under its row. A negative height says it lowers the actor. When the shape's HH_OFFSET is not a number the editor can set (another type), Set heel height stays off until that row is removed.

## Point a shape at a .csm

Use this when a mesh should take its PBR material from one of your sets. CS 2.0 reads a shape's PBR material from the `.csm` file its material name field names, and Tessalith writes one `.csm` per PBR set ([CS PBR records and .csm materials](CS-PBR-Records.md)). Both buttons below need the project's target on Community Shaders.

### A shape without v1 PBR

1. Select the shape's card and open the **Shader** tab.
2. The **Material** row shows the name field, "(none)" when it's empty. Click **Use the set's .csm** beside it; its hover names the path and the set.

The set is the project's PBR set whose diffuse is the shape's slot 1. The button is greyed, with the reason on hover, when there's no such set ("Make a texture set for these textures first."), when the target isn't Community Shaders, on a v1 PBR shape (use the convert below), or when the shape already names that `.csm`. <!-- D:IE14 -->

### A v1 PBR shape

A v1 PBR shape (type 0 or 11 with the PBR flag, SLSF2 23) has the amber warning "Draws vanilla on CS 2.0" ([why](Previews.md#csm-materials-in-3d)).

1. Click **Convert to .csm** on that warning.
2. A set that holds no PBR values yet gets the shape's values. When the set's values differ, the window **The set holds other PBR values** lists them, set beside shape. **Use the set's values** (Enter) leaves the set alone; **Replace the set's values** (red) writes the shape's values into it, and is greyed while the sets can't be edited or when the shape's switches would block the set's `.csm`. A row that is a PGPatcher setting can't be replaced here: change it in the set's PGPatcher settings.

Either way the shape ends up on the set's `.csm`: the convert clears the v1 PBR flags (SLSF2 12, 23 to 27 and 30), sets type 0, keeps the texture slots and writes the `.csm` path into the name field, as one edit. The set's values are a step on the Textures history and the shape change a step on the NIF edit history, so you undo them separately. <!-- D:IE14 -->

### When the shape already has a material name

The name field is replaced without asking when it's empty or already a `.csm` path. Any other name asks first, in **Replace the material name?**, because another tool may use it: **Replace <name>** (red) or **Keep the name** (Enter), which changes nothing. <!-- D:IE14 -->

### No edit writes v1 PBR

- The PBR flag (SLSF2 23) is read-only, and its hover points at **Convert to .csm**.
- An edit that turns the flag on, or a shader type change that makes a shape with the flag v1 PBR, is a red **Pending edits** row, "v1 PBR is gone; use Convert to .csm". It blocks the NIF's export, so an old saved edit that did this can't slip through. <!-- D:IE14 -->

The 3D view draws the shape with the set's `.csm`, unsaved set edits included ([.csm materials in 3D](Previews.md#csm-materials-in-3d)). <!-- D:IE15 -->

## Shared blocks

The panel says when a block has more than one user.
- A shared alpha: "Shared by N shapes: editing will split." The first edit splits it, so only this shape changes. On the Controllers tab you click **Split alpha** first. Split alpha also works on an alpha a clip drives: both alphas keep following the clip.
- A shared texture set: "Shared by N shapes: editing makes a copy for this shape."
- A shared shader: "Shared by N shapes: shader edits change all of them." A texture path edit on a shared shader is refused, because it would change the other shapes.
- A shared interpolator: a value edit is a conflict ("This interpolator is shared by N controllers"), and the value row is greyed. A key edit copies the interpolator and its keys first, so the other controller's curve does not change.

## Warnings

On a PBR shape, the SLSF2 flags that CS reads differently (Fit_Slope, Multi_Layer_Parallax, Soft, Back, Rim and Effect lighting) get a dimmer "PBR: <meaning>" label, and the same text in their hover; a non-PBR shape shows only the vanilla names.

Values are never clamped. A value outside the range the game expects, or a flag combination that looks wrong, shows an amber warning under the field (red for an error), sometimes with a **Fix** button. A red error that your edit introduced blocks the NIF's export. An error that was already in the source only warns. Three flag warnings come from the corpus census: shader type 11 without the Multi_Layer_Parallax flag (SLSF2 24) and without PBR; SLSF2 24 on a shader type other than 11; Fit_Slope (SLSF2 12) without PBR. On a True PBR shape an empty RMAOS slot is amber: "Slot 6 (RMAOS) is empty: in game the mesh renders fully metallic".

**Flags that need a shader type.** The skin tint flag (SLSF1 21) works only on the Skin Tint type and the FaceGen detail flag (SLSF1 10) only on Face Tint (which needs it) or Skin Tint: a mismatch is a red error that always blocks the NIF's export, even in an untouched file, with a **Clear the flag** or **Set the flag** button; a type change fixes the flag in the same undo step and says so in a line under the type, and a flag that cannot apply to the current type is greyed with the reason on hover.

## Pending edits, undo and source changes

**Pending edits (N).** Every edit is a row in this list under the file name, named in words ("BodyMesh: SLSF1 flags"). A slider drag is one row. **Revert** removes that edit. A row in red did not apply, with the reason.

**Undo.** While an edit is open in the Meshes view, the header **Undo** and **Redo**, the command palette and **Ctrl+Z** / **Ctrl+Y** act on the NIF edit. With no edit open they act on the Textures history. The header buttons say which: **Undo** for the Textures history, **Undo NIF edit: Set shader type** (the step's name) for the NIF one, and the hover names the history.

**Source changed.** If the source NIF changes on disk (a mod update), the edits are re-applied by name and role, not by position. The panel says "Source changed: N applied, M conflicts". A conflicting edit has **Take source** (drop your edit) and **Keep mine** (keep it on top of the new source).

## Not supported

- A Skyrim LE NIF opens without a 3D preview ("LE mesh: no 3D preview").
- Splitting a shared shader, or a shared interpolator for a value edit (the edit is refused or is a conflict); an outline for the selected shape.
- Effect-shader controllers: the editor does not read them; only the alpha controllers of such a shape are listed.
