# Set editor and texture set records

For anyone filling a material set's slots, wiring packed channels, or pointing a set at a plugin's texture set record (TXST). Cards, moves and filters are on [Sets column](GUI-Sets-Column.md).

## The set editor

Select a set and the centre column shows its heading row (the input name box, [Bulk editing and renaming](GUI-Bulk-and-Rename.md#input-name-box); the **Match** box on a TruePBR set; **2D | 3D**, see [Previews](Previews.md)), then the slot rows, then two tabs. **Material** (**Set** on a vanilla set) holds the PGPatcher settings ([PGPatcher settings and the JSON](GUI-PGPatcher-JSON.md#whats-on-the-material-tab)) or a material card ([Skin](Skin.md), [PBR Landscape](PBR-Landscape.md)). **Advanced** has the same sections as the [Advanced drawer](GUI-Texture-Panel.md#advanced-drawer).

![The set editor.](../img/gui/GUI-Set-Editor/set-editor.png)

## Slot rows

One row per PGPatcher slot the set's shader uses, in slot order, at most 8. A filled slot the shader can't use stays, amber, with the reason on hover. The cubemap has no row: set it with a slot command.

- Landscape sets add **Projected noise** and **Detail normal**. They aren't PGPatcher slots: [PBR Landscape](PBR-Landscape.md).
- Slot 7 holds the set's `_cnr` or `_f`. With both, the row turns red, lists each with a ✕ to take one out, and export warns "_cnr and _f both use slot 7", because PGPatcher has one slot 7. A `_g` and an `_sk` on slot 3 clash the same way.
- A `_coat` fills Subsurface, which then reads **Coat colour + strength**.
- A complex `_m` has no row. Its **Enable parallax** box sits under the rows ([Texture settings](GUI-Texture-Panel.md#changing-a-setting)).
- **Match on this** (Diffuse, Normal) makes PGPatcher apply the entry to meshes with that exact texture in that slot ([Matching](GUI-PGPatcher-JSON.md#matching)). **Opaque** (Diffuse) strips the alpha.

Drop a file from Explorer on a row to fill it. It must be a texture of the open folder and fit the slot; files are never copied, and the row says why if it refuses. The **Shader** dropdown lists the Mode's shaders, **Auto** clears your pick, and how a shader is chosen is on [Set shaders](Set-Shaders.md#how-the-shader-is-picked).

## Slot menus

Right-click a slot row, or a member chip on a card.

- **Clear**, **From this set** and **Path** are one undo step each. On a borrowed slot, From this set and Clear both drop the borrow. <!-- D:HC7 -->
- **From another set** links the slot to a map of another set. It needs the same slot kind.
- **Pin this slot (always use this file)** keeps the slot's file as it is. A pinned slot refuses drags and slot commands, bulk edits skip it (*slot locked*), and automatic re-typing leaves it alone. Pinning the diffuse of an Advanced skin set stops the Advanced skin reading unless the sub-type is already `skin_albedo`. The pin is saved in `sdc-project.toml`, never in the PGPatcher JSON.
- **Keep the original texture** / **Let PGPatcher replace the texture** toggles the slot's PGPatcher lock (`lock_diffuse`, `lock_normal`, `lock_emissive`, `lock_parallax`, `lock_rmaos`, `lock_cnr`, `lock_subsurface`) on a set that writes a PGPatcher entry. PGPatcher then leaves the mesh's own texture in that slot. The file is still exported, and the card shows a padlock, *kept by PGPatcher*. A `slotN` command on the same slot wins ("slotN overrides the lock"). There is no cubemap lock, because `lock_cubemap` would clear the slot.
- **Show in Explorer** selects the source file.

**Slot commands…** on a row is the same list plus *Pin slot N*.

![The menu of the RMAOS row in the set editor.](../img/gui/GUI-Set-Editor/slot-menu.png)

## Packed channel sources

Under a packed output (`_rmaos`, `_rfaos`, `_m`, `_p`), each channel has a **Source** picker: a helper or **Value**. Picking a helper adds the channel to that helper's `helper.fills` and takes it off the others, as one undo step ([Packing PBR maps](Texture-Packing.md#helper-textures)). **Value** uses the channel's `packing.fill.*` with a **Reset**; it is greyed when the channel's file feeds it by name. The engine still decides: a pick that can't fill the channel shows the helper as not used, with the reason.

## Texture set records

**Map to texture set record…** in a set's menu ties the set to a plugin's TXST record, so its files are written where the record points. It's greyed until a [scan](Mod-Scanning.md) with records exists, and for a PGPatcher PBR set under `cs`: those name their outputs in the JSON, so use **Rename outputs**. A mapped set's menu has **Change record…** and **Remove record mapping**. <!-- D:HG8 -->

- **The picker** lists the scan's winning records by EDID, FormID, plugin and colour path. Typed words must all match. A picked row shows each file's new path or "keeps its own path". **Map** stores it; Enter and a click outside don't. <!-- D:HG8 -->
- **The mapping survives load-order changes.** <!-- D:HG8 --> `[sets.txst]` in `sdc-project.toml` copies the record's paths, so a new scan never moves an output by itself. The card shows `Record: …`.

A slot file is written at the record's path, ahead of **Rename outputs**. A file with no slot in the record keeps its path. A record path outside `textures/` or not ending in `.dds` is never written, and two sets on one path give the duplicate-output row. A file already there that Tessalith didn't write is kept and reported as `OUT-UNKNOWN-EXISTING` ([Validation and output](Validation-and-Output.md#re-runs-and-if-changed)). <!-- D:HG5,HG6 -->

An empty slot of a mapped set reads one of three ways:

| Row | Meaning |
|---|---|
| Amber, borrowed | Another set in the project writes the record's path. Nothing is added to `sdc-project.toml`, and one `_n` is written. <!-- D:HG7 --> |
| Grey **From record: body_s.dds** | No set writes the path. The slot counts as supplied: no missing-map warning. |
| **empty** | The record names nothing for the slot. |

A file the set has, a borrow and a pin all win over a fill. **Clear** turns a fill off and brings its missing-map warning back; **From {set}** or **Use the record's slot** turns it on again. <!-- D:HG7,HG9 --> After a scan, a mapped set whose paths differ from the record gets a Problems row "Record {EDID} changed" with **Use new paths**. A record missing from the scan gets an Info row and the mapping stays. <!-- D:HG8 -->

`sdc convert` doesn't read mappings or fills: [what the CLI reproduces](GUI-Review-and-Export.md#does-the-cli-use-my-gui-edits).
