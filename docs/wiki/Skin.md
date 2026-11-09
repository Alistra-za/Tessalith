# Skin and Advanced skin

## Model-space normals (MSN)

An `_msn` stores each normal in the model's own frame instead of relative to the surface. Vanilla skin and eyes use it. It only fits the mesh and UVs it was baked for, and it handles deformation badly, so I'd pick a tangent-space `_n` for anything morphed (RaceMenu bodies and faces) and keep `_msn` for meshes that never deform.

- **Axis layout.** Skyrim reads R as X, G as Z and B as Y. A bake from another tool needs a swap or an invert to match, which the next section covers.
- **Naming.** `_msn` is typed `msn` and wins over `_n`. The Eye shader re-types an eye's `_n` to `msn`. It goes in texture-set slot 2 (normal).
- **The `_s` companion.** An `_s` in a set with `_msn` is `msn_specular`: BC4, red only, in slot 8. The game reads its red as glossiness. An `_msn` without an `_s` gives `SET-MISSING-COMPANION` (info), except under Eye.
- **The NIF flag.** The mesh needs the `Model_Space_Normals` flag (except for eye envmap), or the game reads the map as tangent space. The [NIF editor](GUI-NIF-Edit.md) names slot 8 "Model-space specular" when it's set.
- **Encoding.** BC7, alpha removed, no green flip, no Z rebuild: Tessalith keeps the vectors as you baked them.

### Baking an MSN for Skyrim

Tell Tessalith which baker made the map and it makes the change when it encodes. The **Baked in** row appears on an `_msn` file and is saved per file (key `channels.msn_source`, see [Project config](Project-Config.md#channels)). Unset means the map is already Skyrim MSN and is stored as it is, the list is non exhaustive, I do not have access to every baking program, these are the ones I've looked at. <!-- D:IA2 -->

| Source | Change to reach Skyrim MSN | **Baked in** |
|---|---|---|
| Blender object-space bake | Swap G and B | Blender object-space (swap G and B) |
| Marmoset default (right-handed) | Invert B | Marmoset right-handed or Substance (invert B) |
| Substance (DirectX and OpenGL give the same MSN) | Invert B | Marmoset right-handed or Substance (invert B) |
| Marmoset left-handed | Invert G and B | Marmoset left-handed (invert G and B) |
| Already Skyrim MSN | None | Skyrim MSN (no change) |

Tessalith doesn't detect the baker or warn about a wrong choice: a wrong guess would silently ruin the map, so it applies exactly what you pick.

This rest is for authors of skin textures for Community Shaders. It covers Advanced skin sets, the `.csskin.json` they write, and skin variants that share maps.

## The Advanced skin card

The set editor shows an **Advanced skin (Community Shaders)** card instead of the TruePBR settings: your skin textures, then the Community Shaders skin settings.

![The Advanced skin card of a body set.](../img/gui/Skin/skin-card.png)

A setting you haven't touched is not set and stores nothing, so Community Shaders uses its own default. Presets, defaults and rules never fill these, and sets don't inherit from each other, because a skin material is one author's decision for one body. Bulk edit doesn't touch them. A value outside its range blocks the export with a red Problems row; it is never clamped.

## The .csskin.json file

Every export writes `<diffuse>.csskin.json` beside the set's exported diffuse: the values you set, the paths of its `_rfaos`, `_wet` and `_detail_n`, and the diffuse's size and hash. A card you never touched writes no values. There is no button and no opt-out. <!-- D:HB1 -->

- **A set with no diffuse, or a borrowed one, writes no file** and gets an amber row. Community Shaders keys a material on the diffuse file, so only the set that exports the diffuse writes it. <!-- D:HB2 -->
- **A texture a slot doesn't export is left out** of the file, with an amber row.
- **A grey `_wet` isn't exported.** Community Shaders doesn't read grey height _wet maps anymore; use a normal with a mask in alpha. The set's other files still write (`SKIN-WET-GREY`). <!-- D:HB3 -->
- **A borrowed `_n` isn't named in the file.** The game reads the normal the mesh names, so point the mesh at the host's `_n`. The `_rfaos` still bakes from the host's `_n`. <!-- D:HC7 -->
- **IMPORTANT** An edited `.csskin.json` is always kept, and Export says so. Delete it to let Tessalith write it again. A file no skin set writes any more is left in place and listed in the export notes.

## Sharing maps between skin variants

A skin set with no colour map, holding only `_n`, `_sk`, `_wet` and `_detail_n`, is a shared-map set: other skin sets borrow its maps. Variants then share one `_n` and `_sk` and keep their own diffuse and `_rfaos`. Only skin sets can borrow the wet and detail slots, and a borrowed slot's menu offers **Clear**. Mapping a set to a target and the slot states are on [the Set editor](GUI-Set-Editor.md#mapping-to-a-target).

To try a skin on a mesh, use the **Advanced Skin preview** card on [the Meshes view](Mod-Scanning.md#shape-cards).


