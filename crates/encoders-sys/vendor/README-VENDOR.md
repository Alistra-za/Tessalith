# Vendored encoder sources and objects

| Path | Upstream | Commit | Licence |
|---|---|---|---|
| bc7enc_rdo/ (rgbcx.* with the rgbcx BC4 round-to-nearest, BC4 search, BC1 dual, BC4 guard cut, BC1 alpha and BC1 perceptual patches, bc7e.ispc pristine) | https://github.com/richgel999/bc7enc_rdo | b9438627eef73a1157e84201b6fa6eb2ffd6d9f0 | rgbcx: MIT or Unlicense; bc7e.ispc: Apache-2.0 |
| ispctc/ (kernel.ispc with patches/0001-bc6h-peak-mode.patch, patches/0003-bc6h-only.patch and patches/0004-bc6h-kernel-fixes.patch; ispc_texcomp.cpp with patch 0003) | https://github.com/GameTechDev/ISPCTextureCompressor | 79ddbc9 + patches 0001, 0003, 0004 | MIT (kernel.ispc and ispc_texcomp.cpp modified; change notices at the top) |
| bc7e/ (bc7e.ispc, patches/0001-exact-opaque-alpha.patch, patches/0002-sdc-max-level.patch, patches/0005-bc7e-fixes.patch) | bc7enc_rdo/bc7e.ispc + sdc patches 0001, 0002 and 0005 | b9438627eef73a1157e84201b6fa6eb2ffd6d9f0 + patches 0001, 0002, 0005 | Apache-2.0 (modified; change notices at the top of the file) |
| ispc-objs/bc7e/ | built from bc7e/bc7e.ispc (pristine + patches 0001, 0002 and 0005, rebuilt on Windows 2026-10-18, fifth target added 2026-11-13, SSE4 and AVX dropped 2026-11-14, rebuilt 2026-11-23 with patch 0005): `-O2 --target=sse2,avx2,avx512skx-x16 --opt=fast-math --opt=disable-assertions`; `bc7e_avx512skx.obj` with ISPC 1.32.0, every other file with ISPC 1.31.0 | — | Apache-2.0 |
| ispc-objs/ispctc/ | built from ispctc/kernel.ispc (with patches 0001 and 0003, rebuilt on Windows 2026-10-31, fifth target added 2026-11-13, SSE4 and AVX dropped 2026-11-14, trimmed to BC6H 2026-11-14 (patch 0003), rebuilt 2026-11-22 with patch 0004): `-O2 --target=sse2,avx2,avx512skx-x16 --opt=fast-math -DSDC_BC6H_ONLY`; `kernel_avx512skx.obj` with ISPC 1.32.0, every other file with ISPC 1.31.0 | — | MIT |

