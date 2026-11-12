# Problems

For anyone getting a folder ready to export: the bottom panel's **Problems** tab lists one row per thing that needs you, each with its fix as a button. What a code means is in [Diagnostic codes](Diagnostic-Codes.md).

## Reading a row

Rows come blockers first, then undecided textures, then warnings, then checks and info rows.

![The Problems tab.](../img/gui/GUI-Problems/problems-tab.png)

| Dot | Meaning |
|---|---|
| Red | **Blocker**: stops the export. Fix it or remove the texture. |
| Amber | **Warning** (the export goes ahead) or a decision you owe. |
| Grey | **Check**, a hint from an artist check ([Texture types](Texture-Types.md#artist-checks)): a map that will probably look wrong in game. It never blocks and never changes "ready". Or an info row. |
| Green, on a tile or card | Nothing to decide. |

A row names where it is (a texture, set or file you can click; "Project" when it belongs to no file) and says the action in words. Hover it for the engine's message and code (`LOSS-ALPHA-DATA`), which is what Troubleshooting uses.

## Rows and their fixes

A row you must act on has its fix as a button.

| Row | Fix |
|---|---|
| A blocker, for example a set that matches nothing | Its own button, or **Go to set**. |
| *Failed: …* | None: the texture failed to plan. |
| *Undecided: …* | The type buttons and **Acknowledge** ([below](#deciding-from-the-list)). |
| *Undecided: not a cube source (…)* | **Set type…**: the only answer is a different sub-type. |
| *Alpha: … — confirm* | The **Alpha type** dropdown and **Confirm**: [Deciding](GUI-Texture-Panel.md#deciding). |
| Warnings on a set | **Go to set**. They never hold back Export or open Review. |
| A file's export warnings, after an export | Click the file. |
| Info rows (things Tessalith changed for you) | **Undo this change** while that step is still the last one. <!-- D:FC3 --> |
| `CHAN-NORMAL-LENGTH`, a short normal | **Rebuild Z**, which sets the file's `channels.reconstruct_z`. |
| A PGPatcher feature clash | **Turn off {feature}** for each of the two; neither is the default. <!-- D:FN5 --> |
| *sdc.toml is read-only* (or `sdc-project.toml`) | **Clear read-only flag**, on that file only. <!-- D:FC10 --> |
| *Looks like complex grass, but is not tagged as grass* | **Tag as grass**, which writes `subtype = "complex_grass"`. <!-- D:FC10 --> |
| *Colour divided by alpha on load (premultiplied EXR)* | **Straight alpha**, which writes `alpha.exr_straight = true`. <!-- D:FC10 --> |
| *No albedo in this set; the mesh keeps {path}…* (a PBR set with no albedo) | No button: the set still exports. |
| *No mesh of the latest scan uses this diffuse yet…* (the PGPatcher entry is still written; LOD meshes don't count) | No button: the set still exports. |
| *No texture set of the latest scan uses this texture* (grey) | No button: the set still exports. |
| *Replaces {file} of the target mod in the load order* (an output the target mod already ships) | No button: the set still exports. Rename its outputs to keep the target's file ([Renaming](GUI-Bulk-and-Rename.md#renaming)). <!-- D:HL6 --> |
| *Preset {id} was skipped: it clashes with {other} on aliases* | **Remove aliases from {id}**: [Presets and targets](Presets-and-Targets.md#checking-and-installing). |

Info rows count in the tab's number but never against "ready".

## Deciding from the list

- An undecided texture with a suggestion shows **Set type: X ✓** (a click decides it) and a **▾** for the type picker, which has a search box and the suggestion on top. With no suggestion it shows **Set type…**. **Accept all suggestions (n)** decides every texture that has one, as one undo step.
- Tab to a row and press Enter or Space to accept its suggestion and move on to the next row.

An accept is an edit in memory: **Save** writes it to `sdc.toml`.

## What the GUI writes on its own

When a plan settles, every sub-type taken from a High-confidence suggestion (including one the scanned meshes or a GRAS grass model gave it) and every alpha meaning taken from the meshes (also `remove`, an alpha they never read) is written to `sdc.toml` as a `[files]` entry, as if you had chosen it.

- It is one undo step, `Set {n} types automatically` (`values` when an alpha meaning is among them), and one Info row with **Undo this change**.
- These types alone don't count as your edits: no title dot, and no unsaved question on close.
- A file you already decided is never touched, and an Undo stays undone until the folder reopens.
- It re-types only while the type is still Tessalith's own pick. A type you changed is kept.
- A file that waits for a type records nothing: [why](Texture-Types.md#how-files-form-a-set).

## Narrowing the list

Click a row's subject to select it in the Textures view. Arriving from a count (**N undecided**, a set's **⚠** badge) opens the list narrowed, with a chip to clear it. The **Checks** pill narrows to check rows.

**Kinds ▾** hides kinds of warning and info row for this project, such as every missing specular. Blocks and questions can't be hidden, and Review still lists every warning before an export.
