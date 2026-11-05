# DECISIONS archive

Rulings here are **not in force**. They moved verbatim from DECISIONS.md under their original IDs (docs/DOCUMENTATION.md §7). Never cite one as a reason for current behaviour.

This file holds 2.x rulings only. Rulings archived before 2.0 are in `.claude/DECISIONS-1.x.md` under `# Archived`.

## Note FM — Tessalith 2.0 (main team, 2026-11-02)

ARCHIVED (wording before the edit; superseded in part by Note GJ3, 2026-11-03)
- **FM27 GUI test seams sit behind a `test-hooks` feature (plan task 16, 2026-11-03).** No encoded byte changes and `identity.json` is untouched.
  - **The feature.** `sdc-gui` has a `test-hooks` feature, off by default. Its own dev-dependency (`sdc-gui = { path = ".", features = ["test-hooks"] }`, the engine's pattern, Note K) turns it on for `cargo test` and `--all-targets`; `cargo build --release` never enables it (resolver 2). `#[doc(hidden)]` is not used in `crates/gui/src`.
  - **The rule for a new seam.** A `pub` item that only tests use, and every counter, field or write that only such an item reads, is `#[cfg(feature = "test-hooks")]`. A production item that tests also call (`finish_cs_pbr`, `finish_cm_json`, `finish_xpmf`, `statics_of`, `apply_settings`, `uv.read`, `StatCache::clear`) stays plain `pub`. When a production path takes a test's replacement (the UV overlay's image builder), the production code calls a private function and the seam is a gated wrapper over it (`uv_overlay_job_with`).
  - **What is gated.** The counters and getters `filtered_builds`, `problems_builds`, `problem_lookups`, `export_files_builds`, `view_builds`, `set_cards_drawn`, `helper_rows_drawn`, `cache_builds`, `gen_lookups`, `header_reads_on_this_thread`, `plan_calls`, `uv_reads_on_this_thread`, `visible_builds`, `row_builds`, `project_builds` and `dds_thumb_requests`; the plan-hold hooks (`hold_plan_results`, `take_held`, `handle_held`, `end_export_with`); the Meshes pill-read gate (`project_gate`, `project_alive`, `project_started`, `hold_rows_read`); the Set 3D encoder and viewport stand-ins (`hold_workers`, `inject`, `inject_as`, `inject_stale`, `has_alt`, `token_of`, `alt_token_of`, `set_plugin_reader`, `attach_test_thread`, `fake_render_thread`, `reply_sender_for_test`, `render_sent_for_test`, `loaded_for_test`); and small getters (`votes_pending`, `generation`, `plan_marks`, `gen_dims_len`, `clear_gen_dims`, `gen_preview_opts`). A build without the feature does none of the counting.
  - **Tests.** Every test that reaches a seam runs under `cargo test -p sdc-gui`; `tools\doc-shots.ps1` runs `cargo test -p sdc-gui --test doc_shots`, so it gets the feature too. Gate: `cargo clippy -p sdc-gui -- -D warnings` (no feature) and `--all-targets` both pass.
  - Supersedes: tune-up #7 item 5 (the public test seams on `AppModel`; the tune-up list is outside the repo). In part: CL19, GC2 and GC11 (the `#[doc(hidden)]` wording).

ARCHIVED (wording before the edit; superseded in part by Note HA, 2026-11-04)
- **FM14 NVTT is removed everywhere; VAL-BC6H-PEAK advice reworded (plan task 4, 2026-11-02).** The optional external encoder backend (`nvcompress`) is gone: `crates/engine/src/nvtt.rs`, the `nvcompress` test fake, `Backend::Nvtt`, `EncoderVersions.nvtt`, `BatchRequest.nvtt`, `--nvtt` on `convert`, the three codes `ENC-NVTT-MISSING`, `ENC-NVTT-VERSION` and `ENC-NVTT-FAILED` (118 codes become 115), the GUI's NVTT path (Advanced drawer, `gui.toml` `nvtt_path`, the picker purpose), the Encoder row in the settings, preset and bulk panels, the Encoder filter in the Textures column (and the saved `filter_encoder`), the `NVTT-Backend` wiki page, the bug template's NVTT dropdown and the NVTT 3 rows of the README charts. A preview now always encodes with the profile's own backend. `encoder.backend = "nvtt"` and an `allow` list naming one of the three codes are errors that name the removal (`migrate::REMOVED`, FM10); `--nvtt` is a usage error (exit 2) from clap. `identity.json` is unchanged: no built-in output moved. `VAL-BC6H-PEAK` advice no longer offers NVTT as the other way to keep the peak: it says to set `encoder.effort = "slowest"`. Supersedes: Y9, all of Note NV, AF3 (and its AF9 sub-bullet), BC9, BC12, BQ3, CV3 and CV15 (moved whole); AF10, AF11, AG40, AG42, AG46, AG111, AM5, BC13, BS3, CN33, CP4, CV21, DM4, DT2, FB8, FE7, FF6, FL2 and the BC7-exactness bullet of Note AH (the NVTT wording edited out, old wording archived). Code comments that cited NV, AF3 or BC9 went with the code they described.

