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

ARCHIVED (wording before the edit; superseded in part by Note HB3, 2026-11-05)
  - **The behaviours stay; only the message goes.** Alpha 251..=254 still counts as opaque (Note L); a `data` alpha that is constant is still written as `data`; mip 0 of a short normal is still renormalised under `mips.normal_base`; `skin_wet_form` still picks BC4 height or BC7 normal (`skin_wet_notice` is gone); a loose map still wins its slot over an `_orm` channel (`Recipe.notes` and the `prefer` helper are gone); a bent layer with no AO source still uses a uniform weight; the env `_m` is still written only for a scanned Environment Map mesh.

ARCHIVED (wording before the edit; superseded in part by Note GU, 2026-11-07)
  - **GUI.** `pbr_export::export_asks` starts with the photo crops. The Files-row table in `model/bottom.rs` lists `DERIVE-NIF-SUGGEST` alone. Files touched under the NIF team's shared GUI paths: `crates/gui/src/model/{bottom,code_text,items,mod,pbr_export}.rs` and `crates/gui/src/ui/review.rs`; `ui/mod.rs` and `worker.rs` are untouched.

ARCHIVED (wording before the edit; superseded in part by Note GU, 2026-11-07)
  - **Warnings are Problems rows.** `export_asks` (the PGPatcher set pitfalls, the portrait photo crop, the CS PBR pitfalls) feed Problems as ordinary Warning rows (`carried`) and nothing else: no Warnings column in Review, no tick, no Acknowledge all, no `{n} warnings in Problems` link, no block. A pitfall alone does not open Review, so the header Export runs when nothing else needs an answer. A set with no match (V4) is one of these rows too. A NIF edit that cannot be written is listed in the Review footer under `NIF edits skipped (n)`, one amber `{path}: {reason}` row each, and never blocks.

ARCHIVED (wording before the edit; superseded in part by Note GU, 2026-11-07)
  - **Exempt textures.** `mips.levels = "none"` skips `pot_dims` (`resize::pot_exempt`): the texture keeps its size and gets `DIM-NPOT`, now a **Warning**, raised only there. `resize` still shrinks an exempt texture. A cube's source is never resized (`target_dims` returns it unchanged); its default face is `pot_dims((cell, cell))`, at most 4096, so a cell within 5 % of the next power of two now rounds up (1000×500 equirect, cell 250 → face 256). An explicit `cubemap.face_size` is unchanged.

ARCHIVED (wording before the edit; superseded in part by Note GV, 2026-11-07)
  - **What is gated.** The counters and getters `filtered_builds`, `problems_builds`, `problem_lookups`, `export_files_builds`, `view_builds`, `set_cards_drawn`, `helper_rows_drawn`, `cache_builds`, `gen_lookups`, `header_reads_on_this_thread`, `plan_calls`, `uv_reads_on_this_thread`, `visible_builds`, `row_builds`, `project_builds` and `dds_thumb_requests`; the plan-hold hooks (`hold_plan_results`, `take_held`, `handle_held`, `end_export_with`); the Meshes pill-read gate (`project_gate`, `project_alive`, `project_started`, `hold_rows_read`); the Set 3D encoder and viewport stand-ins (`hold_workers`, `inject`, `inject_as`, `inject_stale`, `has_alt`, `token_of`, `alt_token_of`, `set_plugin_reader`, `attach_test_thread`, `fake_render_thread`, `render_sent_for_test`, `loaded_for_test`); and small getters (`votes_pending`, `generation`, `plan_marks`, `gen_dims_len`, `clear_gen_dims`, `gen_preview_opts`). A build without the feature does none of the counting.

ARCHIVED (wording before the edit; superseded in part by Note GW3, 2026-11-07)
- **FM12 The manifest-compatibility shims are gone (audit §3, 2026-11-02).** 2.0 re-encodes everything once (FM9), so manifests need not stay valid. `BakeJob::identity_sha256` always hashes the `_n`'s `resize`; its RNM settings still join only when layers apply (`self.rnm.as_ref().map(|_| &np.rnm)`, Note W18). `PackJob::hand_placed` is removed (and from `schema/event.schema.json`): `pack::source_fill` always returns `ch:<rgba>`, the channels the source feeds, after `read:<id>` when there is one (Note DY21), so every packed source of every record names its channels. `PackSource.read` keeps its `skip_serializing_if`. Hashes and records change; output bytes do not (`identity.json` unchanged).

ARCHIVED (wording before the edit; superseded in part by Note GW2, 2026-11-07)
  - **Where the reference lives.** The planner writes it into the job's profile, `Profile::resize_reference: Option<(u32, u32)>` (`None` for `self`, `(0, 0)` for no single map, skipped when `None`; controller ruling T28B-d accepts this over a `target_dims` parameter), so every reader of `target_dims` (pipeline, DIM checks, bake plan and its `_n` profile copy, admission, `estimated_bytes`, the GUI size estimate and preview signatures, RNM preview) and the profile hash agree with no new parameter; a changed reference size moves the hash, so `if-changed` re-encodes. It is not a setting: a `[files]` entry, rule or `--set` naming it is refused as an unknown field, and a value in a preset table is overwritten by the planner for every job. `schemas::partial_profile_schema` drops it from the config and preset schemas (the partial profile and the by-product `Profile` definition there); the profile, manifest and event schemas keep it, since plans and records carry it.

ARCHIVED (wording before the edit; superseded in part by Note GW3, 2026-11-07)
  - **Planning.** `resize::plan_references` runs in `plan_sets` after the derive step, before shaders, RNM, bakes and `FilePlanned`. Sizes are pass 1's (`cube_facts`, else `input_dims`). A packed or derived map's own size, and its size as a reference, is its sources' common size (`api::job_dims_by`, the FM30 size before `target_dims`); a cube's is its source cell (`cube::layout::source_cell`). The pass recomputes `DIM-RESIZED` only for jobs with a reference (`policy::resized_warning`); a `self` job's warnings stay exactly as `check_profile` made them (unit test `resize::a_self_jobs_dim_resized_is_left_alone`). `DIM-SMALL` stays from `check_profile`, which saw the size without the reference; only a `resize_floor` below 4 could make it stale, since the relative size never falls below the floor.

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

ARCHIVED (wording before the edit; superseded in part by Note GT, 2026-11-06)
- GUI Preview and 3D: N, AG, AQ, AU, BS, CE, CJ, CL, CM, DT, FA, GD, GG, GH, GJ, GK, GL, GM, FM, FO, GQ, FS, FT, FY, FZ

ARCHIVED (wording before the edit; superseded in part by Note GT, 2026-11-06)
- render3d: BT, CI, CJ, CM, CV, CX, DL, GA, GG, GJ, GL, GM, GN, GP, FM, GQ

ARCHIVED (wording before the edit; superseded in part by Note GU, 2026-11-07)
- Cubemaps: DL, DN, DS, DT, FA, FC, FM, FO, FQ, FT, FY

ARCHIVED (wording before the edit; superseded in part by Note GU, 2026-11-07)
- Presets and config: G, N, S, T, X, AE, AL, AM, AP, AS, AV, AZ, BC, BJ, BO, BQ, BR, BV, CN, CV, FB, FL, FM, FQ, FX, FY, HB

ARCHIVED (wording before the edit; superseded in part by Note GU, 2026-11-07)
- GUI Settings panel: S, V, X, Z, AF, AG, AJ, AN, AS, AW, BM, BS, CC, CL, CV, FB, FC, FM, FS, FT, FY

ARCHIVED (wording before the edit; superseded in part by Note GU, 2026-11-07)
- GUI Dialogs and Export: AG, BH, BO, BR, BV, BY, CA, CD, CE, CF, CN, CV, DT, FB, FC, FI, FM, FN, FO, FP, FS, FW, FU, HB, HC, HE

