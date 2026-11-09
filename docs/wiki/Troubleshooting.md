# Troubleshooting

For anyone whose conversion skipped a file, looks wrong in game, or whose GUI won't cooperate. Problems are listed by what you see. A code from a Problems row or the `sdc` log is on [Diagnostic codes](Diagnostic-Codes.md), `sdc` error messages are on [CLI reference](CLI-Reference.md#error-messages), and scan errors are on [Mod scanning](Mod-Scanning.md#scan-errors).

## Files are skipped or wait for you

### Every file is "unresolved" on the command line

```text
skip  C:/src/rock.png: SUG-LOW-CONFIDENCE sub-type 'diffuse' is a High-confidence suggestion (colour member, non-PBR set)
```

`sdc` doesn't apply suggestions unless you ask. Add `--accept-suggestions high`, or record the sub-types in `sdc.toml`. The GUI accepts High-confidence suggestions itself ([why the flag matters](CLI-Reference.md#why-you-usually-want---accept-suggestions-high)). A file that says `unknown suffix` is a different case: the flag doesn't help, and [Texture types](Texture-Types.md#how-files-form-a-set) says why.

### A colour map asks what its alpha means

The mesh decides whether a varying alpha is a cutout, a blend or unused, so the image alone can't answer. Set `alpha.policy` on the file, use the alpha dropdown, or scan your mods so the meshes answer ([Alpha from the meshes](GUI-Texture-Panel.md#alpha-from-the-meshes)).

### A normal map waits for a type

A lone `rock_nm.png` has no suffix Tessalith knows, so it waits instead of becoming a colour map. Rename it `_n`, or give it a normal sub-type ([how](Texture-Types.md#suffixes)).

## It looks wrong in game

### A texture is black under torchlight

Look for `CHK-GREEN-GUESS` on its normal map: the ⓘ badge on the set card, or the texture's settings. A normal map whose green points the wrong way lights from the wrong side. The check reads the map's slopes and says it looks OpenGL; it's a hint, and weak on straight ridges, so look at the map too. If it's right, press **Treat as OpenGL (flip green)** or set `channels.green_convention = "opengl"` on the file. A `CHK-Z-SIGN` row on the same map means its blue channel is flipped as well.

### Dark seams at a distance

Look for `CHK-UV-GUTTER`. UV islands closer than 8 texels let a smaller mip blend one island's colour into its neighbour, which shows as dark or bright seams from far away. Move the islands apart in the mesh, or ignore it for a tightly packed atlas that looks fine in game. The check needs a Meshes scan ([UV coverage](Texture-Types.md#uv-coverage)).

## Packing and helpers

### A PBR set isn't packed, or its raw maps are unused

Raw maps are packed only under the target that reads them. Under `vanilla` only metallic or specular builds an `_m`; roughness, AO and the rest are `PACK-UNUSED-INPUT`. Check the target ([packed maps per target](Texture-Packing.md)).

### My opacity map isn't used

The Helpers row says "not used" with the reason, and the `sdc` log gives the same reason as `PACK-UNUSED-INPUT`. Each reason and its fix: [Which data wins](Texture-Packing.md#which-data-wins).

## The GUI

### The GUI is read-only

`sdc.toml` failed to load or was rejected. The banner names the problem: fix the file in a text editor and press **Reload**. If `sdc-project.toml` failed, only sets and the JSON are read-only; its red Problems row names the line and has **Open in editor** and **Reload**. A save refused because the file has Windows' read-only flag leaves a row with **Clear read-only flag**.

### The GUI won't start

It needs the Windows font Segoe UI and says so when it's missing ([what you need](Installation.md#what-you-need)). The previews need a GPU; without one they're off and everything else works.

### The 3D preview shows plain textures, or PBR grass looks different

The PGPatcher simulation needs the `cs` target and a PBR set, and PBR grass is drawn with the plain PBR shader. Both are on [What the 3D view assumes](Previews.md#what-the-3d-view-assumes). The exported files aren't affected.

## Crashes and recovery

### A background job failed

An "internal error in …" dialog appears and the window keeps running. A failed plan or export shows its error in a dialog and in the status line.

### After a crash

The error goes to `%LOCALAPPDATA%\sdc\logs\crash-<date>-<time>.log`, and your unsaved edits are written beside their files as `sdc.toml.recovery` and `sdc-project.toml.recovery`. The originals aren't touched. When you open the folder again the edits come back as one Undo step, "Restore crash edits", with a toast. <!-- D:FB5 --> If a file doesn't parse, its `.recovery` stays and the banner says where.

### The graphics device was lost

The log has a "wgpu device lost" line. It isn't a crash: a message appears with **Restart**, and Tessalith reopens the same folder with your edits restored.

## Reporting a problem

Open an issue on [GitHub](https://github.com/Alistra-za/Tessalith/issues/new/choose); the bug form lists what to attach.

## Quick Q/A

**Will it re-encode my existing DDS files?** No, not by default: [Which files Tessalith reads](Texture-Types.md#which-files-tessalith-reads).

**Does the CLI use my GUI set edits or the scan?** No: [Does the CLI use my GUI edits?](GUI-Review-and-Export.md#does-the-cli-use-my-gui-edits)