## Note HA — README encoder charts: NVTT back as a reference, GPU-decoder scoring (main team, 2026-11-04)

ARCHIVED (wording before the edit; superseded in part by Note HA3, 2026-11-04)
- **HA3 Rows of one chart share their sample (BC6H aside, HA2).** bc7e `max` is charted at its round-1 Δ (+0.15 dB RGB, +0.04 dB premultiplied alpha) like the other BC7 rows, not the 150-texture tuning sample (+0.32); the README states its gain in place of the "separate sample" note. Colour follows the encoder in every chart (bc7e and rgbcx blue, ISPC orange, NVTT aqua, Compressonator violet, texconv magenta), with the stripe patterns kept as a second cue. `tools/readme-charts.py` asserts that every full-width line fits the 780 px chart.

## Index

ARCHIVED (wording before the edit; superseded in part by Note FM, 2026-11-03)
- Bake and RNM: E, M, R, W, Y, AL, BP, CL, CO, DP

ARCHIVED (wording before the edit; superseded in part by Note FM, 2026-11-03)
- render3d: BT, CI, CJ, CM, CV, CX, DL, GA, GG, GJ, GL, GM, GN, GP

ARCHIVED (wording before the edit; superseded in part by Note FN, 2026-11-04)
- Bake and RNM: E, M, R, W, Y, AL, BP, CL, CO, DP, FM

ARCHIVED (wording before the edit; superseded in part by Note FN, 2026-11-04)
- Exports (PGPatcher, CS PBR, CM, XPMF, Also): P, V, AA, AB, AC, AD, AF, AG, AN, AT, BO, BR, BV, BY, CN, CO, CV, GE, GI, FM

ARCHIVED (wording before the edit; superseded in part by Note FN, 2026-11-04)
- GUI Files and Inspector: N, P, S, U, Z, AA, AE, AF, AG, AM, AN, AO, AV, AY, BH, BI, BN, BO, BR, BS, BV, BY, CA, CC, CD, CE, CF, CG, CV, DN, DO, DP, DU, DZ, FA, FB, FC, FD, FI, FK, FL, FM

ARCHIVED (wording before the edit; superseded in part by Note FN, 2026-11-04)
- GUI Dialogs and Export: AG, BH, BO, BR, BV, BY, CA, CD, CE, CF, CN, CV, DT, FB, FC, FI, FM

ARCHIVED (wording before the edit; superseded in part by Note FO, 2026-11-04)
- Cubemaps: DL, DN, DS, DT, FA, FC, FM

ARCHIVED (wording before the edit; superseded in part by Note FO, 2026-11-04)
- GUI Meshes view: BA, BT, CV, CZ, FC, FI, GC, GD, GH, GJ, GK, GN, GO, GP

ARCHIVED (wording before the edit; superseded in part by Note FO, 2026-11-04)
- GUI Preview and 3D: N, AG, AQ, AU, BS, CE, CJ, CL, CM, DT, FA, GD, GG, GH, GJ, GK, GL, GM, FM

ARCHIVED (wording before the edit; superseded in part by Note GQ, 2026-11-04)
- GUI Meshes view: BA, BT, CV, CZ, FC, FI, GC, GD, GH, GJ, GK, GN, GO, GP, FO

ARCHIVED (wording before the edit; superseded in part by Note GQ, 2026-11-04)
- GUI Preview and 3D: N, AG, AQ, AU, BS, CE, CJ, CL, CM, DT, FA, GD, GG, GH, GJ, GK, GL, GM, FM, FO

ARCHIVED (wording before the edit; superseded in part by Note GQ, 2026-11-04)
- modscan: BE, BQ, BT, BU, BW, BY, CK, CM, CN, CV, CW, DA, GA, GB, GC, GD, GJ, GO, GP

