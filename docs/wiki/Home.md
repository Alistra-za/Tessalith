# Tessalith wiki

For mod and modlist authors who make new DDS textures for Skyrim SE/AE and want them to come out right the first time. Tessalith is Windows only. It ships as `tessalith.exe` (the GUI) and `sdc` (the command line); both run one engine and read one `sdc.toml` per project, so they write the same bytes.

It builds for two targets: `cs` (the default) for Community Shaders including TruePBR, and `vanilla` for textures that load without it. [Presets and targets](Presets-and-Targets.md) says which to pick.

## I want to…

- **Convert my first set:** [Getting started](Getting-Started.md).
- **Name my files so they're picked up:** [Texture types and naming](Texture-Types.md).
- **Pack loose PBR maps, or use helper textures:** [Packing PBR maps and helpers](Texture-Packing.md).
- **Pick a target, or make smaller files:** [Presets and targets](Presets-and-Targets.md).
- **Ship PBR, Vanilla and Complex material from one source:** [Set shaders and Also export](Set-Shaders.md).
- **Write the PGPatcher JSON:** [PGPatcher settings and the JSON](GUI-PGPatcher-JSON.md).
- **Set up Advanced skin, landscape or grass:** [Skin and Advanced skin](Skin.md), [PBR Landscape, grass, XPMF and CS PBR records](PBR-Landscape.md).
- **Make a cubemap:** [Cubemaps](Cubemaps.md).
- **Check the result before release:** [Previews](Previews.md), [Validation and output](Validation-and-Output.md).
- **Learn the window:** [GUI guide](GUI-Guide.md).
- **Scan my mods, edit a NIF, animate one:** [Meshes and mod scanning](Mod-Scanning.md), [Editing a NIF](GUI-NIF-Edit.md), [NIF animation](GUI-NIF-Animation.md).
- **Script it:** [CLI reference](CLI-Reference.md). Every `sdc.toml` key is on [Project config](Project-Config.md).
- **Fix something that went wrong:** [Troubleshooting](Troubleshooting.md). A code in a Problems row or the `sdc` log is on [Diagnostic codes](Diagnostic-Codes.md).
- **Know why a default is what it is:** [Research behind the defaults](Research.md).

## Names you'll see

The CLI and the files keep the working name `sdc`. It stayed in the file names so existing projects keep loading.

| You see | What it is |
|---|---|
| `sdc` / `sdc.exe` | The command-line tool. |
| `tessalith.exe` | The desktop application. |
| `sdc.toml` | The project config. The GUI writes it and the CLI reads it. |

Settings, caches and logs: [where Tessalith keeps its files](Installation.md#where-tessalith-keeps-its-files).

## Words the window uses

- **Set:** the textures of one material. They share a base name, and under `cs` one PGPatcher JSON entry.
- **Helper:** a source file that fills a channel of another output. It's never written as a DDS.
- **Undecided:** a texture waiting for your answer. Export skips it until you give one.
- **Not ready:** a set with a problem in itself or in one of its textures.

## Licence

GPL-3.0-or-later. The engine (the `engine` and `encoders-sys` crates) is MPL-2.0, and the vendored encoders keep their own licences: see [THIRD_PARTY_NOTICES.md](https://github.com/Alistra-za/Tessalith/blob/main/THIRD_PARTY_NOTICES.md).
