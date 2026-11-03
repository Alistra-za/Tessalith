# Advanced drawer

The drawer at the right edge of the Textures view: settings that are not per texture, plus a few rarely used texture fields. For anyone switching live encode or the artist checks, or reaching a texture field the panel does not show.

## Advanced drawer

Click the strip at the right edge to open it. With a set selected in 2D, the same sections are the set editor's last tab, **Advanced**, and the open or closed choice is shared by the tab and the drawer. In 3D, with a texture selected, or with several sets selected, the drawer is at the right edge.

![The Advanced drawer with the preset inspector.](../img/gui/GUI-Advanced-Drawer/advanced-drawer.png)

| Section | Contents |
|---|---|
| Preset inspector | Headed *How {file}'s settings resolve* for the selected texture. Under it: the batch preset, and the user presets that loaded (with their files); with none loaded, a **Presets…** button opens the preset creator. Field names read in words (the key shows on hover); long values are cut to fit and show in full on hover. *Show only what differs from sdc-base* hides values equal to the base preset. Built-in presets are read-only. |
| Preview | Live encode. |
| Mipmaps | Levels for the selected texture. |
| Advanced texture fields | BC7 weighting ([BC7 weighting](GUI-Texture-Panel.md#bc7-weighting-against-bc3)), BC4/BC5 search radius (*Default*, *Maximum*, *Fast (preview)*), DDS header (Auto, DX10 extended header, Legacy FourCC), Resize, Half size. |
| Checks | The switches for the artist checks and **Check all UV coverage**. See [Artist checks](GUI-Artist-Checks.md#artist-checks). |
| Also export recipe | One box per `[export.legacy]` value of [Also export as Vanilla and CM](GUI-Also-Export.md#the-recipe). Starts closed; Review's link opens it. |
| Unmatched [files] entries | `[files]` entries that name no file under the current target, with **Remove**. |

The drawer edits one texture at a time, also with several selected.

**Resize** offers *None*, *Down to a power of two* and *Halve if larger than 512*:

- *Down to a power of two* shrinks each side to the largest power of two that fits: 1000×600 becomes 512×512.
- *Halve if larger than 512* halves both sides when the larger is above 512: 1024×512 becomes 512×256.

Results Tessalith reads in the background (output sizes, the *Used by meshes* list, the report save) can appear up to a quarter of a second after a foreground result.