ARCHIVED (wording before the edit; superseded in part by Note GU, 2026-11-07)
- Engine policy (formats, alpha gate, versions, writes): Answers, A, B, D, E, F, I, K, L, M, O, R, S, T, W, Y, AC, AG, AK, AV, AW, AX, AZ, BB, BC, BF, BJ, BL, BQ, BV, DB, DC, DM, DS, DY, FA, FF, FL, GJ, FM, FQ, FV, FZ, HB, HE

ARCHIVED (wording before the edit; superseded in part by Note GU, 2026-11-07)
- GUI Preview and 3D: N, AG, AQ, AU, BS, CE, CJ, CL, CM, DT, FA, GD, GG, GH, GJ, GK, GL, GM, FM, FO, GQ, FS, FT, FY, FZ, GR, GT

ARCHIVED (wording before the edit; superseded in part by Note GV, 2026-11-07)
- Packing and helpers: B, H, I, M, R, W, AL, BB, BP, BQ, BV, CN, DR, DY, FB, FM, FQ, FV, HC, HE, HG

ARCHIVED (wording before the edit; superseded in part by Note GV, 2026-11-07)
- CLI: I, J, K, X, BC, BQ, CN, DY, FC, FM, FX

ARCHIVED (wording before the edit; superseded in part by Note GV, 2026-11-07)
- Engine policy (formats, alpha gate, versions, writes): Answers, A, B, D, E, F, I, K, L, M, O, R, S, T, W, Y, AC, AG, AK, AV, AW, AX, AZ, BB, BC, BF, BJ, BL, BQ, BV, DB, DC, DM, DS, DY, FA, FF, FL, GJ, FM, FQ, FV, FZ, HB, HE, GU

ARCHIVED (wording before the edit; superseded in part by Note GV, 2026-11-07)
- Bake and RNM: E, M, R, W, Y, AL, BP, CL, CO, DP, FM, FN, FO, FV, HD

ARCHIVED (wording before the edit; superseded in part by Note GV, 2026-11-07)
- Presets and config: G, N, S, T, X, AE, AL, AM, AP, AS, AV, AZ, BC, BJ, BO, BQ, BR, BV, CN, CV, FB, FL, FM, FQ, FX, FY, GU, HB

ARCHIVED (wording before the edit; superseded in part by Note GV, 2026-11-07)
- Exports (PGPatcher, CS PBR, CM, XPMF, Also): P, V, AA, AB, AC, AD, AF, AG, AN, AT, BO, BR, BV, BY, CN, CO, CV, GE, GI, FM, FN, FO, FP, FT, FW, FV, HB

ARCHIVED (wording before the edit; superseded in part by Note GV, 2026-11-07)
- GUI Files and Inspector: N, P, S, U, Z, AA, AE, AF, AG, AM, AN, AO, AV, AY, BH, BI, BN, BO, BR, BS, BV, BY, CA, CC, CD, CE, CF, CG, CV, DN, DO, DP, DU, DZ, FA, FB, FC, FD, FI, FK, FL, FM, FN, FO, FP, FT, FS, FW, FU, FY, HB, HC, HE, HG

ARCHIVED (wording before the edit; superseded in part by Note GV, 2026-11-07)
- GUI Dialogs and Export: AG, BH, BO, BR, BV, BY, CA, CD, CE, CF, CN, CV, DT, FB, FC, FI, FM, FN, FO, FP, FS, FW, FU, GU, HB, HC, HE

ARCHIVED (wording before the edit; superseded in part by Note GV, 2026-11-07)
- GUI Settings panel: S, V, X, Z, AF, AG, AJ, AN, AS, AW, BM, BS, CC, CL, CV, FB, FC, FM, FS, FT, FY, GU

ARCHIVED (wording before the edit; superseded in part by Note GV, 2026-11-07)
- Process and docs: Answers, BX, DM, FD, FG, FH, FM, HA, HB, HK, GS

ARCHIVED (wording before the edit; superseded in part by Note GW, 2026-11-07)
- Engine policy (formats, alpha gate, versions, writes): Answers, A, B, D, E, F, I, K, L, M, O, R, S, T, W, Y, AC, AG, AK, AV, AW, AX, AZ, BB, BC, BF, BJ, BL, BQ, BV, DB, DC, DM, DS, DY, FA, FF, FL, GJ, FM, FQ, FV, FZ, HB, HE, GU, GV

ARCHIVED (wording before the edit; superseded in part by Note GW, 2026-11-07)
- Packing and helpers: B, H, I, M, R, W, AL, BB, BP, BQ, BV, CN, DR, DY, FB, FM, FQ, FV, HC, HE, HG, GV

ARCHIVED (wording before the edit; superseded in part by Note GW, 2026-11-07)
- Bake and RNM: E, M, R, W, Y, AL, BP, CL, CO, DP, FM, FN, FO, FV, HD, GV

ARCHIVED (wording before the edit; superseded in part by Note GW, 2026-11-07)
- Presets and config: G, N, S, T, X, AE, AL, AM, AP, AS, AV, AZ, BC, BJ, BO, BQ, BR, BV, CN, CV, FB, FL, FM, FQ, FX, FY, GU, HB, GV

ARCHIVED (wording before the edit; superseded in part by Note GW, 2026-11-07)
- Exports (PGPatcher, CS PBR, CM, XPMF, Also): P, V, AA, AB, AC, AD, AF, AG, AN, AT, BO, BR, BV, BY, CN, CO, CV, GE, GI, FM, FN, FO, FP, FT, FW, FV, HB, GV

ARCHIVED (wording before the edit; superseded in part by Note GW, 2026-11-07)
- CLI: I, J, K, X, BC, BQ, CN, DY, FC, FM, FX, GV

ARCHIVED (wording before the edit; superseded in part by Note GW, 2026-11-07)
- GUI Files and Inspector: N, P, S, U, Z, AA, AE, AF, AG, AM, AN, AO, AV, AY, BH, BI, BN, BO, BR, BS, BV, BY, CA, CC, CD, CE, CF, CG, CV, DN, DO, DP, DU, DZ, FA, FB, FC, FD, FI, FK, FL, FM, FN, FO, FP, FT, FS, FW, FU, FY, HB, HC, HE, HG, GV

ARCHIVED (wording before the edit; superseded in part by Note GW, 2026-11-07)
- GUI Settings panel: S, V, X, Z, AF, AG, AJ, AN, AS, AW, BM, BS, CC, CL, CV, FB, FC, FM, FS, FT, FY, GU, GV

ARCHIVED (wording before the edit; superseded in part by Note GW, 2026-11-07)
- Process and docs: Answers, BX, DM, FD, FG, FH, FM, HA, HB, HK, GS, GV

ARCHIVED (wording before the edit; superseded in part by Note HL, 2026-11-08)
- Packing and helpers: B, H, I, M, R, W, AL, BB, BP, BQ, BV, CN, DR, DY, FB, FM, FQ, FV, HC, HE, HG, GV, GW

ARCHIVED (wording before the edit; superseded in part by Note HL, 2026-11-08)
- Exports (PGPatcher, CS PBR, CM, XPMF, Also): P, V, AA, AB, AC, AD, AF, AG, AN, AT, BO, BR, BV, BY, CN, CO, CV, GE, GI, FM, FN, FO, FP, FT, FW, FV, HB, GV, GW

ARCHIVED (wording before the edit; superseded in part by Note HL, 2026-11-08)
- GUI Files and Inspector: N, P, S, U, Z, AA, AE, AF, AG, AM, AN, AO, AV, AY, BH, BI, BN, BO, BR, BS, BV, BY, CA, CC, CD, CE, CF, CG, CV, DN, DO, DP, DU, DZ, FA, FB, FC, FD, FI, FK, FL, FM, FN, FO, FP, FT, FS, FW, FU, FY, HB, HC, HE, HG, GV, HM, HN, GW, GY, HP, IA

ARCHIVED (wording before the edit; superseded in part by Note HL, 2026-11-08)
- GUI Dialogs and Export: AG, BH, BO, BR, BV, BY, CA, CD, CE, CF, CN, CV, DT, FB, FC, FI, FM, FN, FO, FP, FS, FW, FU, GU, HB, HC, HE, GV, GY

