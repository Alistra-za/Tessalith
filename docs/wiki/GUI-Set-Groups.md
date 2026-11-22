# Set groups

For anyone with many sets who wants to fold the finished ones away. Making and editing the sets themselves is on the [Sets column](GUI-Sets-Column.md).

## Folding finished sets into a group

Fold the sets you've finished into a group, so the column shows only what you're working on. A group is a header row with its name, how many sets it holds and the worst status among them (red blocks export, amber isn't ready, green is ready), and its sets sit indented under it. <!-- D:HF6 -->

1. Right-click a set, or a selection, then **Move to group** → **New group…** and type a name.
2. Click the header to fold the group, and again to open it.

Names are unique, upper or lower case alike; a taken name stays in the dialog with the reason. A group is gone when its last set leaves it. Folding is not an undo step.

## Using a group

- **Join one:** **Move to group** → the group, or drag a set by its **≡** handle onto the header. Dragging a texture onto a header does nothing. <!-- D:HF8 -->
- **Header menu:** **Export group**, **Exclude from export** (**Include in export** takes it back; the header then carries an *Excluded* tag; both are explained in [Exporting part of the project](GUI-Review-and-Export.md#exporting-part-of-the-project)), **Rename…**, **Ungroup** (the sets stay) and **Select all sets**.
- **With a filter on,** a header shows only while a set in it passes, and reads `2 of 7`. A filter never folds or opens a group, and a set with no card isn't counted.
- **Revealing a set** unfolds its group if the set passes the filter. Selecting a different texture reveals its set. Clicking the same texture again leaves a group you folded by hand closed. **Go to set** in Problems reveals the set when the selection changes. **Move to…** reveals the moved set when it has files of its own. <!-- D:HF7 -->

## Where groups are saved

Set groups are one `[[groups]]` table each. `sets` holds set ids, like `[project] order`, and `excluded` is only there while it's `true`:

```toml
[[groups]]
name = "Armor"
sets = ["armor/steel", "armor/iron"]
excluded = true
```

A group sits where its first set sits in the column, and its other sets follow it, in order. A set with no card (a lone file waiting for a type, or a set whose files are all in no set) isn't shown or counted in its group, and being in a group never gives it a card. Whether a group is collapsed is kept per project in `gui.toml`, not in this file.

Next: [moving textures between sets](GUI-Sets-Column.md#moving-textures-between-sets).
