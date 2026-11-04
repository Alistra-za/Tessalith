# Known issues

Things you may run into in version 2.0.0, and what to do about them. For fixes to common errors, see [Troubleshooting](Troubleshooting.md).

## Crashes

**The graphics device can be lost.** The cause is not proven; it often happens after a long session, around alt-tab or screen capture. Tessalith shows a message that the device was lost and that unsaved edits were kept, then restarts or closes. Unsaved edits come back the next time you open the folder ([crash recovery](GUI-Settings.md#when-something-goes-wrong)). The crash log is in the logs folder ([where](Installation.md#where-tessalith-keeps-its-files)); attach it if you report the crash.

## Things that do not work yet

**The Advanced skin card does not export.** The card saves its values in the project, but nothing is written to Community Shaders. Its **Export JSON...** button is greyed out on purpose.

**PBR grass is approximated in the 3D preview.** The preview draws the PBR grass shader type with the normal PBR shader: no wind, no wrapped diffuse, no grass LOD. The exported files are not affected.

**Debug views depend on your Community Shaders build.** The 3D preview's **Debug view** follows the installed build, so a view may differ from the game or be missing.

## Behaviour that may surprise you

**Renaming a colour map's output path loses the shader suggestion.** The suggestion looks meshes up by the new path, and no mesh uses it. Pick the shader by hand.

## Reporting a problem

Open an issue on the Tessalith GitHub page. Include:

- The Tessalith version (the exe's file properties, or `sdc --version`).
- Your GPU and driver.
- Your Community Shaders and PGPatcher versions.
- The crash log, if the app crashed.
- The project's `sdc.toml`.
- What you did, what you expected and what happened.
