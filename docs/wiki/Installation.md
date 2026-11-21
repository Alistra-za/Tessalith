# Installation

If you make mods on Windows or Linux, you can run Tessalith from the Windows x64 zip. On Linux you'll need Wine. Building from source is in [CONTRIBUTING.md](https://github.com/Alistra-za/Tessalith/blob/main/CONTRIBUTING.md#build-from-source).

## Download and run

1. Download `Tessalith-<version>-win-x64.zip` from the [Releases page](https://github.com/Alistra-za/Tessalith/releases). `SHA256SUMS.txt` beside it hashes the release if you want to check it.
2. Unzip it anywhere. There's no installer, and nothing is registered with Windows.
3. On Windows, double-click `tessalith.exe` for the GUI, or open a terminal in the folder and run `sdc`. Add the folder to your `PATH` to run `sdc` from anywhere.

The exes aren't code-signed, so SmartScreen warns on the first run. Click **More info**, then **Run anyway**.

## What you need

- **Windows 10 or 11, x64.** On Linux, follow [Run Tessalith through Wine](#run-tessalith-through-wine-on-linux). There is no native Linux or macOS build.
- **Graphics for the previews.** Without a usable graphics device, the 2D and 3D previews are off and everything else works. The CLI needs none.
- **Fonts.** Tessalith uses Segoe UI, Segoe UI Symbol, Segoe UI Emoji and Consolas, which ship with Windows. Letters they don't have, such as Chinese, Japanese or Korean in file names, come from the other fonts installed on Windows. You can leave Segoe UI out of your Wine prefix. Tessalith can use Arial or a Linux font such as Noto Sans or DejaVu Sans. If it can't find a suitable font, it starts with built-in fonts and tells you in **Messages**. <!-- D:JM1,JM2,JM5 -->

## Run Tessalith through Wine on Linux

1. Install Wine 11.0 and [Winetricks](https://github.com/Winetricks/winetricks) on x86-64 Linux. The 3D preview needs a working Vulkan driver for DXVK.
2. In a terminal, create a separate 64-bit Wine prefix so the preview DLLs only affect Tessalith. Winetricks fetches `d3dcompiler_47.dll` from Microsoft's SDK and installs the 64-bit copy in `drive_c/windows/system32/` so the preview can compile Community Shaders.

   ```sh
   export WINEPREFIX="$HOME/.wine-tessalith"
   WINEARCH=win64 wineboot -u
   winetricks d3dcompiler_47
   ```

3. Get `dxvk-3.1.1.tar.gz` from the [DXVK 3.1.1 release](https://github.com/doitsujin/dxvk/releases/tag/v3.1.1) and unpack it. In the same terminal, change to the extracted folder containing `x64/`. Copy the two Direct3D 11 DLLs into the prefix's 64-bit system folder.

   ```sh
   cp --remove-destination x64/d3d11.dll x64/dxgi.dll "$WINEPREFIX/drive_c/windows/system32/"
   ```

4. To launch, open a terminal in the unzipped Tessalith folder. Select the prefix and tell Wine to load the three native DLLs. Run the last line for the GUI, or use `wine ./sdc.exe --help` for the CLI.

   ```sh
   export WINEPREFIX="$HOME/.wine-tessalith"
   export WINEDLLOVERRIDES="d3d11,dxgi,d3dcompiler_47=n"
   wine ./tessalith.exe
   ```

On Mesa's software Vulkan driver, the GUI opened a two-set project and drew its textures on the 3D preview sphere. Of 352 renderer tests, 349 passed. The run didn't cover a physical GPU on Linux or an export from a real project. [Wine test results](../research/render3d-backends.md#what-wine-did-on-linux)

## Where Tessalith keeps its files

| Path | Holds | Safe to delete? |
|---|---|---|
| `%APPDATA%\sdc\config.toml` | Your user config: default target, preset and overrides for every project. | Yes, you lose those defaults. |
| `%APPDATA%\sdc\presets\*.toml` | Your own presets. Both programs load them on every run, and skip a file that fails to load. | Yes, you lose the presets. |
| `%APPDATA%\sdc\gui.toml` | Window and column layout, recent folders, the default export folder, Textures filters, 3D view controls, the scan source, the analysis cache limit. | Yes, the window and the cache limit reset. |
| `%LOCALAPPDATA%\sdc\scan.db` | The mesh and plugin scan. | Yes, with Tessalith closed. The next scan rebuilds it. |
| `%LOCALAPPDATA%\sdc\analysis-cache\` | What the GUI learned about each image, so a folder you opened before doesn't decode again. At start it is pruned to the **Analysis cache limit** in the [Settings tab](GUI-Main-Window.md#settings-tab) (Cache): 500 MB unless you change it, about 15,000 textures. The entries unused for longest go first, and a new limit applies the next time Tessalith starts. | Yes. |
| `%LOCALAPPDATA%\sdc\shader-cache\` | The 3D preview's compiled shaders. | Yes. |
| `%LOCALAPPDATA%\sdc\logs\` | GUI crash logs, `crash-<date>-<time>.log`. The CLI writes none. | Yes. |
| `<project>\sdc.toml` | The project config. You or the GUI write it. | It's your project: keep it. |
| `<project>\sdc-project.toml` | The GUI's set edits ([what's in it](GUI-Sets-Column.md#where-set-edits-are-saved)). | It's your project: keep it. |
| `sdc.toml.recovery`, `sdc-project.toml.recovery` | Unsaved edits after a crash ([how they come back](Troubleshooting.md#after-a-crash)). | Not until you've opened the folder again. |
| `<output>\_sdc\manifest.jsonl` | The record of every file written. | Keep it with its output: without it a re-run treats the DDS files as unknown and leaves them alone ([why](Validation-and-Output.md#re-runs-and-if-changed)). |

Tessalith never changes your source files. It writes the output folder, the files above, and its own settings. An input rename writes an `[inputs]` alias to `sdc.toml`; an output rename writes PGPatcher **Rename** or `sdc-project.toml`. Files on disk keep their names.

## Back up, move or reset

- **Back up or move to another PC:** copy `%APPDATA%\sdc`. That holds your settings and presets. A project travels with its folder (`sdc.toml` and `sdc-project.toml`).
- **Reset the window:** delete `gui.toml`.
- **Start the scan over:** delete `scan.db`.
- **Free disk space:** delete `analysis-cache` and `shader-cache`. Both refill as you work.

## Uninstall

Delete the folder you unzipped. If you want the settings gone too, delete `%APPDATA%\sdc` and `%LOCALAPPDATA%\sdc`.
