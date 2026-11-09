# Validation and output

For anyone installing or re-running an export: where each file lands, what Tessalith checks before it writes, and why a re-run did or didn't rebuild a file.

## Where the files go

- Each output goes to `<output root>\<path relative to the input folder>` with a `.dds` extension, so the output tree mirrors the input tree.
- Exporter names are normalised: `iron_albedo.png` is written as `iron.dds`, `iron_normalgl.png` as `iron_n.dds`, and raw maps become packed outputs such as `iron_rmaos.dds`.

```text
C:\Work\IronPlate\                          C:\Work\IronPlate-out\
  textures\pbr\armor\ironplate\               textures\pbr\armor\ironplate\
    ironplate_albedo.png            ->          ironplate.dds
    ironplate_normal.png            ->          ironplate_n.dds
    ironplate_roughness.png         -\
    ironplate_metallic.png          ---->       ironplate_rmaos.dds
    ironplate_ao.png                -/
                                              _sdc\
                                                manifest.jsonl
```

| Front end | Output root |
|---|---|
| `sdc convert` | `.\sdc-out` in the current directory, or `-o <dir>`. |
| `tessalith.exe` | The folder you choose for the project (Review asks before it exports). The GUI refuses to write into the folder it reads. |

- The output root and any `_sdc` folder are never read as input, so an output folder inside the source folder is safe.
- Two inputs that would write the same output (`rock.png` and `rock.tga`) are both blocked with `IN-DUPLICATE`: rename or remove one.

### Atomic writes

Every file is written to a temporary `.sdc-*.tmp` file in the target folder, then renamed into place, so an interrupted run never leaves a half-written DDS. A hard abort (a second Ctrl-C, a crash) can leave `.sdc-*.tmp` files behind. Delete them by hand.

### The DDS header

Tessalith writes its own headers: legacy or DX10, picked from the format and the `_SRGB` tag, and never the non-standard `DDPF_NORMAL` flag. The `header` field ([Project config](Project-Config.md#format-and-size)) overrides the pick.

## What is checked

Every converted file is decoded again before it is written, and the decoded result is checked. `sdc validate` runs the checks that need no source on DDS files that already exist: `VAL-HEADER`, `VAL-ALPHA-GUARANTEE`, the `VAL-M-*` checks, `VAL-PGPATCHER-50` and `VAL-GRASS-DETECT`. Mip quality, cutout coverage and the BC6H peak need the encoder input, so `validate` can't run them.

The thresholds are fixed; only `validation.mips` is a setting.

| Check | Rule | Code | Severity |
|---|---|---|---|
| Header | Flags, caps, mip count, format and header form are correct. | `VAL-HEADER` | error |
| Mip quality | Each mip's PSNR against the encoder input is at least 30 dB. With `validation.mips = "smallest"`, only the smallest mip is checked. A BC1, BC3 or BC7 texture with `encoder.perceptual` on is scored by Rec.709 luma PSNR, the measure that encoder optimises. | `VAL-MIP-ERROR` | warning |
| Opaque alpha | With `remove`, decoded alpha is at least 254 for a BC7 file without `exact_opaque`, and exactly 255 for BC7 with `exact_opaque` and for every other format. | `VAL-ALPHA-GUARANTEE` | error |
| Cutout coverage | With `cutout`, `cutout_blend`, or `blend` as BC1 1-bit alpha, each mip's alpha coverage stays within 2 points of mip 0. Checked on levels of 256 texels or more. | `VAL-COVERAGE` | warning |
| Complex `_m` margin | The smallest mip's alpha is at or above 247, close to or past the 251/255 point where Community Shaders turns complex material off. | `VAL-M-COMPLEX-MARGIN` | warning |
| Complex `_m` grey | The smallest mip is grey and not black-with-height, so Community Shaders reads it as a plain env mask. | `VAL-M-GRAYSCALE` | warning |
| Complex `_m` height | Large alpha areas at or below 4/255 or at or above 251/255 give no parallax. | `VAL-M-HEIGHT-CLIP` | warning |
| PGPatcher 50% rule | A plain `_m` that stores alpha has a share of non-255 alpha at or above 50%, so PGPatcher would treat it as complex. | `VAL-PGPATCHER-50` | warning |
| BC6H peak | The decoded peak divided by the source peak is at least 0.8. | `VAL-BC6H-PEAK` | warning |
| Complex grass corner | The encoded bottom-left texel still passes Community Shaders' detection test. | `VAL-GRASS-DETECT` | error |

- A file with a validation error is still written. It is reported as `ok ... [VALIDATION FAILED]`, counted under "failed validation", and makes `sdc convert` exit 4 if nothing worse happened. The next `if-changed` run re-encodes it.
- Cube checks (no mips, a lit +Y on the marker, an `_SRGB` format, a 2D file where a cube is expected) fail with `VAL-HEADER`; the rules are on [Cubemaps](Cubemaps.md#checks).

## The manifest

Every output root has `<output root>\_sdc\manifest.jsonl`: one JSON line per file written. When a file is converted again a new line is appended, and the last line for each output wins. A record holds the resolved settings and where each came from, the source hashes, the encoder versions, the validation summary and the output's size and time. `sdc presets schema manifest` lists every field.

## Re-runs and if-changed

`--on-existing if-changed` is the default. For each output that already exists, Tessalith looks up its manifest record. The output is up to date, and left alone, only if all of these match:

- the encoder versions (and the rgbcx patch for BC3/BC4/BC5);
- the output file's size and modified time;
- the BC1 encode target, for BC1 and BC3;
- the record passed validation;
- the texture type's encode version (bumped only when a release changes that type's output);
- every source file's content (by SHA-256), including packing sources and the baked normal map;
- the resolved settings (by hash).

So editing a source, or changing a setting in `sdc.toml` or a preset, re-encodes exactly the files affected. A new Tessalith version re-encodes only the texture types whose output changed. Touching or replacing an output by hand also re-encodes it.

An existing output with no manifest record is never overwritten by `if-changed`: it is kept and reported as `OUT-UNKNOWN-EXISTING`, and the run exits 1. Tessalith never overwrites a file it can't show it wrote. `--on-existing overwrite` replaces it.

| `--on-existing` | Current record | Changed record | No record |
|---|---|---|---|
| `if-changed` (default) | kept (up to date) | re-encoded | kept, `OUT-UNKNOWN-EXISTING` |
| `skip` | kept | kept | kept |
| `overwrite` | re-encoded | re-encoded | re-encoded |

## Installing the output

- Copy the contents of the output root into your mod so the files land under `Data\textures\…`.
- Leave `_sdc` out: it's Tessalith's.
- A PBR set also needs its PGPatcher JSON: [GUI-PGPatcher-JSON](GUI-PGPatcher-JSON.md).