ARCHIVED (wording before the edit; superseded in part by Note GQ, 2026-11-04)
- esp: GG, GO

ARCHIVED (wording before the edit; superseded in part by Note GQ, 2026-11-04)
- nif: BT, CI, CM, CQ, CR, CS, CU, CV, CY, DJ, DK, GA, GC, GD, GF, GH, GP

ARCHIVED (wording before the edit; superseded in part by Note GQ, 2026-11-04)
- render3d: BT, CI, CJ, CM, CV, CX, DL, GA, GG, GJ, GL, GM, GN, GP, FM

ARCHIVED (wording before the edit; superseded in part by Note FS, 2026-11-04)
- GUI Files and Inspector: N, P, S, U, Z, AA, AE, AF, AG, AM, AN, AO, AV, AY, BH, BI, BN, BO, BR, BS, BV, BY, CA, CC, CD, CE, CF, CG, CV, DN, DO, DP, DU, DZ, FA, FB, FC, FD, FI, FK, FL, FM, FN, FO, FP

ARCHIVED (wording before the edit; superseded in part by Note FS, 2026-11-04)
- GUI Dialogs and Export: AG, BH, BO, BR, BV, BY, CA, CD, CE, CF, CN, CV, DT, FB, FC, FI, FM, FN, FO, FP

ARCHIVED (wording before the edit; superseded in part by Note FT, 2026-11-04)
- Cubemaps: DL, DN, DS, DT, FA, FC, FM, FO, FQ

ARCHIVED (wording before the edit; superseded in part by Note FT, 2026-11-04)
- GUI Files and Inspector: N, P, S, U, Z, AA, AE, AF, AG, AM, AN, AO, AV, AY, BH, BI, BN, BO, BR, BS, BV, BY, CA, CC, CD, CE, CF, CG, CV, DN, DO, DP, DU, DZ, FA, FB, FC, FD, FI, FK, FL, FM, FN, FO, FS, FP

ARCHIVED (wording before the edit; superseded in part by Note FT, 2026-11-04)
- GUI Settings panel: S, V, X, Z, AF, AG, AJ, AN, AS, AW, BM, BS, CC, CL, CV, FB, FC, FM, FS

ARCHIVED (wording before the edit; superseded in part by Note FT, 2026-11-04)
- GUI Preview and 3D: N, AG, AQ, AU, BS, CE, CJ, CL, CM, DT, FA, GD, GG, GH, GJ, GK, GL, GM, FM, FO, GQ, FS

ARCHIVED (wording before the edit; superseded in part by Note FT, 2026-11-04)
- GUI Dialogs and Export: AG, BH, BO, BR, BV, BY, CA, CD, CE, CF, CN, CV, DT, FB, FC, FI, FM, FN, FO, FS, FP

ARCHIVED (wording before the edit; superseded in part by Note FV, 2026-11-04)
- Cubemaps: DL, DN, DS, DT, FA, FC, FM, FO, FT, FQ

ARCHIVED (wording before the edit; superseded in part by Note FV, 2026-11-04)
- Exports (PGPatcher, CS PBR, CM, XPMF, Also): P, V, AA, AB, AC, AD, AF, AG, AN, AT, BO, BR, BV, BY, CN, CO, CV, GE, GI, FM, FN, FO, FP, FT

ARCHIVED (wording before the edit; superseded in part by Note FU, 2026-11-04)
- GUI Files and Inspector: N, P, S, U, Z, AA, AE, AF, AG, AM, AN, AO, AV, AY, BH, BI, BN, BO, BR, BS, BV, BY, CA, CC, CD, CE, CF, CG, CV, DN, DO, DP, DU, DZ, FA, FB, FC, FD, FI, FK, FL, FM, FN, FO, FP, FT, FS

ARCHIVED (wording before the edit; superseded in part by Note FU, 2026-11-04)
- GUI Settings panel: S, V, X, Z, AF, AG, AJ, AN, AS, AW, BM, BS, CC, CL, CV, FB, FC, FM, FT, FS

ARCHIVED (wording before the edit; superseded in part by Note FU, 2026-11-04)
- GUI Preview and 3D: N, AG, AQ, AU, BS, CE, CJ, CL, CM, DT, FA, GD, GG, GH, GJ, GK, GL, GM, FM, FO, GQ, FT, FS

ARCHIVED (wording before the edit; superseded in part by Note FU, 2026-11-04)
- GUI Dialogs and Export: AG, BH, BO, BR, BV, BY, CA, CD, CE, CF, CN, CV, DT, FB, FC, FI, FM, FN, FO, FP, FS