ARCHIVED (wording before the edit; superseded in part by Note HQ, 2026-11-08)
- Manifest and outputs: F, M, N, S, Y, AE, AF, AL, BJ, BQ, BV, CN, CT, DM, DY, FC, FM, FZ, HB, HE

ARCHIVED (wording before the edit; superseded in part by Note HQ, 2026-11-08)
- Exports (PGPatcher, CS PBR, CM, XPMF, Also): P, V, AA, AB, AC, AD, AF, AG, AN, AT, BO, BR, BV, BY, CN, CO, CV, GE, GI, FM, FN, FO, FP, FT, FW, FV, HB, GV, GW, HL

ARCHIVED (wording before the edit; superseded in part by Note HQ, 2026-11-08)
- GUI Dialogs and Export: AG, BH, BO, BR, BV, BY, CA, CD, CE, CF, CN, CV, DT, FB, FC, FI, FM, FN, FO, FP, FS, FW, FU, GU, HB, HC, HE, GV, GY, HL

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

ARCHIVED (wording before the edit; superseded in part by Note GT, 2026-11-06)
- Golden fixtures come from the golden-capture workflow (`SDC_GOLDEN`, folder `cs-1.9.0-d2026.10.11.0`), keyed by CS version; scene WhiterunDragonsReach01 (hour 7.5, Linear Lighting on); skin and hair specular follow the user's CS settings. [CJ1, CJ2, CJ3]

ARCHIVED (wording before the edit; superseded in part by Note GT, 2026-11-06)
- Deferred frames run the user's `DeferredCompositeCS.hlsl` (`CSHADER`) into RT0; blended shapes draw forward on top; debug views skip it; a failed build keeps forward with "Deferred preview unavailable: <error>". A late composite failure rebuilds forward programs synchronously; async deferred-off keeps the target while any draw holds DEFERRED; no DEFERRED draw means a forward frame. [CJ6, CJ7]

ARCHIVED (wording before the edit; superseded in part by Note GU, 2026-11-07)
- Memory admission: a plan-pass cache hit admits at 0 bytes, a cold file costs one stat; a job over the budget runs alone when nothing else is in flight; RNM layers add at the resized base size and a bake's layers at the `_n`'s resized size (sums); a photo cube adds `2 × w × w × 16`; a helper's decode bytes count beside its target's. [AV4, BB2, BB3, BB5, DS19, DY10e]

ARCHIVED (wording before the edit; superseded in part by Note GU, 2026-11-07)
- The world-to-cube basis comes from the in-game test; equirect panoramas take yaw and mirror; mirror-ball and angular-map importers; single-photo approximation (a portrait photo is cropped, the photo faces east); ground fill and horizon. [DS13, DS14, DS15, DS16, DS17, DS26]

ARCHIVED (superseded by Note GU, 2026-11-07)
- The unclear single-image decision offers 360° panorama, Mirror ball, Angular map and Single photo. [DT14, DT32]

ARCHIVED (wording before the edit; superseded in part by Note GU, 2026-11-07)
- A cube DDS loads as its six faces, an h-strip: `analysis.dds_cube` wins over a face tag in the name and over `cubemap.layout`, and an explicit layout other than hstrip is `CFG-INVALID`. Admission sees the real 6w × h strip, and a photo cube adds its fake panorama. [DN7a, DS19]

ARCHIVED (superseded by Note GU, 2026-11-07)
- The unclear-layout reason lists every one-file layout (hcross, vcross, hstrip, vstrip, equirect, mirror_ball, angular, photo; `faces` is left out). `ground_fill` is `"gradient"` or `[r, g, b]` with whole numbers 0..=255, and a fractional or out-of-range element is refused naming those forms. [DS22, DS25]

