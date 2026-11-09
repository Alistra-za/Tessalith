# CLI reference

For anyone scripting `sdc`: batch conversions, CI, checking a downloaded mod. Every command, flag, exit code, log line and error message is here. If you use the GUI, start at [Getting started](Getting-Started.md); field values are on [Project config](Project-Config.md).

## Commands

| Command | What it does |
|---|---|
| [`convert`](#sdc-convert) | Convert images or folders to DDS. |
| [`inspect`](#sdc-inspect) | Show the plan for images, or the header and alpha statistics of DDS files. Writes nothing. |
| [`validate`](#sdc-validate) | Check existing DDS files against a target's rules. |
| [`presets`](#sdc-presets) | List and show presets; print JSON schemas. |
| [`cubemap-marker`](#sdc-cubemap-marker) | Write the 1×1 dynamic-cubemap marker DDS. |

- `sdc --version` prints the Tessalith version and the version of each linked encoder. Paste it into bug reports.
- `sdc --help` and `sdc <command> --help` print the built-in help.
- `convert` writes its log to stderr; `inspect`, `validate` and `presets` print their results to stdout. With `--json`, stdout carries only JSON.

## Flags shared by convert and inspect

| Flag | Default | Meaning |
|---|---|---|
| `<inputs>...` | required | Image files or folders. Several are allowed. |
| `--recursive` | off | Walk folders recursively; otherwise only the files directly inside each folder are read. Files you name are always included. |
| `--target cs\|vanilla` | see [Which target](Presets-and-Targets.md#which-target) | The target game setup. |
| `--preset <id or file.toml>` | see [Which target](Presets-and-Targets.md#which-target) | Select a preset by id, or load a preset file and select the id it declares. |
| `--preset-file <file>` | none | Load a preset file without selecting it, for example one that declares suffix aliases. Repeatable. Loading an id that is already loaded is an error. |
| `--config <file>` | `sdc.toml` in the first input folder | The project config file. |
| `--set <override>` | none | Override a profile field. Repeatable. See [--set syntax](#--set-syntax). |
| `--accept-suggestions none\|high` | `none` | `high` accepts every High-confidence sub-type suggestion. With `none`, a file whose sub-type is only suggested stays unresolved. High-confidence alpha suggestions apply either way. |
| `--allow <CODE>` | none | Acknowledge a blocking code for the whole batch. Repeatable. An unknown code is a usage error. |
| `--json` | off | Write JSON Lines events to stdout. Human output stays on stderr. |

The user config and the user presets always load ([where they are](Installation.md#where-tessalith-keeps-its-files)).

### Why you usually want `--accept-suggestions high`

A known suffix such as `_n` or `_rmaos`, a known exporter term such as `_albedo`, or no suffix at all (the colour map) gives a High-confidence sub-type suggestion. The CLI applies it only with `--accept-suggestions high`; otherwise the file is reported as unresolved:

```text
skip  C:/Work/rock/rock.png: SUG-LOW-CONFIDENCE sub-type 'diffuse' is a High-confidence suggestion (colour member, non-PBR set)
```

A lone file with an unknown suffix (`rock_bump.png`) has no suggestion, so the flag doesn't help: `sdc convert` exits 1 with `unknown suffix: assign a type with --set file:<path>.subtype=<type> or a [files] entry` ([why it waits](Texture-Types.md#how-files-form-a-set)). Give it a type with its path under the input folder:

```text
sdc convert --set "file:textures/x/rock_bump.png.subtype=diffuse"
```

The GUI accepts High suggestions and writes each into `sdc.toml` as a `[files]` entry, so a project saved from the GUI converts the same way here without the flag. What `sdc` doesn't read from the GUI (the mesh scan, the environment-map `_m`) is listed on [Review and Export](GUI-Review-and-Export.md).

### --set syntax

| Form | Applies to | Can't set |
|---|---|---|
| `field=value` | Every file. | `helper.*` |
| `subtype:<id>.field=value` | Every file of that sub-type. | `subtype`, `reencode`, `helper.*` |
| `file:<glob>.field=value` | Files matching the glob ([glob rules](Project-Config.md#rules)). | |
| `set:<set id>.shader=<shader>` | A texture set's shader ([shader ids](Set-Shaders.md)). The set id is what `inspect` prints after `set`; an id with no `/` matches every set with that base name. An id that names no set gives `SET-EDIT-STALE`. | Anything but `shader` |

- `helper.role`, `helper.fills`, `helper.read` and `helper.invert` take only the `file:` form: an unscoped `helper.role=ao` would make every image a helper.
- Values are TOML literals (`true`, `127`, `0.5`); anything else is a string, so `format=bc1` needs no quotes.
- A misspelt field or sub-type is an error, never ignored.

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
| `--on-existing if-changed\|skip\|overwrite` | `if-changed` | What to do when an output exists ([what each does](Validation-and-Output.md#re-runs-and-if-changed)). |
| `--dry-run` | off | Plan and report, write nothing. |

The GUI always runs with the defaults (all cores, 50% of RAM). These two flags are the only way to change them.

### Also export as Vanilla / CM

- With `[export] also` in the project `sdc.toml` ([keys](Project-Config.md#export)), `sdc convert` makes `-o` the parent of `<project> PBR`, `<project> Vanilla` and `<project> CM`. `<project>` is the first input folder's name.
- It needs the `cs` target: `Also export needs the cs mode: the PBR folder is the cs export` (exit 2).
- A bad `[export.legacy]` value or folder name is an error (exit 2) on this path only.
- The CLI reads no mesh scan, so it writes no vanilla environment `_m`.
- On stderr it lists the files in the three folders this export didn't write ("not written by this export"), and each PBR set left out of Vanilla and CM, with the reason.
- What each folder holds: [Set shaders and Also export](Set-Shaders.md#also-export-as-vanilla-and-cm). The GUI's **Copy CLI command** passes the Also export location as `-o`.

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

- Codes of severity warning and above are printed, plus the info code `CFG-FILTER-IGNORED`. Paths use forward slashes.
- A set-level warning prints with the set id as its label: `warning textures/armor/iron/iron: SHADER-MISSING-MAP …`.
- Raw maps consumed by a packed output get no line of their own.

| `done` counter | Counts |
|---|---|
| converted | Files encoded and written in this run. |
| up to date | Outputs left alone by `if-changed`. |
| skipped | Files not converted for any other reason: unresolved, blocked, duplicate output, DDS input, existing output with no record, `--on-existing skip`, a pre-packed file replaced under `packing.prefer = "raw"` (`PACK-CONFLICT`), a generated file replaced by a real map (`GEN-REPLACED`). |
| on purpose | The part of skipped you asked for: DDS inputs not opted in (`IN-DDS-SKIPPED`), `--on-existing skip`, `PACK-CONFLICT` and `GEN-REPLACED`. |
| failed | Files that failed to load, resolve, encode or write. |
| failed validation | Files written whose validation reported an error. |
| cancelled | Files not started because you pressed Ctrl-C. |

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

## sdc validate

```text
sdc validate [OPTIONS] <DDS>...
```

Runs the post-encode checks ([which ones](Validation-and-Output.md#what-is-checked)) on DDS files that already exist, including files made by other tools. Give `.dds` files: a folder is refused (`<path> is a folder; give .dds files`).

| Flag | Default | Meaning |
|---|---|---|
| `--target cs\|vanilla` | `cs` | Whose rules apply. |
| `--subtype <id>` | inferred from the file's suffix, its game path and, for formats with alpha, its mip-0 alpha | What the files are meant to be. A sub-type the target doesn't have is an error. |
| `--alpha-policy remove\|cutout\|blend\|cutout_blend\|data` | inferred | The alpha meaning to check against. Inferred as `remove` when every mip-0 alpha value is at least 254. Otherwise the sub-type's default, except that a sub-type whose default is `remove` is inferred as `data`. |
| `--exact-opaque` | off | Hold BC7 files under `remove` to Tessalith's own guarantee that opaque alpha decodes as exactly 255. Off by default because other encoders don't promise it. |
| `--json` | off | One JSON object per file on stdout; the human report moves to stderr. |

The format and `_SRGB` tag are taken from the file, so third-party files are judged on header and alpha rules, not on Tessalith's preferred format. A format Tessalith never writes has only its header checked and is reported as not judged. A cube DDS is judged as a `cubemap`, and a 2D file whose name ends in `_e` or that sits under `textures\cubemaps\` fails, because the game reads it as a cube ([checks](Cubemaps.md#checks)).

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

## sdc cubemap-marker

```text
sdc cubemap-marker [--color <R,G,B>] [--roughness <0..1>] -o <file.dds>
```

Writes the 152-byte 1×1 dynamic-cubemap marker that Community Shaders and ENB read as "reflect the live scene". What the colour and roughness mean: [Dynamic marker](Cubemaps.md#dynamic-marker).

| Flag | Default | Meaning |
|---|---|---|
| `--color <R,G,B>` | `0,0,0` | The reflection colour as raw code values. |
| `--roughness <0..1>` | none | Stored in alpha. |
| `-o`, `--out <file>` | required | The DDS file to write. |

```powershell
sdc cubemap-marker --color 200,200,210 --roughness 0.3 -o textures\cubemaps\steel_e.dds
```

An existing file Tessalith didn't write, or can't read, is kept. Exit codes: 0 written; 1 kept (such an existing file); 2 usage error (a bad colour, a roughness outside 0 to 1); 3 the write failed.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success. Every file converted (or was up to date or skipped on purpose) and passed validation. |
| 1 | Partial. Some files were unresolved, blocked, failed, cancelled, or had an existing output with no manifest record. |
| 2 | Usage or configuration error: a bad flag, `--set` value, input path, preset or config file. Nothing was converted. |
| 3 | Fatal: an I/O failure on the output root or manifest, an internal error (`sdc: internal error`), or a second Ctrl-C. |
| 4 | Every file was handled, but at least one failed validation. |

When several apply, the highest wins: 3, then 2, then 1, then 4, then 0. Intentional skips, up-to-date files and raw maps consumed by packing never cause exit 1.

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