ARCHIVED (wording before the edit; superseded in part by Note FW, 2026-11-04)
- Exports (PGPatcher, CS PBR, CM, XPMF, Also): P, V, AA, AB, AC, AD, AF, AG, AN, AT, BO, BR, BV, BY, CN, CO, CV, GE, GI, FM, FN, FO, FP, FV, FT

ARCHIVED (wording before the edit; superseded in part by Note FW, 2026-11-04)
- GUI Files and Inspector: N, P, S, U, Z, AA, AE, AF, AG, AM, AN, AO, AV, AY, BH, BI, BN, BO, BR, BS, BV, BY, CA, CC, CD, CE, CF, CG, CV, DN, DO, DP, DU, DZ, FA, FB, FC, FD, FI, FK, FL, FM, FN, FO, FS, FP, FU, FT

ARCHIVED (wording before the edit; superseded in part by Note FW, 2026-11-04)
- GUI Settings panel: S, V, X, Z, AF, AG, AJ, AN, AS, AW, BM, BS, CC, CL, CV, FB, FC, FM, FS, FT

ARCHIVED (wording before the edit; superseded in part by Note FW, 2026-11-04)
- GUI Preview and 3D: N, AG, AQ, AU, BS, CE, CJ, CL, CM, DT, FA, GD, GG, GH, GJ, GK, GL, GM, FM, FO, GQ, FS, FT

ARCHIVED (wording before the edit; superseded in part by Note FW, 2026-11-04)
- GUI Dialogs and Export: AG, BH, BO, BR, BV, BY, CA, CD, CE, CF, CN, CV, DT, FB, FC, FI, FM, FN, FO, FS, FP, FU

ARCHIVED (wording before the edit; superseded in part by Note FY, 2026-11-04)
- Presets and config: G, N, S, T, X, AE, AL, AM, AP, AS, AV, AZ, BC, BJ, BO, BQ, BR, BV, CN, CV, FB, FL, FM, FQ, FX

ARCHIVED (wording before the edit; superseded in part by Note FY, 2026-11-04)
- GUI Files and Inspector: N, P, S, U, Z, AA, AE, AF, AG, AM, AN, AO, AV, AY, BH, BI, BN, BO, BR, BS, BV, BY, CA, CC, CD, CE, CF, CG, CV, DN, DO, DP, DU, DZ, FA, FB, FC, FD, FI, FK, FL, FM, FN, FO, FP, FT, FS, FW, FU

ARCHIVED (wording before the edit; superseded in part by Note FZ, 2026-11-04)
- GUI Preview and 3D: N, AG, AQ, AU, BS, CE, CJ, CL, CM, DT, FA, GD, GG, GH, GJ, GK, GL, GM, FM, FO, GQ, FS, FT, FY

ARCHIVED (wording before the edit; superseded in part by Note HA, 2026-11-04)
- Process and docs: Answers, BX, DM, FD, FG, FH, FM

ARCHIVED (wording before the edit; superseded in part by Note HB, 2026-11-05)
- Engine policy (formats, alpha gate, versions, writes): Answers, A, B, D, E, F, I, K, L, M, O, R, S, T, W, Y, AC, AG, AK, AV, AW, AX, AZ, BB, BC, BF, BJ, BL, BQ, BV, DB, DC, DM, DS, DY, FA, FF, FL, GJ, FM, FQ, FV, FZ

ARCHIVED (wording before the edit; superseded in part by Note HB, 2026-11-05)
- Manifest and outputs: F, M, N, S, Y, AE, AF, AL, BJ, BQ, BV, CN, CT, DM, DY, FC, FM, FZ

ARCHIVED (wording before the edit; superseded in part by Note HB, 2026-11-05)
- Presets and config: G, N, S, T, X, AE, AL, AM, AP, AS, AV, AZ, BC, BJ, BO, BQ, BR, BV, CN, CV, FB, FL, FM, FQ, FX, FY

ARCHIVED (wording before the edit; superseded in part by Note HB, 2026-11-05)
- Exports (PGPatcher, CS PBR, CM, XPMF, Also): P, V, AA, AB, AC, AD, AF, AG, AN, AT, BO, BR, BV, BY, CN, CO, CV, GE, GI, FM, FN, FO, FP, FT, FW, FV

