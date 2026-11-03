# XPMF profiles

How a Landscape or PBR Landscape set writes an XPMF profile on Export, which keys it fills and how it checks them. For anyone making snow, ash or other projected-material landscape mods. The card that edits the profile is on [Set materials](GUI-Set-Materials.md#xpmf-profile-card).

## What XPMF is

XPMF is an SKSE plugin that reads JSON profiles from `Data\SKSE\Plugins\XPMF\*.json`. Each profile retargets single-pass material objects (MATO records) by EditorID pattern: it swaps the four global projected textures, sets the coverage values, and changes vertex colour and alpha at runtime (roof shelter, water fade). Tessalith writes the profiles; it does not edit plugins. Tessalith vendors the profile schema of XPMF 0.8.1 and checks every profile against it before writing. <!-- D:GI8 -->

## The file

Export writes one file per set that has an XPMF block: `SKSE\Plugins\XPMF\tessalith_<name>.json` under the export folder. <!-- D:GI2 -->

- `<name>` is the last `/` part of the set id, lower-cased, with every character outside `a-z`, `0-9`, `_` and `-` replaced by `_`. A set id that gives an empty name is refused.
- Tessalith never writes `snow.json` or `ash.json`, which are XPMF's own profiles: in MO2 a file of that name would hide them.
- Two sets whose ids give one name, for example `rocks/Snow` and `cliffs/snow`, are both refused: the export note names both and neither file is written.
- The file is written after a clean export, with the same guard as the CS PBR files. A file Tessalith did not write is kept and never replaced; Review shows it as foreign. A profile that no set writes any more is reported, not deleted. <!-- D:GI8 -->
- A profile that fails the schema (for example `shelterFade = 500`, `noiseUVScale = 0` or a string in `pbr`, typed by hand in `sdc-project.toml`) is not written. The export note quotes the schema error, and an unknown key in the block refuses the file with a note naming the key. <!-- D:GI8 -->
- A set with no `cs_mato` EDIDs and no typed `editorIds` writes nothing, because the schema requires `editorIds`. The note reads "<set>: no editorIds; XPMF profile not written". A set that is not a Landscape or PBR Landscape set is refused with "<set>: not a landscape set; XPMF profile not written". <!-- D:GI3 -->
- A profile that names a texture of this project that was not exported is not written either.
- **Also export with Vanilla ticked:** a PBR Landscape set's Projected noise and Detail normal maps are written to the Vanilla folder as well, and the Vanilla folder gets a copy of the profile with `pbr` set to false. The copy keeps the values you typed and `specularMult`, and its texture paths are the Vanilla folder's files (without the `pbr` folder). A set left out of Also, or one with a DDS file (which Also does not re-encode), gets none. <!-- D:GI27 -->

## What Tessalith fills

The block lives in `sdc-project.toml` as `[sets.xpmf]`. A key the block does not set takes the value below; every other key is left out of the file, so XPMF uses its own default. An empty text, list or path is also left out. <!-- D:GI7 -->

| Key | Filled with |
|---|---|
| `name` | The set id. |
| `editorIds` | The EDIDs of the set's `cs_mato` block, as exact names. The list takes wildcard patterns when you edit it. <!-- D:GI3 --> |
| `pbr` | On for a PBR Landscape set under the Community Shaders target, off otherwise. <!-- D:GI4 --> |
| `diffuseTexture`, `normalTexture` | The set's albedo (or diffuse) and normal outputs, as Data-relative paths. <!-- D:GI5 --> |
| `noiseTexture`, `detailNormalTexture` | The outputs of the set's [Projected noise and Detail normal](#projected-noise-and-detail-normal) slots when they are filled; otherwise left out, so XPMF keeps the game's texture. <!-- D:GI5 --> |

## The keys

Every key of the schema has a row on the card. <!-- D:GI1 -->

| Keys | Range and meaning |
|---|---|
| `editorIds`, `excludeEditorIds` | EditorID patterns of the material objects the profile applies to and skips. `editorIds` is required. |
| `pbr` | Whether the targeted records are PBR. |
| `diffuseTexture`, `normalTexture`, `noiseTexture`, `detailNormalTexture` | Data-relative paths of the four projected textures. |
| `falloffScale`, `falloffBias` | The projection's coverage falloff. No bounds. |
| `noiseUVScale` | Above 0. |
| `maxAngle` | 0 to 180. |
| `lodIdentifier`, `lodMaterial`, `lodMaterialHD` | Text that DynDOLOD reads; Tessalith passes it through. <!-- D:GI6 --> |
| `neutralizeVertexColors`, `neutralizeVertexColorsSkip`, `neutralizeVertexAlpha`, `neutralizeVertexAlphaSkip` | Neutralise vertex colour or alpha at runtime, except on the EDIDs in the `Skip` list. |
| `roofShelter`, `roofShelterSkip`, `roofShelterFixVertices`, `shelterFade`, `shelterRandomness` | Roof shelter. `shelterFade` and `shelterRandomness` are 0 to 128. |
| `waterFade`, `waterFadeRandomness` | Water fade. `waterFadeRandomness` is 0 to 64. |
| `specularMult` | 0 to 10. The card hides it, and the file omits it, while `pbr` is on, because XPMF ignores it on PBR. <!-- D:GI9 --> |
| `name`, `comment` | Labels. |

The card shows the value Tessalith fills (or XPMF's default) greyed until you override it. The 3D preview shows the texture and coverage keys; every other row says "Not shown in the preview" on hover, because XPMF does that work at runtime. <!-- D:GI1 --> What the preview does with the keys it shows is on [3D preview](GUI-Set-Preview-3D.md).

## Projected noise and detail normal

A Landscape or PBR Landscape set has two optional slots that feed the profile: **Projected noise** and **Detail normal**. They have no PGPatcher slot, so they are not in the PGPatcher JSON, and a file cannot be shared between sets. <!-- D:GI12,GI18 -->

- **Projected noise** is the sub-type `projected_noise`, suffix `_noise`. It is written as BC4 (one linear channel, full mips), half the size of BC7, because Community Shaders reads only its red channel. <!-- D:GI14 --> In a landscape set a `_noise` file is read as projected noise without asking; you can still set another type on the file. <!-- D:GI21 -->
- **Detail normal** is the sub-type `detail_normal`. In a landscape set a file with the suffix `_detail` or `_detailnorm` is the detail normal and is not blended into `_n`. A `_detail_mask` file stays an RNM layer (with the warning `RNM-MASK-ORPHAN`, because its base is missing), and `_detail2` still blends into `_n`. A file named with an explicit base (`rnm.target`, or a name prefix) stays an RNM layer. In any other set `_detail` is unchanged. <!-- D:GI13,GI22 -->
- **A set made only for a material object needs no `_rmaos`.** The projected layer samples only the noise, normal, detail and diffuse textures; roughness, metallic and specular come from the `cs_mato` JSON values. Albedo, `_n`, noise and detail normal export with no blocker and no warning. <!-- D:GI15 -->
- **Community Shaders does not draw the projected normal and detail samples in the build Tessalith was measured against.** They show without Community Shaders, or in a later version of it. The Detail normal slot says so on hover, and the slot still exports. <!-- D:GI16 -->

## Problems and Review

- With `pbr` on, each listed EDID without a wildcard that no `cs_mato` block of this project names gets an amber Problems row: "XPMF skips <EDID>: no PBRMaterialObjects JSON from this project". Only this project's blocks are checked; PBRMaterialObjects files from other mods are not scanned. <!-- D:GI4 -->
- Review's list of what will be written has a row per profile (new, replaced or foreign). A profile the project no longer makes is left in place with a note.
- Texture-Types has the two sub-types ([Texture types](Texture-Types.md)).
