# Installation

For anyone installing the release zip: what to download, what Windows needs, where Tessalith keeps its files, and how to back up, reset or remove it. Building from source is in [CONTRIBUTING.md](https://github.com/Alistra-za/Tessalith/blob/main/CONTRIBUTING.md#build-from-source).

## Download and run

1. Download `Tessalith-<version>-win-x64.zip` from the [Releases page](https://github.com/Alistra-za/Tessalith/releases). `SHA256SUMS.txt` beside it hashes the release if you want to check it.
2. Unzip it anywhere. There's no installer, and nothing is registered with Windows.
3. Double-click `tessalith.exe` for the GUI, or open a terminal in the folder and run `sdc`. Add the folder to your `PATH` to run `sdc` from anywhere.

The exes aren't code-signed, so SmartScreen warns on the first run. Click **More info**, then **Run anyway**.

## What you need

- **Windows 10 or 11, x64.** Linux and macOS don't work: the only build target is `x86_64-pc-windows-msvc`, and the 3D preview uses Direct3D 11 and Windows fonts.
- **A GPU for the previews.** Without one the 2D and 3D previews are off and everything else works. The CLI needs none.
- **Segoe UI.** `tessalith.exe` won't start without it, and says so. Segoe UI Symbol, Segoe UI Emoji and Consolas are used when present and skipped when not. All four ship with Windows; Tessalith ships no fonts.

## Where Tessalith keeps its files

| Path | Holds | Safe to delete? |
|---|---|---|
| `%APPDATA%\sdc\config.toml` | Your user config: default target, preset and overrides for every project. | Yes, you lose those defaults. |
| `%APPDATA%\sdc\presets\*.toml` | Your own presets. Both programs load them on every run, and skip a file that fails to load. | Yes, you lose the presets. |
| `%APPDATA%\sdc\gui.toml` | Window and column layout, recent folders, the default export folder, Textures filters, 3D view controls, the scan source, the analysis cache limit. | Yes, the window and the cache limit reset. |
| `%LOCALAPPDATA%\sdc\scan.db` | The mesh and plugin scan. | Yes, with Tessalith closed. The next scan rebuilds it. |
| `%LOCALAPPDATA%\sdc\analysis-cache\` | What the GUI learned about each image, so a folder you opened before doesn't decode again. At start it is pruned to the **Analysis cache limit** under Advanced → Cache: 500 MB unless you change it, about 15,000 textures. The entries unused for longest go first, and a new limit applies the next time Tessalith starts. | Yes. |
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