ARCHIVED (wording before the edit; superseded in part by Note HB, 2026-11-05)
- GUI Files and Inspector: N, P, S, U, Z, AA, AE, AF, AG, AM, AN, AO, AV, AY, BH, BI, BN, BO, BR, BS, BV, BY, CA, CC, CD, CE, CF, CG, CV, DN, DO, DP, DU, DZ, FA, FB, FC, FD, FI, FK, FL, FM, FN, FO, FP, FT, FS, FW, FU, FY

ARCHIVED (wording before the edit; superseded in part by Note HB, 2026-11-05)
- GUI Dialogs and Export: AG, BH, BO, BR, BV, BY, CA, CD, CE, CF, CN, CV, DT, FB, FC, FI, FM, FN, FO, FP, FS, FW, FU

ARCHIVED (wording before the edit; superseded in part by Note HB, 2026-11-05)
- Process and docs: Answers, BX, DM, FD, FG, FH, FM, HA

## Digest

ARCHIVED (superseded by Note FM, 2026-11-03)
- OPEN: `resize` upscaling (`pot-up`, `pot-nearest`). [S]

ARCHIVED (wording before the edit; superseded in part by Note FN, 2026-11-04)
- Anything the user must act on is a Problems row with its action; hand remedies are row buttons. [FC3, FC10]

ARCHIVED (wording before the edit; superseded in part by Note GQ, 2026-11-04)
- The visible list is cached; row reads never land across a scan start; chips wait for a settled store. [GC11, GC14, GC15]

ARCHIVED (wording before the edit; superseded in part by Note GQ, 2026-11-04)
- The edit panel's footer (`meshes-edit-export`, before the tab strip) shows a status line ("Ready", "Blocked: <reason>", idle "No edits to export."), "Writes <data path> to <export path>", the source-never-changed line and Review…; with no export folder it stays blocked with the reason. The button reads "Export NIF · N edits", "Export NIF" or "Export NIF again". [CZ2, CZ3, CZ8]

ARCHIVED (wording before the edit; superseded in part by Note GQ, 2026-11-04)
- `esp::mato_data` returns None for the whole plugin on any fault its walk reaches (bad group size, bad zlib, a subrecord past its record) and never clamps; the first record with at least 48 bytes of DATA wins. [GG11]

ARCHIVED (wording before the edit; superseded in part by Note GQ, 2026-11-04)
- The writer re-opens per op, writes atomically and never writes what its parser rejects. The reader seeks by the block size array; `Header::check_file_length` and each decoder's exact-size check refuse a file whose sizes are wrong. [CI1, CI4, CI5, CY2]

ARCHIVED (wording before the edit; superseded in part by Note GQ, 2026-11-04)
- Edit source and documents: `pick_source` compares roots ignoring path spelling and never takes the edit mod's own copy or any `sdc NIF Edits` folder. An autosave is deleted only when this session holds it; a session stays unsaved until its save succeeds; a root change waits for the live edit to move (else blocked with the reason, retried when that file changes); the newer of autosave and folder document wins. [CV1, GD11, GD12, GD13, GD20, GD21, GD24, GD27, GD28]