ARCHIVED (wording before the edit; superseded in part by Note GU, 2026-11-07)
- The cube section lists the 18 `cubemap.*` rows from `fields::CUBE_FIELDS`, shows only when every selected item is a cube, and shows out-of-range values rather than fixing them (each drag value is bounded to the engine's range). Ticking Highlight clamp writes 4.0 and Blur 0.35. [DT7, DT8, DT9, DT10]

ARCHIVED (wording before the edit; superseded in part by Note GU, 2026-11-07)
- OPEN: panorama import's fixed 3x3 taps alias when the face is far smaller than the source; static cubes use a Gaussian cone, not GGX. [DS]

ARCHIVED (wording before the edit; superseded in part by Note GU, 2026-11-07)
- The cube settings section is table-driven (`CUBE_FIELDS`, 18 `cubemap.*` rows), drawn only when every selected item is a Cubemap (no bulk cube rows). Drags are bounded to the engine's range; an existing out-of-range value shows unclamped and is written only on change. Unticking or Auto / None removes the key. The layout combo never offers "Six face files". [DT7, DT8, DT9, DT10, DT11, DT13]

ARCHIVED (wording before the edit; superseded in part by Note GV1, 2026-11-07)
- Generate from normal map writes height (`_p` or the `_m` alpha), AO and curvature files beside the normal; a generated map is recognised by its `_generated` name marker only, and a real map for the slot wins over it (`GEN-REPLACED`, an intentional skip). [CO1, CO2, CO5, CO9]

ARCHIVED (superseded by Note GV2, 2026-11-07)
- The height solve is an FFT Poisson solve on de-tilted slopes (mean slope removed, mirror or wrap tiling detected); z is always rebuilt from R and G; a non-finite texel reads flat; a non-power-of-two normal is refused. [CO11, CO13, CO14, CO15, CO17, CO20, CO24]

ARCHIVED (superseded by Note GV1, 2026-11-07)
- Generate sizes are 256/512/1024/2048 within the cap and the normal's longer side; detection runs at fixed sizes so the preview and the full run agree. [CO23, CO26, CO35]

ARCHIVED (wording before the edit; superseded in part by Note GV2, 2026-11-07)
- AO from a normal is a GTAO-style march over the solved height; a generated AO feeds the packed `_rmaos` B (`GEN-AO`), also on a pre-packed `_rmaos` whose B holds no content. [CO78, CO89, CO103, CO104]

ARCHIVED (superseded by Note GV1, 2026-11-07)
- The engine has no writer for generated files: the GUI checks every target with `dds::may_replace` before writing any, then writes atomically, rolling back only what it did on a failure. [CO28, CO49, CO66]

ARCHIVED (superseded by Note GV1, 2026-11-07)
- Generate provenance lives in `[generated]` of `sdc-project.toml`, outside undo; Generate is not an undo step; a stale generated file is flagged. [CO39, CO40, CO53, CO69]

ARCHIVED (wording before the edit; superseded in part by Note GV2, 2026-11-07)
- Height ranges use `R = p99.5(|h|)` of the zero-mean solve-size height and Strength 0.1–1.0 (default 1.0). Half's clamp `[8/255, 247/255]` sits inside the complex-material gate `(4/255, 251/255)`. [CO21, CO22]

ARCHIVED (superseded by Note GV1, 2026-11-07)
- Generate has one `Opts::size`, each map capped: height 1024, up to 2048 where a recipe needs the set's size (`_m` alpha always; `_p` on cs PBR and PBR-landscape sets, defaulting to the albedo size with a cost warning above 512); AO 2048; curvature at albedo size up to 2048. The solve runs at `min(size, 1024)`, a larger height resampled up (periodic bilinear when wrapped, no seam). A map its recipe fixes is written at that size; a locked non-power-of-two size is `SIZE_LOCKED`. [CO25, CO31, CO34, CO67, CO82, CO95, CO112, CO118]

ARCHIVED (superseded by Note GV2, 2026-11-07)
- Green for the solve is DirectX unless the fit margin `max(e_dx, e_gl) / min(e_dx, e_gl)` is at least 1.25, when the lower error wins; below that the dialog says "undetermined". A tiling field whose shorter side is 16 px or less is not judged (wrap, ambiguous). [CO27, CO30]

ARCHIVED (superseded by Note GV2, 2026-11-07)
- Generate returns `InvalidOpts`, never a panic, for relief outside [0, 1], size 0, a zero-sized normal, Strength outside 0.1–1.0, AO radius outside 0–0.25, slices or steps outside 1–64, Depth outside 0.25–4 and Intensity outside 0.5–3. [CO29, CO84]

ARCHIVED (superseded by Note GV2, 2026-11-07)
- AO's march reads the same de-tilted slopes as the solve, uses an angle bias of tan 4°, and samples wrap on a wrapped map and clamp on a mirrored one. [CO79, CO80, CO81]

ARCHIVED (superseded by Note GV1, 2026-11-07)
- A generated height is offered for the Default, Parallax, Pbr and PbrLandscape shaders, and for ComplexMaterial only on vanilla without a file-backed, non-generated `_m`; it is refused for vanilla multilayer, Glow, Envmap, Hair, Eye, Skin, vanilla landscape, AdvancedSkin, PbrGrass and ComplexGrass. [CO43]

ARCHIVED (wording before the edit; superseded in part by Note GV1, 2026-11-07)
- Generated AO is offered only on cs with Pbr, PbrLandscape or PbrGrass; a kept `_ao_generated` stays PBR evidence (`suggest::is_pbr_set`). A pre-packed `_rmaos` counts as an AO source only when its B has content (`pack::b_has_content`: range ≥ 2/255 or mean ≤ 250/255). A loose roughness, glossiness, metallic, specular, fuzz or `_orm` image beside a pre-packed one means the image is never filled, except a specular beside a flat alpha (HC1). [CO92, CO94, CO105, CO106, CO108]

ARCHIVED (wording before the edit; superseded in part by Note GV1, 2026-11-07)
- Curvature (`_curves`) is read only by Also export; it is not offered elsewhere or for a set with no albedo, and reaches a plain run as `PACK-UNUSED-INPUT`. A generated AO changes the base `_n` job's `rnm.ao.file` identity. [CO116, CO115, CO90]

ARCHIVED (superseded by Note GV1, 2026-11-07)
- A cancel is read between stages and writes nothing before the first write; once writing has started the job runs to its end and the files it wrote are recorded. [CO36, CO50]

ARCHIVED (superseded by Note GV1, 2026-11-07)
- "Generate from normal map…" sits below Assign… in the slot row's format column (below the format combo on a filled Height or RMAOS row), absent for packed items, reason on hover when disabled. Its dialog: 160 pt tiles, wheel zoom 1-16x at the pointer, drag pans, double-click fits, one shared crop, zoom resets on reopen. [DP1, DP2, DP3]

ARCHIVED (superseded by Note GV2, 2026-11-07)
- Half's clamp `[8/255, 247/255]` sits inside the complex-material gate `(4/255, 251/255)`. [CO22]

ARCHIVED (wording before the edit; superseded in part by Note GV1, 2026-11-07)
- Provenance and JSON hashes are recorded at write time. [FC11]

ARCHIVED (wording before the edit; superseded in part by Note GV1, 2026-11-07)
- Plain slot and match labels; a one-line Textures heading; a reason on the greyed AO box. [FB13]

ARCHIVED (wording before the edit; superseded in part by Note GW3, 2026-11-07)
- Complex grass has no alpha question (varying alpha: top half cutout, bottom half data); its set gets no `SHADER-EXTRA-MAP` / `SHADER-MISSING-MAP`; a grass stem ending in a raw-map or layer suffix is read as that. [X12, X9, X8]

ARCHIVED (wording before the edit; superseded in part by Note GW3, 2026-11-07)
- Memory admission counts RNM layers and complex grass. [BB1, BB4]

ARCHIVED (wording before the edit; superseded in part by Note GW1, 2026-11-07)
- `_wet` and `_detail_n` (HB4) are suffixes only on `cs` in an Advanced skin set (a real `_rfaos` member or a resolved recipe building `_rfaos`); `group::join_skin_wet` and `group::join_skin_detail` (after grouping, before set edits) move `x_wet` and `x_detail_n` into set `x`, folding Unicode; elsewhere they stay in the name (`road_wet`; `x_detail_n` groups as before). A set listing `wet` keeps a suffix-less `*_wet` member through a Set Rename. [I3, BP2, BV9]

ARCHIVED (wording before the edit; superseded in part by Note GW3, 2026-11-07)
- A hand-set `helper.role` (images only) makes the file a helper even when a rule or `[files]` names its sub-type; `none` converts it alone. Otherwise a helper's role is its own `PackingInput`/`NormalLayer` job's; a raw map typed to convert alone fills nothing. A hand-role file with no known suffix joins its base set in the same output folder. [DY2, DY2a, DY13a, DY20]

ARCHIVED (wording before the edit; superseded in part by Note GW3, 2026-11-07)
### Bake and RNM

ARCHIVED (superseded by Note GW3, 2026-11-07)
- RNM layers blend on the preprocessed base, before the mips; the roughness bake reads the blended `_n`. [W1, W18]

ARCHIVED (superseded by Note GW3, 2026-11-07)
- `rnm.target` is a per-file selector; layer strength 0..=2, `rnm.bent.ao_power` 0.1..=8; bump slope at the bump's own size, DirectX green. [W3, W7, W8]

ARCHIVED (superseded by Note GW3, 2026-11-07)
- Bases: `n` = `normal`, `pbr_normal`, `landscape_normal`; `cnr` = `coat_normal`; an Eye set ignores layers. [W5]

ARCHIVED (superseded by Note GW3, 2026-11-07)
- A layer with no prefix targets `n`, falling back to `cnr` when the set has no `_n`; a prefixed layer targets its prefix; a layer whose targets name no base gets `RNM-NO-BASE`. Candidates for one layer id rank a prefix naming this base, then no prefix, then the other prefix, ties by path; the first wins and each loser gives `RNM-LAYER-SHADOWED` on the base job. [W4]

ARCHIVED (superseded by Note GW3, 2026-11-07)
- Every `rnm.*` field is optional and `Profile.rnm` is skipped at its default, so no existing profile hash moves. `rnm.order` drops duplicates (first kept) and appends the missing ids in the order `bent, bump, bump2, detail, detail2`. `rnm.*` on a sub-type that cannot be a base is reset to the default. [W6]

ARCHIVED (superseded by Note GW3, 2026-11-07)
- The bent layer's AO source, in order: raw AO (R), `_orm` (R), `_rmaos`/`_rfaos` (B), read only for `bent` in `ao_blend` with `rnm.bent.ao = true`, else uniform weight. A file's hand-set `helper.role` (`ao`, `orm`, `rmaos`) says which it is and any other role (`none` included) takes it out; the file name decides only a file with no hand-set role. The manifest role carries the channel (`bent.ao.r`, `bent.ao.b`), so swapping a byte-identical file between them re-encodes the normal. [W9, AL4, W16, FV3]

ARCHIVED (superseded by Note GW3, 2026-11-07)
- RNM decode clamps z to at least 0: a no-op at strength ≤ 1; above 1 it deviates from rnm.py, which does not clamp. [Y5]

ARCHIVED (wording before the edit; superseded in part by Note GW3, 2026-11-07)
- The config schema's `FileEntry` carries `reencode` (boolean) and its own `rnm` (`PartialRnmSpec` plus `target`); a test walks every `FIELD_PATHS` path except `subtype` through it. [BC8]

ARCHIVED (wording before the edit; superseded in part by Note GW3, 2026-11-07)
- A suffix alias naming exporter vocabulary is `CFG-INVALID`: `orm` (message "orm is a built-in packing input"), `curves`, `curvature`, `cavity`, and an RNM layer term such as `bump` or `bentmask`. [AL6, CN4, BV9]

ARCHIVED (wording before the edit; superseded in part by Note GW3, 2026-11-07)
- The map check leaves out `NormalLayer` jobs (layers and masks), gives a `complex_grass` set no `SHADER-EXTRA-MAP`/`SHADER-MISSING-MAP` (one file holds diffuse and normal), and raises no `_msn` without `_s` note when the set's shader is Eye. [BQ9, X9, BQ12]

ARCHIVED (wording before the edit; superseded in part by Note GW1, 2026-11-07)
- A MATO-only landscape set needs no RMAOS. CS does not draw the projected normal and detail samples in this build (slot hover says so); they still export. A landscape `_noise` is always ProjectedNoise; a name-prefixed `_detail` stays an RNM layer. The two new kinds have no slot number, so no slot-keyed map gets them. The card shows every refusal `prepare` can know. The preview takes the profile's textures, coverage, then MATO DATA, cycling the layer when texture paths change. [GI10, GI15, GI16, GI17, GI18, GI19, GI21, GI22, GI25, GI28, GI29]

ARCHIVED (wording before the edit; superseded in part by Note GW1, 2026-11-07)
- Landscape sets have two optional maps, projected noise (BC4_UNORM, `noise` is a global suffix) and `_detail` as its own output. [GI12, GI13, GI14, GI20]

ARCHIVED (superseded by Note GW2, 2026-11-07)
- The RNM section shows a 128 px blended thumbnail (one job in flight, dropped when its key changed); the strength slider writes on release; a failure reads "Preview unavailable: {reason}". [AG115, AG116, AG117, FC13]

ARCHIVED (wording before the edit; superseded in part by Note GW, 2026-11-07)
- Memory admission: a plan-pass cache hit admits at 0 bytes, a cold file costs one stat; a job over the budget runs alone when nothing else is in flight; RNM layers add at the resized base size and a bake's layers at the `_n`'s resized size (sums); a helper's decode bytes count beside its target's. [AV4, BB2, BB3, BB5, DY10e]

ARCHIVED (wording before the edit; superseded in part by Note GW, 2026-11-07)
- Curvature (`_curves`, `_curvature`, `_curves_generated`) is read only by Also export, and reaches a plain run as `PACK-UNUSED-INPUT`. An `_ao_generated` file changes the base `_n` job's `rnm.ao.file` identity. [CO116, CO115, CO90]

ARCHIVED (wording before the edit; superseded in part by Note GW4, 2026-11-07)
- A raw map given a sub-type converts on its own, not as a packing role. [BP3, W2]

## Note GQ — NIF trace-debug fixes (NIF team, 2026-11-04)

ARCHIVED (superseded by Note GT, 2026-11-06)
- **GQ6** The preview compiles the user's Game Gamma: `preview_defines` adds `CS_GAME_GAMMA=<f32 to 9 decimals>` (CS's own form, `1.600000024`) only when Linear Lighting is on (the shaders read it only under `ENABLE_LL`, TransferFunctions.hlsli) and `"Linear Lighting".gameGamma` is a usable value other than 1.6, so default users and LL-off users keep their defines and program keys; the Lighting programs and the composite share it through the feature list, and `settings_define` treats it as a settings define. `ENABLE_ACESCG` stays uncompiled (shortcut: the readback has no ACEScg conversion). A non-UTF-8 Info.ini is now a note with the Features-ini fallback, like an unreadable one, not a parse failure that kept the old settings.

