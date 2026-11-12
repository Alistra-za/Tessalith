# Set shaders and Also export

For modders converting whole materials: which shader a set is made for, what maps it needs, and how to ship one PBR mod as Vanilla and Complex material (CM) versions too. Grass, landscape and skin cards have their own page: [Community Shaders materials](PBR-Landscape.md).

## Which maps each shader needs

Each set is made for one Skyrim shader. The shader says which maps the set should have and can change some members' types and settings. **vanilla** is whether the Vanilla target has the shader.

| Shader | Required maps | Optional maps | vanilla |
|---|---|---|---|
| `pbr` | albedo, `_n`, `_rmaos` | `_g` emissive, `_s` subsurface/coat, `_cnr`, `_f`, `_p` | no |
| `pbr_landscape` | albedo (or landscape diffuse), `_n` | `_rmaos`, `_p` | no |
| `pbr_grass` | albedo, `_n`, `_rmaos` | `_s` | no |
| `advanced_skin` | skin albedo, `_n`/`_msn`, `_rfaos` | `_sk`, `_wet` | no |
| `default` | diffuse, `_n` | `_b`/`_bl` | yes |
| `envmap` | diffuse, `_n` | `_m`, `_e` | yes |
| `complex_material` | diffuse, `_n`, complex `_m` | `_e` | yes |
| `glow` | diffuse, `_n`, `_g` | | yes |
| `parallax` | diffuse, `_n`, `_p` | | yes |
| `skin` | diffuse, `_n`/`_msn` | `_sk`, `_s` (with `_msn`) | yes |
| `hair` | diffuse, `_n` | `_flow` (`cs` only; vanilla hair doesn't read it) | yes |
| `eye` | diffuse, `_n` (as msn) | `_sk`, `_m`, `_e` | yes |
| `multilayer` | diffuse, `_n`, `_i` | `_m`, `_e`, `_b` | yes |
| `landscape` | diffuse, `_n` | | yes |
| `complex_grass` | the complex-grass texture ([layout](PBR-Landscape.md#complex-grass)) | | yes |

- **A missing required map** gives `SHADER-MISSING-MAP`, and **a member the shader doesn't use** gives `SHADER-EXTRA-MAP`. Both are warnings; extra members still convert. The GUI drops the first when a scanned mesh or texture set record already fills the map beside the set's diffuse.

## How the shader is picked

Pick it yourself in the [set editor](GUI-Set-Editor.md#the-set-editor). Otherwise the first rule that fits decides, and `SHADER-AUTO` names it:

1. Advanced skin members or an `_rfaos`: `advanced_skin`.
2. A PBR set or PBR members: `pbr_grass` when a grass record uses the albedo (GUI, after a scan with plugins); under `cs`, `pbr_landscape` for a landscape set (GUI; [what counts](PBR-Landscape.md)); otherwise `pbr`.
3. A complex-grass member: `complex_grass`.
4. A complex `_m`: `complex_material`.
5. A `_p`: `parallax`.
6. In the GUI, after a mesh scan: the shader most of the meshes using the diffuse have (a strict majority).
7. An `_m` env mask: `envmap`.
8. `_g`: `glow`. `_i`: `multilayer`. `_sk`: `skin`. `_flow`: `hair`, on both targets, unless the path has a `water` or `landscape` folder.
9. A `hair` folder in the path: `hair`. An `eyes` folder: `eye`.
10. Landscape textures: `landscape`.
11. Otherwise `default`.

## What a shader changes in its members

| Shader | Effect on members |
|---|---|
| `hair` | Diffuse: `alpha.policy = "cutout_blend"` and `mips.coverage = "on"`. |
| `eye` | `_n` is treated as `msn` (no tangent space re-normalization). `_m` is BC4. |
| `landscape` | Colour becomes `landscape_diffuse` (alpha is the snow mask). |
| `pbr_landscape` | Nothing: members convert exactly as under `pbr`. |
| `multilayer` | Diffuse alpha is `data`. |
| `envmap` | `_m` is BC4. |

In the GUI, a format or tag the new type forbids is reset ([Texture settings](GUI-Texture-Panel.md#how-edits-are-saved)).

## Also export as Vanilla and CM

For a `cs` mod author shipping one PBR mod in three flavours. Each PBR set is converted from its albedo, `_n`, `_rmaos` (or raw roughness, metallic and AO maps) and `_p`; every other texture is copied from the PBR folder, or encoded when the copy would differ.

### Turn it on

- **GUI:** **Export to ▾** (**More ▾** with no project open) → **Also export as Vanilla** and **Also export as Complex material**, or the **Also export as** column in Review. While a tick is on, Export always opens Review.
- **Saved as** `[export] also` in the project `sdc.toml`.
- **Ticking only CM** still writes the Vanilla folder (for every other map): CM installs on top of it.

### What you get

Three folders side by side, where `<project>` is the opened folder's name:

- `<project> PBR`: the plain `cs` export, byte for byte, with the PGPatcher, CS PBR and complex-material JSON.
- `<project> Vanilla` and `<project> CM`: see the table. The multipliers are `[export.legacy]` keys: [names, defaults and ranges](Project-Config.md#export).

| Shader | Vanilla folder | CM folder |
|---|---|---|
| PBR | `<name>.dds` diffuse from the albedo, darkened by AO and by metal; `_n` = the normal, with specular alpha derived from gloss × AO, boosted with metal; vanilla envmask `_m` = metal × gloss, only exported when the meshes use the environment-map shader | `_m`: R (envmask) = gloss × metal, G (glossiness) = gloss (at least 5/255), B = metal, A = `_p` height (5/255–250/255; 0 without a `_p`). With **Keep metal brightness**, also a diffuse without the metal darkening |
| PBR Landscape | landscape diffuse with a lighter AO factor and no metal factor, alpha = the `_p`; landscape `_n` with alpha from gloss × AO; the set's `_p` (for Terrain Helper) | `_m` with R = B = 0 |
| PBR grass | the converted diffuse only | the complex-grass stack (diffuse on top, `_n` below) |

**Install** Vanilla alone, or CM on top of Vanilla. With keep metal brightness off, the CM folder holds only `_m` files (plus each PBR grass set's complex-grass stack).

### Where the folders go

In Review's **Also export as** column:

- **Also folders go in** (**Choose where the Also folders go…**) is `[project] also_location` in `sdc-project.toml`, by default the export folder.
- The three folder names: empty means the default name, shown as the hint.
- **Keep metal brightness** and its **Strength** (CM only).
- **Skip these sets**: tick a set to leave it out of Vanilla and CM. The set editor's Material tab has the same switch, **Skip vanilla/CM for this set**. Saved as `[export] skip`.

### Things that need care

- **The env `_m` needs a Meshes scan.** It is written only for sets whose meshes use the environment-map shader, which Tessalith learns from the scan.
- **`_curves`, `_curvature` and `_cavity` maps** join their PBR set and shade the diffuse: a soft light with the curves map (0.5 = no change), then a multiply with the cavity map, both at [`curves_opacity`](Project-Config.md#export). They are read only here; a plain export reports them as `PACK-UNUSED-INPUT`.
- **NIF edits follow the PBR folder**, so one mod folder holds the textures and the meshes.

### The recipe

**Advanced → Also export recipe** (closed at first; Review's **Recipe settings are in Advanced** opens it) has one box per `[export.legacy]` key. Keys, defaults and ranges are on [Project config](Project-Config.md#export).

### Files this export did not write

Tessalith never overwrites a file it didn't write, and never deletes one either. A leftover file in one of the folders (an earlier CM diffuse after Keep metal brightness went off, the output of a set you skipped since) stays, and the export notes list it as "not written by this export".

- **IMPORTANT:** delete such files yourself. A stale CM file overrides the Vanilla file of the same name in game.
- To start from scratch, delete the three folders together.
