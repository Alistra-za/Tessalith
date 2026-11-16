# CS PBR records and .csm materials

For TruePBR authors on Community Shaders 2.0: the `.csm` material file each PBR set exports, Community Shaders' own PBR texture-set and material-object records, and what to do when a set's `.csm` isn't written. PBR Landscape, Advanced skin, grass and XPMF are on [PBR Landscape, Advanced skin, grass and XPMF](PBR-Landscape.md).

## Get a set's .csm into the game

1. Use the `cs` target. Every PBR set except PBR Landscape writes a `.csm`, PBR grass included. <!-- D:IE10 -->
2. Export. After a clean export, Tessalith writes `materials\<output path>.csm` into the export folder. The set `architecture/stone/cobble`, whose textures go to `textures\pbr\architecture\stone\cobble…`, gets `materials\pbr\architecture\stone\cobble.csm`. Review lists it under **What will be written**.
3. Point the meshes at it: a shape uses a `.csm` when its material name field holds the `.csm` path. In the NIF editor that's **Use the set's .csm**, or **Convert to .csm** for a v1 PBR shape ([Point a shape at a .csm](GUI-NIF-Edit.md#point-a-shape-at-a-csm)).

PGPatcher 2.3.1 writes v1 PBR into the meshes it patches, and Community Shaders 2.0 draws v1 PBR as vanilla. So a set that gets a `.csm` shows, in the set editor's head, "PGPatcher 2.3.1 writes v1 PBR, which CS 2.0 draws vanilla. A PGPatcher with .csm support is needed for these meshes in game." Until such a PGPatcher exists, a mesh draws the set's material only through one of two routes: its name field names the `.csm`, or its texture set record is one the set's PBR texture set file binds (that file holds `{"csm": …}`, [CS PBR records](#cs-pbr-records)). <!-- D:IE13 -->

## What the .csm holds

The cobble set above, with a specular level set in its texture-set block, writes:

```json
{
  "Version": 1,
  "DiffuseTexture": "pbr/architecture/stone/cobble.dds",
  "NormalTexture": "pbr/architecture/stone/cobble_n.dds",
  "DisplacementTexture": "pbr/architecture/stone/cobble_p.dds",
  "RmaosTexture": "pbr/architecture/stone/cobble_rmaos.dds",
  "PBREnabled": true,
  "PBRSpecularLevel": 0.04
}
```

Notice: texture paths are relative to `textures` and use `/`, as the `.csm` format expects.

- **Textures** are the ones the set's PGPatcher entry names. A locked slot is left out, so the mesh keeps its own texture there; a cleared slot is written as `""`. <!-- D:IE10 -->
- **Switches** come from the set's PGPatcher settings (subsurface, foliage, hair, Multilayer and its coat options, fuzz) and from its texture-set block (glint, thin surface, foliage, fuzz roughness from texture). The coat options count only with **Multilayer** on, because a `.csm` coat needs the two-layer feature.
- **Numbers** are only the ones the texture-set block holds, clamped to the `.csm` range with a Problems row. A colour is `#rrggbb`, so a value above 1.0 is written as 1.0. <!-- D:IE1,IE7 -->
- **Foliage** is written as `PBRFoliageEnabled`, a key Community Shaders hasn't defined yet. Tessalith writes it only when foliage is on, because a CS that rejects unknown keys would draw the file vanilla. <!-- D:IE1 -->
- `innerLayerDisplacementOffset` has no `.csm` key: its row is hidden on such a set, and a value other than the default gets a Problems row.

## Why is my set's .csm missing?

Each case is an amber Problems row naming the set. That set's `PBRTextureSets` file isn't written either, because it would point at a missing file; the rest of the export goes on. <!-- D:IE10 -->

- **"no .csm written: the set has no RMAOS texture; Community Shaders PBR needs Diffuse, Normal and RMAOS"**, naming whichever of the three is missing. The slot is empty and not locked. Fill it, or lock it so the mesh keeps its own texture.
- **"no .csm written:" followed by a rule.** The switches break a `.csm` rule, for example hair beside another feature. Change the set's PGPatcher settings.
- **"the set writes no .csm this run (it has no PGPatcher entry)".** The `.csm` is built from the entry, so a set without one gets none ([Why is my set missing from the JSON?](GUI-PGPatcher-JSON.md#quick-qa)).

Each reason adds "a .csm and texture-set JSON from an earlier export stay in force in game until removed": Tessalith doesn't delete them, so remove them by hand if the game shouldn't use them. <!-- D:IE35 -->

The set editor's head shows the same reason in amber, and the set preview draws no `.csm` for that set. A blocked set's line ends "the shapes draw as PGPatcher 2.3.1 patches them (vanilla in CS 2.0)"; a set with no PGPatcher entry ends "PGPatcher has no entry for this set, so nothing patches its meshes; they keep the material their NIF names". <!-- D:IE34 -->

## Fuzz roughness from a texture

**Fuzz roughness from texture** reads the fuzz roughness from the green channel of the parallax (height) slot's texture, so that texture holds two channels: encode it BC5 or BC1, not BC4. <!-- D:ID46 --> With the switch on, the set's `_p` keeps the source's green channel and takes the height from red. <!-- D:IE16 --> A `channels.grayscale_source` other than `r` on that file isn't used, and Review lists a row saying so. <!-- D:IE36 -->

With the switch and **Fuzz** both on, a `_p` in a format with no green channel blocks Export: "<set>: Fuzz roughness texture needs two channels (height in R, fuzz roughness in G): use BC5 or BC1". **Use BC5** on the row sets the `_p` to BC5, as an undoable edit. <!-- D:IE18 -->

## CS PBR records

A set can write Community Shaders' own runtime files: a **PBR texture set** (`<export>\PBRTextureSets\<EDID>.json`) and a **PBR material object** (`<export>\PBRMaterialObjects\<EDID>.json`). On a landscape set they're on the [PBR Landscape card](PBR-Landscape.md#pbr-landscape-sets); on any other set, open **CS PBR records** in the Material tab and add either block. Adding a texture set to a set whose shader is **Auto** sets it to **PBR** in the same step, so it keeps its `.csm` instead of becoming PBR Landscape. <!-- D:IE25 -->

- **A PBR texture set** of a set that writes a `.csm` holds only the `.csm` path, `{"csm": "materials\\pbr\\architecture\\stone\\cobble.csm"}`; the values are in the `.csm`. A PBR Landscape set's file holds the block's values. <!-- D:IE10 -->
- **Template** fills a block from the built-in library: PBR landscape template values by Faultier (https://www.nexusmods.com/profile/Faultierfan?gameId=1704), from Faultier's PBR landscape modders resource. It never removes EDIDs you added.
- **EDIDs** are the records the file is for, and the file name is the EDID exactly, case included, because Community Shaders matches the exact case. After a plugin scan, texture-set EDIDs are suggested from the texture sets that use the diffuse, and material objects can be picked from the scanned records. An amber line flags an EDID that isn't in the scan; a material object that isn't single-pass is ignored by Community Shaders. An EDID that can't be a file name, or that two sets bind, isn't written.
- A texture set also drives the set's PGPatcher entry (roughness, specular, displacement, subsurface, coat and glint come from the block, so the two files can't drift). With no scan the entry isn't written. After a scan it's written whether or not a mesh (not LOD) uses the diffuse yet, and an info row says when none does; **PGPatcher entry for statics** forces it on or off.
- **IMPORTANT** A record or `.csm` you edited outside Tessalith is never overwritten: the export report says so, and you delete it to let Tessalith write it again. A file Tessalith wrote that no set writes any more is reported, not deleted. For a `.csm`, the note adds that meshes may still name the old path: **Use the set's .csm** in the NIF editor points a mesh at its set's new one. <!-- D:IE35 -->

### Material object layer

**Material object layer** in the 3D preview draws the snow or ash layer the game puts on statics that use the object, from the block's values and the record's DATA in your load order. Coverage goes on most non-skinned shapes; the block's material values (colour scale, roughness, specular, glint) change PBR shapes only. <!-- D:GI10,GZ1,IE23 --> When the set has an [XPMF profile](PBR-Landscape.md#xpmf-profiles), the layer takes the profile's four texture paths, `falloffScale`, `falloffBias` and `noiseUVScale` in place of the load order's. <!-- D:GI17 -->

### CS PBR 2.0 keys

A texture set block has eight rows beyond the CS 1.x ones. A PBR Landscape set writes the ones that hold a value into its `PBRTextureSets` file; any other PBR set writes them into its `.csm`. <!-- D:ID44 --> On a set that writes a `.csm`, every number row drags within the `.csm` range, and its hover names the `.csm` key. <!-- D:IE11 --> The PBR Landscape card hides the eight rows like its other unread rows: while **Also write into the PGPatcher JSON** is off and a row holds no value, because CS's landscape shader doesn't read them. <!-- D:ID47 -->

| Row | Texture-set key | `.csm` key | Texture-set value |
|---|---|---|---|
| **Subsurface balance** | `subsurfaceBalance` | `PBRThinSurfaceSubsurfaceBalance` | 0 to 1, default 0.75 |
| **Thin surface** | `thinSurface` | `PBRThinSurfaceEnabled` | true or false, default false |
| **Foliage** | `foliage` | `PBRFoliageEnabled` | true or false, default false |
| **Thin-surface specular weight** | `thinSurfaceSpecularWeight` | `PBRThinSurfaceSpecularWeight` | 0 to 1, default 0.5 |
| **Thin-surface back specular weight** | `thinSurfaceBackSpecularWeight` | `PBRThinSurfaceBackSpecularWeight` | 0 to 1, default 0.5 |
| **Thin-surface back roughness scale** | `thinSurfaceBackRoughnessScale` | `PBRThinSurfaceBackRoughnessScale` | 0 to 3, default 1 |
| **Fuzz roughness** | `fuzzRoughness` | `PBRFuzzRoughness` | -1 to 1, default -1; a negative value uses the base roughness |
| **Fuzz roughness from texture** | `fuzzRoughnessTexture` | `PBRFuzzRoughnessTexture` | true or false, default false; see [Fuzz roughness from a texture](#fuzz-roughness-from-a-texture) |

Next: check the material on a mesh in [Previews](Previews.md#csm-materials-in-3d).