All objects are produced by `tools/regen-ispc.ps1` for the three ISPC targets, with two compilers: ISPC 1.31.0
(`-Ispc`) builds every file except the two `_avx512skx` objects, which ISPC 1.32.0 (`-Ispc512`) builds because it
fixes an AVX-512 miscompile of 1.31.0 (ISPC issue 3891). The script also passes `--arch=x86-64 --target-os=windows`,
runs with the repo root as working directory and a repo-relative source path, and normalises line 2 of the generated
headers, so a rebuild on any machine is byte-identical. Run `tools/regen-ispc.ps1 -Ispc <1.31.0> -Ispc512 <1.32.0>`
to check (exit 0 = all MATCH: with ISPC 1.31.0 and `-Ispc512` ISPC 1.32.0 it matches 10/10; without `-Ispc512`,
8 MATCH and the two `_avx512skx` objects SKIP). A vendored `.obj` the build does not produce (a dropped target's) is
reported as `EXTRA` and counts as a difference; `-Update` never deletes one. After `-Update` (which
needs both compilers), run `tools/hash-vendor.ps1`.

Provenance of the first objects: the bc7e objects are byte-identical to the ones the bench measured
(`<bench>/build/bc7enc_rdo`). The ISPC TC objects were rebuilt from `ispctc/kernel.ispc`
because the bench's copies embed the bench's absolute source path; same source and flags,
but the files differ from the bench's.

Three vendored encoders carry sdc patches: bc7e (patches 0001 and 0002 below), ISPC TC (patches 0001 and 0003 below) and rgbcx (the BC4 round-to-nearest, BC4 search, BC1 dual, BC4 guard cut, BC1 alpha and BC1 perceptual patches at
the end of this file).

Patch 0001 "exact opaque alpha" (`bc7e/patches/0001-exact-opaque-alpha.patch`) changes bc7e. `bc7e/bc7e.ispc` is the pristine `bc7enc_rdo/bc7e.ispc` with that patch applied:
it renames the unused `bc7e_compress_block_params::m_unused0` to `m_exact_opaque_alpha` (same struct
layout) and, when that flag is set, restricts the mode-6 encodes of opaque blocks to p-bits = 1 so bc7e
optimises RGB under the constraint and alpha decodes to exactly 255. With the flag false the output was
bit-identical to upstream until patch 0005 (the `exact-opaque-native` gate of the bench tool, which lives on branch `bench`). The patch file is produced with
`git diff --no-index --text crates/encoders-sys/vendor/bc7enc_rdo/bc7e.ispc crates/encoders-sys/vendor/bc7e/bc7e.ispc`.
Since patch 0001 the bc7e objects are built from the patched source and no longer match the bench's.

Patch 0002 "sdc_max level" (`bc7e/patches/0002-sdc-max-level.patch`, 2026-10-18) applies on top of patch 0001. It adds
four trailing `bool` fields to `bc7e_compress_block_params` (`m_endpoint_search`, `m_refine_partitions`, two unused), so
the struct grows and the generated header changes; `bc7e_compress_block_params_init` clears them. `m_endpoint_search` runs
a greedy +-1 search over the quantised endpoints (one channel of one endpoint at a time, selectors re-assigned) at the end
of `color_cell_compression`; `m_refine_partitions` makes the mode 1/3/7 partition ranking refine every candidate instead of
only the winner once more than two are tried. The new export `bc7e_compress_block_params_init_sdc_max` is veryslow plus
4 partitions tried, uber 4, 2 least-squares passes and both flags; sdc maps `encoder.effort = "max"` to it. With both flags
false every existing level was bit-identical to the patch-0001 source until patch 0005: `encoders-sys`
`tests/level_identity.rs` pinned the output of every shipped level for both perceptual and both exact-opaque settings,
and passed against Linux objects built from the patched source. The patch file is `git diff --no-index --text <patch-0001 bc7e.ispc> crates/encoders-sys/vendor/bc7e/bc7e.ispc`.
Measurements: docs/research/bc7e-tuning.md.

**Objects.** The vendored `ispc-objs/bc7e/*.obj` and `bc7e_ispc.h` were rebuilt from the patch-0002 source on Windows
(2026-10-18) and again from the patch-0005 source (2026-11-23) with `tools/regen-ispc.ps1 -Update -Bc7eOnly` and `tools/hash-vendor.ps1`; a second `regen-ispc.ps1` run
matches every file it writes (10/10 since SSE4 and AVX were dropped). `build.rs` requires both `m_exact_opaque_alpha` (patch 0001) and `m_endpoint_search` in the header and stops the
build without either (Note FM), so `effort = "max"` and `alpha.exact_opaque` always work. Levels 0 to 6 encoded byte-identically to the patch-0001 objects on each of the four ISPC
targets of the time until patch 0005 (`tests/level_identity.rs`, which pins every level per target because ISPC's sse2 path (and the sse4 and avx paths of the time),
its avx2 path and its avx512skx path encode differently; engine goldens 95/95). The fifth target's row was recorded
when it was added and re-recorded with every row from the patch 0005 objects; a test build reads it only with the AVX2
cap lifted (`SDC_TEST_AVX512=1`).

bc7e patch 0005 "bc7e fixes" (`bc7e/patches/0005-bc7e-fixes.patch`, 2026-11-23) applies on top of 0001 and 0002. The patch
file is produced with `git diff --output=<patch file> <base> -- crates/encoders-sys/vendor/bc7e/bc7e.ispc`, where `<base>`
is the commit the fixes branch was cut from, and the objects are built with both compilers as above. One sentence per
change follows. S1: `handle_alpha_block_mode4` writes the end state of an alpha slot that is 255 on every texel (the
state its passes reach) without running them; BC7 bytes unchanged: every `level_identity` hash, `identity.json` and every
measured image's hash on three paths equal; `tests/level_identity.rs` `s1_fixture_is_pinned` pins it on blocks that reach
it. Q2t: `fixDegenerateEndpoints` spreads a channel only when its float endpoints differ by more than the new trailing
params field `m_flat_tolerance` (0.5/255 at every level, set per call through the shim's tuning slot 14), not by any
amount, so the refit's float noise no longer spreads a constant channel (an opaque block's rotated alpha slot decoded
239); proof test `tests/bc7.rs` `opaque_noise_never_double_nudges_the_rotated_slot`; changes bytes. Q1 and Q4: mode 7's
one-colour candidate (`pack_mode7_to_one_color`) is scored on its endpoints expanded to 8 bits as the decoder reads them,
not on the 6-bit codes (Q1), and its table resets its best endpoints per p-bit pair (Q4), so mode 7 no longer loses a
solid subset of an alpha block to a worse fit on a wrong score; proof tests `tests/bc7.rs` `mode7_takes_its_solid_subset`
(Q1) and `mode7_table_keeps_each_p_bit_pairs_own_best` (Q4); changes bytes (alpha blocks only: opaque blocks never try
mode 7, and ultrafast turns it off). XC: exact constants, the trailing params field `m_exact_mask` (R, G and B bits; 0
at every level, set per call through the shim's tuning slot 15): when a subset's masked channel is 0 on every texel, or
255 on every texel, a solution that does not decode it exactly scores 2^60 and a one-colour p-bit choice that misses it
is skipped; proof test `tests/bc7.rs` `exact_constants_keep_a_flat_channel`; changes bytes only with the mask set (mask
0 is every level's default and writes the bytes it wrote before). The two fields change the generated header (Note LR).
These changes alter BC7 bytes (the pins in `tests/level_identity.rs`, `identity.json` and the measured images). The engine's BC7-only tag `bc7e_fixes` hashes this patch file whole, so the
file is frozen once merged: a later bc7e change goes in a new patch file with its own tag.

ISPC TC patch 0001 "BC6H peak mode" (`ispctc/patches/0001-bc6h-peak-mode.patch`, 2026-10-31) changes
`ispctc/kernel.ispc` (`git diff main -- crates/encoders-sys/vendor/ispctc/kernel.ispc` at the time). It adds a
peak-preserving BC6H mode that is on when a `bc6h_enc_settings` has both `slow_mode` and `fast_mode` set, a combination
no upstream profile uses (`fast_mode` is read only when `slow_mode` is off), so the settings struct,
`ispc_texcomp.h/.cpp` and the generated header do not change. In that mode the encoder finds each block's brightest
texel and guards only highlight blocks, where that texel is at least 32 times the block's geometric-mean luminance;
other blocks encode exactly as veryslow. In a guarded block every candidate is ranked by its squared error against the
block, and one that decodes the brightest texel below 0.9 of its source luminance ranks after every candidate that
keeps it. A second search then runs on a copy with every channel lifted to at least 2^-12 of its texel's largest
channel; it is ranked against the unlifted block too, and the better of the two results is kept. The shim builds the
profile as veryslow plus `fast_mode` (`g_bc6[5]`, `Bc6hLevel::VeryslowPeak`). The five upstream profiles encoded
byte-identically to the pre-patch objects until patch 0004: `encoders-sys` `tests/bc6h.rs` pins their output per
path, in three lists, all re-recorded from the patch 0004 objects: `RECORDED` the AVX2 path, `RECORDED_PRE_AVX2` the
SSE2 path (the SSE4 and AVX paths of the time wrote the same bytes before patch 0004), and `RECORDED_AVX512` the
AVX-512 path, read only with the test builds' AVX2 cap lifted (`SDC_TEST_AVX512=1`). Rebuilt with
`tools/regen-ispc.ps1 -Update` and `tools/hash-vendor.ps1`; the bc7e objects did not change and a second `regen-ispc.ps1` run matched every file (12 then; 10 since SSE4 and AVX were dropped). Why and
measurements: .claude/DECISIONS.md (Note FE, "BC6H peak-preserving mode") and docs/research/encoder-benchmark.md
("BC6H (HDR)").

ISPC TC patch 0003 "BC6H only" (`ispctc/patches/0003-bc6h-only.patch`, 2026-11-14) applies on top of patch 0001. It is
numbered 0003 because 0001 and 0002 are bc7e's and 0001 is also ISPC TC's: 0003 is the next number not used anywhere
under `vendor/`. It puts `#ifndef SDC_BC6H_ONLY` around the BC1-BC5, BC7 and ETC1 kernels and the helpers only they use
in `ispctc/kernel.ispc`, and around their wrappers in `ispctc/ispc_texcomp.cpp` (marked "sdc patch 0003"); no line is
moved or edited. Without the define the source behaves as upstream plus patches 0001 and 0004 (the BC7 kernels call the
same `block_quant`, so 0004's fix #1 would also reach them there; `channel_opt_quant` keeps its own `int best_err`, as
upstream). `tools/regen-ispc.ps1` passes `-DSDC_BC6H_ONLY` to ISPC for `kernel.ispc` only (both compilers), and
`build.rs` defines it for the C++ build. At the time (before patch 0004), BC6H bytes were checked equal on every path
(SSE2, AVX2, AVX-512): the five-level and peak-mode pins of `tests/bc6h.rs` on one synthetic 64x64 HDR image, and 18
Poly Haven HDR files (1024x512) at all six levels. Object sizes before and after
(KB of 1,024 bytes): `kernel.obj` 4.9 to 2.0, `kernel_sse2.obj` 349.8 to 156.5, `kernel_avx2.obj` 326.8 to 145.1,
`kernel_avx512skx.obj` 241.3 to 93.2. The file's own `clamp` must stay unguarded (guarded, ISPC's standard-library
`clamp` takes its place): only the regeneration check (10 MATCH) and the real-file hashes catch that, not the synthetic
pins (Note JB5).

ISPC TC patch 0004 "BC6H kernel fixes" (`ispctc/patches/0004-bc6h-kernel-fixes.patch`, 2026-11-22) applies on top of
patches 0001 and 0003. It is produced with
`git diff --output=<patch> <base> -- crates/encoders-sys/vendor/ispctc/kernel.ispc` against the post-0003 kernel at
`fca6cb41a6703aaad61461700e083670cbcc3bd6` (the patch file is always regenerated whole, never edited by hand), and it
is built with both compilers as before (Note LO1). Fix #6: `bc6h_bounds` declared
`float rgb_span[0]` and wrote and read three elements; it is now `rgb_span[3]`. ISPC 1.31.0 printed no diagnostic for
it. BC6H bytes unchanged: every pin and every measured image hash equal (Note LO2). Fix #7: `bc6h_enc_2p` clamps the
partition count to the 32 entries of `part_list`; no profile in Tessalith asks for more than 32, so it is unreachable
here and guards other callers. BC6H bytes unchanged (Note LO3). Fix #5: `get_span` returned a float through an `int`,
which truncated mode 2's 204.8 to 204, so a block whose widest channel spans 99 half codes (204.39 UF16 units) was kept
out of mode 2 at Fast and Basic; it now returns the float span. Proof test:
`mode_2_takes_a_span_inside_its_fractional_limit`. Fix #5 changes BC6H bytes (Note LO4). Fix #1:
`block_quant` stored each texel's float error in an `int`, which overflows above 2^31 (a black
texel beside one brighter than about 1344.0), and the overflowed value beat every honest candidate;
it now keeps the float. Proof test: `black_beside_bright_keeps_both` (on the pre-fix objects it failed at Slow and
Veryslow, and at VeryslowPeak on the one-black block). Fix #3: `bc6h_enc_1p` submitted its last refinement pass, which
can be worse than its first quantisation or an earlier pass; it still refines in upstream's order but submits the best.
Proof test: `refinement_never_ends_worse_than_veryfast` (on the pre-fix objects Basic and Veryslow ended certainly worse
than Veryfast on seeded blocks). Veryfast and Fast do not refine a one-region encode, so their bytes did not change.
Fix #2: `ep_quant_bc6h` rounded an endpoint by `ep * (2^bits - 1) / 65535`, but the decoder rebuilds an interior code q
at (q + 0.5) * 65536 / 2^bits, so the code could be one too high near 0 and one too low near the top; below 15 bits it
now takes the code the decoder rebuilds nearest to the endpoint (16-bit mode 13 keeps upstream's rule, already nearest).
Proof test: `one_region_endpoints_take_the_nearest_code` (mode 10 at Veryfast; on the pre-fix objects every block's G
and B fields missed the nearest code). The code comes from one multiply (the interior code
`ceil(ep * 2^bits / 65536) - 1`, plus one compare with an end code beside it) in place of a first three-candidate
search, with the same bytes, because an interior code rebuilds at the centre of its cell and only the uncentred end
codes need a compare (Note LO9). Finding #4 (negative and non-finite halves read as large values) is not fixed:
`quantize.rs` `clamp_half` clamps every BC6H input first, so the kernel never receives them (Note LO). Fixes #5, #1, #3
and #2 change BC6H bytes (Note LO4, LO7, LO8, LO9); #6 and #7 change none. Base against final, on AVX2, each of the six
levels changed all 22 measured images (18 `hdr_real` and 4 `hdr_grad`: 0 identical at every level), and the pins of
`tests/bc6h.rs` moved at #5 (15 of 18), #3 (12) and #2 (18), not at #1 (Note LO6). Quality and counts: Note LO10 and
docs/research/encoder-benchmark.md ("Kernel fixes (patch 0004, 2026-11-22, Note LO)").

Patch "rgbcx BC4 round-to-nearest" (M6 Track D, 2026-10-12) edits `bc7enc_rdo/rgbcx.h` in place, marked
`sdc patch (M6 Track D): round to nearest to match hardware/bcdec`. `bc4_block::get_block_values6/8`, the BC4 palette
that `encode_bc4_hq` (BC4, both BC5 halves, BC3 alpha) and `unpack_bc4` use, floored the interpolants (`(6l+h)/7`);
they now use bcdec_rs 0.2.0's `bc4_block` fixed-point weights verbatim (`(w0*l + w1*h + 32768) >> 16`), so the
endpoint search minimises the error a rounding decoder sees (docs/research/encoder-benchmark.md, "BC4/BC5 search radius"). Nothing else in
rgbcx changes; BC1 is untouched. Test: `encoders-sys` `bc4_search_model_matches_bcdec_for_every_endpoint_pair`.
Upstream `rgbcx.h` at b943862 has SHA-256 `bd51b3d07a2a6e158cf9761ac3bd76b9f761c0f770c317e15ed7609e74e092fa`.

Patch "rgbcx BC4 search" (2026-11-01) applies on top of round-to-nearest, marked `sdc patch (bc4 search, see
README-VENDOR.md)`. `rgbcx.h` gains two `mode_flag` bits, `BC4_SDC_SEARCH = 4` and `BC4_SDC_GUARD128 = 8`; `rgbcx.cpp`
gains `namespace sdc_bc4` (helpers) and a block at the end of `encode_bc4_hq`. With `BC4_SDC_SEARCH` (and no forced
selectors), after the unchanged mode loop the block it chose is the start, and further endpoint pairs replace it only on
a strictly lower error: a 6-value search around the inner range (values other than 0 and 255), least-squares endpoint
refits of the best and of a fresh other-mode search, 6-value searches on trimmed inner ranges, and an exhaustive +-3
around the best pair. `BC4_SDC_GUARD128` keeps the mode-loop block when the result would flip more texels across 128.
The search equals `search` in `crates/encoders-sys/tests/support/bc4ref.rs` byte for byte (proved by `crates/encoders-sys/tests/bc4ref.rs`; the engine path by `engine_bc4_bytes_equal_reference` in `crates/engine/tests/encode.rs`); its least-squares step is f64, so `build.rs` compiles with `/fp:precise`; do not add `/arch:AVX2` (or enable FP contraction) without re-running
`ffi_equals_reference`. `BC4_SDC_SEARCH` requires `BC4_USE_ALL_MODES`. Without the new
bits `encode_bc4_hq` is unchanged. The shim sets `BC4_SDC_SEARCH` for BC4 and both BC5 halves, plus `BC4_SDC_GUARD128`
for BC3 alpha (`encode_bc3_hq` passes its flags through). Measurements: docs/research/bc4-family.md.
Before this patch (round-to-nearest only) `rgbcx.cpp` had SHA-256 `6f4d4664daa957a9d49f9fe881eab4a04c3d7c58a381280da0ecea1cf354c51c`
(the upstream b943862 file; round-to-nearest did not touch it) and `rgbcx.h` `38f1bd4551dab3700552d9a9d726946a527bb9072dc3337dc58794f3d9fef87e`.

`bc7enc_rdo/rgbcx_table4_small.h` was removed: `rgbcx.cpp` includes it only when `RGBCX_USE_SMALLER_TABLES` is 1, so that macro must stay 0.

Patch "rgbcx BC1 dual" (2026-11-02) applies on top of BC4 search, marked
`sdc patch (bc1 dual, see README-VENDOR.md)`. `rgbcx.h` gains `bc1_approx_mode::cBC1Dual = 4`, an encode-only target
that `encoders_sys::init` selects for every BC1 and BC3 colour block: under it every BC1 candidate is scored against both the NVidia and the AMD palette, each texel's selector minimises
err_nv + err_amd (squared RGB error), and the block error is that sum. `rgbcx.cpp` adds the single-colour table builder
for it (`init`; error = squared NVidia + squared AMD error of the interpolant, no 3% term), `bc1_get_dual_palette`,
`bc1_find_sels4_dual` and `bc1_find_sels3_dual` (3-colour black costs its error twice), and a leading
`if (cBC1Dual)` dispatch in `bc1_find_sels4_noerr`, `bc1_find_sels4` and `bc1_find_sels3_fullerr`. Under cBC1Dual every
4-colour selector search is the full per-texel search whatever the level's MSE flags. Endpoint fits (least squares,
total orderings, endpoint search) are unchanged. `unpack_bc1_block_colors` asserts the mode is not cBC1Dual (decoding
uses mode 1 or 2). BC4 and BC5 take no new code path. Before this patch `rgbcx.cpp` had SHA-256
`cf4d034b3277c9f942814896d7ce70eeaeb5d33c2011a897f784389b1858da95` and `rgbcx.h`
`e49b480432cb4de1985295999a9c9397b969ee701bb5638cda3a1db259202845`. Measurements: the bc1-dual research report (`reports/bc1-dual/REPORT-dual-mode.md`, outside the repo).

Patch "rgbcx BC4 guard cut" (2026-11-02) applies on top of BC1 dual, marked `sdc patch (bc4 guard cut)`. `BC4_SDC_GUARD128`
keeps the mode-loop block when the search would flip more texels across the alpha-test cutoff; the cutoff was the
constant 128 and is now bits 8..15 of `mode_flag` (a texel passes when alpha >= cutoff; 0 means 128, so every caller
that sets no bits there is unchanged). `rgbcx.h` documents the bits; in `rgbcx.cpp` `sdc_bc4::flips` takes the cutoff and
`encode_bc4_hq` reads it from `mode_flag` before the guard compare. The shim's `sdc_bc3_encode` takes the cutoff and
ORs `cut << 8` into the flags; `encoders_sys::bc3_encode_block` passes it on (the engine's `alpha_cut`, Note FM). The
reference is `search(v, radius, Some(cut))` in `crates/encoders-sys/tests/support/bc4ref.rs`; with cutoff 128 the output
is byte-identical to the BC1 dual patch. Before this patch `rgbcx.cpp` had SHA-256
`9e5245dcbbfbde3fbbc0472545a5dbe64ef76df9759c08feadc7a660749bd99f` and `rgbcx.h`
`204c7d75e361fc22e700467b97f0f09c17fd6efebc527dbff1f1519036219ffd`.

Patch "rgbcx BC1 alpha" (2026-11-02) applies on top of BC4 guard cut, marked `sdc patch (bc1 alpha, see README-VENDOR.md)`.
`rgbcx.h` declares and `rgbcx.cpp` adds `encode_bc1_alpha(level, pDst, pPixels, transparent_mask)`, with its helper
`bc1_alpha_find_sels`; no existing function changes. Bit i of the mask marks texel i transparent. Mask 0 calls
`encode_bc1(level, pDst, pPixels, true, false)`, so the bytes equal the plain BC1 encode; mask 0xFFFF writes both
endpoints 0 and every selector 3. Any other mask gives a 3-colour block (color0 <= color1). Its endpoints come from the
fit of `try_3color_block_useblack` (principal axis, extreme texels, least-squares refits) run over the opaque texels
instead of the non-black ones, then a greedy +-1 search over the six endpoint channels (at most 64 rounds). Opaque texels
pick selector 0, 1 or 2 on the cBC1Dual error (`bc1_get_dual_palette`, `bc1_dual_err`), masked texels get selector 3,
and the least-squares solve skips selector 3, so masked texels never steer the fit and an opaque texel never decodes
transparent (opaque black is reached through an endpoint). The shim adds `sdc_bc1_encode_alpha`;
`encoders_sys::bc1_encode_block_alpha` calls it at level 18. Before this patch `rgbcx.cpp` had SHA-256
`d5d785fd7a8ad61cee47a8a7bc6c4ffc235bd98ff0dc59d80480ff8ca5c6933a` and `rgbcx.h`
`c5defdaa3daf151116517a386cfb38145010b2490000976f524baf6a6dc97592`.
The principal-axis and extreme-texel setup is a copy of the one in `try_3color_block_useblack`; on an upstream update,
keep the two in sync.

Patch "rgbcx BC1 perceptual" (2026-11-03) applies on top of BC1 alpha, marked `sdc patch (bc1 perceptual, see
README-VENDOR.md)`. `rgbcx.h` declares and `rgbcx.cpp` adds `set_bc1_perceptual(bool)`, which sets per-thread
(`thread_local`) weights of the R, G and B terms of the BC1 colour error: 54, 183, 19 (Rec.709 luma scaled to 256) when
on, 1, 1, 1 when off. They multiply the 14 error sums of `bc1_find_sels4_fasterr`, `_check2_err`, `_fullerr`, the
forced-selector path of `bc1_find_sels4` and `bc1_find_sels3_fullerr`, and both palettes of `bc1_dual_err` (the
cBC1Dual scoring that `bc1_find_sels4_dual`, `bc1_find_sels3_dual` and `bc1_alpha_find_sels` use; `bc1_dual_palette`
carries the weights). The PCA axis, the least-squares fits, the orderings, the single-colour tables and the BC4 code are
unweighted. Off, every sum is the old integer times 1, so the bytes are unchanged (`encoders-sys`
`tests/bc1_perceptual.rs` pins the pre-patch output of BC1, BC1 alpha and BC3; `tests/bc1_dual.rs` keeps its pinned
error). The shim's `sdc_bc1_encode`, `sdc_bc1_encode_alpha` and `sdc_bc3_encode` take a `perceptual` flag and set the
weights for that call only (reset on return), so encodes on other threads keep their own (`perceptual_is_per_thread`).
Before this patch `rgbcx.cpp` had SHA-256 `04073b629c49abd8c17347e5d01c0036fedb39567d90e4e36c3157630aa4c189` and
`rgbcx.h` `863b8e8e1100fbc0695cc65fea10399c345020530b06911dd6ad17312d2ab72d`. Measurements:
docs/research/encoder-benchmark.md ("Luma-weighted BC1 error").