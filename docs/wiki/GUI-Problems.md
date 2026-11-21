# Problems

Before export, open **Problems** in the bottom panel for issues and their fixes. **Notes** explains Tessalith’s automatic choices. See [Diagnostic codes](Diagnostic-Codes.md) for code meanings.

## Reading a row

Rows come blockers first, then undecided textures, then warnings, then checks. Notes that have a button come last ([Problems and Notes](#problems-and-notes)). <!-- D:JO3 -->

![The Problems tab.](../img/gui/GUI-Problems/problems-tab.png)

| Dot | Meaning |
|---|---|
| Red | **Blocker**: stops the export. Fix it or remove the texture. |
| Amber | **Warning** (the export goes ahead) or a decision you owe. |
| Grey | **Check**, a hint from an artist check ([Texture types](Texture-Types.md#artist-checks)): a map that will probably look wrong in game. It never blocks and never changes "ready". |
| Grey, a note | **Info**: something Tessalith did for you, and why the output is fine. **Intended**: a row you marked as intended. |
| Green, a note | **Queued**: a fix that waits for the next export. <!-- D:JO9 --> |
| Green, on a tile or card | Nothing to decide. |

Click a row’s texture, set or file to open it. “Project” means the row has no file. Hover for the engine message and code (such as `LOSS-ALPHA-DATA`) used in Troubleshooting. After the action sentence come the fix, any second fix, **Mark as intended**, then **Go to set** on set rows. <!-- D:JO10,JO13 -->

## Problems and Notes

**From the NIF you are editing (k)** groups its counted problems. Hover the tab for the NIF's share. <!-- D:JY9 -->

`Problems (2) · Notes (67)` means two issues to check and 67 notes. <!-- D:JO3 -->

- **A problem** is a row about an output that may be wrong or a file that isn't written. A set with one isn't ready ([what Ready means](GUI-Sets-Column.md#reading-a-set-card)). Check rows count here too, but never against Ready. <!-- D:JO1,JO5 -->
- **A note** needs nothing from you: Tessalith used a default and the row says why that's fine (no metallic map, so the surface reads as not metal), you marked the row as intended, or a fix waits for the next export. Notes never count against Ready. <!-- D:JO1,JO2 -->
- **Notes (67)** lists every note, or the selected set's notes under a set filter. The default list includes notes with buttons. <!-- D:JO3 -->
- **Nothing left:** with no row at all the tab reads `Problems`. With only notes it reads `Problems (0) · Notes (67)`, and the list's first line reads "No problems. Notes (67) lists what Tessalith did for you." <!-- D:JO3 -->

## Rows and their fixes

A row you must act on has its fix as a button.

| Row | Fix |
|---|---|
| A blocker, for example a set that matches nothing | Its own button, or **Go to set**. |
| *Failed: …* | None: the texture failed to plan. |
| *Undecided: …* | The type buttons and **Acknowledge** ([below](#deciding-from-the-list)). |
| *Undecided: This type isn't available in this mode…* | **Set type…**, or change the mode in **Settings…**. A type with a fallback has **Acknowledge** instead: [Sub-types](Texture-Types.md#sub-types-what-each-file-becomes). <!-- D:JF3 --> |
| *Undecided: not a cube source (…)* | **Set type…**: the only answer is a different sub-type. |
| *Alpha: … — confirm* | The **Alpha type** dropdown and **Confirm**: [Deciding](GUI-Texture-Panel.md#deciding). |
| Warnings on a set | **Go to set** ([below](#go-to-set)). You can export with these warnings. They still bring up Review if you've opted to skip it on clean exports. |
| A file's export warnings, after an export | Click the file. |
| A note about an automatic change with its own Undo step | **Undo this change** while that step is still the last one. While it has the button the note stays in the list, after the problems. <!-- D:FC3,JO3 --> |
| `CHAN-NORMAL-LENGTH`, a short normal | **Rebuild Z**, which sets the file's `channels.reconstruct_z`. On a file you already exported the row then reads *Rebuild Z is on for this file: export again to rewrite it*, a queued note with no button, until the next export. <!-- D:JO9 --> |
| `CHK-Z-SIGN` (a check) | **Rebuild Z**, which clears the row. <!-- D:JO15 --> |
| `SHADER-EXTRA-MAP`, a note: *{file} isn't read by the {shader} shader…* | **Remove from set** for that file, or **Change shader…**. The file leaves the set as **Remove member** does: [Textures in no set](GUI-Sets-Column.md#textures-in-no-set). For the set's packed output (`_rmaos`, `_rfaos`) there is no **Remove from set**: the row offers **Clear recipe** ([when](#clear-recipe)) and **Change shader…**, or **Change shader…** alone. <!-- D:JO15,JR10 --> |
| `PACK-RECIPE-CONFLICT`, `PACK-RECIPE-UNBUILDABLE` | **Clear recipe** ([when](#clear-recipe)), or no button. |
| `SHADER-MISSING-MAP`, `SHADER-UNAVAILABLE` | **Change shader…**, a menu of the shaders the set's shader combo offers. <!-- D:JO15 --> |
| *{file} is shared with a regular set, which reads its alpha as specular; its alpha is not kept* (a note) | **Keep alpha** writes `alpha.policy = "data"` for this normal. [When the button is available](#keep-alpha). |
| *The file name and the contents point to different types* | **Set type…**, the type picker. <!-- D:JO15 --> |
| *Parallax is on but the source has no alpha* | **Turn parallax off**, which writes `alpha.parallax = false`. <!-- D:JO15 --> |
| A PGPatcher feature clash | **Turn off {feature}** for each of the two; neither is the default. <!-- D:FN5 --> |
| *sdc.toml is read-only* (or `sdc-project.toml`) | **Clear read-only flag**, on that file only. <!-- D:FC10 --> |
| *Looks like complex grass, but is not tagged as grass* | **Tag as grass**, which writes `subtype = "complex_grass"`. <!-- D:FC10 --> |
| *Colour divided by alpha on load (premultiplied EXR)* | **Straight alpha**, which writes `alpha.exr_straight = true`. <!-- D:FC10 --> |
| *No albedo in this set; the mesh keeps {path}…* (a PBR set with no albedo) | No button: the set still exports. |
| *No mesh of the latest scan uses this diffuse yet…* (the PGPatcher entry is still written; LOD meshes don't count) | No button: the set still exports. |
| *No texture set of the latest scan uses this texture* (grey) | No button: the set still exports. |
| *Replaces {file} of the target mod in the load order* (an output the target mod already ships) | No button: the set still exports. Rename its outputs to keep the target's file ([Renaming](GUI-Bulk-and-Rename.md#renaming)). <!-- D:HL6 --> |
| *Preset {id} was skipped: it clashes with {other} on aliases* | **Remove aliases from {id}**: [Presets and targets](Presets-and-Targets.md#checking-and-installing). |
| *{n} saved settings name no file in this mode ({mode})* (grey, on Project; hover lists them) | **Entries ▾** lists each `[files]` entry with its own **Remove**; **Remove all** deletes every one as one undo step. [Before you remove one](#unmatched-files-entries). <!-- D:JI6 --> |

**Remove from set**, **Delete set**, **Remove match**, **Turn off {feature}**, **Turn parallax off**, **Clear read-only flag**, **Clear recipe** and **Remove aliases from {id}** run on a mouse click only. Enter and Space do nothing on them, so a key press never takes a file out of a set or switches a feature off. <!-- D:JO16 -->

### Clear recipe

On the `cs` target, **Clear recipe** appears when this set has a recipe other than `auto` on its `_rmaos` or `_rfaos` file in `sdc.toml` and Problems flags the recipe or packed map. It removes `packing.recipe` from both file entries. One Undo restores the values you had. A project or preset recipe can apply next and keep the warning. Otherwise the shader and maps choose the recipe. <!-- D:JR10 -->

### Keep alpha

On Community Shaders, this Note appears when a regular set’s Normal slot borrows a tangent-space normal from PBR, PBR Grass, PBR Landscape or Advanced Skin, including through a texture set record. A type assigned by a preset rule counts. The normal must have a conversion job, varying alpha, a planned policy other than **Data**, and no alpha policy in a matching `[files]` table. A kept DDS gets no Note. Your **Keep alpha** choice survives ending the share. For automatic writes, see [Sharing specular alpha](GUI-Sets-Column.md#sharing-specular-alpha). <!-- D:JS7 -->

Hover over a disabled **Keep alpha** for the reason. After a file edit, a partial plan can make the button available while the rest of the project is still being checked. A pin on the owner’s Normal slot or a read-only `sdc.toml` still blocks the edit. So do several matching full-name tables without an exact spelling, or a key that also controls another input. An extensionless table works if it controls this input alone. If the destination changes between display and click, check Problems again.

### Mark as intended

A warning you checked and want to keep as it is (a `_flow` map you made small on purpose) needs no fix:

1. Click **Mark as intended** on its row. The row moves to Notes with a grey dot and stops counting against Ready.
2. To take it back, click the **Notes (n)** pill, then **Undo mark** on the row.

Each click is one undo step. <!-- D:JO4 -->

- **Where it's kept:** `[project] intended` in `sdc-project.toml` records the code, location and sentence. Marks survive a set rename. Deleting the set drops them. A changed sentence shows again. <!-- D:JO4 -->
- **Warnings and checks only:** blockers, questions and a file that failed validation in the last export have no mark. Fix those, or export again. <!-- D:JO4 -->
- **Greyed** with no project open and while `sdc-project.toml` is read-only; the hover says why. <!-- D:JO4 -->
- To hide a whole kind, use [Kinds ▾](#hiding-kinds-of-row). <!-- D:JO4 -->

#### Reading marked warnings

- **In hovers:** a tile's hover, a file's warnings in the **Files** tab and a set's check badge list marked rows last, under "Marked as intended:". <!-- D:JO4 -->
- A marked warning drops out of the count beside its file in **Files**. If every warning is marked, the row shows a grey **n marked**. Hover it to read the warnings. **Has warnings** leaves that file out. <!-- D:JY18 -->

### Go to set

**Go to set** closes an expanded texture preview and opens the set in the [Set editor](GUI-Set-Editor.md). In **2D**, a missing or unused map scrolls to its slot or file and outlines it for two seconds if the editor draws that row. Whole-set rows and **3D** open the set without an outline. <!-- D:JO8,JO14 -->

### Unmatched [files] entries

A `[files]` entry in `sdc.toml` whose key names no file does nothing. That happens when the file was renamed, moved or deleted. Problems lists every such entry in one grey row.

Check the mode in the row before you remove anything. Some files are planned only under `cs`, so an entry that names no file in `vanilla` can still match in `cs`.

- **Remove** and **Remove all** are greyed while the plan runs and while `sdc.toml` is read-only.
- **Remove all** is also greyed while `sdc.toml` names a different mode from the one in the row. Its hover lists what it deletes.

## Deciding from the list

- An undecided texture with a suggestion shows **Set type: X ✓** (a click decides it) and a **▾** for the type picker, which has a search box and the suggestion on top. With no suggestion it shows **Set type…**. **Accept all suggestions (n)** decides every texture that has one, as one undo step.
- Tab to a row and press Enter or Space to accept its suggestion and move on to the next row.

An accept is an edit in memory: **Save** writes it to `sdc.toml`.

## What the GUI writes on its own

After planning, Tessalith records High-confidence types and mesh-derived alpha policies in memory as `[files]` entries for `sdc.toml`. Mesh scans and GRAS grass models can supply the type. Meshes that ignore alpha can give it the `remove` policy.

To check which normals can follow a shader, Tessalith reads the recorded types, file entries and slot locks once for the whole pass. <!-- D:JS5 -->

- Automatic types and alpha survive Undo of your edits. They have no Undo step. An established normal following its shader gets a separate step with **Undo this change** while it is last. <!-- D:JY6 -->
- Automatic recordings alone leave **Save** off and close, open or reload without prompting. **Save** with your edits or **Export** writes them. <!-- D:JY6 -->
- Your own type or alpha changes, including Undo, [need a save](GUI-Main-Window.md#header).
- Saving presets can give a recorded normal a different type for that pass. A normal can follow its set into PBR with suggestion acceptance off. Its Info row omits confidence. If ownership can't be recorded in `sdc-project.toml`, the suggested type stays. <!-- D:JS5 -->
- A file that waits for a type records nothing: [why](Texture-Types.md#how-files-form-a-set).

### Sets that left Advanced Skin

A `_fuzz` or `_sheen` map alone doesn't make a set Advanced Skin ([why](Texture-Types.md#suffixes)). On open or reload, Tessalith clears `skin_albedo` from a diffuse typed as skin on that evidence alone. The other maps determine the shader. A resulting PBR set without roughness or metallic warns about its missing `_rmaos`. <!-- D:JQ8,JQ56 -->

- The row names how many sets left Advanced Skin. Hover for each file, its change from skin albedo, and other types recorded in the same step. <!-- D:JQ19 -->
- **Undo this change** sits on the row while the switch is the last step. The switch and the types recorded with it are one undo step. Opening the folder leaves it clean. Undoing the switch gives you an edit to save. <!-- D:JQ17,JQ56,JY6 -->
- **An undone switch runs again** the next time `sdc.toml` is read. To keep a set skin, choose **Advanced Skin** as its shader in the set editor. <!-- D:JQ56 -->
- **Not switched:** a set whose shader you picked, a set whose diffuse is pinned, a set with a written `packing.recipe`, a set that other skin maps or types keep skin ([which](Texture-Types.md#suffixes)), a set whose only fuzz map is shared in from another set, and every set while `sdc.toml` is read-only. <!-- D:JQ8,JQ20,JQ55 -->

## Narrowing the list

Counts such as **N undecided** filter the list. Click the chip to clear it. **Checks** shows check rows.

The ordinary filter follows a set when its first file arrives. It prefers matching engine keys over ids and shows no rows for empty ids. A reason that opens Problems shows its card's row in either case. Focus goes to its first action (fix, second fix, **Mark as intended**). With a disabled or missing action, the row takes focus unless Enter or Space would answer it. Then nothing takes focus. <!-- D:JV1,JV2,JV5 -->

## Hiding kinds of row

**Kinds ▾** hides kinds of warning and info row for this project, such as every missing specular. Blocks and questions can't be hidden, and neither can a queued note. Hiding changes what the list and the tab's count show; a hidden row still counts for Ready. <!-- D:JO6,HC4 -->

- **The tab** counts hidden problem rows apart: `Problems (12 · 60 hidden)`. A hidden note counts nowhere. <!-- D:JO6 -->
- **The list's first line** says what is hidden, with the project's numbers: "2 kinds hidden in this project (60 rows). Hidden rows still count for Ready." <!-- D:JO6 -->
- **Kinds ▾ (n hidden)** counts hidden rows in this list, including notes. <!-- D:JO6 -->
- Restore kinds in **Kinds ▾** or with [**+k more in Problems**](GUI-Set-Editor.md#is-the-set-ready).
