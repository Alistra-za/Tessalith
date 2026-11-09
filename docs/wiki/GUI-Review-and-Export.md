# Review and Export

For anyone ready to export: what **Export** does, why Review opened instead, what the Files tab shows, and which of your GUI edits `sdc convert` reproduces.

## The Export button

It reads **Review n files** while something is undecided, blocked or waiting, and **Export n files** in the accent colour when Review would write without skipping anything. <!-- D:FB1 --> Pending NIF edits add ` + k NIF edits`, and one that Export would skip shows as `(skip 1 NIF edit)`. While an export runs, the lower half of the button is a progress bar.

## What Export does

1. Saves `sdc.toml` and `sdc-project.toml`.
2. Opens [Review](#review) instead of exporting when something needs an answer.
3. Plans again if a source changed on disk (**Reload** in the Textures column re-reads just those files).
4. Opens the [Files tab](#the-files-tab).
5. Writes the PGPatcher JSON after a complete export ([Where the JSON goes](GUI-PGPatcher-JSON.md#where-the-json-goes)), then any NIF edits ([Editing a NIF](GUI-NIF-Edit.md)).

Export can't write into the folder it reads. With no export folder chosen, Review blocks: see [Export to](GUI-Main-Window.md#export-to). With **Also export** on, Review always opens and its **Export** writes three folders ([Set shaders and Also export](Set-Shaders.md#also-export-as-vanilla-and-cm)).

**IMPORTANT:** files an earlier export wrote that the project no longer produces stay in the export folder. Tessalith never deletes them.

Discarding edits, a crash or a read-only `sdc-project.toml` never makes the next export ask about Tessalith's own JSON. <!-- D:FC11 -->

## Exporting part of the project

Use these when you're tuning one set and don't want to wait for the whole mod, or when finished sets should stop being rewritten.

- **Export set** (a card's right-click menu), **Export n sets** (the menu of a selection) and **Export group** (a group header's menu) export only those sets. A set that borrows a slot from another set brings that set along whole, so the borrowed file is fresh. <!-- D:HF10 -->
- It runs like **Export**: at once when those sets are ready, otherwise through Review, whose columns then list only their questions and blockers and whose button reads **Export group Armor** (or **Export set …**). Other sets' problems don't hold it back; a project-wide blocker, like a missing export folder, still does. The scope ends when Review closes or the export ends, so the next **Export** is the whole project again.
- **Exclude from export** on a group header leaves its sets out of every whole-project **Export** until **Include in export**. **Export group** on an excluded group still exports it, and the group stays excluded. A set borrowing from an excluded set doesn't bring it in: the excluded set's file on disk is used as it is. <!-- D:HF11 -->
- Files of the sets left out stay exactly as they are in the export folder: Tessalith doesn't touch, rewrite or report them. Their per-set JSON files stay too, and the PGPatcher JSON keeps their entries ([Where the JSON goes](GUI-PGPatcher-JSON.md#where-the-json-goes)). <!-- D:HF12 -->
- **Also export** runs only on a whole-project export. A partial export writes the PBR folder only, and an excluded group gets no Vanilla or CM files either.

## Review

Review opens in the bottom panel, not a dialog, for a blocker, an undecided texture, a PGPatcher JSON edited outside Tessalith, or pending NIF edits. It doesn't lock the window: keep editing and its lists update. It never exports by itself, even when you decide the last texture, and Enter does nothing there. Its **Export** is the only way out.

![The Review tab.](../img/gui/GUI-Review-and-Export/review-tab.png)

- **Blockers** (red) have **Go to set** or their own button. A missing export folder, a missing preset (**Choose preset…**) and a read-only `sdc.toml` count as blockers. <!-- D:FP8 --> Export stays greyed with the reason ("Fix 2 blockers first").
- **Undecided** rows carry the same buttons as Problems, and **Accept all suggestions**: [Deciding from the list](GUI-Problems.md#deciding-from-the-list). Warnings aren't listed here; they are Problems rows and never hold back Export.
- **What will be written** counts the DDS files and gives each JSON a state. **NIF edits** lists each edited NIF; one that can't be exported is listed in amber with its reason and never holds back the textures. A cancelled export writes no NIF edits.
- **Export 20 files (skip 2 undecided)** exports the decided textures and never decides for you. **Close** cancels an export it had queued.

### Overwrite questions

A PGPatcher JSON edited outside Tessalith, or a NIF whose export path holds a file Tessalith didn't write, asks **Keep the edited file** or **Overwrite it**. Neither is preselected, and Review's **Export** stays off until each is answered. The answer is tied to the file as it is now and applies to the next export only. The Community Shaders PBR and complex-material JSON are never overwritten when edited outside; they are kept, with a note.

## The Files tab

The **Files** tab opens when Export starts, with one row per file and its state. **Failed only** and **Has warnings** narrow it, and **Show in Explorer** on a row selects the written output.

- **Save report…** writes the report as JSON Lines or Markdown.
- **Copy CLI command** copies the matching `sdc convert` line ([below](#does-the-cli-use-my-gui-edits)).
- **Notes** lists what the export did besides converting: a JSON written, not written or kept, borrowed-slot copies, files left in place. The Markdown report has the same lines. <!-- D:FN4 -->

When the export ends, one Info line counts the artist-check warnings.

## Does the CLI use my GUI edits?

Partly. **Copy CLI command** gives the `sdc convert` line for this export: the same folder with `--recursive`, the same output and thread count, and `--accept-suggestions high`. Export saved `sdc.toml` first, so the command writes the same manifest records and DDS bytes for everything `sdc.toml` decides. That includes the types and alpha meanings the GUI wrote from scan evidence.

| Stays in the GUI | Why it differs on the CLI |
|---|---|
| `sdc-project.toml`: moved files, created sets, borrows, set renames, output names, set shaders, texture set record mappings and fills | The CLI doesn't read it, so those sets come out differently: an `_rfaos` bakes without the shared normal, and a shared set's maps type as `normal` and `backlight_tint`. |
| The mod scan: mesh and grass evidence | The CLI doesn't read the scan database. A mesh **Remove** and the `_m` that Also export derives from meshes stay GUI-only. |
| The PGPatcher JSON | Only the GUI writes it, after a complete export. |
