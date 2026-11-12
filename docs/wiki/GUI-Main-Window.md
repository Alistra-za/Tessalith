# Main window

For anyone opening a first folder: how it opens, what the header does and how to find a texture in the Textures column. The GUI saves your decisions in the folder's `sdc.toml`.

## Opening a folder

Click **Open folder** on the start screen, choose **Open > Open folder…**, pick one from **Recent** (the last 10), or drag a folder onto the window. A dropped file opens the folder that holds it. Tessalith reads the folder recursively and plans the conversion. Nothing is written to your textures.

![The start screen with two recent folders.](../img/gui/GUI-Main-Window/start-screen.png)

The columns and the Advanced drawer appear once a folder is open, and the widths you dragged come back.

While it opens, a progress bar counts the files to plan and **Cancel** returns to the folder you had before, with its undo history. Thumbnails land when the whole plan does; settings stay off until then.

### When `sdc.toml` is rejected

An `sdc.toml` that can't be parsed, or that the engine refuses (an unknown field, an unknown sub-type), is left out of the plan. A banner shows the error and the GUI stays read-only until you fix the file and press **Reload**. Most out-of-range values aren't a rejection: they are reported per file as `CFG-INVALID` ([Diagnostic codes](Diagnostic-Codes.md)).

When the rejected value came from a GUI edit, that edit is the top **Undo** step. Undo takes it back and makes the file editable again.

## Header

- **Undo** and **Redo** go up to 200 steps (**Ctrl+Z**, **Ctrl+Y**). Undoing back to the last save clears the unsaved dot in the title.
- **MO2…** picks the scan source: [Mod scanning](Mod-Scanning.md). **Preset** picks the preset the folder converts with and saves it as `preset` in `sdc.toml`; its last entry, **Presets…**, opens the **Presets** tab where the Mode is set: [Presets and targets](Presets-and-Targets.md).
- **Save** (**Ctrl+S**) writes `sdc.toml` and `sdc-project.toml`. **Export** is on [Review and Export](GUI-Review-and-Export.md#the-export-button).

### Export to

**Export to ▾** holds the export folder, saved per project in `sdc-project.toml`. With none chosen it reads **Export to: not chosen ▾** and Review blocks the export: a project never takes over the folder you picked for another mod. <!-- D:FB1 -->

![The Export to menu.](../img/gui/GUI-Main-Window/export-to-menu.png)

The menu also holds **Open export folder**, the checkable **Also export as Vanilla** and **Also export as Complex material** ([Set shaders and Also export](Set-Shaders.md#also-export-as-vanilla-and-cm)) and, under `cs`, **PGPatcher JSON editor…** ([PGPatcher settings and the JSON](GUI-PGPatcher-JSON.md#the-json-editor)). With no project open it is a plain **More ▾**.

### Status line and banner

A dot in the window title means unsaved edits. The status line shows file and set counts, a VRAM estimate for the selection and the folder, and the progress of whatever runs. A banner appears under it when something needs you, such as a rejected `sdc.toml`. Everything it says is also kept in [Messages](GUI-Guide.md#messages).

## Finding a texture

One tile per source file, with a thumbnail, name and badges. An amber border means the texture waits for an answer ([Deciding](GUI-Texture-Panel.md#deciding)). Click selects a tile, double-click opens the 2D preview ([Previews](Previews.md)). Selecting several is on [Bulk editing and renaming](GUI-Bulk-and-Rename.md#editing-them-together).

Helper files (raw AO, metallic, roughness and other masks) are rows under the tiles, not tiles ([Packing PBR maps](Texture-Packing.md#helper-textures)). A cube made of six face files is one item ([Cubemaps](Cubemaps.md)).

### Filters

The pills *Undecided*, *Warnings*, *Failed* and *Not in a set* show how many textures each would list. *Not in a set* also counts a lone texture in a one-file set, which does export; only the **No set** label on a row means the texture isn't written ([Textures in no set](GUI-Sets-Column.md#textures-in-no-set)). *Undecided* and *Warnings* leave out textures in no set. <!-- D:HF1 --> **Filter ▾** adds *Sub-type*, *Output format*, *Set membership* and *Sort*, and the search box and the tile size buttons sit beside them. A texture shows only if it passes every filter, and a filter only hides: a selection keeps its hidden textures. **Clear filters** keeps the sort and the tile size. The filters, sort and tile size are remembered in `gui.toml` ([Installation](Installation.md)).