## Note FT — backlog batch B4: gui model 2 (main team, 2026-11-04)

ARCHIVED (superseded by Note GT, 2026-11-06)
- **FT6 A non-UTF-8 shaders root does not fail every `gui.toml` save (T39).** `GuiSettings::save` drops `render3d.shaders` when its root is not Unicode, like the other paths. Test `settings::a_non_unicode_shaders_root_does_not_stop_the_settings_saving`.

## Note FN — User-facing audit leads: generate sizes, fill boxes, read-only reasons, report, feature clash (main team, 2026-11-04)

ARCHIVED (superseded by Note GV2, 2026-11-07)
- **FN1 Generate from normal follows the automatic power of two (lead 32).** The export writes a normal with mips at `resize::pot_dims` (FM29), so Generate solves there too.
  - Engine: `from_normal::generate` resamples a non-power-of-two normal to `pot_dims` before the FFT (`to_pot`: `resize::enlarge` on the axes that grow, then `mips::area_resample` down, the order `resize_work` uses). `GenerateError::NotPowerOfTwo` is gone. A 1000² normal is solved at 1024² (a 2.4 % upsample), so the height lines up texel for texel with the written normal. A normal kept at its own size by `mips.levels = "none"` is still solved at its power of two; the generated map is a mip-mapped input like any other.
  - GUI: `AppModel::gen_normal_size` (the normal's `pot_dims`) feeds the offered sizes, the dialog's paths and Generate. `locked_to` locks AO beside other `_rmaos` sources, and curvature, to the sources' `pot_dims`, and refuses with "written W×H, another aspect ratio" when that step changes the aspect (1000×600 is written 1024×512), because a pack or derive cannot join different aspects (FM30). The NPOT reasons in the entry and in curvature are gone.
  - Tests: engine `generate_writes_the_requested_size_and_solves_npot_at_its_power_of_two`; GUI `an_npot_normal_is_offered_at_its_power_of_two_size`, `a_locked_size_takes_the_power_of_two_the_maps_are_written_at`, `a_disabled_generate_button_shows_the_reason`, plus the size and reason tests adjusted in `generate_from_normal.rs`.

## Note FV — backlog batch B5: Also derive, RNM AO, PSD (main team, 2026-11-04)

ARCHIVED (superseded by Note GV2, 2026-11-07)
- **FV5 An NPOT normal solved at its power of two is unchanged (R1; no fix).** The torus test came out green: 1000² against 1024² differ by a mean 0.0001 in height, and the resampled normal's mean slope is 1.0238 times the 1024² one's, against the 1.024 the per-pixel slope convention predicts, so `to_pot` keeps resampling the normal. The slope comparison is the pin, because the height is range-normalised and would hide a uniform slope bias (`from_normal::an_npot_torus_solves_like_its_power_of_two_twin`).

ARCHIVED (wording before the edit; superseded in part by Note GW3, 2026-11-07)
Engine fixes for trace-debug minors 26, 27, 28 and 40 and backlog R1. `golden` and `encode_versions` are unchanged. No `encode_version` arm is set: each change reaches only inputs handled wrongly before, and the identity of those outputs changes by itself (FV2: the cavity source carries `invert`; FV3: the manifest role names the AO file and channel).

ARCHIVED (superseded by Note GW3, 2026-11-07)
- **FV3 The RNM bent layer's AO follows a hand-set `helper.role` first (T40; supersedes W9 in part).** An image member whose `helper.role` is `ao`, `orm` or `rmaos` is that AO source (R, R, B); any other hand-set role, `none` included, takes the file out; a file with no hand-set role is read by its name as W9 and AL4 say. The order stays AO, `_orm`, `_rmaos`. `_n` bytes change only for a set with a hand-set role. Test `rnm_plan::a_hand_set_role_picks_the_bent_layers_ao_before_the_name`.

## Note FY — backlog batch B8: gui refactors and dead code (main team, 2026-11-04)

ARCHIVED (wording before the edit; superseded in part by Note GV2, 2026-11-07)
- **FY5 One auto-flag entry point (L57).** `checks`, `from_normal` (two sites) pass `auto_flags::hint` to `resolved_for`, as the PGPatcher export and Parameters panel do; `auto_flags` and `resolved` (no scan evidence) exist only under `test-hooks`. A PBR grass set's checks and generate-from-normal now read the grass-only auto keys as its export does (test `auto_flags::a_pbr_grass_sets_checks_read_the_keys_its_export_writes`); the visible result of those readers (`height_neutral`) is the same, because a grass shader is `Half` either way.

ARCHIVED (wording before the edit; superseded in part by Note GV2, 2026-11-07)
- **FY6 The two warning-text parsers carry `shortcut:` notes (L76).** `from_normal::replaced_by` (GEN-REPLACED) and `Cx::errors` in the cube section (CFG-INVALID); the upgrade is structured warning data, which needs an engine event change.

ARCHIVED (wording before the edit; superseded in part by Note GW2, 2026-11-07)
- **FY1 One commit-once-per-gesture widget (L29).** `advanced::held` holds the in-flight and the sent value in egui memory (pending, sent, busy: R2) and settles on a drag end, on leaving the box (FN2: typed text and arrow keys on a number box are one edit), or on a click. `settled_drag` (Advanced fill boxes, busy), the cube section's `num`, the RNM `drag_slider` and `held_value` (busy false) wrap it; `colour` and `settings_panel::commit_slider` stay. The RNM slider's value box is built with `update_while_editing(false)`, so typed text is one edit too, and the result is clamped to the range. A slider passes `on_blur` false, so an arrow key settles at once (as `the_rnm_strength_slider_does_not_snap_back_while_the_plan_runs` pins); a number box defers it to leaving the box. `held_value` also settles on `lost_focus` now, which changes nothing a caller sees: its boxes took an arrow key at once and still do. Tests in `ui/advanced.rs`, `cube_section.rs` and `rnm_section.rs` (typed value, arrow key and click commit once).

## Note HC — 2.1 bug batch (main team, 2026-11-05)

ARCHIVED (wording before the edit; superseded in part by Note GV1, 2026-11-07)
- **HC1 A specular fills the flat alpha of a pre-packed `_rmaos`/`_rfaos` image (owner, 2026-11-05).** Before, the image won under `packing.prefer = prepacked` and the specular read "not used", even with a hand-set `rmaos.a` fill. Now, when the set's only other loose map is a specular (a generated AO the image's empty B takes may join it) and the image's alpha is flat, the image joins the recipe as CO104 does: A from the specular, the other channels from the image. "Flat" is the alpha verdict `plan_fills` reads (`AlphaFacts::empty`), so any constant counts, not only 255. An alpha with content, or another loose map, keeps the M9 result. A pre-packed DDS is never joined (images only, as CO103). No `encode_version` arm moves: the recipe and its bytes for given sources are unchanged, and the new source list re-encodes that set by itself. Tests `generated_files::a_specular_fills_the_flat_alpha_of_a_prepacked_rmaos` and its neighbours.

