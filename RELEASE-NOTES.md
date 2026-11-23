## Unreleased

- BC1, BC3, BC4 and BC5 encode more than four times as fast, with the same files byte for byte: 300 textures as BC1 take about 14 seconds where 3.0.0 took over a minute.
- BC7 and BC6H encode about 1.5 times as fast on CPUs with AVX-512. Their bytes differ a little from an AVX2 CPU's, at the same quality. On CPUs without AVX2 they are slower than before.
- BC6H (HDR sky and PBR emissive) gets four encoder fixes: closer to the source at the default effort (about +0.07 dB), and 2 to 5 % slower. Existing BC6H files are kept; to encode them again, run `sdc convert <project> --on-existing if-changed --ev --property bc6h_uf16`.
- After an update, outputs whose encode version changed are kept and counted, not encoded again, until you ask with `--ev` or with **Re-encode every output** in the GUI's **Tools** menu.
- New **Max** effort for BC3, BC4 and BC5. What 3.0.0 called `max` is now **Slowest**: a project or preset that says `max` gets the new, slower search, and its BC3, BC4 and BC5 files are encoded again on the next run. Set it to `slowest` first to keep them.
- Also new: a `fast` BC1 search in about a third of the time, and Alpha ×4, ×32 and ×64 BC7 weighting. A config that uses `fast`, `slowest` or the new weightings can't be read by 3.0.0.
- If you set `mips.normal_base = "keep"`: a base normal stored as BC5 with Keep now fails with `CFG-INVALID`. Choose BC7, or set `renormalize`. Normal outputs re-encode only with `--ev`; the commands are in the changelog.
- Validation after encoding is off by default, so exports finish sooner. Turn it on with `--validate` or **Validate exported DDS** in **Settings**. A script that relies on exit code 4 needs `--validate`, and one that matches message text needs updating: messages say "type" where they said "sub-type".
- `_fuzz` and `_sheen` maps follow the set's shader. In a PBR set they are the PBR fuzz texture (`_f.dds`), and they no longer make a set Advanced skin on their own. Choose **Advanced Skin** as the shader to keep a set skin. Delete the old `<base>_fuzz.dds`, `_rfaos` and `.csskin.json` yourself: Tessalith leaves them on disk.
- In the GUI, normals follow their set's shader too: in a PBR or PBR Landscape set an untouched `_n` drops its alpha by default. If shared specular would be lost, Problems offers **Keep alpha**.
- GUI: Review opens before an export (tick **Export without review next time** to skip it), and Problems can reach zero (notes split out, **Mark as intended**, more one-click fixes). The Advanced drawer is gone: Size, Levels and DDS header are in the texture panel, and a new **Settings** tab holds Mode, Preset, the export folder and app settings. The 3D preview renders in under half the time.
- Lots of smaller fixes in menus, lists, sharing, NIF edits and screen-reader names.

Full changelog: https://github.com/Alistra-za/Tessalith/blob/main/CHANGELOG.md
