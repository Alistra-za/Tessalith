# Cubemap sources

Every kind of source a cubemap can be made from: six images, a cross, a strip, an existing cube DDS and the dynamic marker. For modders making environment maps; the rest of the topic is on [Cubemaps](Cubemaps.md).

## Ways to make one

| You have | What it looks like | How Tessalith knows |
|---|---|---|
| [Six images](#six-images) | six squares | file names end in a face suffix |
| [A cross](#crosses-and-strips) | a T-shaped net, 4:3 or 3:4 | from the shape |
| [A strip](#crosses-and-strips) | six squares in a row, 6:1 or 1:6 | from the shape |
| [A cube DDS](#an-existing-cube-dds) | a `.dds` cube | the file says so |
| [Nothing: a dynamic marker](#dynamic-marker) | no picture | `sdc cubemap-marker` |

A file is treated as a cubemap when its name ends in `_e` (for example `chrome_e.png`) or it sits under a `cubemaps` folder. Tessalith suggests the `cubemap` type for these with High confidence.

Only a crisp shape is a cube source: 4:3, 3:4, 6:1 or 1:6. A square or 2:1 image typed as a cube is Unresolved and skipped, with "not a cube source: {w}x{h} is not six faces, a cross or a strip. Change this file's sub-type." A panorama, mirror ball, angular map or photo has to be turned into six faces, a cross or a strip first; the sister program Texalith is meant for that.

You can set the layout yourself when the shape doesn't settle it:

```toml
[files."textures/cubemaps/studio_e.png"]
cubemap.layout = "hcross"   # faces, hcross, vcross, hstrip, vstrip
```

### Six images

Name them with the base name plus a face suffix:

```text
textures\cubemaps\
  chrome_e_posx.png   east
  chrome_e_negx.png   west
  chrome_e_posy.png   north
  chrome_e_negy.png   south
  chrome_e_posz.png   sky
  chrome_e_negz.png   ground
```

They become one `chrome_e.dds`. The six files are one set, so the shader check sees a single `_e`. A face tag on any other name is left alone: `rock_px.png` is its own texture.

Tessalith never guesses a face. These are errors (`CUBE-FACES`), and the faces are not converted:
- two files for one face;
- a missing face;
- a face that is not square;
- faces of different sizes;
- faces that resolve to different settings (one file has its own `[files]` entry, for example). Set things for all six with a rule or `[defaults]`, so they match.

### Crosses and strips

![Four layouts. Horizontal cross: +Y north on top; −X west, +Z sky, +X east, −Z ground across the middle; −Y south below. Vertical cross: the same, with −Z ground at the bottom, turned 180°. Horizontal strip and vertical strip: +X, −X, +Y, −Y, +Z, −Z in order.](../img/cube-layouts.svg)

The shape tells Tessalith the layout: 4:3 is a horizontal cross, 3:4 a vertical cross, 6:1 a horizontal strip and 1:6 a vertical strip. The match is exact. Cell positions are the ones texassemble uses.

Cells are read as stored faces: Tessalith never moves a face to another cell. In Skyrim, the sky goes in the **centre** cell of a cross (+Z), not the top one.

- A cross made by a tool that puts the sky on top shows the sky to the north in game. Rearrange the cells first, use six images, or export a cross or strip with the sky in the centre cell.
- Tools disagree on the vertical cross's bottom cell. Tessalith turns it 180° by default. If the ground comes out upside down, set `cubemap.vcross_negz_rotate = false`.
- A 4:3 photo named `_e` is not mistaken for a cross: a cross's empty cells must be flat, or the file is refused with a message pointing at `cubemap.layout`. A layout you set yourself is trusted.
- A layout that does not fit the image (`hstrip` on a 512×512 file) is `CUBE-FACES`.

### Face size

The face size defaults to one source cell rounded to a power of two, as for any texture ([Format and size](Project-Config-Fields.md#format-and-size)), at most 4096 ([`cubemap.face_size`](Project-Config-Fields.md#cubemap)). A 1536×1152 cross has 384-pixel cells and becomes 256. Larger values scale up. Tessalith notes `DIM-RESIZED` when the face differs from the cell. A cube is never checked for `DIM-NPOT`.

### An existing cube DDS

A cube DDS goes through the same size, filter, blur and format settings as any other source. Like any DDS, it is skipped (`IN-DDS-SKIPPED`) unless you set `reencode = true` on the file. That costs some quality, because the faces are decoded and encoded again.

- It is always read as its six mip-0 faces. `cubemap.layout` must be unset (or `hstrip`) for it.
- It can only be re-encoded as a cubemap.
- Cube arrays are refused.

### Dynamic marker

![Six one-pixel faces. +X holds a colour with roughness in alpha; +Y and every other face is black.](../img/cube-marker.svg)

Community Shaders and ENB read a 1×1 cube whose +Y is black as an instruction to reflect the live scene instead of a picture (a dynamic cubemap). In Community Shaders the +X pixel then sets the reflection's colour (RGB) and roughness (alpha). For ENB, PGPatcher already applies a dynamic cubemap with every feature ENB supports.

Write one from the command line:

```text
sdc cubemap-marker --color 200,200,210 --roughness 0.3 -o textures\cubemaps\steel_e.dds
```

- `--color` is written as raw code values. Community Shaders reads F0 as `(code/255)^1.6`.
- The colour sits on the +X face. +Y stays black, so the file is always a dynamic marker.
- `--roughness` is 0 to 1 and is stored in alpha as `min(round(r × 255), 254)`. Leave it out and alpha is 255, which gives Community Shaders' defaults (F0 1, roughness 1/7).

The file is 152 bytes ([CLI reference](CLI-Reference.md#sdc-cubemap-marker)). Tessalith never writes a picture cube at 1×1, so a real cube can't turn into a marker.