ARCHIVED (wording before the edit; superseded in part by Note GQ, 2026-11-04)
- Meshes list: picking a mod never deselects; a reload keeps the picked mod while it has NIFs and is not derived, else Any mod, and reselects a NIF picked under Any mod once the store settles. Wins hides under Any mod; Overridden lists every losing copy; Conflicted is `n_copies > 1`; Problems is a problem or an LE mesh. Search matches path (no `meshes/`) or mod, ASCII case-insensitive, `\` as `/`; the texture chip (`users_of_texture`) combines with it; "Show in Meshes" sets Any mod, clears search and chips. Row chips list warnings first; "In game" is short, full text as its AccessKit name. [BT5, GC4, GC6, GC7, GC8, GC9, GC10, GC16, GC17, GC19, GC20]

ARCHIVED (wording before the edit; superseded in part by Note GQ, 2026-11-04)
- `settle` compiles the keys the pool already took; `set_material` evicts every texture-less missing entry and keeps decode-error entries; a replaced model load drops its queued shader compiles. [BT1, BT2, CX1, CV24]

ARCHIVED (wording before the edit; superseded in part by Note FQ, 2026-11-04)
- Engine policy (formats, alpha gate, versions, writes): Answers, A, B, D, E, F, I, K, L, M, O, R, S, T, W, Y, AC, AG, AK, AV, AW, AX, AZ, BB, BC, BF, BJ, BL, BQ, BV, DB, DC, DM, DS, DY, FA, FF, FL, GJ, FM

ARCHIVED (wording before the edit; superseded in part by Note FQ, 2026-11-04)
- Encoders: F, O, S, T, BZ, CB, CP, FE, FF, FJ, FM

ARCHIVED (wording before the edit; superseded in part by Note FQ, 2026-11-04)
- Packing and helpers: B, H, I, M, R, W, AL, BB, BP, BQ, BV, CN, DR, DY, FB, FM

ARCHIVED (wording before the edit; superseded in part by Note FQ, 2026-11-04)
- Cubemaps: DL, DN, DS, DT, FA, FC, FM, FO

ARCHIVED (wording before the edit; superseded in part by Note FQ, 2026-11-04)
- Presets and config: G, N, S, T, X, AE, AL, AM, AP, AS, AV, AZ, BC, BJ, BO, BQ, BR, BV, CN, CV, FB, FL, FM

ARCHIVED (wording before the edit; superseded in part by Note FS, 2026-11-04)
- GUI Files and Inspector: N, P, S, U, Z, AA, AE, AF, AG, AM, AN, AO, AV, AY, BH, BI, BN, BO, BR, BS, BV, BY, CA, CC, CD, CE, CF, CG, CV, DN, DO, DP, DU, DZ, FA, FB, FC, FD, FI, FK, FL, FM, FN, FO

ARCHIVED (wording before the edit; superseded in part by Note FS, 2026-11-04)
- GUI Settings panel: S, V, X, Z, AF, AG, AJ, AN, AS, AW, BM, BS, CC, CL, CV, FB, FC, FM

ARCHIVED (wording before the edit; superseded in part by Note FS, 2026-11-04)
- GUI Dialogs and Export: AG, BH, BO, BR, BV, BY, CA, CD, CE, CF, CN, CV, DT, FB, FC, FI, FM, FN, FO

ARCHIVED (wording before the edit; superseded in part by Note FS, 2026-11-04)
- GUI Preview and 3D: N, AG, AQ, AU, BS, CE, CJ, CL, CM, DT, FA, GD, GG, GH, GJ, GK, GL, GM, FM, FO, GQ

ARCHIVED (wording before the edit; superseded in part by Note FP, 2026-11-04)
- Exports (PGPatcher, CS PBR, CM, XPMF, Also): P, V, AA, AB, AC, AD, AF, AG, AN, AT, BO, BR, BV, BY, CN, CO, CV, GE, GI, FM, FN, FO

ARCHIVED (wording before the edit; superseded in part by Note FP, 2026-11-04)
- GUI Files and Inspector: N, P, S, U, Z, AA, AE, AF, AG, AM, AN, AO, AV, AY, BH, BI, BN, BO, BR, BS, BV, BY, CA, CC, CD, CE, CF, CG, CV, DN, DO, DP, DU, DZ, FA, FB, FC, FD, FI, FK, FL, FM, FN, FO

ARCHIVED (wording before the edit; superseded in part by Note FP, 2026-11-04)
- GUI Dialogs and Export: AG, BH, BO, BR, BV, BY, CA, CD, CE, CF, CN, CV, DT, FB, FC, FI, FM, FN, FO

ARCHIVED (wording before the edit; superseded in part by Note FU, 2026-11-04)
- GUI Files and Inspector: N, P, S, U, Z, AA, AE, AF, AG, AM, AN, AO, AV, AY, BH, BI, BN, BO, BR, BS, BV, BY, CA, CC, CD, CE, CF, CG, CV, DN, DO, DP, DU, DZ, FA, FB, FC, FD, FI, FK, FL, FM, FN, FO, FS, FP

ARCHIVED (wording before the edit; superseded in part by Note FU, 2026-11-04)
- GUI Dialogs and Export: AG, BH, BO, BR, BV, BY, CA, CD, CE, CF, CN, CV, DT, FB, FC, FI, FM, FN, FO, FS, FP

ARCHIVED (wording before the edit; superseded in part by Note FV, 2026-11-04)
- Engine policy (formats, alpha gate, versions, writes): Answers, A, B, D, E, F, I, K, L, M, O, R, S, T, W, Y, AC, AG, AK, AV, AW, AX, AZ, BB, BC, BF, BJ, BL, BQ, BV, DB, DC, DM, DS, DY, FA, FF, FL, GJ, FM, FQ

ARCHIVED (wording before the edit; superseded in part by Note FV, 2026-11-04)
- Packing and helpers: B, H, I, M, R, W, AL, BB, BP, BQ, BV, CN, DR, DY, FB, FM, FQ

ARCHIVED (wording before the edit; superseded in part by Note FV, 2026-11-04)
- Bake and RNM: E, M, R, W, Y, AL, BP, CL, CO, DP, FM, FN, FO

ARCHIVED (wording before the edit; superseded in part by Note FV, 2026-11-04)
- Exports (PGPatcher, CS PBR, CM, XPMF, Also): P, V, AA, AB, AC, AD, AF, AG, AN, AT, BO, BR, BV, BY, CN, CO, CV, GE, GI, FM, FN, FO, FP

ARCHIVED (wording before the edit; superseded in part by Note FV, 2026-11-04)
- The bent layer's AO source, in order: raw AO (R), `_orm` (R), `_rmaos`/`_rfaos` (B), read only for `bent` in `ao_blend` with `rnm.bent.ao = true`, else uniform weight. The manifest role carries the channel (`bent.ao.r`, `bent.ao.b`), so swapping a byte-identical file between them re-encodes the normal. [W9, AL4, W16]

ARCHIVED (wording before the edit; superseded in part by Note FT, 2026-11-04)
- Exports (PGPatcher, CS PBR, CM, XPMF, Also): P, V, AA, AB, AC, AD, AF, AG, AN, AT, BO, BR, BV, BY, CN, CO, CV, GE, GI, FM, FN, FO, FP

ARCHIVED (wording before the edit; superseded in part by Note FT, 2026-11-04)
- GUI Files and Inspector: N, P, S, U, Z, AA, AE, AF, AG, AM, AN, AO, AV, AY, BH, BI, BN, BO, BR, BS, BV, BY, CA, CC, CD, CE, CF, CG, CV, DN, DO, DP, DU, DZ, FA, FB, FC, FD, FI, FK, FL, FM, FN, FO, FP

ARCHIVED (wording before the edit; superseded in part by Note FT, 2026-11-04)
- GUI Settings panel: S, V, X, Z, AF, AG, AJ, AN, AS, AW, BM, BS, CC, CL, CV, FB, FC, FM

ARCHIVED (wording before the edit; superseded in part by Note FT, 2026-11-04)
- GUI Preview and 3D: N, AG, AQ, AU, BS, CE, CJ, CL, CM, DT, FA, GD, GG, GH, GJ, GK, GL, GM, FM, FO, GQ

ARCHIVED (wording before the edit; superseded in part by Note FT, 2026-11-04)
- Cubemaps: DL, DN, DS, DT, FA, FC, FM, FO

ARCHIVED (wording before the edit; superseded in part by Note FY, 2026-11-04)
- Presets and config: G, N, S, T, X, AE, AL, AM, AP, AS, AV, AZ, BC, BJ, BO, BQ, BR, BV, CN, CV, FB, FL, FM, FQ

ARCHIVED (wording before the edit; superseded in part by Note FY, 2026-11-04)
- GUI Files and Inspector: N, P, S, U, Z, AA, AE, AF, AG, AM, AN, AO, AV, AY, BH, BI, BN, BO, BR, BS, BV, BY, CA, CC, CD, CE, CF, CG, CV, DN, DO, DP, DU, DZ, FA, FB, FC, FD, FI, FK, FL, FM, FN, FO, FS, FP, FU, FT

ARCHIVED (wording before the edit; superseded in part by Note FY, 2026-11-04)
- GUI Settings panel: S, V, X, Z, AF, AG, AJ, AN, AS, AW, BM, BS, CC, CL, CV, FB, FC, FM, FS, FT

ARCHIVED (wording before the edit; superseded in part by Note FY, 2026-11-04)
- GUI Preview and 3D: N, AG, AQ, AU, BS, CE, CJ, CL, CM, DT, FA, GD, GG, GH, GJ, GK, GL, GM, FM, FO, GQ, FS, FT

ARCHIVED (wording before the edit; superseded in part by Note FY, 2026-11-04)
- Cubemaps: DL, DN, DS, DT, FA, FC, FM, FO, FQ, FT

ARCHIVED (wording before the edit; superseded in part by Note FX, 2026-11-04)
- Presets and config: G, N, S, T, X, AE, AL, AM, AP, AS, AV, AZ, BC, BJ, BO, BQ, BR, BV, CN, CV, FB, FL, FM, FQ

ARCHIVED (wording before the edit; superseded in part by Note FX, 2026-11-04)
- CLI: I, J, K, X, BC, BQ, CN, DY, FC, FM

ARCHIVED (wording before the edit; superseded in part by Note FW, 2026-11-04)
- PGPatcher complex-material JSON is `<job output>.json` under the export root, validated against the vendored schema, written only after an export with no failures; problems get a note. No `_m.json` importer; an `_m.json` Tessalith did not write is never overwritten; `disabled` round-trips. [AT3, GE13]

ARCHIVED (wording before the edit; superseded in part by Note FW, 2026-11-04)
- Exports (PGPatcher, CS PBR, CM, XPMF, Also): P, V, AA, AB, AC, AD, AF, AG, AN, AT, BO, BR, BV, BY, CN, CO, CV, GE, GI, FM, FN, FO, FP, FT

ARCHIVED (wording before the edit; superseded in part by Note FW, 2026-11-04)
- GUI Dialogs and Export: AG, BH, BO, BR, BV, BY, CA, CD, CE, CF, CN, CV, DT, FB, FC, FI, FM, FN, FO, FP, FS

ARCHIVED (wording before the edit; superseded in part by Note FW, 2026-11-04)
- GUI Files and Inspector: N, P, S, U, Z, AA, AE, AF, AG, AM, AN, AO, AV, AY, BH, BI, BN, BO, BR, BS, BV, BY, CA, CC, CD, CE, CF, CG, CV, DN, DO, DP, DU, DZ, FA, FB, FC, FD, FI, FK, FL, FM, FN, FO, FP, FT, FS

ARCHIVED (wording before the edit; superseded in part by Note FZ, 2026-11-04)
- Encoders: F, O, S, T, BZ, CB, CP, FE, FF, FJ, FM, FQ

ARCHIVED (wording before the edit; superseded in part by Note FZ, 2026-11-04)
- Manifest and outputs: F, M, N, S, Y, AE, AF, AL, BJ, BQ, BV, CN, CT, DM, DY, FC, FM

ARCHIVED (wording before the edit; superseded in part by Note FZ, 2026-11-04)
- Engine policy (formats, alpha gate, versions, writes): Answers, A, B, D, E, F, I, K, L, M, O, R, S, T, W, Y, AC, AG, AK, AV, AW, AX, AZ, BB, BC, BF, BJ, BL, BQ, BV, DB, DC, DM, DS, DY, FA, FF, FL, GJ, FM, FQ, FV

ARCHIVED (wording before the edit; superseded in part by Note FZ, 2026-11-04)
- GUI Preview and 3D: N, AG, AQ, AU, BS, CE, CJ, CL, CM, DT, FA, GD, GG, GH, GJ, GK, GL, GM, FM, FO, GQ, FT, FS

ARCHIVED (wording before the edit; superseded in part by Note HB4, 2026-11-05)
- Advanced skin sub-types: `skin_albedo`, `skin_rf`, `skin_sk`, `skin_wet`. `rfaos` is not a sub-type id. [R-skin-1, R-review, FM11]

ARCHIVED (wording before the edit; superseded in part by Note HB4, 2026-11-05)
- `_wet` is a suffix only on `cs` in an Advanced skin set (a real `_rfaos` member or a resolved recipe building `_rfaos`); `group::join_skin_wet` (after grouping, before set edits) moves `x_wet` into set `x`, folding Unicode; elsewhere `_wet` stays in the name (`road_wet`). A set listing `wet` keeps a suffix-less `*_wet` member through a Set Rename. [I3, BP2, BV9]

ARCHIVED (wording before the edit; superseded in part by Note HB1, 2026-11-05)
- Advanced skin sets get no PGPatcher JSON. [AG90]

ARCHIVED (superseded by Note HB1, 2026-11-05)
- OPEN: Community Shaders Advanced Skin override JSON export (the skin card's "Export JSON…" is a disabled placeholder, spec pending). [AG90]

ARCHIVED (wording before the edit; superseded in part by Note HB5, 2026-11-05)
- The skin card has the 20 `SkinProfile` fields in CS README order and key names, grouped Specular (5), Physical (4), Edge & detail (5), SSS (3), Fuzz (3); stored in the set's reserved `skin` table (explicit values; unknown keys survive), 4 decimals, one undo step per edit (a drag merges), never bulk; `_rfaos`, `_sk`, `_wet` members show as text. [AG80, AG82, AG84, AG85, AG89]
