# GUI guide

A map of the GUI (`tessalith.exe`) for anyone who just opened it, plus the keys, the bottom panel and the dialog rules that apply everywhere. For a first set from source folder to DDS, start with [Getting started](Getting-Started.md).

## The main window

![Tessalith's main window with a folder open: the header on top, the Textures and Sets columns, the settings of the selected texture, the Advanced strip at the right and the bottom panel with its Problems tab. Numbered red markers point at the areas listed below.](../img/gui/GUI-Guide/main-window.png)

The picture shows a small made-up mod. Double-clicking a tile replaces the centre with the 2D preview: see [Previews](Previews.md).

1. **Open, Undo, Redo**: [Main window](GUI-Main-Window.md#header).
2. **MO2…, Preset**: the scan source and the preset the folder converts with. The Mode (the target, `cs` or `vanilla`) is set in the **Presets** centre tab, opened from **Preset ▾ → Presets…**: [Presets and targets](Presets-and-Targets.md).
3. **Textures / Meshes tabs and the column eyes**: [Main window](GUI-Main-Window.md#header); the Meshes view is [Mod scanning](Mod-Scanning.md).
4. **Export to ▾**: [Export to](GUI-Main-Window.md#export-to).
5. **Save**: writes `sdc.toml` and `sdc-project.toml`.
6. **Export**: [Review and Export](GUI-Review-and-Export.md#the-export-button).
7. **Texture filters**: [Finding a texture](GUI-Main-Window.md#finding-a-texture).
8. **Texture tiles**: [Finding a texture](GUI-Main-Window.md#finding-a-texture). An amber border means the texture waits for an answer: [Deciding](GUI-Texture-Panel.md#deciding).
9. **Sets filters**: [Finding a set](GUI-Sets-Column.md#finding-a-set).
10. **A set card**: [Sets column](GUI-Sets-Column.md#reading-a-set-card).
11. **Settings of the selected texture**: [Texture settings](GUI-Texture-Panel.md). Selecting a set opens the [Set editor](GUI-Set-Editor.md) instead.
12. **Advanced** (right edge): [Texture settings](GUI-Texture-Panel.md#advanced-drawer).
13. **Bottom panel tabs**: **Problems** ([Problems](GUI-Problems.md)), **Review** (only while an export is being reviewed: [Review](GUI-Review-and-Export.md#review)), **Messages** ([below](#messages)) and **Files** ([Files tab](GUI-Review-and-Export.md#the-files-tab)).

The line under the header is the [status line](GUI-Main-Window.md#status-line-and-banner).

## The bottom panel

Click a tab to open the panel on it. Click the tab that is showing, or the **▼** at the right, to collapse it. Drag its top edge to resize.

### Messages

Every status-line note and every toast (*Saved*, *Export finished*, *Scan finished*, the *Undid / Redid* note) is also logged in the **Messages** tab, newest first. <!-- D:CD5 --> **Clear** empties it, and it is gone when you close the window. **Ctrl+M** opens the tab, or collapses the panel when it already shows Messages.

Anything that needs a decision is a [dialog](#dialogs), not a toast.

## Keyboard

Press **F1** or **?** for the list of shortcuts, grouped by where they work. **Ctrl+K** opens the command palette: type part of an action's, a texture's or a set's name, and Enter runs it or selects it. A fresh palette highlights nothing, so Enter never starts Export by chance.

### Keys in text fields

While a text field has the keyboard, only **Ctrl+K**, **Ctrl+S** and **Esc** act, so a shortcut never eats what you type.

- **Ctrl+S**, **Save**, **Export** and Review's **Export** end the edit first and keep what you typed, then save or export.
- **Esc** in a field that commits when you leave it (a name, a match path) cancels the edit.
- No shortcut acts while a dialog, a menu or a box is open. A key whose action is off, such as Ctrl+Z with nothing to undo, does nothing.

### All shortcuts

| Area | Keys | Action |
|---|---|---|
| Global | Ctrl+K | Command palette |
| Global | F1 or ? | Keyboard shortcuts |
| Global | Ctrl+M | Messages |
| Global | Ctrl+S | Save |
| Global | Ctrl+Z | Undo |
| Global | Ctrl+Y or Ctrl+Shift+Z | Redo |
| Textures grid | Ctrl+A | Select every visible texture (every set when a set is active) |
| Textures grid | Left | Select the texture to the left |
| Textures grid | Right | Select the texture to the right |
| Textures grid | Up | Select the texture above |
| Textures grid | Down | Select the texture below |
| Textures grid | Shift+Left | Extend the selection to the left |
| Textures grid | Shift+Right | Extend the selection to the right |
| Textures grid | Shift+Up | Extend the selection up a row |
| Textures grid | Shift+Down | Extend the selection down a row |
| Textures grid | Enter or Space | Open the expanded preview of the active texture |
| Preview | Esc | Close the preview |
| Preview | [ | Previous texture |
| Preview | ] | Next texture |
| Preview | L | Magnifier lens on or off |
| Preview | T | Tile the picture 3×3 on or off |
| Preview | 1 | Show RGBA |
| Preview | 2 | Show R |
| Preview | 3 | Show G |
| Preview | 4 | Show B |
| Preview | 5 | Show A |
| Preview | 6 | Show the α-mask |
| Preview | PageUp | Larger mip |
| Preview | PageDown | Smaller mip |
| Sets | F2 | Rename the outputs of the selected PGPatcher PBR sets, or the inputs of the selected textures |
| 3D preview | Home | Recentre the 3D view (as a double-click) |
| 3D preview | V | Next 3D debug view (Lit, Albedo, Normals, …) |
| 3D preview | Ctrl+Shift+C | Copy the 3D view as an image |
| JSON editor | F8 | Next error in the PGPatcher JSON editor |
| JSON editor | Shift+F8 | Previous error in the PGPatcher JSON editor |
| Problems list | Enter or Space | Accept the focused Problems row's suggestion and go to the next row |
| Dialogs | Enter | Press the dialog's safe default |
| Dialogs | Esc | Cancel the dialog |

## Dialogs

- **Esc**, or a click outside, cancels. The rename dialogs, **Copy settings** and **Fill from names** ignore a click outside, so a stray click can't lose what you typed.
- **Enter** presses the filled teal button, the safe default, and never saves, deletes or exports. <!-- D:FC5 -->

Where to go next: [Troubleshooting](Troubleshooting.md) covers crash recovery.
