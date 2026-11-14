# Validation and output

For anyone installing or re-running an export: where each file lands, what Tessalith checks before it writes, and why a re-run did or didn't rebuild a file.

## Where the files go

- Each output goes to its game path under the output root, with a `.dds` extension: the output tree mirrors the input tree from the `textures` folder the game path starts at. Open `MyMod\textures` or `MyMod\textures\armor` and the files land in `<output root>\textures\armor\`, the path the game loads, with the `_sdc` manifest in `<output root>\textures\armor\_sdc`; folder names keep the case they have on disk. Set names don't change. An input folder with no `textures` folder in its path gets none added. With the output root set to the input folder itself, or to a folder that holds it, each DDS goes beside its source. <!-- D:HQ1 -->
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

The GUI writes to the folder you choose for the project (Review asks before it exports) and refuses to write into the folder it reads. The command line has its own default: [CLI reference](CLI-Reference.md#sdc-convert).

- The output root and any `_sdc` folder are never read as input, so an output folder inside the source folder is safe.
- Two inputs that would write the same output (`rock.png` and `rock.tga`) are both blocked with `IN-DUPLICATE`: rename or remove one.

### Atomic writes

Every file is written to a temporary `.sdc-*.tmp` file in the target folder, then renamed into place, so an interrupted run never leaves a half-written DDS. A hard abort (a crash, a killed process) can leave `.sdc-*.tmp` files behind. Delete them by hand.

### The DDS header

Tessalith writes its own headers: legacy or DX10, picked from the format and the `_SRGB` tag, and never the non-standard `DDPF_NORMAL` flag. The `header` field ([Project config](Project-Config.md#format-and-size)) overrides the pick.

## What is checked

Every converted file is decoded again before it is written, and the decoded result is checked. Mip quality, cutout coverage and the BC6H peak need the encoder input, so they run only during a conversion. The other checks also work on DDS files you already have, from the command line: [CLI reference](CLI-Reference.md#sdc-validate).

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

- A file with a validation error is still written and counted under "failed validation". The next `if-changed` run re-encodes it.

## The manifest

Every output root has `<output root>\_sdc\manifest.jsonl`: one JSON line per file written. When a file is converted again a new line is appended, and the last line for each output wins. A record holds the resolved settings and where each came from, the source hashes, the encoder versions, the CPU path that encoded it (`isa`: `avx512`, `avx2`, `avx`, `sse4` or `sse2`), the validation summary and the output's size and time. The JSON Schema for it is `manifest`: [CLI reference](CLI-Reference.md#sdc-presets).

## Re-runs and if-changed

Re-runs default to `if-changed`. For each output that already exists, Tessalith looks up its manifest record. The output is up to date, and left alone, only if all of these match:

- the encoder versions (and the rgbcx patch for BC3/BC4/BC5);
- the output file's size and modified time;
- the BC1 encode target, for BC1 and BC3;
- the record passed validation;
- the texture type's encode version (bumped only when a release changes that type's output);
- every source file's content (by SHA-256), including packing sources and the baked normal map;
- the resolved settings (by hash).

So editing a source, or changing a setting in `sdc.toml` or a preset, re-encodes exactly the files affected. A new Tessalith version re-encodes only the texture types whose output changed. Touching or replacing an output by hand also re-encodes it.

The CPU is not on that list. BC7 and BC6H bytes differ between a CPU with AVX-512, one with AVX2 and an older one. AVX-512 against AVX2, the one pair measured, is at equal quality (PSNR within 0.002 dB). A file is not re-encoded because the CPU changed: move a project to another PC and only files that are stale for another reason are encoded again, so one output folder can hold files from both CPUs. Each record's `isa` says which ISPC path BC7 and BC6H encode on (BC1, BC3, BC4 and BC5 are encoded in Rust, with the same bytes on every CPU). <!-- D:HZ2 -->

An existing output with no manifest record is never overwritten by `if-changed`: it is kept and reported as `OUT-UNKNOWN-EXISTING`. Tessalith never overwrites a file it can't show it wrote. The command line can replace it: [CLI reference](CLI-Reference.md#re-runs-and-if-changed).

## Installing the output

- Copy the contents of the output root into your mod so the files land under `Data\textures\…`.
- Leave `_sdc` out: it's Tessalith's.
- A PBR set also needs its PGPatcher JSON: [GUI-PGPatcher-JSON](GUI-PGPatcher-JSON.md).
