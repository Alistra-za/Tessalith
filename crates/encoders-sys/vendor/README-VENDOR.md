# Vendored encoder sources and objects

| Path | Upstream | Commit | Licence |
|---|---|---|---|
| bc7enc_rdo/ (rgbcx.* with the rgbcx BC4 round-to-nearest, BC4 search, BC1 dual, BC4 guard cut, BC1 alpha and BC1 perceptual patches, bc7e.ispc pristine) | https://github.com/richgel999/bc7enc_rdo | b9438627eef73a1157e84201b6fa6eb2ffd6d9f0 | rgbcx: MIT or Unlicense; bc7e.ispc: Apache-2.0 |
| ispctc/ (kernel.ispc with patches/0001-bc6h-peak-mode.patch, patches/0003-bc6h-only.patch, patches/0004-bc6h-kernel-fixes.patch, patches/0006-bc6h-speed.patch and patches/0007-bc6h-quality.patch; ispc_texcomp.cpp with patch 0003) | https://github.com/GameTechDev/ISPCTextureCompressor | 79ddbc9 + patches 0001, 0003, 0004, 0006, 0007 | MIT (kernel.ispc and ispc_texcomp.cpp modified; change notices at the top) |
| bc7e/ (bc7e.ispc, patches/0001-exact-opaque-alpha.patch, patches/0002-sdc-max-level.patch, patches/0005-bc7e-fixes.patch) | bc7enc_rdo/bc7e.ispc + sdc patches 0001, 0002 and 0005 | b9438627eef73a1157e84201b6fa6eb2ffd6d9f0 + patches 0001, 0002, 0005 | Apache-2.0 (modified; change notices at the top of the file) |
| ispc-objs/bc7e/ | built from bc7e/bc7e.ispc (pristine + patches 0001, 0002 and 0005, rebuilt on Windows 2026-10-18, fifth target added 2026-11-13, SSE4 and AVX dropped 2026-11-14, rebuilt 2026-11-23 with patch 0005): `-O2 --target=sse2,avx2,avx512skx-x16 --opt=fast-math --opt=disable-assertions`; `bc7e_avx512skx.obj` with ISPC 1.32.0, every other file with ISPC 1.31.0 | — | Apache-2.0 |
| ispc-objs/ispctc/ | built from ispctc/kernel.ispc (with patches 0001 and 0003, rebuilt on Windows 2026-10-31, fifth target added 2026-11-13, SSE4 and AVX dropped 2026-11-14, trimmed to BC6H 2026-11-14 (patch 0003), rebuilt 2026-11-22 with patch 0004, rebuilt 2026-11-24 with patch 0006, rebuilt 2026-11-25 with patch 0007): `-O2 --target=sse2,avx2,avx512skx-x16 --opt=fast-math -DSDC_BC6H_ONLY`; `kernel_avx512skx.obj` with ISPC 1.32.0, every other file with ISPC 1.31.0 | — | MIT |

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
mode 7, and ultrafast turns it off). XC: exact constants, the trailing params field `m_exact_mask` (R, G and B bits, and
A's under XC (A) below; 0 at every level's init and 8 in the shim's exact-opaque tables, set per call through the shim's
tuning slot 15): when a subset's masked channel is 0 on every texel, or 255 on every texel, a solution that does not
decode it exactly pays a penalty above any real error and a one-colour candidate takes the p-bit choice whose entries
lose the least, except that with exact opaque on mode 6's one-colour choice stays in its forced p-bits (alpha 255 wins);
proof test `tests/bc7.rs` `exact_constants_keep_a_flat_channel`; changes bytes only with the mask set (mask 0, every
level's init with exact opaque off, writes the bytes it wrote before). The two fields change the generated header (Note
LR). Q3 (all sixteen indices): with linear weights that are not all equal, `evaluate_solution` scores every palette
entry of a mode-6 alpha block with the weighted error, where it projected each texel in the unweighted metric and scored
the two indices around the projection; equal weights run the unchanged code; proof test `tests/bc7.rs`
`mode6_alpha_selectors_follow_the_weights`; changes bytes only for linear weights that are not all equal (a user's
`encoder.bc7_weights = "alphaN"`). DG: four float divisions are guarded: `evaluate_solution`'s linear projection factor,
opaque and alpha, whose denominator is 0 whenever the trial's decoded endpoints coincide (a flat subset, and some
subsets of low spread), and the two estimators' `scale`, whose denominator is 0 on a flat subset; the numerator is then
0 too, so the guard gives the index the clamp gave; unit check `tests/bc7.rs` `flat_subsets_decode_sane`; no byte
change. XC priority: when guarded channels cannot all be kept, a solution keeps the higher one, G before B before R,
across every subset (penalties of 2^58, 2^56 and 2^54 per lost channel and subset, added to the solution's real error,
so the closest solution wins within a tier: solutions that lose their channels in the same number of subsets), and a
one-colour candidate charges only the channels its subset's source holds at 0 or 255, not a mean that rounds there;
proof tests `tests/bc7.rs` `guarded_channels_conflict_by_priority`, `a_lost_channel_keeps_the_closest_solution`,
`a1_witnesses_pick_the_closest_loser` and `one_colour_packers_key_on_source_constants`; changes bytes only with the mask
set. XC (A): exact opaque is the mask's bit 3 (the shim's exact-opaque tables set it, and tuning slot 15 adds R, G and B
without clearing it), so `m_exact_opaque_alpha` is read nowhere and the generated header does not change; opaque blocks
keep the forced mode-6 p-bits (alpha 255, their bytes unchanged); in alpha blocks a subset whose alpha is 0 or 255 on
every texel keeps it exactly where the kernel finds such a solution, above G, B and R (penalty 2^60), and a rotated mode
4/5 trial charges source alpha at A's tier in the colour slot that holds it; proof test `tests/bc7.rs`
`channel_guard_a_keeps_flat_alpha`; changes bytes with exact opaque on, in alpha blocks; with A off, an RGB guard no
longer flags source alpha in a rotated mode 4/5 slot (the slot's bit was the RGB channel's before), which can change
only blocks whose alpha is 0 throughout. WP: with linear weights that are not all equal (perceptual off), the mode 6 and
mode 7 one-colour packers choose their p-bit pair by the weighted sum of the table entries' errors, where they chose it
by the unweighted sum and weighed only the chosen pair (with alpha at 64 times the colour, mode 7's packer picked for a
solid subset at (0, 0, 0, 1) the pair that decodes (0, 0, 0, 0), weighted error 64 per texel, where the pair that
decodes (1, 1, 1, 1) costs 3; the test's witness 37, solid (0, 1, 0, 1), decoded (0, 0, 0, 0) at weighted error 65
against a best pair's 2); equal weights and perceptual run the unchanged sum, and the mode 0 and mode 1 one-colour
packers keep the unweighted choice, which ranks as the weighted one only while the R, G and B weights are equal (every
shipped tuning); proof test `tests/bc7.rs` `one_colour_p_bits_follow_the_weights` (mode 7: no block was found where mode
6's change shows); changes bytes only for linear weights that are not all equal (a user's `encoder.bc7_weights =
"alphaN"`), where a mode 6 or 7 candidate packs a solid subset or a subset's mean colour as one colour. KS: a mode-4
scalar that is one value v across the block started the scalar search at la = ha = min((v + 2) >> 2, 63), which the
refit cannot move (every selector 0), so where no uber grid runs (ultrafast, veryfast and fast) a constant alpha of 252
decoded 255; mode 4 now also scores the closest 6-bit code pair and selector for v from two 256-entry tables (2-bit and
3-bit indices, built once at init), kept only under a strict `<`, so 0 keeps its start (exact) and 255 keeps S1's
shortcut (the pair is exact for every v at 3 bits and for all but 2, 6, 10, 249 and 253 at 2 bits, which it decodes 1
off); proof tests `tests/bc7.rs` `a_constant_alpha_decodes_exactly` and `every_constant_alpha_value_decodes_exactly`;
changes bytes of blocks whose mode-4 scalar is one value other than 0 and 255 (a constant alpha, or a constant colour
channel in a rotated trial), at every level, linear and perceptual: at uber 1 and above the grid then searches around
the new pair. M5: mode 5 was at neither one-colour site and the one-colour test compared alpha even where the colour
part does not score it, so a flat colour beside a varying scalar could decode a flat channel 1 off where its value is no
7-bit end's decode (an alpha block's flat R, G and B at mode 5, rotation 0); mode 5 now packs a one-colour part from
`g_bc7_mode_5_optimal_endpoints` (7-bit ends at the 2-bit optimal index, exact for every value, no p-bits) at the solid
start and as the mean-colour candidate, scored like the other packers (its weighted error plus, with a guard mask, the
penalty of the flagged constants it loses), and the one-colour test compares alpha only where it is scored, so mode 4's
one-colour shortcut is live for such blocks too; proof test `tests/bc7.rs`
`a_flat_colour_beside_a_varying_scalar_stays_exact` (the packer at the mean-colour site; the alpha-agnostic test has no
witness, since with it reverted alone the test still passes, and is held by the byte match to the measured reference
arm); changes bytes of blocks with a flat colour part beside a varying scalar: alpha blocks at every level, linear and
perceptual (mode 4's shortcut at every level, mode 5 at every level but veryfast and fast, which run no mode 5 for alpha
blocks), and opaque blocks whose rotated mode 5 holds two flat channels (linear, basic and above), and of any mode 5
colour part whose mean colour beats its fit. SF: `handle_alpha_block_mode4` and `handle_alpha_block_mode5` (alpha and
opaque calls) run the scalar search before the colour compress and skip the colour work when every active lane's scalar
error alone is already at or above the best so far, which the callers pass in; a trial scores colour plus scalar plus
any guard penalty, never below the scalar alone, so the callers' strict `<` could not have taken a skipped trial, and
the scalar search is integer and reads nothing of the colour result; BC7 bytes unchanged: every `level_identity` hash on
SSE2, AVX2 and AVX-512, `identity.json`, and every measured image's hash on AVX2 and AVX-512 equal; no test holds the
skip on its own. Made unsafe (taken when the scalar error exceeds half the best): at mode 4's site alone it moves every
linear `tests/level_identity.rs` row from veryfast to max on three paths; at mode 5's site alone, returning the larger
of the scalar error and the best so that no half-written result is taken, no pin or `tests/bc7.rs` test fails, so the
pins do not hold mode 5's skip; at both sites, with mode 5 returning the scalar error alone, `tests/bc7.rs` fails four
tests and `tests/level_identity.rs` crashes (the return contract broken: the caller can then accept a trial whose colour
part was never written). L16: a linear (non-perceptual) call ranks errors as the same call with every weight x16 would:
the uber threshold is compared at 1/16 and the two partition estimators (`color_cell_compression_est` and `_est_mode7`)
cast 16 times their float error, which writes exactly the bytes of the weights x16 (scaling integer weights by 16
commutes with every float step, so those two sites are the only ones where x16 changes bytes; the measured
decomposition); the weights passed and perceptual calls are unchanged, so callers pass x1-form weights (`[1, 1, 1, N]`
for Alpha xN) and weights x16 would rank at x256; proof test `tests/level_identity.rs`
`linear_calls_write_the_x16_bytes` (the plain call and `[1, 1, 1, N]` write the bytes recorded from the x16 calls before
L16); changes bytes of linear outputs from veryfast up (ultrafast runs neither site). The other changes alter BC7 bytes
(the pins in `tests/level_identity.rs`, `identity.json` and the measured images). The engine's BC7-only tag `bc7e_fixes`
hashes this patch file whole, so the file is frozen once merged: a later bc7e change goes in a new patch file with its
own tag.

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
moved or edited. Since patch 0007 `block_quant` and `opt_endpoints` are outside every BC6H path (BC6H calls its forks
`bc6h_block_quant` and `bc6h_opt_endpoints`), so they are unused in the shipped build though not guarded. Without the
define the source behaves as upstream plus patches 0001, 0004, 0006 and 0007. The BC7 kernels
call the same `block_quant`, so 0004's fix #1 would also reach them there (0007's S3 does not: BC7 keeps upstream's
`block_quant`, and BC6H calls the fork `bc6h_block_quant`). 0006's R2 helper changes (`block_pca_axis`
and `get_pca_bound` leave the unwritten covariance entries alone) reach BC7's three-channel calls, and its zero-div
guard in `block_quant` reaches them too; by reading the code both are value-neutral there (R2 because the skipped
covariance entries are never read with three channels, the guard because BC7's endpoints are integers), but that build
is neither compiled nor shipped, so the claim is not tested. 0007's `compute_axis` re-seed is also shared: BC7's PCA
calls reach it without the define, but it acts only on a block whose power-iteration vector has an exactly zero norm,
the same degenerate inputs as in BC6H (by reading; not compiled). No other 0007 item edits a helper BC7 calls:
astra #4's refit is the BC6H-only fork `bc6h_opt_endpoints` (upstream's `opt_endpoints` is unchanged), and R1, which
would have changed `compute_stats_masked` and `compute_covar_dc_masked`, was dropped. `channel_opt_quant` keeps its own
`int best_err`, as upstream. `tools/regen-ispc.ps1` passes `-DSDC_BC6H_ONLY` to ISPC for `kernel.ispc` only (both
compilers), and `build.rs` defines it for the C++ build. At the time (before patch 0004), BC6H bytes were checked equal
on every path (SSE2, AVX2, AVX-512): the five-level and peak-mode pins of `tests/bc6h.rs` on one synthetic 64x64 HDR
image, and 18 Poly Haven HDR files (1024x512) at all six levels. Object sizes before and after
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

ISPC TC patch 0006 "BC6H speed" (`ispctc/patches/0006-bc6h-speed.patch`, 2026-11-24) applies on top of patches 0001,
0003 and 0004. It is produced with `git diff --output=<patch> 8f98807a -- crates/encoders-sys/vendor/ispctc/kernel.ispc`
(the patch file is always regenerated whole, never edited by hand), and it is built with both compilers as before. BC6H
bytes do not change: after each item below, every pin, the random strips and the 22 measured images on SSE2, AVX2 and
AVX-512 hash equal. R2: `ep_quant_bc6h` and `ep_dequant_bc6h` loop over the three channels of each endpoint (index
`i * 4 + p`) and no longer read or write slots 3 and 7 (11 and 15 with two pairs); `block_pca_axis` scales only the
covariance entries `covar_from_stats` wrote and adds its epsilon to `covar[9]` only for four channels; `get_pca_bound`
scales only those entries (Note LT). S2's cache copy and `best_qep` still copy the unused slots, which changes no output
value. Zero-div guard: `block_quant` forms `div = max(div, 1.0f)` before `proj /= div`. Every BC6H endpoint is an
integer UF16 value, so `div` is 0 only for a region whose endpoints are equal in every channel, where 0/0 gave NaN and
`(int)` turned it into INT_MIN and the clamp into index 1; with `div >= 1` the numerator is still exactly 0, so the
index is the same, without the NaN. Q6 skip-retry: `bc6h_peak_retry` sets a per-lane `lifted` flag when the lift raises
any channel of a guarded block and runs the second search only for a lane that set it. An unlifted block would repeat
the identical search from the same reset `best_err` (1e99 here; patch 0007 makes it the finite 1e30f), and the retry
keeps its own result on a tie, which is the same data.
VeryslowPeak only (the retry runs once per guarded block). S1 rank-once: the two-region partition ranking (now part of
`bc6h_block_cache`, which S2 renamed from `bc6h_rank_partitions`; the body `bc6h_enc_2p` had, writing the new
`state->part_list`) runs once per block at the end of `bc6h_setup` and once more in `bc6h_peak_retry` for a lifted lane,
when `fastSkipTreshold > 0`; `bc6h_enc_2p` only searches the stored list. The ranking depends on the block alone, so
each two-region mode ranked the same list before (six times at Slow, Veryslow and VeryslowPeak, twice at Basic, once at
Fast). The `(bound, partition id)` order and `partial_sort_list` are unchanged. S2 pca-once: `bc6h_block_cache` also
fills `state->pca_ep_1p[8]` (the one-region `block_segment_core` fit, at every level, in `bc6h_setup` and again for a
lifted lane in `bc6h_peak_retry`) and, for each of the `n = bc6h_2p_depth(state) = min(fastSkipTreshold, 32)` ranked
partitions k, `state->pca_ep_2p[k * 16 ..]` (both regions' fits, when `fastSkipTreshold > 0`); the one helper gives the
fill and `bc6h_enc_2p` the same depth. `bc6h_enc_2p_part_fast` (given the rank) and `bc6h_enc_1p` copy those floats into
their local `ep` before `ep_quant_dequant_bc6h`, which overwrites it, so the cache stays immutable. The fit depends on
the block alone, so each mode computed the same endpoints before: 2n fits per two-region mode (six modes at Slow and
above, two at Basic, one at Fast, and only gangs that entered a mode paid at Fast and Basic) and one one-region fit per
mode (1 at Veryfast to Basic, 4 at the slow levels). Now `block_segment_core` runs 2n + 1 times per block (once more
after a lift), and every gang pays the fill. Cost: about 32.5 KiB of state per gang on AVX-512 (520 floats x 16 lanes x
4 bytes = 33,280 bytes). 7a qbounds-once: at Veryfast, Fast and Basic each admitted `enc = false` selector in
`bc6h_test_mode` built the endpoint bounds (`compute_qbounds` or `compute_qbounds2`, an 8-value `ep_quant_bc6h`) that
the next admitted selector overwrote, and only the last reached `bc6h_enc_2p` or `bc6h_enc_1p`. Now each branch records
its span and widened channel in `state->q_span` and `state->q_wide` (-1 when no channel is doubled) and sets `epb` and
`mode` as before; `bc6h_build_qbounds` builds the bounds from them (`compute_qbounds` when `q_wide` is -1,
`compute_qbounds2` otherwise), in `bc6h_test_mode` when `enc` is set and in `CompressBlockBC6H_core` right before
`bc6h_enc_2p` and before `bc6h_enc_1p`. The slow levels (all `enc = true`) build once per mode as before. S3 (a BC6H
`block_quant` that loads each region's endpoints once) was tried twice and dropped, because both attempts changed BC6H
bytes, so it is not in this patch (patch 0007 keeps it, below; Note LT). Both attempts changed the `hdr_sky.dds` entry
of `encode_versions`. The first (a hoisted reciprocal) left the pins and the random strips equal. The second (a
per-texel division) changed some of the 22 measured images at every level, from the one-region path at Veryfast on, on
AVX2. S4 prune: at Slow, Veryslow and VeryslowPeak, `bc6h_test_mode` computes `bc6h_window_bound` after
`bc6h_build_qbounds` and before the encode: the sum over the 16
texels and 3 channels of each texel channel's squared distance to the window
`[unpack_to_uf16(qbounds[p], epb), unpack_to_uf16(qbounds[4 + p], epb)]`, unpacked at the mode's endpoint bits. Every
endpoint a candidate uses is clamped to `qbounds` before it is unpacked and each decoded value lies between its two
endpoints, so the bound is below every candidate's error. A lane skips the mode's encode when the bound exceeds
`best_err * (1 + 1e-4) + 1` (a candidate replaces the best only on a strictly lower error; the slack covers float
summation order), never in a guarded block (peak mode with `peak_k >= 0`, scored against the unlifted block); the gang
skips the call only when every lane skips. A scratch build that asserted the bound against each encoded mode's best
error fired no assert, on the 22 measured images at the three slow levels and in
`random_blocks_encode_at_every_level_on_every_path` on SSE2, AVX2 and AVX-512 (counts:
docs/research/encoder-benchmark.md, "Mutations and proofs").

ISPC TC patch 0007 "BC6H quality" (`ispctc/patches/0007-bc6h-quality.patch`, 2026-11-25) applies on top of patches
0001, 0003, 0004 and 0006. It is produced with
`git diff --output=<patch> 9c452929 -- crates/encoders-sys/vendor/ispctc/kernel.ispc` (9c452929 is the speed phase's
last kernel; the patch file is always regenerated whole, never edited by hand), and it is built with both compilers as
before. It changes BC6H bytes: against the kernel of patches 0004 and 0006 (which write the same bytes) every one of
the 22 measured images (18 `hdr_real` and 4 `hdr_grad`) differs at all six levels on SSE2, AVX2 and AVX-512 (Q5 alone
does that), and the pins of `tests/bc6h.rs` and `identity.json` `hdr_sky` moved as each item says; quality and time
per level are in docs/research/encoder-benchmark.md ("Audit follow-up, quality phase"). Kept: the PCA re-seed, Q5,
astra #4, Q2 (first quantisation only), Q1, Q4, Q6, astra #6, astra #5, Q3 (N = 2), S3 (dropped in the speed phase
above, kept here as a byte-changing speed item) and the S4 guard, which changes no byte. Not kept: R1 (dropped) and
astra #2 (skipped). Each item's counts are against the kernel before it.
PCA re-seed: `compute_axis` renormalises the power-iteration vector every second iteration; when its squared norm is
exactly 0 (a covariance whose rows sum to 0 with the epsilon absorbed) it now restarts from the unit vector of the
channel with the largest covariance diagonal (the first on a tie) instead of multiplying by `RSQRT(0)`, which was NaN.
A non-zero norm takes the old path, so only such blocks change bytes. Test: `pca_survives_a_zero_covariance_row_sum`.
It changes BC6H bytes only on a path where the zero vector was reached (the AVX-512 object, seen on the Ryzen: Veryfast
to Basic), and no pin, `identity.json` entry or measured image moved. Both helpers `block_pca_axis` and `get_pca_bound`
share it (Note LT).
Q5: `bc6h_setup` converts each half code h to the UF16 target `(h + 0.5) / 31 * 64`, the centre of its decode range
`[64h/31, 64(h+1)/31)`, instead of the bottom edge `h / 31 * 64`, so a constant block's code decodes to h and not h - 1
(the peak gate above it still reads the source half bits; `orig` and the lift in `bc6h_peak_retry` follow the new
targets). Test: `constant_blocks_decode_to_their_half_code`. It changes BC6H bytes at all six levels on SSE2, AVX2 and
AVX-512 (the 18 pins, `identity.json` `hdr_sky` and every measured image).
astra #4: the least-squares refit of `bc6h_enc_1p` and `bc6h_refine_2p` (`bc6h_enc_2p_list`'s refine passes before Q3
moved them there) calls a BC6H-only fork,
`bc6h_opt_endpoints`, that fits index q at the decoder's weight `table[q] / 64` (`get_unquant_table`) instead of
`q / (levels - 1)`, with the determinant from the exact integer form `n * Sww - Sw * Sw` (a zero determinant flattens
to the mean, as before); upstream's `opt_endpoints` is unchanged and only BC7 calls it, so in this build it is no
longer called. Test: `refit_uses_the_decoder_weights`. It changes BC6H bytes at Fast, Basic, Slow, Veryslow and
VeryslowPeak on SSE2, AVX2 and AVX-512 (15 of the 18 pins, `identity.json` `hdr_sky` and every measured image at those
levels); Veryfast, which has no refit, keeps its bytes.
Q2: `bc6h_enc_1p` (its first quantisation) and `bc6h_refine_2p` (in `bc6h_enc_2p_list`, for each kept candidate since
Q3: the chosen partition's first quantisation before it) hand the
candidate to `bc6h_bracket_collapsed`; the refine passes then refit from the indices it kept and are not bracketed
themselves. For a region whose two endpoints share one code in every channel, it gives each channel that varies in the
region the largest code that rebuilds at or below the region's minimum and the smallest at or above its maximum (in the
other order for a channel whose covariance with the region's widest channel is negative), clamped to `qbounds`, and
keeps the pair when the whole block's error, peak guard included, is lower. Test:
`collapsed_regions_take_a_bracketing_pair`. It changes BC6H bytes at all six levels on SSE2, AVX2 and AVX-512: of the
pins only SSE2 Fast's, and `identity.json` `hdr_sky` not; every measured image at Fast and slower but one at Basic; at
Veryfast 11 or 12 of the 18 `hdr_real` images and 2 or 3 of the 4 `hdr_grad` ones, by path. Bracketing every refine
pass as well was measured and not kept: the same quality at 1.049 of the time against 1.018 in total.
Q1: `bc6h_refine_2p` (`bc6h_enc_2p_list`'s refine passes before Q3) keeps a working index block `cur_qblock`, which
starts as the ranked partition's indices (after
Q2's bracket) and becomes each refine pass's indices whether or not the pass won, so the next pass refits from the last
pass, as `bc6h_enc_1p` does since patch 0004, and no longer from the best's indices again (a failed pass repeated
exactly); the best `(qep, qblock, err)` is updated only on a lower error, and the block is packed once after the loop.
Test: `two_region_refine_keeps_its_best_pass`. It changes BC6H bytes at Basic, Slow, Veryslow and VeryslowPeak on SSE2,
AVX2 and AVX-512 (none of the 18 pins; `identity.json` `hdr_sky`, all 18 `hdr_real` images and 3 of the 4 `hdr_grad`
ones at those levels); Veryfast has no two-region refine pass and Fast has one, with no earlier pass to refit from, so
both keep their bytes (profiles: 2 passes at Basic and slower).
Q4: `bc6h_test_mode` gates mode 2 and 6 (the modes that double the widest channel's window) on
`max_span * margin > 2 * span` for the widest channel and `second_span * margin > span` for the next widest, instead of
on the widest against `span`, so a block with R span 6000 and G, B 2000 reaches mode 6 (window 7372.6) at Fast and
Basic where it fell to mode 1; it only admits more blocks, and the later admitted mode replaces the earlier one without
a comparison. Modes 0, 1, 5, 9 and 10 to 13 keep the old test. The slow levels pass margin 0 and never return from
either test, and Veryfast has no two-region search (its `fastSkipTreshold` is 0, so the gate is never run). Test:
`a_wide_channel_block_takes_the_doubled_window`. It changes BC6H bytes at Fast and Basic on SSE2, AVX2 and AVX-512 (6 of
the 18 pins; `identity.json` `hdr_sky` and the Veryfast, Slow, Veryslow and VeryslowPeak levels keep their bytes).
Q6: `bc6h_setup` floors each texel's luminance at `max(peak * 2^-12, 1e-6)`, not `1e-6` alone, in the geometric mean of
the peak gate, so exact-black texels (each pulled the mean down by 13.8 / 16) no longer guard a block with no
highlight: four to six of them did at luminance 0.5 to 20, and seven or more still guard by darkness alone. An
all-black block keeps the `1e-6` floor and stays unguarded. Test: `exact_black_alone_does_not_guard_a_block`. It
changes BC6H bytes only at VeryslowPeak (the other levels run no peak mode), and only for blocks the old floor guarded
wrongly: none of the 18 pins, `identity.json` `hdr_sky`, the 18 `hdr_real` or the 4 `hdr_grad` images (0 of 256 guarded
blocks change gate).
astra #6: at the slow levels (Slow, Veryslow, VeryslowPeak) `bc6h_test_mode` no longer widens only the widest channel
for mode 2's and mode 6's families. After that variant's search it builds the other two channels' `qbounds` and their
S4 window bounds (`bc6h_window_bound`); on lanes where the lower of them is below the widest variant's bound, that
channel's variant (`mode + alt`, `q_wide = alt` and its bounds rebuilt by `bc6h_build_qbounds`) runs one extra
two-region search and competes through `best_err` (S4's prune test applies to it, with its own bound). Trying all three
variants was measured too (about +0.004 dB mPSNR at Veryslow); this one-extra-search form keeps about a third of that
gain. Test: `asymmetric_modes_try_the_better_wide_channel`. It changes BC6H bytes at Slow, Veryslow and VeryslowPeak on
SSE2, AVX2 and AVX-512 (none of the 18 pins; `identity.json` `hdr_sky`, all 18 `hdr_real` images and 3 of the 4
`hdr_grad` ones at those levels); Veryfast, Fast and Basic keep their bytes.
astra #5: the lift in `bc6h_peak_retry` (a guarded block's second search) raised each channel to 12 half exponent steps
below its texel's largest, which leaves the normal range under half 0.25: the lift fell short there and, under 0.125,
changed nothing. For a texel whose largest channel's half code `hm` is below `0x3400` the lift now takes `2^-12` of the
value as a subnormal half code (`hm - hlo` codes below, `hlo = round(half(hm) * 4096)`); at or above `0x3400` the old
expression is kept, so blocks whose lifted texels are all that bright keep their bytes. Test:
`peak_mode_keeps_a_dim_blue_zero_sun`. It changes BC6H bytes only at VeryslowPeak and only for guarded blocks with such
a texel: none of the 18 pins, `identity.json` `hdr_sky`, the 18 `hdr_real` or the 4 `hdr_grad` images.
Q3: `bc6h_enc_2p_list` keeps the `Q3_N` = 2 lowest-error first quantisations of the ranked partitions (a lower error
takes a slot and the old one moves down, an equal one stays behind) instead of only the lowest, and runs each through
`bc6h_refine_2p`, the bracket and refine passes moved out of the function unchanged; a later candidate replaces the best
only on a strictly lower error, so the lowest one's result is never lost. Only `state->slow_mode` keeps more than one
(Slow, Veryslow and VeryslowPeak); Veryfast, Fast and Basic keep their bytes. Test:
`two_region_refine_tries_more_partitions`. It changes BC6H bytes at Slow, Veryslow and VeryslowPeak on SSE2, AVX2 and
AVX-512 (the 9 pins of those levels; `identity.json` `hdr_sky`, all 18 `hdr_real` and all 4 `hdr_grad` images at those
levels, except that AVX-512 keeps 1 of the 4 `hdr_grad` images at Veryslow and VeryslowPeak). `Q3_N` = 4 was measured
too and costs 1.26 times the previous kernel's VM time at Veryslow and VeryslowPeak (over the 10 % limit); it is not
built.
S4 guard (changes no byte): `bc6h_mode_cannot_win` prunes only when `state->best_err < 1e30f`, and the state's two
`best_err` initialisers (`CompressBlockBC6H` and the reset in `bc6h_peak_retry`) are the finite `1e30f` instead of
`1e99`, which is +inf in float, so the state's `best_err`, the value the S4 prune reads, is never an infinity. Every
scored error is far below 1e30f (the largest, `bc6h_peak_guard`'s `(terr + 1) * 1e13f`, is about 2e24), so the first
candidate still always wins. Q3's own `cand_err` sentinels in `bc6h_enc_2p_list` are `1e30f` too, so a folded compare
cannot leave a kept partition at -1. Only that function's local `best_err = 1e99` stays as upstream wrote it: it is
only compared with `<` against finite errors and never feeds the prune. Proof that no byte moved (for the state
sentinels and again for `cand_err`): the 22 measured images on SSE2, AVX2 and AVX-512 equal the previous kernel's in
every column but the time (132 of 132 rows each), the random strips (48 hashes, 72 with AVX-512), the pins and
`identity.json` unchanged, and the scratch assert build of S4 (above) fired on none of the lane encodes of the 22
images on AVX2 (about 15.97 million) or of `random_blocks_encode_at_every_level_on_every_path` (212,686; 319,029 with
AVX-512).
S3: every BC6H quantisation (`bc6h_enc_1p`, `bc6h_enc_2p_part_fast`, `bc6h_refine_2p`, `bc6h_bracket_collapsed`) calls
`bc6h_block_quant`, which loads each region's endpoints once and multiplies by each region's reciprocal distance, formed
once after the zero-div guard, instead of gathering 12 endpoint values and dividing per texel; the tie rule and the
per-texel error sum are `block_quant`'s, and the BC7 kernels keep upstream's `block_quant` (no longer called in this
build). It changes BC6H bytes in some of the 22 measured images at every level on SSE2, AVX2 and AVX-512 (every
quality mean delta +0.000), but no pin, random strip or `identity.json` entry; Veryslow and VeryslowPeak take about
5.5 % less time, Slow 5 % more. No test fails without it, so its evidence is a mutation: swapping the region select in
`bc6h_block_quant` fails 8 `bc6h` tests and moves 45 of the 72 strip hashes (Note LT).
R1 (dropped): centring the covariance sums of the PCA (`compute_stats_masked`, `compute_covar_dc_masked`) would have
kept the covariance of bright smooth coloured blocks, which float32 raw second moments lose (test
`a_brighter_copy_is_not_worse`: without it the bright copy is certainly worse on all 8 witnesses on AVX2, with it all 8
decode as the dim copy shifted). `hdr_grad` sky_sun lost 0.074 dB mPSNR at Veryslow and VeryslowPeak, against gains of
1.375 dB at Veryfast and 0.902 dB at Fast, and the owner dropped it. The kernel keeps the uncentred sums; the test and
its witness search stay in the reverted commit 8fca6e78 only.
astra #2 (skipped by the owner): giving a guarded block's peak texel a palette entry that decodes at 0.9 of its source
luminance or more was not built; the bench holds 2 blocks whose peak texel decodes below that (both in satara_night),
and no other index reaches 0.9 for either, so no byte and no test come from it.

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