ARCHIVED (wording before the edit; superseded in part by Note HL5, 2026-11-08)
- **HC5 A PBR pick without a PBR albedo is not called vanilla (owner's screenshot, 2026-11-05).** The shader pick reads any PBR map (a lone `_f`), while `SetView::pbr` and the JSON need a `pbr_albedo` diffuse, so the card said "PBR (auto)" over a "vanilla" pill. The pill now says "PBR, no JSON". Nothing else changes: the set still writes no JSON. Test `pbr_landscape::a_pbr_pick_without_a_pbr_albedo_is_not_called_vanilla`.

## Note HE — 2.1 leftovers (main team, 2026-11-06)

ARCHIVED (wording before the edit; superseded in part by Note GW3, 2026-11-07)
- **HE7 Two raw maps of one name are not duplicates (owner's report, 2026-11-06, `mouth_ao.psd` + `mouth_ao.png` in one Advanced Skin set: "cannot use either, is stuck using fill even when one is removed from the set"; the ruling is an agent ruling).** A raw map, a hand-set normal layer (`helper.role` layer) and a skin `_f` write no file at their own output (final review M5, agent ruling: a normal layer found only by its name, `x_detail.png` + `x_detail.tga`, is not counted, so two of those stay `IN-DUPLICATE`; the rule is not extended): pack planning consumes them or reports `PACK-UNUSED-INPUT` "not converted". So `prepare` leaves such an image out of the `IN-DUPLICATE` comparison (AF7 already compares only the jobs that write): `x_ao.png` + `x_ao.tga` give no `IN-DUPLICATE`, the first by path feeds the channel (M13) and the other is `PACK-UNUSED-INPUT`, which the AO source picker can swap. The rule follows the writer, not the name: a file whose sub-type is requested, or whose `helper.role` is `none`, writes and is compared as before, so two of those are still `IN-DUPLICATE`; two files with no raw name (`wall.png` + `wall.tga`) stay duplicates; a `_f` outside an Advanced Skin set is a PBR map that writes. My calls (agent ruling): a `[[rules]]` entry that types a raw name is invisible to `prepare` (rules need the analysis), and the BV2 writer dedupe is its backstop, so two rule-typed twins never both write; a file removed from the set by `SetEdit::Remove` is a set of its own that packs the same default output, so the BV2 dedupe still flags that one file. The fast re-plan runs the same rule (`api::waiting_images`) on the old and the new state over every set and returns `NeedsFull(DUPLICATE_CHANGE)` when a file moves in or out while another file shares its output, so a skin `_f` typed by hand, or a neighbour's sub-type that unmakes the skin set, is seen; on a cs PBR set `PBR_SUBTYPE` already forced a full plan for such edits. Tests in `crates/engine/tests/raw_twins.rs`, `a_sub_type_edit_on_a_twin_plans_in_full` in `fast_replan.rs`, `picker_can_switch_between_twins` in `helper_picker.rs`.

ARCHIVED (wording before the edit; superseded in part by Note HP, 2026-11-07)
- **HN1 Helpers are docked on the chips they fill (Decisions 3-5; crates/gui/src/ui/sets_column.rs).** A helper is drawn as a 12 px badge in the top-right corner of each slot chip it fills, on a filled chip and on a packed output chip (HN2), with a 1 px panel-colour outline; a second helper takes the top-left corner, and K helpers that do not fit fold into one `+K` cell (hover lists them). A padlock (kept by PGPatcher, locked pin) keeps its corner and the badge uses the next free one. A badge is a Button named "<helper file>, fills <slot> <channel>"; hover shows the file and the helper panel's fills line; click selects the helper (opens the helper panel); right-click is the member menu. A chip with docked helpers is named "<letter>, <item>, filled by <helper files>"; a chip without helpers keeps its bare letter. Only a helper that got a badge leaves the trailing row; one folded into `+K` or with no free cell stays there as well. This amends CD8 ("Chips are named by their letter") for chips with helpers; CD8's plain `_m` thumbnail look is kept.

ARCHIVED (wording before the edit; superseded in part by Note HP, 2026-11-07)
- **HN2 The empty-chip reading (Decision 2).** The "empty target chip" of the spec is a packed output with no source file of its own, such as RMAOS built from helpers only. The planned output is a member of the set, so that chip is an `Own` chip and takes its badges. A `SlotFill::Empty` chip (the slot has no file at all) draws only its letter and takes none.

ARCHIVED (wording before the edit; superseded in part by Note HP, 2026-11-07)
- **HN3 The trailing row holds what no chip shows, and names each member (M1).** It holds a helper that fills nothing (dimmed by the disabled alpha, hover "not used: …"), a helper whose target has no chip on the card, a `_m` env mask and a clashing second member of a kind. A helper reads "<file>, <fills text>", any other member "<file>, <role>", also in its hover. This replaces "Members in no chip follow as plain thumbnails" with a row that says why each member is there.

ARCHIVED (wording before the edit; superseded in part by Note HP, 2026-11-07)
- **HN4 The width change reveals the open card (F1).** When the Sets column's width changes from a known width and exactly one set is selected, that set is revealed on that frame (the FM32 reveal). FB2's narrowing is unchanged.

ARCHIVED (wording before the edit; superseded in part by Note HP, 2026-11-07)
- **HN6 Card titles are elided in the middle (F3; the pattern of CD4).** The title is shortened to what the column's width holds, measured on the laid-out text: the whole id, then the middle elided (`header::elide_middle`: first segment and leaf), then `…/<leaf>`, then the end of the leaf. When the leaf does not fit beside the position and handle, the title takes its own line. The full id is the label's AccessKit name and, when shortened, its only hover line.

ARCHIVED (wording before the edit; superseded in part by Note HP, 2026-11-07)
- **HN7 Filter names, shader counts and the drop hint (F4, F5, F6).** The search box is named "Search sets"; the shader menu "Sets shader filter", and "Sets shader filter: <shader>" while one is chosen, its visible text unchanged. The popup lists "<shader> <n>" with the number of keyed sets per shader; a zero entry is weak but clickable, so every Mode shader stays listed (extends CD11, A9 kept). "Drop here to start a new set" is `weak_text_color()` and the selection accent while a drag is over it.

ARCHIVED (wording before the edit; superseded in part by Note HP, 2026-11-07)
- **FM32 Sets column: keyboard and scroll (plan task 21, 2026-11-03).** Heights: A change of the column width of 0.5 pt or more (`DropState::card_w`) moves each remembered height by the chip rows it adds or removes times (chip + item spacing): rows come from the set's slot row, missing entries and extra members at `CHIP` wide with the card's margins (`chip_rows`, `reflow_heights`). An id no longer in the views is forgotten. The estimate leaves out the name row, which wraps below the pill when the column is narrow; drawing a card measures it exactly.
- Anchor: The first card in view is remembered with its top in the content, summed from the remembered heights and not from the laid-out rects (egui rounds each allocation, which drifted a pixel over 200 cards). The next frame sets the scroll offset so that card stays where it was on screen while the heights above it change (the mean of the measured ones moves, a width change re-flowed them). A card that comes into view with a height other than its estimate still moves the cards below it for that one frame.

ARCHIVED (wording before the edit; superseded in part by Note HP, 2026-11-08)
- Stand-ins: A card outside the clip rect (CF7) still registers its widget id: a focus stop and an AccessKit node (Button, "Set {id}, position {n} of {N}", selected when the set is, the Focus action, the bounds of its allocated rect; the drawn card's own node carries the same name, role and selected state through `card_node`). A stand-in that takes the focus this frame (the focus was elsewhere at the end of the last one: `DropState::focus`) sets `nav.reveal_set`, or, for a created set with no key, calls `scroll_to_rect`; the next frame draws the card in view. (The rest of the item is unchanged.)

ARCHIVED (wording before the edit; superseded in part by Note HP, 2026-11-07)
- Sets filter: "Search sets" (folds like set ids, tests id or label), pills Not ready / Ready / No JSON with counts (keyed sets only) and Filter ▾ with Any and the shaders, AND-ed; no "No sets match" before the first plan. [CD11, CD12, CD17, HN7]

ARCHIVED (wording before the edit; superseded in part by Note HP, 2026-11-08)
- A set card uses a one-row heading only when the name gets at least 80 px. [FI13]

ARCHIVED (wording before the edit; superseded in part by Note HP, 2026-11-08)
- **HN7 Filter names, shader counts and the drop hint (F4, F5, F6).** The search box is named "Search sets"; the shader menu "Sets shader filter", and "Sets shader filter: <shader>" while one is chosen, its visible text unchanged. The popup lists "<shader> <n>" with the keyed sets that pass the filter as it would be after that click (HP4); a zero entry is weak but clickable, so every Mode shader stays listed (extends CD11, A9 kept). "Drop here to start a new set" is `weak_text_color()` and the selection accent while a drag is over it.

ARCHIVED (wording before the edit; superseded in part by Note HP, 2026-11-08)
- **HP3 The card combo keeps its own row under the title (SC1, SC9; owner answer 2).** It is right-aligned and sized to its value (`combo_w` returns the capped and the full width, measured in `TextStyle::Button` because the ComboBox lays out in Button), capped at half the card's inner width and cut with `…`; the full value is in the hover and the AccessKit value. The left part of that row is free card area and opens the set. The combo is named "Shader for <set id>". The titles keep their row. **Amends CD7** ("the shader combo at full width").

ARCHIVED (wording before the edit; superseded in part by Note HP, 2026-11-08)
- **HP4 Every count in the column is what its click would show (SC5; owner answer 3).** A pill's count and a shader-menu entry's count are the keyed sets that pass the filter as it would be after that click (the search, the other pills, the shader), through one predicate. A zero pill is weak (greyed) only while it is off and stays clickable; a selected zero pill paints as a normal selected pill, because weak text on the selected fill risks 4.5:1 (HP8). A selected pill counts the sets shown with it on (the list itself), not what turning it off would show: a selected facet whose number jumps to the wider total reads as wrong (controller, 2026-11-08; owner to confirm). **Amends CD12** ("Not ready plus Ready count every keyed set") **and HN7** ("keyed sets per shader").

ARCHIVED (wording before the edit; superseded in part by Note HP, 2026-11-08)
- **HP6 The set editor's room comes from Textures first (SC2).** When the editor needs its floor (`set_editor::ROWS_MIN`), the shortfall comes from Textures down to its minimum, then from Sets. `TEXTURES_W` 380 and `SETS_W` 260 replace the literals in `ui/mod.rs`. Opening a set at 1055 no longer narrows the Sets column. **Amends FB2** (the split) **and HN4** ("FB2's narrowing is unchanged").

ARCHIVED (wording before the edit; superseded in part by Note HP, 2026-11-08)
- **HN3 The trailing row holds what no chip shows, and names each member (M1).** It holds a helper that fills nothing (dimmed by the disabled alpha, hover "not used: …"), a helper whose target has no chip on the card, a `_m` env mask and a clashing second member of a kind. A helper reads "<file>, <fills text>" (the HP2 wording), any other member "<file>, <role>", also in its hover. Each tile also paints a short tag bottom-left (HP9): its slot letter when the slot is taken, "unused" for an unused helper, the first fill ("M G") for a helper whose fills land on no chip, otherwise the member's tail ("_m", or "—" when empty); a tag wider than the tile is cut with `…`. This replaces "Members in no chip follow as plain thumbnails" with a row that says why each member is there.

ARCHIVED (wording before the edit; superseded in part by Note HP, 2026-11-08)
- **HP10 Test support.** The `overlap` frame check spares a badge centred on its chip's corner: a pair is not a finding when the smaller rect is at most 24 px a side and its centre is within 1.5 px of a corner of the larger on each axis; a badge centred mid-edge is still flagged. `tests/ui_proof.rs` has `replay_with(fixture, before, lines)` and `replay_mid` (narrows the window between a set's open and the later lines, because narrowing before the click leaves the card unclickable for the driver); `sets_column_pins.rs` has `sets_width(h, w)`, which persists the Sets column's chosen width and settles 3 frames, for pins that need a narrow column now that opening a set at 1055 no longer narrows it.

ARCHIVED (wording before the edit; superseded in part by Note HL6, 2026-11-08)
- A set with no active match shows a Warning row and is left out of the JSON. [V4]

ARCHIVED (wording before the edit; superseded in part by Note HL6, 2026-11-08)
- PGPatcher JSON is GUI-only, so GUI and CLI agree for non-PBR sets only. A texture-set block's `statics = "auto"` takes the non-LOD winning meshes of the set's key from the scan (no scan, no entry); a PBR Landscape set uses its card's box. [P14, AD7]

ARCHIVED (wording before the edit; superseded in part by Note HF1, 2026-11-08)
- **HE7 Two raw maps of one name are not duplicates (owner's report, 2026-11-06, `mouth_ao.psd` + `mouth_ao.png` in one Advanced Skin set: "cannot use either, is stuck using fill even when one is removed from the set"; the ruling is an agent ruling).** A raw map and a skin `_f` write no file at their own output (final review M5, agent ruling): pack planning consumes them or reports `PACK-UNUSED-INPUT` "not converted". So `prepare` leaves such an image out of the `IN-DUPLICATE` comparison (AF7 already compares only the jobs that write): `x_ao.png` + `x_ao.tga` give no `IN-DUPLICATE`, the first by path feeds the channel (M13) and the other is `PACK-UNUSED-INPUT`, which the AO source picker can swap. The rule follows the writer, not the name: a file whose sub-type is requested, or whose `helper.role` is `none`, writes and is compared as before, so two of those are still `IN-DUPLICATE`; two files with no raw name (`wall.png` + `wall.tga`) stay duplicates; a `_f` outside an Advanced Skin set is a PBR map that writes. My calls (agent ruling): a `[[rules]]` entry that types a raw name is invisible to `prepare` (rules need the analysis), and the BV2 writer dedupe is its backstop, so two rule-typed twins never both write; a file removed from the set by `SetEdit::Remove` is a set of its own that packs the same default output, so the BV2 dedupe still flags that one file. The fast re-plan runs the same rule (`api::waiting_images`) on the old and the new state over every set and returns `NeedsFull(DUPLICATE_CHANGE)` when a file moves in or out while another file shares its output, so a skin `_f` typed by hand, or a neighbour's sub-type that unmakes the skin set, is seen; on a cs PBR set `PBR_SUBTYPE` already forced a full plan for such edits. Tests in `crates/engine/tests/raw_twins.rs`, `a_sub_type_edit_on_a_twin_plans_in_full` in `fast_replan.rs`, `picker_can_switch_between_twins` in `helper_picker.rs`.

## Note HG — Texture set records in the sets model (main team, 2026-11-06)

ARCHIVED (wording before the edit; superseded in part by Note HL4, 2026-11-08)
- **HG8 The picker, the record read and the record-change rows (Task 6; the key is the owner's, the rest agent rulings).** (a) **Key, owner (2026-11-06):** "Key is plugin + EDID, not FormID. Should probably have formid somewhere in there, written as 0x (no header, first two digits, no leading 0's)... edid within a plugin itself is not a forced unique." The settled reading is plugin + FormID with the EDID as a label: `0x05000814` shows as `0x814` (`formid_text`). It is computed from `rec.local`, never `rec.formid`, whose load-order digits move (a light plugin's `FE4CD814` would keep `4CD`). (b) **The defining plugin, agent:** the plugin in a record's key is `masters_json[local >> 24]`, or the winning plugin itself when the index is at or past the master count (`clamp_local`), so an override finds the same record when the winner changes; `TxstRecord.winner` is shown beside it when they differ. The read is one query over `rec` / `rec_winner` / `plugin` / `txst_slot` with `owner_ok = 1`, ordered by EDID; a record with no path is not listed; `txst_slot.slot` is converted by `tx_to_slot` there (HG1). Like `supplies` (K1) it sits behind `unwrap_or_default`, so a failing record query empties only the picker. (c) **The picker, agent:** **Map to texture set record…** is greyed with "Scan your mods first" when the scan holds no record, and with "PGPatcher PBR sets name their outputs in the JSON; use Rename outputs" for a PBR set on Community Shaders, the same set `record_entries` ignores (HG5); the scan reason comes first. The dialog writes nothing until **Map**; it has no Enter default, so Enter never maps; a click outside does not close it (it holds the typed words). The list is a `show_rows` list of one fixed row height, filtered again only when the words or the scan change (`record_hits`, kept in `Pending::MapRecord`). **Change record…** opens on the set's current record when the scan has it. (d) **Record changes, agent:** a mapped set whose stored slots differ from `record_of` (plugin + FormID, ASCII-folded, the winner not compared) gets a Warning row "Record {EDID} changed" with **Use new paths**, which copies the record's paths and EDID in as one undo step. A record missing from a non-empty scan gets an Info row (no fix, no Review block), and the mapping and stored paths stay. An empty scan says nothing, so the mapping survives a scan that has not run. A PBR set on Community Shaders gets neither row: its mapping is ignored. (e) **A supplied slot shows where the shader has a row for it, agent:** the set editor writes "From record: {file}" in an empty slot row the record supplies. The Advanced skin editor has no `_s` row, so a supplied slot 8 shows nowhere there; its warning is still dropped. The card's "Record:" line is small, not weak, and its hover lists every slot path, marking each the set does not write itself with "(from the record)". (f) **One rule for what a member writes, agent:** `member_write` says what a member of a mapped set writes: the output key, the record's path and where it goes under the export folder. `record_entries` (the plan) and the picker's preview both call it, so a packed output with a job on a record slot is previewed, and a kept DDS with no job, a refused path, a packing source and a helper say "keeps its own path". Path collisions are not predicted in the preview. (g) **A new record starts with every fill on, agent:** `map_record` to a different key (plugin ASCII-folded + FormID) and `unmap_record` clear the set's `no_fill` in the same edit, so one undo restores it; the same key (**Use new paths**, **Change record** to its own) keeps it. (h) **Arrow keys, agent:** Up and Down in the picker move the highlight (no highlight to the first row on Down) and scroll it into view; they never map, and Enter still does nothing. (i) **`record_ignored`, agent:** `AppModel::record_ignored(&SetView)` is the one test for "a PBR set on Community Shaders, whose mapping is ignored"; the plan, the preview, the menu and the Problems rows all use it.

## Note HL — Target mod and in-development guard rails (builder, 2026-11-07)

ARCHIVED (wording before the edit; superseded in part by Note HQ, 2026-11-08)
- **HL5** Every PBR set gets its PGPatcher JSON entry on Community Shaders, also with no PBR albedo. `SetView::json` = `pbr`, or a set whose shader pick is `Shader::Pbr` and that is no skin or shared set and has no `pbr_albedo` diffuse; `pbr` keeps its meaning (the albedo set, the prefix owner) and the JSON readers (`json_entries`, blocks, asks, `is_json_set`, auto flags) read `json`. `derived_match`, and `match_from` (`MatchFrom::{Albedo, Mapping, BasePath}`), come from the first that exists: the mapping's base, a record's slot 1 (not for an albedo set), the albedo or diffuse, the set id's own path. A no-albedo set keeps its meshes' textures: `slot1` is the mapping's slot 1 else `textures\<derived path>.dds` (PGPatcher skips a shape whose slot 1 would point at a missing `textures\pbr` copy), every filled slot names its output path, an empty slot 2 or 6 takes the mapping's stored path else `lock_normal` / `lock_rmaos` is written; an explicit `slotN` or lock still wins. A filled slot no path names (a kept DDS with no job) is locked too (`lock_empty_slots`), so PGPatcher keeps the mesh's own. The slot paths are game paths: an output path gets `textures\` in front when the project root is itself a `textures` folder (`slots_of`), and the entry is left out, with its "mesh keeps" row, when a typed `match1` matches nothing. A set with a PBR map and no filled slot has nothing to write, so `json` also needs at least one filled slot (`finish_view` in `model/sets.rs`; an all-DDS set gets no entry). On a non-CS target such a set stays "PBR, no JSON", badge "PBR set (no JSON on this target)"; on CS a PBR pick with no filled slot has the badge `BADGE_PBR_NOTHING_TO_WRITE`, since the target is not the reason. Tests `target_mod::a_no_albedo_set_without_a_mapping_keys_on_its_base_path`, `a_mapped_no_albedo_set_keys_on_the_mapping`, `pgpsim_patches_the_written_no_albedo_entry`, `the_json_slots_name_where_the_files_were_written`.

## Note HF — Set groups, partial export and textures in no set (builder, 2026-11-08)

ARCHIVED (wording before the edit; superseded in part by Note HF9, 2026-11-09)
- **HF2 The engine side is one edit, `SetEdit::NoSet { file }`.** `engine_inputs` appends one per `no_set` entry after the other edits. `apply_set_edits` treats it as `Remove` (a one-file set), and `prepare` collects its paths (`sets::no_set_paths`) and leaves them out of the `counts` closure with `waiting_images`, so a no-set file blocks no same-named file with `IN-DUPLICATE` (`SetEdit::Remove` still does). A `no_set` change changes `set_edits`, so it is always a full plan. Until the `write_only` filter lands (a later item of this note) a no-set file whose type resolves can still reach the writer, where the BV2 dedupe catches a clash; there is no second no-write mechanism. Test `crates/engine/tests/no_set_twin.rs`.
