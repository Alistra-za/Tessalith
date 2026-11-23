## Unreleased

- BC1, BC3, BC4 and BC5 encode more than four times as fast. Tessalith now runs its own Rust port of rgbcx, which writes the same files byte for byte: 300 textures as BC1 take about 14 seconds where 3.0.0 took over a minute.
- BC7 and BC6H encode about 1.5 times as fast on CPUs with AVX-512. Their bytes differ a little from an AVX2 CPU's, at the same quality. CPUs without AVX2 now run the SSE2 code, which is slower there.
- BC6H (HDR sky and PBR emissive) gets four encoder fixes: closer to the source at the default effort (about +0.07 dB), and 2 to 5 % slower. Existing BC6H files are kept; to encode them again, run `sdc convert <project> --on-existing if-changed --ev --property bc6h_uf16`.
- An update no longer re-encodes everything. Outputs written by an older encoder version are kept until you ask for them with `--ev`, or with **Re-encode every output** in the GUI's **Tools** menu.
- New encoder settings: **Max** effort for BC3, BC4 and BC5 (what 3.0.0 called `max` is now **Slowest**), a `fast` BC1 search in about a third of the time, and Alpha ×4, ×32 and ×64 BC7 weighting.
- Validation after encoding is off by default, so exports finish sooner. Turn it on with `--validate` or **Validate exported DDS** in **Settings**. A script that relies on exit code 4 needs `--validate`.
- `_fuzz` and `_sheen` maps follow the set's shader. In a PBR set they are the PBR fuzz texture (`_f.dds`), and they no longer make a set Advanced skin on their own. Choose **Advanced Skin** as the shader to keep a set skin. Old outputs stay on disk: delete them yourself.
- Normals follow their set's shader too: in a PBR or PBR Landscape set an untouched `_n` drops its alpha by default.
- GUI: Review opens before an export (tick **Export without review next time** to skip it), Problems that can reach zero (notes split out, **Mark as intended**, more one-click fixes), a **Settings** tab in place of the Advanced drawer, linked and shared slots on set cards, and a 3D preview that renders in under half the time.
- Lots of smaller fixes in menus, lists, sharing, NIF edits and screen-reader names.

Full changelog: https://github.com/Alistra-za/Tessalith/blob/main/CHANGELOG.md
