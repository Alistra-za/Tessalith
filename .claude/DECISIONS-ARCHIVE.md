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

## Note HE — 2.1 leftovers (main team, 2026-11-06)

ARCHIVED (wording before the edit; superseded in part by Note GW3, 2026-11-07)
- **HE7 Two raw maps of one name are not duplicates (owner's report, 2026-11-06, `mouth_ao.psd` + `mouth_ao.png` in one Advanced Skin set: "cannot use either, is stuck using fill even when one is removed from the set"; the ruling is an agent ruling).** A raw map, a hand-set normal layer (`helper.role` layer) and a skin `_f` write no file at their own output (final review M5, agent ruling: a normal layer found only by its name, `x_detail.png` + `x_detail.tga`, is not counted, so two of those stay `IN-DUPLICATE`; the rule is not extended): pack planning consumes them or reports `PACK-UNUSED-INPUT` "not converted". So `prepare` leaves such an image out of the `IN-DUPLICATE` comparison (AF7 already compares only the jobs that write): `x_ao.png` + `x_ao.tga` give no `IN-DUPLICATE`, the first by path feeds the channel (M13) and the other is `PACK-UNUSED-INPUT`, which the AO source picker can swap. The rule follows the writer, not the name: a file whose sub-type is requested, or whose `helper.role` is `none`, writes and is compared as before, so two of those are still `IN-DUPLICATE`; two files with no raw name (`wall.png` + `wall.tga`) stay duplicates; a `_f` outside an Advanced Skin set is a PBR map that writes. My calls (agent ruling): a `[[rules]]` entry that types a raw name is invisible to `prepare` (rules need the analysis), and the BV2 writer dedupe is its backstop, so two rule-typed twins never both write; a file removed from the set by `SetEdit::Remove` is a set of its own that packs the same default output, so the BV2 dedupe still flags that one file. The fast re-plan runs the same rule (`api::waiting_images`) on the old and the new state over every set and returns `NeedsFull(DUPLICATE_CHANGE)` when a file moves in or out while another file shares its output, so a skin `_f` typed by hand, or a neighbour's sub-type that unmakes the skin set, is seen; on a cs PBR set `PBR_SUBTYPE` already forced a full plan for such edits. Tests in `crates/engine/tests/raw_twins.rs`, `a_sub_type_edit_on_a_twin_plans_in_full` in `fast_replan.rs`, `picker_can_switch_between_twins` in `helper_picker.rs`.
