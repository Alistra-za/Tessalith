# CLI reference

For anyone scripting `sdc`: batch conversions, CI, checking a downloaded mod. Everything about the command line is on this page: the quick start, every command and flag, what `sdc` reads and doesn't read from the GUI's files, exit codes, log lines and error messages. The other wiki pages describe the GUI and the concepts; they link here when the command line differs.

## Quick start

For a modder with a folder of source PNGs. It does the iron plate from [Getting started](Getting-Started.md#the-example-set) without the GUI.

### Look first

`sdc inspect` plans the whole run and prints what would happen. It encodes and writes nothing.

```powershell
cd C:\Work\IronPlate
sdc inspect C:\Work\IronPlate --recursive --accept-suggestions high
```

`--recursive` is there because the images sit several folders down. `--accept-suggestions high` applies High-confidence guesses the way the GUI does; the CLI default is `none`, which leaves every guessed file unresolved. Abridged, the output looks like this:

```text
set textures/pbr/armor/ironplate/ironplate  shader pbr (auto:pbr)
  pack textures/pbr/armor/ironplate/ironplate_rmaos.dds (rmaos) run
    R source:ironplate_roughness.png:R  G source:ironplate_metallic.png:R  B source:ironplate_ao.png:R  A default:1
C:\Work\IronPlate\textures\pbr\armor\ironplate\ironplate.png
  suggested pbr_albedo (high confidence: colour member of a PBR set)
  sub-type pbr_albedo  format bc7  tag srgb  alpha remove  output textures/pbr/armor/ironplate/ironplate.dds
  status ready
...
```

| Look for | It means |
|---|---|
| `status ready` | The file will be converted. `unresolved`, `blocked` or `error` means it won't, and the reasons follow. |
| `pack … run` | The packed output will be built. `conflict`, `failed` or `disabled` means it won't. |
| `warning` or `blocking` lines | A diagnostic code: look it up on [Diagnostic codes](Diagnostic-Codes.md). |

What the other line parts mean is under [sdc inspect](#sdc-inspect).

### Record decisions in sdc.toml

Anything you'd otherwise type as a `--set` flag every run belongs in `C:\Work\IronPlate\sdc.toml`:

```toml
schema_version = 1
target = "cs"

# This set has no specular map. Use 0.5 (128) instead of the default 1.0.
[files."textures/pbr/armor/ironplate/ironplate_rmaos"]
packing.fill.specular = 128
```

Notice: a packed output's key has no extension, and `[files]` keys are relative to the folder holding `sdc.toml`. `sdc` reads `sdc.toml` from the first input folder by itself. [Project config](Project-Config.md) has the cutout, OpenGL normal and `subtype` examples.

### Convert

```powershell
sdc convert C:\Work\IronPlate --recursive --accept-suggestions high -o C:\Work\IronPlate-out
```

Progress goes to stderr, one line per file, with forward slashes:

```text
ok    C:/Work/IronPlate/textures/pbr/armor/ironplate/ironplate.png -> C:/Work/IronPlate-out/textures/pbr/armor/ironplate/ironplate.dds
ok    C:/Work/IronPlate/textures/pbr/armor/ironplate/ironplate_n.png -> C:/Work/IronPlate-out/textures/pbr/armor/ironplate/ironplate_n.dds
ok    ... -> C:/Work/IronPlate-out/textures/pbr/armor/ironplate/ironplate_rmaos.dds
done: 3 converted, 0 up to date, 0 skipped (0 on purpose), 0 failed, 0 failed validation, 0 cancelled
```

The three raw maps get no line: the packed output consumed them. Without `-o`, output goes to `.\sdc-out` ([layout](Validation-and-Output.md#where-the-files-go)). Exit code `0` means every file converted and validated; the others are under [Exit codes](#exit-codes).

### Run it again

Run the same command and every file reads `up to date`. `--on-existing if-changed` is the default: it compares each output with its record in `C:\Work\IronPlate-out\_sdc\manifest.jsonl` and re-encodes only what changed. Edit `ironplate_roughness.png` and only the `_rmaos` rebuilds ([what counts as changed](Validation-and-Output.md#re-runs-and-if-changed)).

### A whole mod folder

Point `sdc convert --recursive` at the folder. Expect unresolved files on the first run ([why files wait](Texture-Types.md#how-files-form-a-set)). Answer them once in `sdc.toml`.

## Commands

| Command | What it does |
|---|---|
| [`convert`](#sdc-convert) | Convert images or folders to DDS. |
| [`inspect`](#sdc-inspect) | Show the plan for images, or the header and alpha statistics of DDS files. Writes nothing. |
| [`validate`](#sdc-validate) | Check existing DDS files against a target's rules. |
| [`presets`](#sdc-presets) | List and show presets; print JSON schemas. |
| [`cubemap-marker`](#sdc-cubemap-marker) | Write the 1×1 dynamic-cubemap marker DDS. |
| [`uv`](#sdc-uv) | Write an image of the UV islands of the meshes that use a texture. |
| [`dilate`](#sdc-dilate) | Fill the texels outside a `_matid` or `_uvmask` mask. |

- `sdc --version` prints the Tessalith version and the version of each linked encoder. Paste it into bug reports.
- `sdc --help` and `sdc <command> --help` print the built-in help.
- `convert` writes its log to stderr; `inspect`, `validate` and `presets` print their results to stdout; `uv` and `dilate` print one stdout line per written file and their errors to stderr. With `--json`, stdout carries only JSON.

## Flags shared by convert and inspect

| Flag | Default | Meaning |
|---|---|---|
| `<inputs>...` | required | Image files or folders. Several are allowed. |
| `--recursive` | off | Walk folders recursively; otherwise only the files directly inside each folder are read. Files you name are always included. |
| `--target cs\|vanilla` | see [Target and preset](#target-and-preset-from-the-command-line) | The target game setup. |
| `--preset <id or file.toml>` | see [Target and preset](#target-and-preset-from-the-command-line) | Select a preset by id, or load a preset file and select the id it declares. |
| `--preset-file <file>` | none | Load a preset file without selecting it, for example one that declares suffix aliases. Repeatable. Loading an id that is already loaded is an error. |
| `--config <file>` | `sdc.toml` in the first input folder | The project config file. |
| `--set <override>` | none | Override a profile field. Repeatable. See [--set syntax](#--set-syntax). |
| `--accept-suggestions none\|high` | `none` | `high` accepts every High-confidence sub-type suggestion. With `none`, a file whose sub-type is only suggested stays unresolved. High-confidence alpha suggestions apply either way. |
| `--allow <CODE>` | none | Acknowledge a blocking code for the whole batch. Repeatable. An unknown code is a usage error. See [Acknowledging blocking codes](#acknowledging-blocking-codes). |
| `--json` | off | Write JSON Lines events to stdout. Human output stays on stderr. |

The user config and the user presets always load ([where they are](Installation.md#where-tessalith-keeps-its-files)).

### Why you usually want `--accept-suggestions high`

A known suffix such as `_n` or `_rmaos`, a known exporter term such as `_albedo`, or no suffix at all (the colour map) gives a High-confidence sub-type suggestion. The CLI applies it only with `--accept-suggestions high`; otherwise the file is reported as unresolved:

```text
skip  C:/Work/rock/rock.png: SUG-LOW-CONFIDENCE sub-type 'diffuse' is a High-confidence suggestion (colour member, non-PBR set)
```

A lone file with an unknown suffix (`rock_bump.png`) has no suggestion, so the flag doesn't help: `sdc convert` exits 1 with `unknown suffix: assign a type with --set file:<path>.subtype=<type> or a [files] entry` ([why it waits](Texture-Types.md#how-files-form-a-set)). `--allow SUG-LOW-CONFIDENCE` doesn't convert it either. <!-- D:GY5 --> Give it a type with its path under the input folder:

```text
sdc convert --set "file:textures/x/rock_bump.png.subtype=diffuse"
```

The GUI accepts High suggestions and writes each into `sdc.toml` as a `[files]` entry, so a project saved from the GUI converts the same way here without the flag. What `sdc` doesn't read from the GUI (the mesh scan, the environment-map `_m`) is under [Does the CLI use my GUI edits?](#does-the-cli-use-my-gui-edits).

### --set syntax

| Form | Applies to | Can't set |
|---|---|---|
| `field=value` | Every file. | `helper.*` |
| `subtype:<id>.field=value` | Every file of that sub-type. | `subtype`, `reencode`, `helper.*` |
| `file:<glob>.field=value` | Files matching the glob ([glob rules](Project-Config.md#rules)). | |
| `set:<set id>.shader=<shader>` | A texture set's shader ([shader ids](Set-Shaders.md)). The set id is what `inspect` prints after `set`; an id with no `/` matches every set with that base name. An id that names no set gives `SET-EDIT-STALE`. | Anything but `shader` |

- Every profile field works with `--set` ([the field list](Project-Config.md#fields)), except `target`, `category` and `resize_reference`, which the planner alone writes. `subtype` takes the `file:` form only: the sub-type is decided per file.
- `helper.role`, `helper.fills`, `helper.read` and `helper.invert` take only the `file:` form: an unscoped `helper.role=ao` would make every image a helper.
- Values are TOML literals (`true`, `127`, `0.5`); anything else is a string, so `format=bc1` needs no quotes.
- A misspelt field or sub-type is an error, never ignored.
- A `--set` ranks above everything in `sdc.toml`, presets and suggestions ([layer order](Project-Config.md#how-settings-layer)).
- `reencode` takes the `file:` form only.

```powershell
# Every texture at fast effort
sdc convert src --recursive --accept-suggestions high --set encoder.effort=fast

# All diffuse maps as BC1 (a diffuse that keeps its alpha then raises FMT-BC1-ALPHA)
sdc convert src --recursive --accept-suggestions high --set subtype:diffuse.format=bc1

# One file is a cutout at threshold 100
sdc convert src --recursive --accept-suggestions high --set "file:**/fence01.png.alpha.policy=cutout" --set "file:**/fence01.png.alpha.threshold=100"

# Pick a set's shader
sdc convert src --recursive --accept-suggestions high --set "set:textures/armor/iron/iron.shader=envmap"
```

### Acknowledging blocking codes

A blocking code stops its file until you acknowledge it ([which codes](Diagnostic-Codes.md)). `--allow <CODE>` acknowledges it for the whole batch.

- `--allow CFG-UNAVAILABLE` converts a sub-type or format the target lacks as the fallback the message names. With no fallback, change the sub-type or the target ([which sub-types the Vanilla target lacks](Texture-Types.md#sub-types-what-each-file-becomes)).
- `--allow FMT-CS-SKIN-LOADER` does nothing: that code can't be lifted.
- `--allow SUG-LOW-CONFIDENCE` converts nothing that has no suggestion ([above](#why-you-usually-want---accept-suggestions-high)).

## Target and preset from the command line

Each flag is the first thing checked; `sdc.toml`, the user config and the defaults follow ([the chain](Presets-and-Targets.md#which-target)).

- `--target cs|vanilla` beats `target` in `sdc.toml`, the user config and the preset's own target.
- `--preset <id>` beats `preset` in `sdc.toml`, the user config and the target's default.
- `--target cpm` and `--preset cpm-quality` are errors ([why](Presets-and-Targets.md#which-target)).
- A preset named by `sdc.toml` that isn't installed makes `sdc convert` exit 2. In the GUI the same preset blocks the export.
- Load a preset file for one run with `--preset-file <file>` (load only) or `--preset <file.toml>` (load and select); copy it into `%APPDATA%\sdc\presets\` to keep it ([how to write one](Presets-and-Targets.md#writing-a-preset-by-hand)). A file given with `--preset-file` that fails to load is an error.
- `sdc presets list` shows each preset's suffix aliases, and keeps presets marked `hidden = true` out.
- `sdc presets --preset-file my-armor.toml show my-armor --subtype diffuse` shows what a preset resolves to for one sub-type, before you convert with it.

## sdc convert

```text
sdc convert [OPTIONS] <INPUTS>...
```

Plans the batch, then encodes every file whose status is ready.

| Flag | Default | Meaning |
|---|---|---|
| `-o, --output <dir>` | `.\sdc-out` | Output root ([layout](Validation-and-Output.md#where-the-files-go)). With `[export] also` in the project `sdc.toml`, the folder the PBR, Vanilla and CM folders go in. |
| `--jobs <n>` | `0` (all cores) | Worker threads. |
| `--mem-budget <size>` | 50% of RAM | Memory budget for files in flight. Bytes, or a `G`/`M` suffix (binary): `8G`, `512M`. |
| `--on-existing if-changed\|skip\|overwrite` | `if-changed` | What to do when an output exists ([below](#re-runs-and-if-changed)). |
| `--dry-run` | off | Plan and report, write nothing. |

The GUI always runs with the defaults (all cores, 50% of RAM). These two flags are the only way to change them.

### Also export as Vanilla / CM

- With `[export] also` in the project `sdc.toml` ([keys](Project-Config.md#export)), `sdc convert` makes `-o` the parent of `<project> PBR`, `<project> Vanilla` and `<project> CM`. `<project>` is the first input folder's name.
- It needs the `cs` target: `Also export needs the cs mode: the PBR folder is the cs export` (exit 2).
- A bad `[export.legacy]` value or folder name is an error (exit 2) on this path only.
- The CLI reads no mesh scan, so it writes no vanilla environment `_m`.
- On stderr it lists the files in the three folders this export didn't write ("not written by this export"), and each PBR set left out of Vanilla and CM, with the reason.
- What each folder holds: [Set shaders and Also export](Set-Shaders.md#also-export-as-vanilla-and-cm). The GUI's **Copy CLI command** passes the Also export location as `-o`. Skip sets with `[export] skip`, using the set ids `inspect` prints.

### Reading the log

Each line on stderr has one of these shapes:

```text
ok    <input> -> <output>
ok    <input> -> <output>  [VALIDATION FAILED]
skip  <input>: <CODE> <reason>
FAIL  <input> at <Stage>: <error>
warning <input>: <CODE> <message>
blocking <input>: <CODE> <message>
error <input>: <CODE> <message>
info  <input>: CFG-FILTER-IGNORED <message>
done: N converted, N up to date, N skipped (N on purpose), N failed, N failed validation, N cancelled
```

- Codes of severity warning and above are printed, plus the info code `CFG-FILTER-IGNORED`; other info codes are recorded but not printed. Paths use forward slashes.
- A set-level warning prints with the set id as its label: `warning textures/armor/iron/iron: SHADER-MISSING-MAP …`.
- Raw maps consumed by a packed output get no line of their own.
- A file with a validation error is still written. It prints as `ok ... [VALIDATION FAILED]`, counts under "failed validation", and makes `sdc convert` exit 4 if nothing worse happened.
- The CLI raises none of the `CHK-` artist checks; the engine measures `CHK-LOW-DETAIL` only when the GUI asks.

| `done` counter | Counts |
|---|---|
| converted | Files encoded and written in this run. |
| up to date | Outputs left alone by `if-changed`. |
| skipped | Files not converted for any other reason: unresolved, blocked, duplicate output, DDS input, existing output with no record, `--on-existing skip`, a pre-packed file replaced under `packing.prefer = "raw"` (`PACK-CONFLICT`), a generated file replaced by a real map (`GEN-REPLACED`). |
| on purpose | The part of skipped you asked for: DDS inputs not opted in (`IN-DDS-SKIPPED`), `--on-existing skip`, `PACK-CONFLICT` and `GEN-REPLACED`. |
| failed | Files that failed to load, resolve, encode or write. |
| failed validation | Files written whose validation reported an error. |
| cancelled | Files not started because you pressed Ctrl-C. |

### Re-runs and if-changed

`--on-existing` decides what happens to an output that already exists. The rules that make an output "up to date" are on [Validation and output](Validation-and-Output.md#re-runs-and-if-changed).

| `--on-existing` | Current record | Changed record | No record |
|---|---|---|---|
| `if-changed` (default) | kept (up to date) | re-encoded | kept, `OUT-UNKNOWN-EXISTING` |
| `skip` | kept | kept | kept |
| `overwrite` | re-encoded | re-encoded | re-encoded |

- An existing output with no manifest record is kept under `if-changed`, reported as `OUT-UNKNOWN-EXISTING`, and the run exits 1. `--on-existing overwrite` replaces it.

### Cancelling

- The first Ctrl-C prints `cancelling: finishing current bands... (Ctrl-C again to exit immediately)`. Running files stop at their next band boundary and no new file starts. Nothing half-written is left (outputs are written to a temporary file and renamed). The cancelled files are listed in `batch_done`, and the exit code is 1. A Ctrl-C during planning counts too.
- A second Ctrl-C exits at once with code 3. That can leave `.sdc-*.tmp` files in the output folders (delete them by hand) and, with `--json`, no `batch_done` event. A file renamed into place just before the abort may have no manifest record; the next run reports it as `OUT-UNKNOWN-EXISTING`, and `--on-existing overwrite` replaces it.

### Examples

```powershell
# Plain conversion of a mod's textures folder
sdc convert "C:\Mods\MyMod\textures" --recursive --accept-suggestions high -o "C:\Mods\MyMod-out"

# Vanilla output, smaller files
sdc convert src --recursive --accept-suggestions high --target vanilla --preset vanilla-compact

# A quick look at the result while you iterate
sdc convert src --recursive --accept-suggestions high --preset fast-preview -o preview-out

# Rebuild everything, 8 threads, 6 GiB budget
sdc convert src --recursive --accept-suggestions high --on-existing overwrite --jobs 8 --mem-budget 6G

# Acknowledge a texture under 4 pixels
sdc convert src --recursive --accept-suggestions high --allow DIM-SMALL
```

## sdc inspect

```text
sdc inspect [OPTIONS] <INPUTS>...
```

Takes the [shared flags](#flags-shared-by-convert-and-inspect), writes nothing, and treats a missing input as a usage error. A DDS input is always reported by its header (format, size, mips, header form, per-mip alpha) and never re-encoded, even with `reencode`; header problems are `header issue:` lines.

For image inputs it runs the whole plan as a dry run and prints, per set:

```text
set <set id>  shader <shader> (<why>)
  <severity> <CODE> <message>
  pack <output> (<sub-type>) run|conflict|failed|disabled[: reason]
    R <source>  G <source>  B <source>  A <source>
```

and per file:

```text
<input path>
  suggested <sub-type> (<confidence> confidence: <reason>)
  sub-type <id>  format <format>  tag <srgb|unorm|n/a>  alpha <policy>  output <relative output path>
  status <ready|unresolved|blocked|error|packing_input> <reasons or codes>
  <severity> <CODE> <message>
```

Skipped and failed files are the ones `convert` would report: the path, then an indented `skipped <CODE> <reason>` or `cannot be read: <error>` line. Exit codes: 0, 1 if an input couldn't be read, 2 for a usage or config error.

With `--json`, each field's provenance (which layer set it) is in the `file_planned` events ([JSON event stream](#json-event-stream)).

### Checking a config file

`sdc inspect <folder> --recursive > $null` stops with the error and names the problem, so it checks `sdc.toml` and the presets without writing anything. `sdc presets schema config` prints the JSON Schema for `sdc.toml` (the repository has it as `schema/config.schema.json`, for editor completion).

## sdc validate

```text
sdc validate [OPTIONS] <DDS>...
```

Runs the post-encode checks that need no source on DDS files that already exist, including files made by other tools: `VAL-HEADER`, `VAL-ALPHA-GUARANTEE`, the `VAL-M-*` checks, `VAL-PGPATCHER-50` and `VAL-GRASS-DETECT` ([what each checks](Validation-and-Output.md#what-is-checked)). Mip quality, cutout coverage and the BC6H peak need the encoder input, so `validate` can't run them. Give `.dds` files: a folder is refused (`<path> is a folder; give .dds files`).

| Flag | Default | Meaning |
|---|---|---|
| `--target cs\|vanilla` | `cs` | Whose rules apply. |
| `--subtype <id>` | inferred from the file's suffix, its game path and, for formats with alpha, its mip-0 alpha | What the files are meant to be. A sub-type the target doesn't have is an error. |
| `--alpha-policy remove\|cutout\|blend\|cutout_blend\|data` | inferred | The alpha meaning to check against. Inferred as `remove` when every mip-0 alpha value is at least 254. Otherwise the sub-type's default, except that a sub-type whose default is `remove` is inferred as `data`. |
| `--exact-opaque` | off | Hold BC7 files under `remove` to Tessalith's own guarantee that opaque alpha decodes as exactly 255. Off by default because other encoders don't promise it. |
| `--json` | off | One JSON object per file on stdout; the human report moves to stderr. |

The format and `_SRGB` tag are taken from the file, so third-party files are judged on header and alpha rules, not on Tessalith's preferred format. A format Tessalith never writes has only its header checked and is reported as not judged.

```text
ok    C:/out/rock.dds (diffuse, alpha remove)
FAIL  C:/out/rock_n.dds (normal)
  error VAL-HEADER ...
skip  C:/out/old.dds (diffuse)
  note: not judged: format not produced by sdc (Bc2)
validated 3: 1 ok, 1 failed validation, 0 unreadable, 1 not judged
```

Exit codes: 1 if any file couldn't be read, otherwise 4 if any failed, otherwise 0.

## sdc presets

```text
sdc presets [--preset-file <file>]... <COMMAND>
```

`--preset-file` loads extra preset files after the user presets folder. A user preset that fails to load is skipped with `CFG-PRESET-SKIPPED` on stderr.

| Command | What it does |
|---|---|
| `list [--json]` | Lists selectable presets: id, version, target, description, suffix aliases, and the source file for user presets. |
| `show <id> [--subtype <id>] [--target <t>]` | Prints the preset's resolved profile for each sub-type (or one) as JSON, with the provenance of each field. |
| `schema <which> [--out <dir>]` | Prints a JSON Schema: `profile`, `preset`, `config`, `event`, `manifest`, `dds-info`, `dds-validation`. `all` writes every schema into `--out`, which it requires. |

```powershell
sdc presets list
sdc presets show cs-quality --subtype diffuse
sdc presets schema all --out schemas
```

`sdc presets schema manifest` lists every field of a manifest record ([the manifest](Validation-and-Output.md#the-manifest)).

## sdc uv

```text
sdc uv <PATTERN> [--recursive] (--s N | --w W --l L) [--meshes <folder>] [--scan <db>] [--out <dir>] [--overwrite]
```

Draws where the meshes put a texture, so you can lay out and pad a texture before you paint it. It writes one 8-bit grayscale PNG per texture, `<name>_uvmask.png`, at the size you give. Use it as a layer in your image editor, or hand it to a dilation tool.

| Pixel | Meaning |
|---|---|
| 255 | An island: a texel a mesh triangle covers, even in part. |
| 127 | The recommended padding: within the padding distance of one island. |
| 191 | Too close: within the padding distance of two or more different islands. Their padding overlaps, so mips will bleed one into the other. |
| 0 | Nothing nearby. |

The padding is 1/256 of the longer side, at least 2 texels: 2 at 512, 4 at 1024, 8 at 2048, 16 at 4096 and at 2048×4096, 32 at 8192. Islands are groups of touching texels (corners count). Touching meshes, or two meshes over the same texels, are one island. UVs outside 0 to 1 wrap. The padding stops at the image edge: it doesn't wrap to the other side of a tiling texture.

| Argument | Meaning |
|---|---|
| `<PATTERN>` | A texture path as you'd type it: `\` or `/`, `textures\` and `.dds` optional, any case. `*` and `?` match inside one folder or file name. A folder matches the textures directly in it; `--recursive` takes every texture below it. |
| `--recursive` | With a folder pattern, take every texture below it, not only those directly in it. |
| `--s N` (`--size`) | The texture is N×N. 1 to 16384. |
| `--w W --l L` (`--width`, `--length`) | The texture is W wide and L long. Give both; not with `--s`. |
| `--meshes <folder>` | Read every `.nif` below this folder (subfolders included) instead of the scan. |
| `--scan <db>` | Read this scan database. Default: the one the GUI's **Meshes** scan writes. Not with `--meshes`. |
| `--out <dir>` | Where the images go. Default: the current folder. |
| `--overwrite` | Replace an existing image. Otherwise it is kept and reported. |

A mesh uses a texture when the path is in any slot of its lighting shader's texture set, or is the source texture of an effect shader. All maps of one shape share its UVs.

Without `--meshes`, `sdc uv` reads the scan database and takes only the meshes the game loads (the winning copy of each path, loose or in a BSA). Run the Meshes scan in the GUI first; without a database `sdc uv` stops and says so. With `--meshes`, the folder is read as it is and the scan isn't touched. A mesh it can't read, and a Skyrim LE mesh, is listed on stderr and skipped.

The images mirror the matched path below the folders of the pattern that come before its first `*` or `?`. `sdc uv Harry2135\FantasySeries6\FS6_Arrow_IR --s 2048` writes `fs6_arrow_ir_uvmask.png`; `sdc uv Harry2135 --recursive --s 2048` writes `fantasyseries6\fs6_arrow_ir_uvmask.png`. Each written file prints one line: its path, the island count, the padding, the number of 191 texels and the number of meshes read. A texture whose meshes have no usable triangles writes nothing and says so.

```powershell
sdc uv Harry2135\FantasySeries6\FS6_Arrow_IR --s 2048 --out uvmasks
sdc uv armor\* --recursive --w 2048 --l 4096 --meshes .\meshes
```

Exit codes: 0 every image written; 1 a texture wrote nothing (no UV triangles, or the file exists and `--overwrite` is missing); 2 a usage error: a bad size, a pattern that matches no texture, a missing scan database; 3 a write failed.

## sdc dilate

```text
sdc dilate <PATTERN>... [--recursive] [--mask <file>] [--out <dir>] [--overwrite]
```

Fills the texels outside a mask with colour pulled in from the texels inside it, so mips and filtering don't bleed the empty background into your islands. The masked texels are written back exactly as they were. Use it with the `_uvmask.png` that `sdc uv` writes, or with a `_matid` image.

| Argument | Meaning |
|---|---|
| `<PATTERN>` | An image file, a folder or a glob on disk (`*` and `?` match inside one name). A folder takes the PNG, TGA and TIFF files directly in it. Give several. |
| `--recursive` | With a folder, take the images below it too. |
| `--mask <file>` | Use this mask for every input. Default: the mask beside each input (below). |
| `--out <dir>` | Where the images go. Default: a `dilated` folder beside each input. |
| `--overwrite` | Replace an existing image. Otherwise it is kept and reported. |

The mask for `rock_n.png` is `rock_matid.png` (or `.tga`, `.tif`, `.tiff`), then `rock_uvmask.png`, then the same two for the full name `rock_n`. `rock` is the name without its texture suffix, the way Tessalith groups a set. An input with no mask is skipped and says so; a mask of another size than its image is an error for that input. In a `_uvmask` image only 255 counts as inside (127 and 191 are padding, and padding is what gets filled). In any other mask, a texel with red, green or blue above 0 is inside.

How the fill treats the map depends on the suffix, as Tessalith reads it. Colour maps are filled in linear light. Normal maps are filled as stored and the filled texels renormalised to unit length. Everything else is filled as stored, and so is a name Tessalith can't place (`rock_x`); the output line says so. Alpha is always filled as data.

Files named `*_matid.*` or `*_uvmask.*`, and any file in a folder named `dilated` below the file, folder or glob you name, are never inputs; a `dilated` folder above what you name doesn't count. Inputs are 8- and 16-bit PNG, TGA and TIFF files; other files a pattern matches are ignored, and a pattern that matches no image is named on stderr: exit 2 if none of the patterns match, exit 1 (the others still run) if only some do. A 32-bit float TIFF is refused. Two inputs that would write the same output file (`rock.png` and `rock.tga`, or one name from two folders with `--out`) are not both written: the first is, and the later ones are refused by name, even with `--overwrite`. The output is a PNG with the same name, bit depth and channels (grey, grey and alpha, RGB, RGBA), so `rock_n.png` stays `rock_n.png` and Tessalith still reads its suffix. Each written file prints one line: its path, the kind, the mask and the number of texels filled.

```powershell
sdc dilate textures\rock_n.png
sdc dilate textures\armor --recursive --mask uvmasks\armor_uvmask.png --out filled
```

Exit codes: 0 every image written; 1 an input wrote nothing (no mask, a mask of another size, a float image, a second input for the same output, the file exists and `--overwrite` is missing); 2 a usage error: no image matches; 3 a write failed.

## Cubemaps from the command line

Cube sources, layouts and `cubemap.*` keys are on [Cubemaps](Cubemaps.md). On the command line:

- Every `cubemap.*` key works with `--set`, for example `--set "file:**/chrome_e*.cubemap.style=vanilla_metal"`.
- A cube DDS is skipped like any DDS unless you set `reencode`.
- `sdc cubemap-marker` makes the dynamic marker.
- `sdc inspect` and `sdc validate` check cube files you already have.

### sdc cubemap-marker

```text
sdc cubemap-marker [--color <R,G,B>] [--roughness <0..1>] -o <file.dds>
```

Writes the 152-byte 1×1 dynamic-cubemap marker that Community Shaders and ENB read as "reflect the live scene" ([what it is](Cubemaps.md#dynamic-marker)). The bytes equal what the GUI's **Make marker cube…** writes.

| Flag | Default | Meaning |
|---|---|---|
| `--color <R,G,B>` | `0,0,0` | The reflection colour as raw 0 to 255 code values on the +X face. +Y stays black, so the file is always a marker. |
| `--roughness <0..1>` | none | Stored in alpha. Leave it out and Community Shaders uses its defaults. |
| `-o`, `--out <file>` | required | The DDS file to write. |

```powershell
sdc cubemap-marker --color 200,200,210 --roughness 0.3 -o textures\cubemaps\steel_e.dds
```

An existing file Tessalith didn't write, or can't read, is kept. Exit codes: 0 written; 1 kept (such an existing file); 2 usage error (a bad colour, a roughness outside 0 to 1); 3 the write failed.

### Checking cube files

`sdc inspect` and `sdc validate` flag broken cube files (as `VAL-HEADER` in `validate`, each failing the file):

- a cube wider than 1 pixel with only one mip (a 1×1 marker is fine);
- a flat 2D texture named `_e`, or under `textures\cubemaps\`, where a cube is expected: exit 4, because the game reads those files as cubes <!-- D:FM22 -->;
- a 1×1 cube whose +Y isn't black (Community Shaders shows a nearly black static reflection);
- a cube in an sRGB format.

`sdc validate` judges a cube file as a `cubemap` unless you pass `--subtype`. A marker passes.

## Packing and helpers from the command line

[Texture packing](Texture-Packing.md) explains what packs and what a helper is. On the command line:

- `--set packing.fill.metallic=255` fills a channel for a whole run. The GUI does it per output on the set editor's card.
- `--set "file:**/<name>.helper.role=opacity"` marks a file as a helper; `helper.*` takes the `file:` form only ([keys](Project-Config.md#files)). The same keys on a `[files]` entry do it without a flag.
- `--set "set:<id>.shader=complex_material"` makes the set build the complex `_m` from roughness, glossiness, metallic, height or `_orm` maps.
- A helper that no output uses shows in the log as `PACK-UNUSED-INPUT`, with the reason; each reason and its fix is under [Which data wins](Texture-Packing.md#which-data-wins).

## Set shaders from the command line

[Set shaders](Set-Shaders.md) lists the shaders and how the first rule that fits picks one. On the command line:

- Pick one with `--set "set:<id>.shader=<shader>"`.
- The CLI never picks **PBR grass** by itself, because that needs a scan that included plugins. Use `--set "set:<id>.shader=pbr_grass"`.
- The CLI never tags complex grass either: without `subtype = "complex_grass"` on the `[files]` entry, `sdc convert` treats the file as a diffuse.
- Nothing resets a format or tag the shader's new type forbids: your value stays, and the file is blocked or reports an error.

## Does the CLI use my GUI edits?

Partly. **Copy CLI command** (Files tab, [Review and Export](GUI-Review-and-Export.md#the-files-tab)) gives the `sdc convert` line for this export: the same folder with `--recursive`, the same output and thread count, and `--accept-suggestions high`. Export saved `sdc.toml` first, so the command writes the same manifest records and DDS bytes for everything `sdc.toml` decides. That includes the types and alpha meanings the GUI wrote from scan evidence, and the `[inputs]` aliases from an input rename. A GUI edit reaches `sdc convert` only after you **Save**.

| Stays in the GUI | Why it differs on the CLI |
|---|---|
| `sdc-project.toml`: moved files, created sets, borrows, set renames, output names, set shaders, texture set record mappings and fills | The CLI doesn't read it, so those sets come out differently: an `_rfaos` bakes without the shared normal, and a shared set's maps type as `normal` and `backlight_tint`. The `[project] target_mod` it holds isn't read either. |
| The mod scan: mesh and grass evidence | The CLI doesn't read the scan database. A mesh **Remove** and the `_m` that Also export derives from meshes stay GUI-only. |
| The PGPatcher JSON | Only the GUI writes it, after a complete export. `sdc` never does, so a set exported with `sdc` has no entry in it. |

## Troubleshooting on the command line

### Every file is "unresolved"

```text
skip  C:/src/rock.png: SUG-LOW-CONFIDENCE sub-type 'diffuse' is a High-confidence suggestion (colour member, non-PBR set)
```

`sdc` doesn't apply suggestions unless you ask. Add `--accept-suggestions high`, or record the sub-types in `sdc.toml`. The GUI accepts High-confidence suggestions itself ([why the flag matters](#why-you-usually-want---accept-suggestions-high)). A file that says `unknown suffix` is a different case: the flag doesn't help, and [Texture types](Texture-Types.md#how-files-form-a-set) says why.

### The run stops with `sdc: <message>`

That is an error that stops a run, exit 2: see [Error messages](#error-messages). A code on a `skip` or `warning` line is on [Diagnostic codes](Diagnostic-Codes.md).

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success. Every file converted (or was up to date or skipped on purpose) and passed validation. |
| 1 | Partial. Some files were unresolved, blocked, failed, cancelled, or had an existing output with no manifest record. |
| 2 | Usage or configuration error: a bad flag, `--set` value, input path, preset or config file. Nothing was converted. |
| 3 | Fatal: an I/O failure on the output root or manifest, an internal error (`sdc: internal error`), or a second Ctrl-C. |
| 4 | Every file was handled, but at least one failed validation. |

When several apply, the highest wins: 3, then 2, then 1, then 4, then 0. Intentional skips, up-to-date files and raw maps consumed by packing never cause exit 1.

A value outside its range in a config, preset, rule or `--set` (`CFG-INVALID`) stops the run with exit 2. Reached after layering, only that file fails.

## Error messages

An error that stops a run prints `sdc: <message>` to stderr and exits 2 (3 for an internal error). The message names the cause; these are the ones worth a fix. Scan errors are on [Mod scanning](Mod-Scanning.md#scan-errors).

### Inputs and paths

| Message | Cause and fix |
|---|---|
| `input not found: <path>` | The path doesn't exist. |
| `<path> is a folder; give .dds files` | `validate` takes files, not folders. |
| `--mem-budget '<s>': …` | Not a whole number of bytes with an optional `G` or `M` suffix, or zero. Leave the flag out for the default. |

### `--set` and `--allow`

| Message | Cause and fix |
|---|---|
| `--set '<s>': unknown field '<f>'` and `unknown sub-type '<id>'` | A misspelt field or sub-type. [The field list](Project-Config.md#fields). |
| `--set '<s>': a sub-type scope cannot set the sub-type` | A `subtype:` scope can't set `subtype`, `reencode` or `helper.*`, and an unscoped `--set` can't set `helper.*`: use `file:<glob>.…`. |
| `--set '<s>': a set scope sets only shader` | `set:<id>.` takes `shader` and nothing else. |
| `--allow <X>: unknown code` | A misspelt code. [All codes](Diagnostic-Codes.md). |
| `--subtype <id> is unavailable under <Target> (CFG-UNAVAILABLE)` | `validate` with a sub-type the target lacks. |

### Presets and config

| Message | Cause and fix |
|---|---|
| `unknown preset '<id>'` | A preset id that isn't loaded. `sdc presets list` shows the ids. |
| `preset inheritance cycle at '<id>'` | Presets whose `base` chain loops. |
| `<file>: config schema_version N is newer than this sdc supports (1)` | The file was written by a newer version. A preset gives `preset schema_version N is newer …`. |
| `<file>: <key>: removed in <version>; <fix>` | A key or value Tessalith doesn't read; the message names the replacement (for example `use skin_rfaos`). The text names the key, the version and what to write; `--set` gives the same text. The GUI stays read-only until the file is fixed. <!-- D:FM10 --> |
| `[files] is only valid in a project sdc.toml, not in <path>` | `[files]`, `[inputs]` or `[export]` in the user config. |

### While running

| Message | Cause and fix |
|---|---|
| `source changed since planning; re-run` and `size changed since planning; re-run` | A source file changed between planning and encoding. Run again. |
| `sdc: internal error` | A crash inside `sdc` (exit 3). Report it with the input that caused it. |

## JSON event stream

`convert --json` and `inspect --json` write one JSON object per line to stdout, in `seq` order. The event schema (version 2) is authoritative: `sdc presets schema event`. Every event carries `schema`, `seq`, `ts`, `batch`, `kind`, and, when they apply, `set`, `path` and `job_key` (the output path relative to the output root).

| `kind` | Extra fields |
|---|---|
| `batch_started` | `target`, `preset`, `inputs` (count). |
| `file_discovered` | `output`. |
| `set_resolved` | `members`, `pbr`, `bake`, `recipes`, `shader` (`shader`, `provenance`, `wanted`), `warnings`. |
| `file_planned` | `job`: the resolved job with its profile, the provenance of every field, suggestions, warnings and status. |
| `file_started` | none. |
| `progress` | `stage`, `done`, `total`. |
| `warning` | `code`, `severity`, `message`, `data`. |
| `file_done` | `output`, `validation` (`passed`, `levels`, `min_mip_psnr_db`, `alpha`, `coverage`, `bc6h_peak_ratio`). |
| `file_skipped` | `skip`, `code`, `reason`. |
| `file_failed` | `stage`, `error`. |
| `batch_done` | `converted`, `up_to_date`, `skipped`, `skipped_intentional`, `failed`, `validation_failed`, `cancelled` (list of inputs). |

- `skip` values: `up_to_date`, `on_existing_skip`, `unresolved`, `blocked`, `duplicate`, `dds_input`, `packing_input`, `unknown_existing`, `pack_conflict`, `packed_source`, `generated_replaced`.
- `stage` values: `discover`, `group`, `load`, `analyse`, `resolve`, `preprocess`, `mipmaps`, `compress`, `write`, `validate`.
- `validate --json` writes one `dds_validation` record per file (`schema` 1; `path`, `readable`, `subtype`, `alpha_policy`, `judged`, `note`, `passed`, `summary`, `warnings`). An unreadable file gets a shorter record: `kind`, `schema`, `path`, `readable` (false), `passed` (false), `error`.
- `inspect --json` writes `dds-info` objects for DDS inputs and the event stream for images.
