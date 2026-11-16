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

ARCHIVED (wording before the edit; superseded in part by Note HI, 2026-11-09)
  - **One line.** An undecided texture that asks its alpha shows `Alpha: <Type> (proposed) — confirm` (`code_text::alpha_confirm_text`; the type names are the five that the old buttons used). It replaces three lines: the Undecided sentence, the "carries data the format would drop" line with the engine's "non-constant alpha would be discarded", and the Acknowledge button. A row that asks only its alpha has no `Undecided:` prefix; a row that also asks something else keeps the prefix and joins the parts. Only "(proposed)" exists: a texture whose meshes decide its alpha is Ready and never asked, so there is no "(from mesh)" text.

ARCHIVED (wording before the edit; superseded in part by Note HI, 2026-11-09)
  - **One dropdown.** `settings_panel::alpha_dropdown` is the control in the Problems row, Review's inline row and the settings box. It opens on the proposal (`items::alpha_asked`: `suggest_alpha` over the cached analysis, else the profile's own policy). Picking an entry, the shown one too, records the answer with the old buttons' write (`alpha.policy` in the texture's own entry through `decide` or `answer_undecided_row`, nothing in `allow`). Enter or Space on a focused Problems row that asks only its alpha confirms the shown type (a row that also asks a sub-type ignores the key, so it never answers just one of the two); Accept all suggestions stays type-only. The `Decide…` menu and the five buttons are gone. The dropdown's accessible name is `Alpha type: <shown>`.

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

ARCHIVED (wording before the edit; superseded in part by Note HF, 2026-11-09)
- Engine policy (formats, alpha gate, versions, writes): Answers, A, B, D, E, F, I, K, L, M, O, R, S, T, W, Y, AC, AG, AK, AV, AW, AX, AZ, BB, BC, BF, BJ, BL, BQ, BV, DB, DC, DM, DS, DY, FA, FF, FL, GJ, FM, FQ, FV, FZ, HB, HE, GU, GV, GW, GY

ARCHIVED (wording before the edit; superseded in part by Note HF, 2026-11-09)
- Packing and helpers: B, H, I, M, R, W, AL, BB, BP, BQ, BV, CN, DR, DY, FB, FM, FQ, FV, HC, HE, HG, GV, GW, HL

ARCHIVED (wording before the edit; superseded in part by Note HF, 2026-11-09)
- GUI Files and Inspector: N, P, S, U, Z, AA, AE, AF, AG, AM, AN, AO, AV, AY, BH, BI, BN, BO, BR, BS, BV, BY, CA, CC, CD, CE, CF, CG, CV, DN, DO, DP, DU, DZ, FA, FB, FC, FD, FI, FK, FL, FM, FN, FO, FP, FT, FS, FW, FU, FY, HB, HC, HE, HG, GV, HM, HN, GW, GY, HP, IA, HL

ARCHIVED (wording before the edit; superseded in part by Note HI, 2026-11-09)
- GUI Dialogs and Export: AG, BH, BO, BR, BV, BY, CA, CD, CE, CF, CN, CV, DT, FB, FC, FI, FM, FN, FO, FP, FS, FW, FU, GU, HB, HC, HE, GV, GY, HL, HF, HQ, HH

ARCHIVED (wording before the edit; superseded in part by Note IE, 2026-11-16)
- GUI Settings panel: S, V, X, Z, AF, AG, AJ, AN, AS, AW, BM, BS, CC, CL, CV, FB, FC, FM, FS, FT, FY, GU, GV, GW, HP, IA, IB, HI, HJ, HV, ID
- GUI Meshes view: BA, BT, CV, CZ, FC, FI, GC, GD, GH, GJ, GK, GN, GO, GP, FO, GQ, GR, HS, ID
- pgpsim: AU, CM, GE, ID

ARCHIVED (wording at the task base b06542ad; superseded in part by Note IE, 2026-11-16)
- GUI Files and Inspector: N, P, S, U, Z, AA, AE, AF, AG, AM, AN, AO, AV, AY, BH, BI, BN, BO, BR, BS, BV, BY, CA, CC, CD, CE, CF, CG, CV, DN, DO, DP, DU, DZ, FA, FB, FC, FD, FI, FK, FL, FM, FN, FO, FP, FT, FS, FW, FU, FY, HB, HC, HE, HG, GV, HM, HN, GW, GY, HP, IA, HL, HF, IB, HJ, HR, HV, HS, ID
- Packing and helpers: B, H, I, M, R, W, AL, BB, BP, BQ, BV, CN, DR, DY, FB, FM, FQ, FV, HC, HE, HG, GV, GW, HL, HF, HH, HR, HS
- Presets and config: G, N, S, T, X, AE, AL, AM, AP, AS, AV, AZ, BC, BJ, BO, BQ, BR, BV, CN, CV, FB, FL, FM, FQ, FX, FY, GU, HB, GV, GW, IA, IB
- CLI: I, J, K, X, BC, BQ, CN, DY, FC, FM, FX, GV, GW, GY, IB, HS
- GUI Settings panel: S, V, X, Z, AF, AG, AJ, AN, AS, AW, BM, BS, CC, CL, CV, FB, FC, FM, FS, FT, FY, GU, GV, GW, HP, IA, IB, HI, HJ, HV, ID, IE
- GUI Meshes view: BA, BT, CV, CZ, FC, FI, GC, GD, GH, GJ, GK, GN, GO, GP, FO, GQ, GR, HS, ID, IE
- pgpsim: AU, CM, GE, ID, IE
- Process and docs: Answers, BX, DM, FD, FG, FH, FM, HA, HB, HK, GS, GV, HM, HN, HO, GW, HP, HT, HU, HW, HX, HZ

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

ARCHIVED (wording before the edit; superseded in part by Note HF, 2026-11-09)
- Sets: only created sets can be deleted; a set whose files are all gone still shows, as missing; delete and order are GUI-only. [P2, P3, P18]

ARCHIVED (wording before the edit; superseded in part by Note HJ, 2026-11-09)
- One match key per entry; a renamed set gets no extra match but carries `match_diffuse`. [AN1, AN2, AA8]

ARCHIVED (wording before the edit; superseded in part by Note HJ, 2026-11-09)
- `rename` is a file name (a `\` or `/` blocks export, one `.dds` cut); the prefix comes from the lowest active `matchN` under `textures\pbr\`. The TruePBR box: visual settings, Matching, then collapsed Advanced TruePBR settings (`path_contains`, `nif_filter`, `rename`, UV, vertex, locks, inner UV scale, coat colour). [V3, V7]

ARCHIVED (wording before the edit; superseded in part by Note HS3, 2026-11-10)
- A filled output records its helper last as a source with `fill` (`a:<r|g|b|a|luma>`, plus `:inv`); a packed source records `read:<id>` for an explicit `helper.read`, then `ch:<rgba>`. `process_file` and `api::current_sources` both call `pack::source_fill`, so a changed helper, read or invert re-encodes; outputs without a helper record no `fill`. [DY14, DY21, FM12]

ARCHIVED (wording before the edit; superseded in part by Note HS9, 2026-11-10)
- Save, header Export and Review's Export blur a focused text field on press so it commits and the release clicks; Ctrl+S in a text field ends the edit and saves next frame. [FC8, FC14]

ARCHIVED (wording before the edit; superseded in part by Note HS12, 2026-11-10)
- Edit source and documents: `pick_source` compares roots ignoring path spelling and never takes the edit mod's own copy or any `sdc NIF Edits` folder, nor any folder holding our edit document for this NIF. An autosave is deleted only when this session holds it; a session stays unsaved until its save succeeds; a root change waits for the live edit to move (else blocked with the reason, retried when that file changes); the newer of autosave and folder document wins. [CV1, GD11, GD12, GD13, GD20, GD21, GD24, GD27, GD28, GQ3]

ARCHIVED (wording before the edit; superseded in part by Note IE4, 2026-11-16)
- Scan schema 10 (GO4/GO5); every schema bump empties scan.db until the next scan. Schema 9 dropped the dead tables, columns and writers; Plugin overrides lists every record. [GJ7, GO5]

ARCHIVED (wording before the edit; superseded in part by Note IE5, 2026-11-16)
- The env cube's mips are CS's specular irradiance, built by CS's SpecularIrradianceCS on the GPU; the CPU chain samples the texel corner. [GL1, GM1, GM2]

ARCHIVED (wording before the edit; superseded in part by Note IE5 and Note IE6, 2026-11-16)
- MATO snow/ash layer (Set preview toggle): the record is the first scan `edids` match (ASCII case-insensitive), read on a worker (one plugin at a time, keyed cache); a missing or bad record draws default coverage (falloff 0.35 / 0.4, noise 48) with a grey line, never an error; so does a missing projected texture (`Renderer::projected_missing()`). Skinned shapes, shader types 4, 5, 14, 16, SLSF1.12, and on a vanilla shape types 3 and 11 and SLSF2.24 skip the layer (a PBR shape of type 3 or 11, or a PBR SLSF2.24 coat, keeps it; GZ1, GZ3). Bad JSON values take CS defaults (one bad `baseColorScale` component resets all three; glint only for boolean `true`). Changed layer textures show after a toggle off and on. Glint noise ports `noisegen.cs.hlsl`; vertex colour follows `has_vertex_colors`. [GG3, GG4, GG5, GG6, GG7, GG8, GG9, GG10, GG12, GG13, GG19, GG20, GG21]

ARCHIVED (wording before the edit; superseded in part by Note IE24, 2026-11-16)
- A MATO-only landscape set needs no RMAOS. CS does not draw the projected normal and detail samples in this build (slot hover says so); they still export. A landscape `_noise` is always ProjectedNoise; the detail names are `detail_normal` there (GW1). The two new kinds have no slot number, so no slot-keyed map gets them. The card shows every refusal `prepare` can know. The preview takes the profile's textures, coverage, then MATO DATA, cycling the layer when texture paths change. [GI10, GI15, GI16, GI17, GI18, GI19, GI21, GI25, GI28, GI29]

ARCHIVED (wording before the edit; superseded in part by Note IE26 and Note IE29, 2026-11-16)
- On a PBR shape the Shader tab shows "PBR: <meaning>" beside SLSF2 bits 12, 24, 25, 26, 27, 30 (`render3d::pbr_bit_meaning`). Three warnings only: type 11 without PBR and without bit 24; bit 24 on a non-11 type; Fit_Slope (bit 12) without PBR. Soft/Rim/Back lighting and Effect_Lighting are never warned. [CI26]

ARCHIVED (wording before the edit; superseded in part by Note IE26, 2026-11-16)
- Controllers tab: curve graph, key table, Linear/Quadratic combo, no Play controls; the key cache is rebuilt in `reapply`, never per frame. PBR shapes relabel LSCF 9 "Specular level", LSCF 10 "Roughness scale", LSCC 0 by coat/subsurface bit. A new `Op` variant updates every exhaustive match in the same commit, including gui `keep_base`. [CM12, CM15, CM29]

ARCHIVED (wording before the edit; superseded in part by Note IE26 and Note IE29, 2026-11-16)
- Env map scale (LSCF 8) on a PBR shape crashes the game: an export-blocking Error; on a shape PGPatcher makes PBR it is a red In game line plus an amber Warning (cached simulation), not blocking. An empty RMAOS slot is an amber row (CS's default RMAOS is fully metallic). [CM25, GD6, CM27, CM32]

## Note GQ — NIF trace-debug fixes (NIF team, 2026-11-04)

ARCHIVED (superseded by Note GT, 2026-11-06)
- **GQ6** The preview compiles the user's Game Gamma: `preview_defines` adds `CS_GAME_GAMMA=<f32 to 9 decimals>` (CS's own form, `1.600000024`) only when Linear Lighting is on (the shaders read it only under `ENABLE_LL`, TransferFunctions.hlsli) and `"Linear Lighting".gameGamma` is a usable value other than 1.6, so default users and LL-off users keep their defines and program keys; the Lighting programs and the composite share it through the feature list, and `settings_define` treats it as a settings define. `ENABLE_ACESCG` stays uncompiled (shortcut: the readback has no ACEScg conversion). A non-UTF-8 Info.ini is now a note with the Features-ini fallback, like an unreadable one, not a parse failure that kept the old settings.

ARCHIVED (wording before the edit; superseded in part by Note HS5, 2026-11-10)
- **GQ5** A set member edit in the set preview no longer pushes the whole scene: the member re-encodes and its new bytes are swapped in (`sync_textures`), as a cap change already was. Only a member the edit newly blocks pushes the scene, so the game's copy of its texture comes back: `Encoder::sync` sets `scene_dirty` and `SetPreview::check` takes it in the same frame. Before, every member edit pushed the scene again.

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

ARCHIVED (wording before the edit; superseded in part by Note HS13, 2026-11-10)
- **HF4 Set groups live in `[[groups]]` and the views decide what shows (owner's spec; agent rulings for the code).** `[[groups]]` holds `name`, `sets` (set ids, as `[project] order` does; a set a hand edit lists again, in its own or a later group, is read in its first group only, `SetsDoc::groups`) and `excluded` (left out when false; `SetsDoc::groups`). Membership is kept there whatever the cards do: what a group shows reads `views`, so a set with no card (Note GY5's waiting lone file, or no set) is neither shown nor counted in it, membership never keeps a card, and a group whose members all lack a card has no header (`AppModel::groups`, `group_of`) while its entry stays and the header comes back with a card. A group goes with its last set only when that member leaves the membership (Delete set, Move to group or No group, Ungroup), not when its cards hide. Names are unique ignoring case; Rename refuses a taken one with the reason, and a group's own name in another case is its own. `move_to_group`, `rename_group`, `ungroup` and `set_group_excluded` return `Result` (the read-only reason, or the refusal), are one undo step and do not re-plan. Delete set takes the set out of its group in its own undo step, so undo brings back `no_set`, the entry and the membership together. `rename_set` and `retarget_auto` carry membership, also for an automatic set that is only in a group and has no entry, and for one whose new id already has an entry (the entries stay put; membership still follows). A collapsed group is a `gui.toml` key by project and group name (`group_collapsed`, same key as `match_slots`), stored only while collapsed, with no undo step.

## Note IB — UV checker leaves the GUI; sdc uv and sdc dilate (NIF team for the owner, 2026-11-08)

ARCHIVED (wording before the edit; superseded in part by Note HS10, 2026-11-10)
- **IB11 The output is `<same file name>.png` in a `dilated` folder beside the input (`--out` overrides), same bit depth and channel layout.** The name stays so Tessalith still reads the suffix. An existing output is kept and reported unless `--overwrite`; an output that would replace its own input is refused. Exit codes as `sdc uv` (IB6): 0 all written; 1 an input wrote nothing (no mask, size mismatch, float, no valid texel, exists); 2 no image matches; 3 a write failed. One stdout line per file: path, kind, mask, filled texel count.

## Note HY — bcenc, rgbcx's block encoders in Rust (main team, 2026-11-13)

ARCHIVED (wording before the edit; superseded in part by Note HY9, 2026-11-13)
- **HY1 `bcenc` holds the Rust BC4-family search; the C++ is the reference.** `crates/bcenc` has no dependencies and no `unsafe`. `bcenc::bc4` is the BC4 search that lived in `encoders-sys/tests/support/bc4ref.rs` (moved; behaviour unchanged, branch counters added), and `bc4_encode_block`, `bc5_encode_block` and `bc4_encode_alpha` (BC3's alpha half, with the cut) wrap it. They are proven byte-equal to `encoders_sys` on generated block classes at radii 4, 5 and 10 (`crates/bcenc/tests/bc4.rs`) and, in the `#[ignore]` test, on 20.02 million blocks at radius 5 (57.9 s on 16 threads, 0 mismatches). The vendored C++ stays the specification and is never edited. A `hit!` counter marks each branch and compiles to nothing outside the crate's unit tests; `every_ported_branch_is_taken` fails if a generator stops reaching one.

## Note GT — Bundled Community Shaders source (owner, 2026-11-06)

ARCHIVED (wording before the edit; superseded in part by Note ID54, 2026-11-14)
The 3D preview uses a pinned CS tree embedded in the exe, with CS's default settings; it never looks for the user's CS (owner, 2026-11-06).
- **GT1 The pinned tree is CS 1.9.0 (AIO build 2026-10-03), from jiayev's development fork, 57 files (481 KiB) in `crates/render3d/shaders/cs/` plus `SettingsDefault.json` (owner, 2026-11-06).** It holds only the include closure of `Lighting.hlsl`, `DeferredCompositeCS.hlsl` and `DynamicCubemaps/SpecularIrradianceCS.hlsl`, copied byte for byte by `tools/cs-bundle.py`, which exits non-zero on an unresolved include. `.gitattributes` marks it `-text`. Its licences are in `THIRD_PARTY_NOTICES.md` and `LICENSES/`, and CLAUDE.md names it as the one exception to "no game or mod assets in the repo". Updating the pin is a commit that also refreshes the goldens. `d2026.10.11.0`, in the capture folder name, is the version MO2 holds for that mod, not a CS build. The tree matches no public commit found in `community-shaders/skyrim-community-shaders`; the fork was not searched.
- **GT2 Shader text is read through `render3d::ShaderSource` (`Bundled`, the default, or `Folder`).** `build.rs` embeds `shaders/cs` as a table sorted by lowercased path; `bundled::read` folds ASCII case and `\` to `/`. `includes::expand` and `SourceSet` take the source and keep the includer-folder-then-root order, the depth cap of 64 and root-relative `#line` names, so a bundled expansion is byte-identical to the folder it came from. A release build always reads `Bundled`; `Folder` exists only under `test-hooks` (GT4).
- **GT4 The preview compiles the bundled tree with `cs_settings::defaults()` and never locates the user's CS (owner, 2026-11-06).** `render3d::locate`, `Fingerprint`, `Change`, `SettingsPaths`, `read_settings`, `Info.ini` and `feature_inis` handling, the change hash, `Renderer::set_shaders`/`shaders()`, the GUI locate flow ("Shader folder", the not-found state, the moved popup, the settings re-read on focus) and the `gui.toml` `render3d.shaders` key are gone, with the `walkdir` dependency; an old `[render3d] shaders` key is ignored on load and dropped on the next save. `ShaderSource::Folder` exists only under the `test-hooks` feature, through `Renderer::with_source` and `Viewport3d::use_shader_folder`. The preview no longer follows the user's `SettingsUser.json`, the MO2 overwrite copy, `Shaders/Features` inis, their enabled features, or Game Gamma: it draws CS's defaults, so a user who changed them sees the default look. This supersedes GQ6 (the Game Gamma define); `preview_defines` keeps the code path but defaults give 1.6, so no `CS_GAME_GAMMA` is added. All 48 features load by default, so the "Off in this preview" line is long: it is drawn on one row, cut with an ellipsis, with the full list on hover and in the accessible name.
- **GT5 Tests that only need the shader source read the bundled tree and no longer skip; tests that need owner data stay gated (agent ruling, 2026-11-06).** The `Lighting.hlsl` compile, expand, draw, deferred, debug-view, composite, prefilter and missing-fill checks in `crates/render3d/tests` use `ShaderSource::Bundled` with no `SDC_CS_SHADERS`; they add 7.6 s to `--skip render_` and run in the default parallel pass, so none is renamed `render_`. The shared cache was waived. Golden and replay tests (`golden`, `golden_material`, `replay_real_fixture_smoke`) compile the bundled tree too, since the captures are of the pinned build, but still need `SDC_GOLDEN` (and `SDC_ACHERUS` for the meshes); they passed on the pinned captures under the new default defines; `SDC_CS_SHADERS` stays only for the two folder comparisons (`bundled_expansion_equals_the_folder`, `feature_names_equal_the_pinned_features_folder`). The docs state the pin and the defaults once, in `docs/wiki/GUI-Set-Preview-3D.md`, with a CHANGELOG line for the removed folder flow.

ARCHIVED (wording before the edit; superseded in part by Note IE5, 2026-11-16)
- **GT3 `cs_settings::defaults()` is the bundled `SettingsDefault.json` with every feature loaded.** It parses the embedded file once, takes `Linear Lighting`, `gameGamma` and the other values from it, and counts the 48 `Shaders/Features/*.ini` stems in `FEATURE_NAMES` as loaded, so `preview_defines` gives `CS_HAIR, CS_SKIN, DYNAMIC_CUBEMAPS, ENABLE_LL, EXTENDED_MATERIALS, EXTENDED_TRANSLUCENCY, VANILLA_FRESNEL` and Linear Lighting is on. A test checks `FEATURE_NAMES` against the pinned `Features` folder when `SDC_CS_SHADERS` is set. The `coverage` drift report, `read_settings`, `SettingsPaths` and the change hash are gone (GT4).

ARCHIVED (wording before the edit; superseded in part by Note IE5, 2026-11-16)
The 3D preview uses pinned CS trees embedded in the exe, with CS's default settings; it never looks for the user's CS (owner, 2026-11-06). Tree A (CS 1.9.0) is drawn by default, tree B (the TruePBR 2.0 test build) while the "CS PBR 2.0 (experimental)" box is on (Note ID54).
- **GT1 The pinned tree is CS 1.9.0 (AIO build 2026-10-03), from jiayev's development fork, 57 files (481 KiB) in `crates/render3d/shaders/cs/` plus `SettingsDefault.json` (owner, 2026-11-06).** It holds only the include closure of `Lighting.hlsl`, `DeferredCompositeCS.hlsl` and `DynamicCubemaps/SpecularIrradianceCS.hlsl`, copied byte for byte by `tools/cs-bundle.py`, which exits non-zero on an unresolved include. `.gitattributes` marks it `-text`. Its licences are in `THIRD_PARTY_NOTICES.md` and `LICENSES/`, and CLAUDE.md names it as the one exception to "no game or mod assets in the repo". Updating the pin is a commit that also refreshes the goldens. `d2026.10.11.0`, in the capture folder name, is the version MO2 holds for that mod, not a CS build. The tree matches no public commit found in `community-shaders/skyrim-community-shaders`; the fork was not searched. A second pinned tree, tree B in `crates/render3d/shaders/cs2/` (CS PBR2Test1, the TruePBR 2.0 test build), is bundled by the same tool, with one file overridden (`Skin/Skin.hlsli` from jiayev/advanced-skin be6afe5, Note ID26), and drawn only while the "CS PBR 2.0 (experimental)" box is on (Note ID31); the CS 1.9.0 tree is tree A.
- **GT2 Shader text is read through `render3d::ShaderSource` (`Bundled`, the default, `BundledB` for tree B (Note ID26), or `Folder`).** `build.rs` embeds `shaders/cs` and `shaders/cs2` as two tables (`FILES`, `FILES_B`), each sorted by lowercased path; `bundled::read` folds ASCII case and `\` to `/`. `includes::expand` and `SourceSet` take the source and keep the includer-folder-then-root order, the depth cap of 64 and root-relative `#line` names, so a bundled expansion is byte-identical to the folder it came from. A release build reads `Bundled` or `BundledB` (the box, ID31), never `Folder`; `Folder` exists only under `test-hooks` (GT4).
- **GT4 The preview compiles the bundled tree A with `cs_settings::defaults()` and never locates the user's CS (owner, 2026-11-06); while the CS PBR 2.0 box is on it compiles tree B with `cs_settings::defaults_in(Tree::B)` instead (Note ID29-ID31).** `render3d::locate`, `Fingerprint`, `Change`, `SettingsPaths`, `read_settings`, `Info.ini` and `feature_inis` handling, the change hash, `Renderer::set_shaders`/`shaders()`, the GUI locate flow ("Shader folder", the not-found state, the moved popup, the settings re-read on focus) and the `gui.toml` `render3d.shaders` key are gone, with the `walkdir` dependency; an old `[render3d] shaders` key is ignored on load and dropped on the next save. `ShaderSource::Folder` exists only under the `test-hooks` feature, through `Renderer::with_source` and `Viewport3d::use_shader_folder`. The preview no longer follows the user's `SettingsUser.json`, the MO2 overwrite copy, `Shaders/Features` inis, their enabled features, or Game Gamma: it draws CS's defaults, so a user who changed them sees the default look. This supersedes GQ6 (the Game Gamma define); `preview_defines` keeps the code path but defaults give 1.6, so no `CS_GAME_GAMMA` is added. Every feature of the tree loads by default (48 on tree A, 42 on tree B, ID29), so the "Off in this preview" line is long: it is drawn on one row, cut with an ellipsis, with the full list on hover and in the accessible name.
- **GT5 Tests that only need the shader source read the bundled tree and no longer skip; tests that need owner data stay gated (agent ruling, 2026-11-06).** The `Lighting.hlsl` compile, expand, draw, deferred, debug-view, composite, prefilter and missing-fill checks in `crates/render3d/tests` use `ShaderSource::Bundled` with no `SDC_CS_SHADERS`; they add 7.6 s to `--skip render_` and run in the default parallel pass, so none is renamed `render_`. The shared cache was waived. Golden and replay tests (`golden`, `golden_material`, `replay_real_fixture_smoke`) compile the bundled tree too, since the captures are of the pinned build, but still need `SDC_GOLDEN` (and `SDC_ACHERUS` for the meshes); they passed on the pinned captures under the new default defines; `SDC_CS_SHADERS` stays only for the two folder comparisons (`bundled_expansion_equals_the_folder`, `feature_names_equal_the_pinned_features_folder`). The docs state the pin and the defaults once, in `docs/wiki/Previews.md`, with a CHANGELOG line for the removed folder flow.

## Note GZ — MATO layer: PBR coat shapes and rotated statics (NIF team, 2026-11-08)

ARCHIVED (wording before the edit; superseded in part by Note IE5 and Note IE6, 2026-11-16)
- **GZ1 A PBR shape with SLSF2.24 gets the layer.** On a PBR shape SLSF2.24 is not multi-layer parallax: CS's TruePBR.cpp turns it into `PBRFlags::TwoLayer` (the coat) at load and clears the flag, and its PBR permutation list has TruePbr | VC | ProjectedUV with technique None. The coat reads `PBRParams2` and `MultiLayerParallaxData`; the layer writes `MaterialObjectRGBScale`, `ParallaxOccData` and `SparkleParams` and samples t3, t8, t10 and t11, so nothing collides. `projected_eligible` now skips SLSF2.24 only on a non-PBR shape (GZ3 does the same for types 3 and 11); a PBR coat toggle stays a cbuffer edit. **Amends GG6** (DECISIONS-1.x.md, frozen; the digest line cites GZ1).

ARCHIVED (wording before the edit; superseded in part by Note IE5 and Note IE6, 2026-11-16)
- **GZ3 A PBR shape of type 3 or 11 gets the layer.** CS drops the parallax and MLP techniques under TruePBR (`permutation::defines`; the captured Arch_Eye_Moisture, a PBR shape (SLSF2.23) of type 11, has technique 0), so the `PROJECTED_UV` exclusion at Lighting.hlsl:1748 does not apply to it. A coat (SLSF2.24) or fuzz (SLSF2.25) shape keeps type 11 because its MultiLayer floats, which carry the coat and fuzz values (`write_pbr`), load only for type 11, so GZ1 alone missed every such coat. `projected_eligible` skips types 3 and 11 only on a non-PBR shape; FaceGen (4, 5), sparkle snow (14) and eye (16) keep their technique and stay skipped. Tests `eligible_shapes` (a PBR type 3 or 11 shape, with or without SLSF2.24, compiles the type 0 program on both stages) and `render_projected_rotated_static_and_coat` (a type 11 coat draws as a type 0 one; only the coat was rendered).

ARCHIVED (wording before the edit; superseded in part by Note IE23, 2026-11-16)
- **GZ1 A CS 2.0 PBR shape with SLSF2.24 gets the layer.** On a PBR shape SLSF2.24 is not multi-layer parallax: CS 1.x's TruePBR.cpp turned it into `PBRFlags::TwoLayer` (the coat) and its PBR permutation list has ProjectedUV with technique None; on CS 2.0 the coat is the `.csm`'s Two-layer feature and PBR draws the Default technique (ID35). The coat reads `PBRParams2` and `MultiLayerParallaxData`; the layer writes `MaterialObjectRGBScale`, `ParallaxOccData` and `SparkleParams` and samples t3, t8, t10 and t11, so nothing collides. `projected_eligible` skips SLSF2.24 only on a non-PBR shape (GZ3 does the same for types 3 and 11), and reads the drawn material, so a v1 shape, drawn vanilla, is non-PBR there (IE5); a PBR coat toggle stays a cbuffer edit. **Amends GG6** (DECISIONS-1.x.md, frozen; the MATO digest line holds the current rule).

ARCHIVED (superseded by Note IE5 and Note IE23, 2026-11-16)
- **GZ3 A CS 2.0 PBR shape of type 3 or 11 gets the layer.** CS drops the parallax and MLP techniques under TruePBR (ID35; the captured Arch_Eye_Moisture, a PBR shape of type 11, has technique 0), so the `PROJECTED_UV` exclusion of those techniques does not apply to it. `projected_eligible` skips types 3 and 11 only on a non-PBR shape, and a v1 shape draws vanilla (IE5); FaceGen (4, 5), sparkle snow (14) and eye (16) keep their technique and stay skipped. Tests `eligible_shapes` and `render_projected_rotated_static_and_coat` (a coat draws as a type 0 one).

## Note ID — CS TruePBR 2.0 (type 21) (NIF team, 2026-11-14)

ARCHIVED (superseded by Note IE7, 2026-11-16)
- **ID1 `nif` decodes a type-21 BSLightingShaderProperty structurally, and `Pbr2` lives in the trailer.** `decode_lighting` reads the shared head through refraction, then `version: u32`, `features: u32` and 25 parameters, and checks the size (180 + 4N; the vanilla 100 + 4N body is a `BlockSize` error). Any version, feature bits and values decode, even NaN: whether they are valid is a later layer's job. The glossiness, specular colour, specular strength and lighting-effect fields do not exist on type 21 and stay 0.0. The payload is `LightingTrailer::Pbr2 { version, features, params }`, so every exhaustive match on the trailer has to decide what to do with it: `names()` gives no trailer field names, the `render3d` constant buffer and the Meshes editor's `trailer_fields` write and draw nothing for it. `nif::pbr2` holds the 25 names and the spec defaults. How `render3d` parses and draws a type-21 shape per tree is ID33 and ID34. `modscan` keeps `SCHEMA_VERSION` 10: a type-21 NIF scanned before this change stays an unread row until the file changes.

ARCHIVED (superseded by Note IE7, 2026-11-16)
- **ID2 `decode_lighting` reads PBRThinSurface and PBRFuzz for type 21 only.** It walks the property's extra-data refs, takes the first NiFloatsExtraData named `PBRThinSurface` and the first named `PBRFuzz`, and carries their floats as written on `LightingParams.pbr2_extra: Option<Pbr2Extra { thin, fuzz }>` (a wrong count is kept: the validator checks 4 and 2; a block that does not decode is skipped; `None` when neither exists). `scan` and `read_model` both get it from `decode_lighting`. `ExtraValue::Floats` is read only (not in `EDITABLE`). The Extra data tab lists only the shape's and the root's extra data, so a PBRThinSurface or PBRFuzz on the shader's list is not shown there; one on a shape or the root shows as kind + size, and NiFloatsExtraData is in compact's `NO_REFS`. `pbr2_extra` is skipped when `None`, so a v1 shape's `params` JSON in the scan store is byte-identical.

ARCHIVED (superseded by Note IE7, 2026-11-16)
- **ID3 `fields::locate` has `PbrVersion` (p+56), `PbrFeatures` (p+60) and `PbrParam(i)` (p+64+4i, i < 25) for type 21 only, and the vanilla fields refuse type 21.** `Glossiness`, `SpecularColor`, `SpecularStrength`, `LightingEffect1/2` and `Trailer` return an error on a type-21 block, because their offsets fall inside the PBR payload and a `SetField` would have overwritten it; the PBR fields error on any other type. A refused op leaves the bytes unchanged. `Field` gained variants only, so saved edit documents still load. The editor labels them "PBR version", "PBR features" and the `nif::pbr2::PARAM_NAMES` entry (out of range: "PBR parameter n").

ARCHIVED (wording before the edit; superseded in part by Note IE7, 2026-11-16)
- **ID4 `nif::pbr2` holds the CS 2.0 rules as pure data and functions, and `validate` runs them on a type-21 shader.** The data is `VERSION`, `PARAM_COUNT`, the `feat` masks, defaults, `PARAM_RANGE`, the banned and v1 flag masks and the controller variable sets; the functions are `feature_errors`, `allowed_with`, `param_error` and `clamp_param` (the fix value; the default for a non-finite value). On type 21 `validate` skips the glossiness, specular strength and trailer checks and the vanilla type-flag, Multi_Layer_Parallax, Fit_Slope, Eye_Environment and controller rules; the Emissive Multiple, Alpha, Refraction and clamp range checks stay, because they are real head fields.

ARCHIVED (wording before the edit; superseded in part by Note IE7, 2026-11-16)
- **ID5 Every type-21 Error starts "CS 2.0 won't draw this shape: " and always blocks export.** The errors are version not 2, a broken feature rule, a bad parameter, a banned SLSF1 or SLSF2 bit, filled reserved slots, empty required slots, `bsver` not 100, a controller variable CS does not read, and type 22; `validate::always_blocks` (used by `new_errors`) is true for an `ALWAYS_BLOCK` text or any text with that prefix, because the texts name the field. The rules run only on a shader some shape uses (S-P2), and an absent texture set counts as every slot empty; the type-22 Error is not under that gate.

ARCHIVED (superseded by Note IE7, 2026-11-16)
- **ID6 Warns on type 21, and the pgpsim skip.** A v1 PBR flag (SLSF2 12, 23, 25, 26, 27, 30) is a Warn with a fix that clears it (owner 2026-11-14: hidden on 21, never blocks); a PBRThinSurface count not 4 or PBRFuzz count not 2 is a Warn ("CS ignores it"); controller variables CS runs without a visible change (float 9, 10; colour 0) are a Warn. `pgpsim` returns `SimResult::Skip` for a type-21 shape before any entry applies, because PGPatcher 2.3.1 would re-type it or mix v1 and v2 data.

ARCHIVED (superseded by Note IE7, 2026-11-16)
- **ID9 `nif::pbr2::to_v1` (v2 to v1, approximate).** It is the table in reverse for tree A's preview and always sets SLSF2 23. Every set feature the chosen v1 form can't carry goes to `dropped` by `FEATURE_NAMES`: thin surface and foliage, subsurface, fuzz and glint under two-layer, anything but hair under hair, and glint beside fuzz (fuzz wins the trailer). Its trailer is `None` (type 0) unless two-layer, fuzz or glint is on, and specular colour defaults to white and lighting effect 1 to 0 when no feature carries them.

ARCHIVED (superseded by Note IE7, 2026-11-16)
- **ID10 `set_shader_type` converts to and from type 21 and returns edit notes (`Applied.notes`, `(op index, sentence)`).** To 21 (SE only: "Type 21 needs a Skyrim SE NIF" on any other bsver; 20 and 22+ refused) the body is the head to refraction plus version 2, features and 25 parameters from `from_v1` (a v1 PBR shape, SLSF2 23) or `PBR2_DEFAULTS` with features 0 (anything else), and `V1_SLSF2 | BANNED_SLSF2` and `BANNED_SLSF1` are cleared. Notes: "Converted from v1 PBR: <features>" (only for a v1 PBR source, "none" when 0), one "<param> clamped from x to y" per clamp, one "<flag> flag cleared: type 21 doesn't use it" per cleared bit; the flag tables are full, so no "SLSF bit N" fallback exists.

ARCHIVED (superseded by Note IE7, 2026-11-16)
- **ID11 From 21 the body is the head plus glossiness, specular colour, strength, LE1 and LE2 from `to_v1` (type 0 or 11) or the builtin defaults 80, [1,1,1], 1, 0.3, 2.0 (any other target), then `trailer_defaults(t)` with the `to_v1` MLP floats on 11.** SLSF2 loses `V1_SLSF2` and bit 24 first; `slsf2_set` is added only for 0 and 11, and only then is "Dropped: <names>" noted (other targets are plain vanilla, so nothing is being dropped from a PBR shape). Texture slots are untouched.

ARCHIVED (superseded by Note IE7, 2026-11-16)
- **ID12 The GUI shows the notes as the notice.** `NifEdit.notes` keeps the last `reapply`'s notes; after a push the notice is the flag notice plus the notes of ops pushed now, one sentence per note, see `NifEdit::notice_for`; the flag notice is kept byte for byte and a merged op's notes are kept (ID15). The shader type combo offers type 21 only with the setting on and on a bsver 100 NIF (ID21), and a conversion to 21 pushes no flag-fix ops and one from 21 pushes no SLSF2 fix (`set_shader_type` rewrote those words; a second SetBits would conflict), but the SLSF1 type fix still goes in the same undo step (ID13).

ARCHIVED (superseded by Note IE14, 2026-11-16)
- **ID13 v1 PBR means type 0 or 11 with SLSF2 23, for the conversion to 21.** Any other source type (a type-1 or type-14 shape that has the bit) gets `PBR2_DEFAULTS` with features 0 and no "Converted from v1 PBR" note; the refusal text for a bad type names it ("shader type 22 is not a Skyrim type"). The GUI suppresses both flag fixes on a conversion to 21, and only the SLSF2 fix on one from 21.

ARCHIVED (superseded by Note IE7, 2026-11-16)
- **ID14 A type-21 shape set to type 0 never keeps a trailer form.** When `to_v1` chose the type-11 form (two-layer, fuzz or glint), the type-0 shape keeps only SLSF2 23 and 26, the MLP floats are discarded, and "Dropped:" names every feature the trailer carried (two-layer and its coat features, fuzz, glint) beside thin surface and foliage. Type 11 and the other targets are as in ID11.

ARCHIVED (superseded by Note IE7, 2026-11-16)
- **ID15 The notice is built by `NifEdit::notice_for`.** An existing flag notice is left as it was when no op noted anything, notes are appended as one sentence each, and a push that merged into the last op (the op count did not grow) reads that op's notes.

ARCHIVED (superseded by Note IE7, 2026-11-16)
- **ID16 Picking type 21 unlinks the controllers CS 2.0 refuses, PGPatcher PR-882 style, and deletes no block (`structural::unlink_refused`).** The chain is relinked past each one, its next and target become -1, its controlled blocks leave every clip (a clip can end empty and its manager keeps it), and a note reads Unlinked the <label> controller from clip <name>. Refused means the one predicate `pbr2::var_refused_on_21(float, var)`: not in FLOAT_VARS or COLOR_VARS and not in the no-effect sets, which the validator, this unlink and AddController all share. Only lighting shader float and colour controllers are checked; any other controller class on the shader stays as it is. The pick is refused when a refused controller is shared by two properties. A controller already on a foreign type-21 shader is not unlinked by loading; the validator reports it.

ARCHIVED (superseded by Note IE7, 2026-11-16)
- **ID17 AddController on a type-21 shader refuses a refused variable with <label> isn't animated by CS 2.0.** The no-effect variables (float 9 and 10, colour 0) can still be added, since the validator only Warns on them; the label is the editor's `var_label`.

ARCHIVED (superseded by Note IE7, 2026-11-16)
- **ID18 `apply_anim` ignores float 9, float 10 and colour 0 on a type-21 base, and float 8 has no constant there.** CS 2.0 runs those controllers with no visible change. Float 11, 12, 20-23 and colour 1 animate as before; a v1 base is unchanged.

ARCHIVED (superseded by Note IE7, 2026-11-16)
- **ID19 An undo of the pick restores the source bytes.** The unlink is part of the pick op, which re-applies from the source, so removing it gives the source back (GUI history test). A clip block is matched with `anim::bound_cbs`, by ref or by strings, so a block bound by name also leaves the clip.

ARCHIVED (superseded by Note IE5, 2026-11-16)
- **ID20 `GuiSettings.cs_pbr2` is the "CS PBR 2.0 (experimental)" box in the Advanced drawer's Preview section.** It is per user (gui.toml, `#[serde(default)]`, off), not per project; `AppModel::set_cs_pbr2` only saves it, and the NIF editor, preview tree and exports read it in later tasks. The box's AccessKit name is its label, and drawing it saves nothing.

ARCHIVED (superseded by Note IE5 and Note IE7, 2026-11-16)
- **ID21 The app copies `cs_pbr2` into `MeshesModel::cs_pbr2` each frame, and the Shader tab reads that.** `SHADER_TYPES` has 21 entries; type 21 is `TYPE_21_NAME` ("TruePBR 2.0"), listed as "TruePBR 2.0 (21)" only with the box on and bsver 100. With the box off a type-21 shape shows that name in a disabled combo (hover "Tick CS PBR 2.0 in Settings to change it") and its features and parameters are read-only, while the shared fields and the flag lists stay editable; an LE NIF never lists 21.

ARCHIVED (superseded by Note IE7, 2026-11-16)
- **ID22 The Shader tab on type 21 hides glossiness, specular colour, strength, LE1, LE2 and the trailer.** After refraction it shows "PBR version N" (the Task 4 Error and Fix under it when N is not 2), ten feature checkboxes, and the parameters of Base plus each group whose feature is on (Subsurface, Coat = two-layer, Fuzz, Glint, Thin surface) as ranged drag values over `Field::PbrParam`. A feature bit that `allowed_with` refuses is disabled when off, with the first rule it would newly break as hover; an on bit can always be turned off.

ARCHIVED (superseded by Note IE7, 2026-11-16)
- **ID23 On type 21 the SLSF lists hide the v1 PBR bits and lock the banned ones.** `V1_SLSF2` bits are not drawn (a set one shows its Warn and Fix under the SLSF2 list), `BANNED_SLSF1` and `BANNED_SLSF2` stay visible locked off with hover "Not allowed on CS 2.0 PBR", and the "PBR: meaning" suffix is not drawn. Slots 0-8 are named by `PBR2_SLOTS` (spec section 6) whatever SLSF2 23 says.

ARCHIVED (superseded by Note IE7, 2026-11-16)
- **ID24 The Controllers tab on type 21 offers `FLOAT_VARS` then `NO_EFFECT_FLOAT_VARS` (colour: 1, then 0), and the no-effect items carry the hover "Runs, but CS 2.0 shows no change".** A remembered Add variable CS 2.0 refuses falls back to 20 (float) or 1 (colour). `float_value` needs no type-21 case: the decoder holds 0 for glossiness, strength and specular colour there, so a flat start on variable 9, 10 or colour 0 is 0. A controller row on those variables shows the value as "—" (ID25), not that 0.

ARCHIVED (superseded by Note IE7, 2026-11-16)
- **ID25 On type 21 the Controllers tab's Value row shows "—" (hover "Runs, but CS 2.0 shows no change") for float variables 9 and 10 and colour 0, in place of the interpolator's drag values.** The shape card's slot list (the "slot N (name)" lines) names a type-21 shape's slots with `edit_slot_label(21, ..)`, the table the editor uses. A type-21 shape without a `Pbr2` trailer cannot be reached (a short block does not decode to a lighting model), so the section has no fallback line.

ARCHIVED (wording before the edit; superseded in part by Note IE5, 2026-11-16)
- **ID26 Tree B is `crates/render3d/shaders/cs2/`: the include closure (53 shader files plus `SettingsDefault.json`) of the three entry files in the installed CS PBR2Test1 build, with `Skin/Skin.hlsli` overridden by tree A's copy (jiayev/advanced-skin be6afe5).** `tools/cs-bundle.py --override <rel>=<path>` (repeatable) copies the given file in place of the build's and follows its includes. The build's own `Skin.hlsli` differs from be6afe5. `tree_b_lighting_compiles` compiles `Lighting.hlsl` with the override for seven permutations: PS and VS default; PS `TRUE_PBR`; the VS of a PBR shape (CS defines no `TRUE_PBR` for the vertex stage); and with `CS_SKIN` (the only define that includes `Skin/Skin.hlsli`) PS `TRUE_PBR`, PS and VS of a type-5 skinned shape. For the type-5 PS the test also shows that `CS_SKIN` changes the bytecode, so the Skin code is compiled; none failed, so the STOP rule (rebuild without the override) did not fire. `ShaderSource::BundledB` and `bundled::read_in(Tree, rel)` read it; `Bundled` still means tree A.

ARCHIVED (wording before the edit; superseded in part by Note IE5, 2026-11-16)
- **ID27 The PBR2Test1 mod ships no `SettingsDefault.json` (CS keeps its defaults in the DLL), so tree B's copy is tree A's file byte for byte.** `cs_settings::defaults_in(Tree::B)` reads that copy with tree B's feature list (ID29), so the tree-B preview draws CS 1.9's default settings until a CS 2.0 settings file is found.

ARCHIVED (superseded by Note IE5, 2026-11-16)
- **ID28 Tree B's closure differs from tree A's in which files exist, not only in content.** B lacks `Common/ViewMedium.hlsli`, `InverseSquareLighting/`, `NRD/NRDReblurSH.hlsli`, `PhysicalSky/` (2 files), `PseudoSunBounce/sunbounce.hlsli` and `ScreenSpacePointLightShadows/SSPLS.hlsli`, and adds `Common/LandscapeSeams.hlsli`, `DynamicCubemaps/CubemapCommon.hlsli` and `LightLimitFix/Attenuation.hlsli`. `tree_b_holds_only_its_closure` counts what the bundler wrote (54), not tree A's 58.

ARCHIVED (wording before the edit; superseded in part by Note IE5, 2026-11-16)
- **ID29 `cs_settings::defaults_in(tree)` reads that tree's own `SettingsDefault.json` and loads that tree's feature list.** `defaults()` is tree A's. Tree B loads `FEATURE_NAMES_B` (the 42 `Features/*.ini` stems of PBR2Test1, pinned by `feature_names_b_equal_the_pbr2_features_folder` on `SDC_CS_SHADERS_B`); the JSON is the same bytes (ID27), so the feature set is what differs: B has no VanillaFresnel, so its preview defines have no `VANILLA_FRESNEL`, and `off_in_preview` lists B's own features.

ARCHIVED (superseded by Note IE5, 2026-11-16)
- **ID30 `Renderer::set_tree(tree)` starts the shader state over and re-permutes every shape.** It swaps `source` and `sources`, and resets the old-LL probe, the CS settings (that tree's defaults, the non-settings defines kept), the composite, the prefilter, `compiled` and the queued jobs (the load epoch moves on), then `begin_all`, so old draws stay while programs compile. The program disk cache stays shared (content-addressed). The same tree does nothing; a test stand-in folder counts as tree A for `tree()` and is replaced by `set_tree`.

ARCHIVED (superseded by Note IE5, 2026-11-16)
- **ID31 The GUI follows `cs_pbr2` from `SdcApp`'s per-frame hand-over, not from `AppModel`.** The two `Viewport3d`s (Meshes and set preview) live in the app, so the same place that copies the setting into `MeshesModel::cs_pbr2` calls `Viewport3d::set_tree` on both. A running render thread gets `V3Cmd::SetTree` (counted as a reconfiguration, answered like `SetFeatures`); a thread not started yet starts on the tree's source. With the box off at startup nothing is sent and the source is `Bundled`, so tree A behaves as before. A test's stand-in folder (`use_shader_folder`) is never replaced by a tree change: `Viewport3d::set_tree` keeps the folder source and sends nothing. (`Renderer::set_tree` itself replaces a folder source with the bundled tree.)

ARCHIVED (wording before the edit; superseded in part by Note IE5, 2026-11-16)
- **ID32 A `Renderer` seeds its settings from its source's tree, and the render loop tells a failed deferred rebuild after a tree or features change.** `build` takes `defaults_in(source.tree())` (a folder: tree A), so a renderer made on `BundledB` (the box on at startup) matches one switched to B later: B's feature list, no `VANILLA_FRESNEL`. `set_tree` and `set_features_on` share `refresh_settings_defines` with `set_settings`. The render loop's `SetFeatures` and `SetTree` arms are one arm: after the change it retries the look's deferred choice and, if that fails with an error not told yet, sends the `Notice` once (`deferred_told`, as `SetDeferred`).

ARCHIVED (wording before the edit; superseded in part by Note IE5 and Note IE6, 2026-11-16)
- **ID33 `render3d` draws a material per tree through `material::drawn(m, tree)`, and the stored material stays the source.** Tree A draws a type-21 material as the v1 shape a conversion to type 11 (when `to_v1` gives MLP floats) or type 0 writes, `nif::pbr2::v1_body` (ID40), with its extra data dropped. Tree B draws a v1 shape (SLSF2 23, not type 21) with bit 23 cleared, as CS 2.0 hands it to the vanilla reader, so bits 25-27 are the vanilla soft, rim and back lighting and type 11 keeps its MLP technique. Every other material is drawn as it is, so tree A draws everything but type 21 as before. `pbr_mode(m, tree)` = `own_pbr(&drawn(m, tree))` (`Pbr::None`, `V1`, `V2`; type 21 wins over bit 23) and `permutation::Material.pbr` replaces the bit-23 test in `defines`. The renderer resolves `drawn` with its own tree at every permutation, cbuffer and texture site, so `set_tree` flips a shape between the two, and `apply_material` keeps a new source material even when the drawn change is `Same`.

ARCHIVED (superseded by Note IE7, 2026-11-16)
- **ID34 `render3d::parse` is one for both trees: it records a type-21 shape's first CS 2.0 Error and hides nothing.** It runs `nif::edit::validate::type_21_warnings` (the ID5 rules; the NIF is read with `read_model` once, only when it has a type-21 shape) and puts the first Error's text in `material.pbr2_error`; the shape is never hidden and gets no per-tree warning. What the verdict means per tree is the renderer's (ID38).

ARCHIVED (wording before the edit; superseded in part by Note IE6 and Note IE8, 2026-11-16)
- **ID35 A type-21 shape draws with the Default technique plus `TRUE_PBR` on tree B, and `GLINT` when its Glint feature is set.** Tree B's `Lighting.hlsl` has no type-21 define (its defines beyond tree A's are `LANDSCAPE_SEAMS`, `TERRAIN_BLENDING_OBJECTS` and `VSHADER`), so the technique is inferred: CS 2.0's C++, which picks it, is not in the repo.

ARCHIVED (wording before the edit; superseded in part by Note IE7, 2026-11-16)
- **ID36 `write_pbr2` fills tree B's PBR members from the 25 parameters and the extra data.** `PBRFlags` maps each feature to its `PBR::Flags` bit (Foliage also sets ThinSurface, as PBRMath.hlsli does), a 2-float PBRFuzz with `[1]` = 1 sets HasFuzzRoughnessTexture, and the Has-bits come from the bound registers as on v1. `PBRParams1` = (p0, p2, p1); `PBRParams2` = p7-p10 on two-layer, else p3-p6 on subsurface; `MultiLayerParallaxData` = (p11, p12, 0, 0) on two-layer, else p13-p16 on fuzz, else the glint row (ID37); `LightingEffectParams` = a 4-float PBRThinSurface, else p21-p24; `MaterialObjectRGBScale` = (1, 1, 1, fuzz roughness), where a 2-float PBRFuzz `[0]` is kept when negative (base roughness), else clamped to 0-1, and NaN or no PBRFuzz is -1. Per spec section 7 the extra data is ignored on two-layer and hair and when its count is wrong. Which feature writes a shared member, the clamps and the two-layer/hair gate of HasFuzzRoughnessTexture come from the survey and the spec, not from CS's C++ or an HLSL line; the Task 13 and 15 renders confirm them.

ARCHIVED (wording before the edit; superseded in part by Note IE7, 2026-11-16)
- **ID37 p18 is the glint log microfacet density as authored, the same number as v1's trailer[1], and only the shader packing takes 40 - it.** CS 1.x reads `logMicrofacetDensity = parameters[1]` straight from the v1 trailer (BSLightingShaderMaterialPBR.cpp:305) and sends `40 - logMicrofacetDensity` to the shader (TruePBR.cpp:992); spec section 4 has p18 as the density, default 40. So `from_v1` sets p18 = trailer[1], `to_v1` sets trailer[1] = p18 (ID7 and ID9 convert glint one to one), and `write_pbr2`'s glint row is (p17, 40 - p18, p19, p20), which tree B's shader takes 40 - y back from (Lighting.hlsl:2076). Tree A's stand-in of a glint shape packs the same row through `write_pbr`.

ARCHIVED (superseded by Note IE5 and Note IE7, 2026-11-16)
- **ID38 The renderer decides that tree B skips a type-21 shape CS 2.0 refuses, and `hidden` stays the user's.** The verdict travels on the material as the preview-only `MaterialState::pbr2_error` (never in the NIF, like `skin_preview`), so an edit that changes it reaches the renderer as a material change. `material::skipped(m, tree)` = `hidden`, or tree B and type 21 with `pbr2_error`; the renderer applies it at load, in `begin_all` (so `set_tree` hides or redraws such a shape) and in `apply_material`. A shape only tree B skips still has its slots resolved at load and on every edit (`resolve_skipped`), because `begin_all` has no resolver, so a switch back to A draws it as a load on A would. `material::tree_note` gives the per-tree note of a type-21 shape the user has not hidden: [`APPROXIMATE`] ("Approximate: tick CS PBR 2.0 to preview CS 2.0 shading") on tree A, the Error text on tree B. The notes are not in the `LoadReport`: `Renderer::tree_notes` lists them for the renderer's tree, and the GUI's 3D notes list the load's warnings then `Viewport3d::tree_notes` (from the shapes last reported and the tree in use), so a tree switch or an edit updates them. The GUI's debug views ask `availability_in` with that tree.

ARCHIVED (superseded by Note IE7, 2026-11-16)
- **ID39 A NIF that does not read as a model draws its type-21 shapes unchecked, with a warning.** When `read_model` fails, `parse` leaves `pbr2_error` None and warns "<shape>: CS 2.0 rules not checked: <error>", so tree B draws the shape. Hiding it would hide shapes CS may well draw over a reader gap of ours; the warning says the verdict is missing.

ARCHIVED (superseded by Note IE7, 2026-11-16)
- **ID40 `nif::pbr2::v1_body` builds the v1 shape of a type-21 shape for both the conversion and the preview.** `set_shader_type`'s conversion from 21 to type 0 or 11 writes it (ID11, ID14), and tree A's stand-in is `v1_body` at `stand_in_type` (11 when `to_v1` keeps trailer floats, else 0), so the preview draws what a conversion would write, down to the type-11 trailer's envmap strength from `trailer_defaults(11)`.

ARCHIVED (superseded by Note IE5, 2026-11-16)
- **ID41 `render3d/tests/tree_compare.rs` (`render_tree_b_vs_a_report`, ignored, needs `SDC_GOLDEN`) replays every subject golden.rs runs (profiles A, A_RT, A_RT2, A_DC, A_DC_N; skip rules shared as `golden::prepare`; the capture profile "B" and DBG are not in it) deferred on tree A and on tree B from the same capture. It writes one row per subject: B against A per MRT and on the composite (max, mean, over `diff_tol` with the format tolerance and the subject's noise limit), and for a TRUE_PBR subject a second B pass with TRUE_PBR and GLINT dropped and its own remap. Per group the summary lists the changed shader files the group's permutations pull (`D3DPreprocess` of the expanded source). The differences are rows (pre-flight ruling); the test fails when a subject does not draw, a render on either tree fails (a failed composite is rendered again without it so the row is still written, and still fails the test) or tree B's constants cannot be rebuilt.

ARCHIVED (superseded by Note IE5, 2026-11-16)
- **ID42 Tree B gets the capture's constants rebuilt by name, not its bytes.** `remap` copies each leaf variable (struct members by `struct.member`) from tree A's reflected offset to tree B's, so the reordered FeatureData reads the captured settings; a tree B leaf with no tree A twin reads 0, and `MaterialObjectRGBScale.w` (float3 on A, float4 on B) gets -1, "no fuzz roughness". Tree B's objects read the scene depth at t17 for Terrain Blending, which no CS 1.9 capture bound (unbound reads 0, so every pixel counts as buried and its normal bends up), so the B pass turns `terrainBlendingSettings.Enabled` off when the fixture has no t17.

ARCHIVED (wording before the edit; superseded in part by Note IE5 and Note IE10, 2026-11-16)
- **ID44 The 8 CS 2.0 texture-set keys (`cs_pbr::TXST2_FIELDS`) are edited and exported only with the "CS PBR 2.0 (experimental)" box on; off, the CS PBR export is byte for byte main's (`box_off_export_is_byte_identical`).** No local copy of `PBRTextureSetData` at 92af7dd exists, so the types, ranges and defaults are the plan's; the .csm schema (`PBRThinSurface*`, `PBRFuzzRoughness` max 1 and no minimum, `PBRFuzzRoughnessTexture` a bool) and spec sections 4 and 7 agree with them. `block_of` reads the 8 keys whatever the box says (the sets document has no settings), and every reader filters with `field(kind, name, pbr2)`: off, they are unknown keys, so no "wrong type" note or label; templates (CS 1.x values) never touch them. Ticking a bool row writes `false`, CS's default.

ARCHIVED (superseded by Note IE13, 2026-11-16)
- **ID45 The set preview gives a type-21 shape, with the box on, its payload, then the set's texture set (`txst_overrides`), then the PBRThinSurface / PBRFuzz extra data (`set_preview::apply_txst21`).** Only the keys the block holds change the payload (CS 1.x's `ApplyTextureSetData` wrote every field, missing ones at their defaults; the 2.0 code is not local). `glintParameters.enabled`, `thinSurface` and `foliage` switch their feature bit; `innerLayerDisplacementOffset` has no type-21 field; `fuzzRoughness` / `fuzzRoughnessTexture` have no payload field, so they become a PBRFuzz when the NIF has no 2-float one. A value outside its parameter's range is passed through unclamped, and a PBRThinSurface applies by `nif::pbr2::thin_surface_used`, the rule render3d's `write_pbr2` uses. The path is unreachable while a set with a texture-set block is plane-locked (Note BM, R1); the .csm rework owns unlocking it.

ARCHIVED (wording before the edit; superseded in part by Note IE16 and Note IE18, 2026-11-16)
- **ID46 The `fuzzRoughnessTexture` row's hover says the fuzz roughness comes from the parallax slot's green channel, so that texture is two-channel and should be BC5 or BC1, not BC4 (owner 2026-11-14; tree B Lighting.hlsl `HasFuzzRoughnessTexture`).** The engine presets are unchanged on this branch; that belongs to the .csm rework.

ARCHIVED (superseded by Note IE7, 2026-11-16)
- **ID48 The Task 15 type-21 golden subjects are made by `nif/tests/pbr2_subjects.rs` (`write_pbr2_subjects`, ignored, needs `SDC_PBR2_SUBJECTS_MANIFEST` and `SDC_PBR2_SUBJECTS_OUT`): every v1 PBR shader of a v1 mesh goes to 21 by `SetField ShaderType`, the manifest's feature bits (replace or `+` add) and parameters follow, and PBRFuzz / PBRThinSurface are appended to the shader's extra-data list by hand, since no edit op adds NiFloatsExtraData there.** A subject with any Error or an `always_blocks` warning after the edits is refused; every subject is converted and every destination checked absent before the first write, so a refusal or an existing file writes nothing, and files are created new, never replaced. The sources are v1 meshes on forms outside the A groups, so the A goldens keep their v1 meshes in the same run; there is no v1 PBR hair mesh in the load order, so hair is the HAIR bit alone on a plain v1 mesh, and fuzz uses the scalar PBRFuzz roughness because the rug's slot-3 texture is BC4.

ARCHIVED (superseded by Note IE5, 2026-11-16)
- **ID49 A golden root replays on the tree its captures were drawn with: `golden::tree_of(root)` is tree B only for a folder named exactly in `TREE_B_FIXTURES` (`cs-1.9.0-d2026.11.14`, the golden-capture cs-version key of the PBR2Test1 captures, which replaced the brief's `pbr2test1`), and tree A for any other folder.** `replay_matches_capture_per_mrt` sets that tree and checks `golden::baseline_file(tree)` (`golden-baseline.txt` for A, `golden-baseline-b.txt` for B) with the root's own noise floor (`golden::noise_path`); `extraction_matches_capture` and `trace_values_match_unit_functions` draw and reflect on that tree, and extraction has a subject list per tree. Tree A's baseline, fixtures and replay are unchanged (the A run's subject list is byte for byte the baseline).

ARCHIVED (wording before the edit; superseded in part by Note IE5, 2026-11-16)
- **ID50 Tree B's noise floor is its own, measured as tree A's was: `golden_noise.py --animated=Core01` on the two A1 captures (frame67390, frame67391), written beside the captures.** Its static limits (2x the measured max: RT0 0.019, RT4 0.0127, RT6 0.00098, MainRW 0.024, the other RTs 0) are 2-6 times tighter than tree A's file, which is not reused. The A1 pair differs by a flickering cell fire light, so this is the floor for lit frames.

ARCHIVED (wording before the edit; superseded in part by Note IE5, 2026-11-16)
- **ID51 `golden-baseline-b.txt` holds all 394 subjects of the 14 PBR2Test1 captures: 137 RUN (3 of them no-clean-composite), 205 empty-mask, 45 overlay-mask, 4 blended and 3 two-sided-alpha-tested; 0 over the limit; the largest RT difference is 0.002 (RT6) and the clean composite's 6.1e-5.** The P0 probe capture (frame51691) is profile A and kept as extracted: its 322 draws give 74 RUN, 205 empty-mask and 43 overlay-mask. All 20 type-21 draws (8 subjects in T1, T1L, T2 and T2L) are RUN and bit-identical to the capture on every RT and on the clean composite (the full-mask composite, printed only, differs on two T1L draws through overdraw), so no new outcome code was needed; the hair-only CookingStone01 changes no RT4, RT5 or composite pixel, as the game drew it.

ARCHIVED (wording before the edit; superseded in part by Note IE7 and Note IE8, 2026-11-16)
- **ID53 `extraction_matches_capture` on tree B: sdc's own type-21 material gives the capture's pixels exactly (every RT and the composite) for every type-21 subject it can swap: 7 of the 8 feature groups, glint aside (T1 base, subsurface, two-layer, fuzz; T2 hair, thin surface, foliage; T2L hair).** t17 (the scene depth tree B's Terrain Blending reads) counts as an engine texture, so the swap keeps it. Fields CS 2.0 writes that sdc does not (PBRParams2 (1, 1, 1, 1) and MultiLayerParallaxData defaults without the feature, foliage without the ThinSurface bit 0x4000, which the shader implies) change no pixel there. The glint altar is not swapped: the capture's decoded defines add ANISO_LIGHTING beside GLINT and sdc's do not (ID35). The v1 and vanilla subjects on tree B differ in defines (sdc adds SPECULAR, and SOFT_LIGHTING and BACK_LIGHTING on the type-11 crystal, which the capture lacks); these are recorded, not changed here. The capture's defines are decoded from the PSO descriptor with the CS 1.9 table, and the descriptor itself differs: the coffin is 0x211 in the CS 1.9 capture and 0x11 in the PBR2Test1 one, so the game's own pipeline state dropped the SPECULAR bit.

ARCHIVED (superseded by Note IE5, 2026-11-16)
- **ID54 Note GT is amended in place for tree B: its opening line names both pinned trees, GT1 names `shaders/cs2` as the second one (with its one overridden file), GT2 adds `ShaderSource::BundledB`, the second embedded table and the release build reading either tree, and GT4 says the preview compiles tree B with `defaults_in(Tree::B)` while the box is on and loads 42 features there.** GT5's doc pointer moves from the removed `GUI-Set-Preview-3D.md` to `Previews.md`, which states the preview's two builds once. The old wording of the opening line, GT1, GT2, GT4 and GT5 is in the archive under Note GT. GT3 and GT6 stand as written: `defaults()` is still tree A's, and tree B carries the same licence rows (`THIRD_PARTY_NOTICES.md`).

ARCHIVED (superseded by Note IE5 and Note IE19, 2026-11-16)
- **ID55 This branch is pushed, not merged (owner, 2026-11-14).** A later branch reworks type-21 support around CS's material files (.csm JSON). When CS 2.0 ships, its shaders replace tree A as the one pinned tree and the "CS PBR 2.0 (experimental)" box goes, under a new ruling: the owner has ruled that the shipped version offers one option, with no checkbox.

ARCHIVED (wording before the edit; superseded in part by Note IE8 and Note IE17, 2026-11-16)
- **ID56 Open follow-ups from the Task 15 captures, for the .csm rework.** Tree B compiles SPECULAR, SOFT_LIGHTING and BACK_LIGHTING on v1 and vanilla shapes where the PBR2Test1 captures have none (the coffin's PSO descriptor is 0x211 in the CS 1.9 capture, 0x11 in PBR2Test1), so the wiki tells users to judge those shapes with the box off. The glint altar's capture adds ANISO_LIGHTING beside GLINT and sdc's defines do not; whether SLSF2 bit 21 survives the conversion to 21 is not checked. The hair golden (CookingStone01) has 0 evidence pixels on RT4, RT5 and the composite, and the glint subject is never swapped in extraction, so neither hair nor glint shading is proven against the game. `extraction_matches_capture` reports rows and asserts none, so a `write_pbr2` regression on the 7 matched groups would not fail a test.

ARCHIVED (wording before the edit; superseded in part by Note IE5, 2026-11-16)
- **ID43 Tree B's HLSL still reads `MaterialObjectRGBScale.xyz` as the MATO base colour scale (cs2 Lighting.hlsl:1937 with LL, 1941 without, under TRUE_PBR) and `.w` only as the fuzz roughness (2138), so `write_projected` writing three floats over `write_pbr2`'s (1, 1, 1, w) is right.** The HasFuzzRoughnessTexture two-layer/hair gate of ID36 is not in the HLSL (2145 tests only Fuzz) and cannot change a pixel: hair skips fuzz (PBR.hlsli:318, 487) and on two-layer `MultiLayerParallaxData.w`, the fuzz weight, is 0. The w clamp matches CreateFuzzLayer (PBR.hlsli:132, 0.01-1, negative = base roughness), except that a NaN sent raw would take the energy-conserving throughput (137) where our -1 takes the legacy one.

ARCHIVED (wording before the edit; superseded in part by Note IE5, 2026-11-16)
- **ID47 The PBR Landscape card hides the 8 CS 2.0 rows with the other landscape-unread rows (Note BM) while the PGPatcher box is off and the row holds no value.** Tree B's landscape PBR material reads per layer only the PBR params and glint (Lighting.hlsl `PerMaterial` under `TRUE_PBR && LANDSCAPE`); the fuzz code is not compiled for landscape.

ARCHIVED (wording before the edit; superseded in part by Note IE5, 2026-11-16)
- **ID52 CookingStone01's PBRParams2 is `(64, 0, 0, 0)` as integers in T2 and 0 in T2L in the capture itself; sdc writes 0 in both, because `write_pbr2` fills PBRParams2 only for two-layer or subsurface (ID36).** The value is inert: tree B reads PBRParams2 only under Subsurface, TwoLayer and InterlayerParallax (Lighting.hlsl 1232/1306, 2094, 2112), and hair (PBRFlags 0x402) has none of them; both frames replay bit-identically from the capture's bytes, and with sdc's material swapped in.

ARCHIVED (superseded by Note IE29, 2026-11-16)
- **ID5 A CS 2.0 Error starts "CS 2.0 won't draw this shape: " (`validate::WONT_DRAW`) and always blocks export: `validate::always_blocks` (used by `new_errors`) is true for an `ALWAYS_BLOCK` text or any text with that prefix.** The one such Error is type 22 (`TYPE_22`, "type 22 is runtime only"), raised whether or not a shape uses the shader.

ARCHIVED (wording before the edit; superseded in part by Note IE30, 2026-11-16)
- **ID36 `write_pbr2` fills the CS 2.0 PBR members from the 25 parameters.** `PBRFlags` maps each feature to its `PBR::Flags` bit (Foliage also sets ThinSurface, as PBRMath.hlsli does), `FUZZ_ROUGHNESS_TEXTURE` sets HasFuzzRoughnessTexture (IE6), and the Has-bits come from the bound registers as on v1. `PBRParams1` = (p0, p2, p1); `PBRParams2` = p7-p10 on two-layer, else p3-p6 on subsurface; `MultiLayerParallaxData` = (p11, p12, 0, 0) on two-layer, else p13-p16 on fuzz, else the glint row (ID37); `LightingEffectParams` = p21-p24; `MaterialObjectRGBScale` = (1, 1, 1, fuzz roughness), where -1 is the base roughness (IE3). Which feature writes a shared member comes from the survey and the spec, not from CS's C++ or an HLSL line; the extraction confirms it against the captures (ID53).

ARCHIVED (wording before the edit; superseded in part by Note IE30, 2026-11-16)
- **ID7 `nif::pbr2::from_v1` (v1 to v2).** It follows `render3d::pbr_flags` (SLSF2 24, else 27, else 26 with 25 or 12) and needs SLSF2 23, else it gives no features. Only a type 11 shape with a MultiLayer trailer fills the coat, fuzz and glint rows; any other shape keeps their defaults.

## Note IE — CS PBR 2.0 .csm materials (NIF team, 2026-11-15)

ARCHIVED (wording before the edit; superseded in part by Note IE13, 2026-11-16)
- **IE12 The Meshes view draws a shape's `.csm`: the render thread looks a `materials/…` or `textures/…` path its maps lack up in the scan (`MeshSpec.scan_db`, `mat_winner` and `tex_winner`, loose before BSA) when the renderer asks, so a `.csm`, its roots and the textures it names need no pre-pass; a `.csm` that does not resolve is an amber Problems row, "Material file materials\a.csm not found; drawn vanilla" or "Material file <path>: <error>; drawn vanilla".** Each render thread keeps one read-only connection to the scan database: opened on the first path that needs it, kept for the thread's life, and opened again when the path or the scan revision changes or after a query failed (`MeshSpec.scan_db` is a `ScanDb`, the path and `MeshesModel`'s completed-scan count, so a rescan also reloads the view). A path the maps lack costs one indexed query per load. The database is in WAL mode (the scan's read-write open), so the idle connection holds no lock a scan waits for; a test rescans with it open. The rows show in the Meshes view and its edit preview, not in the set preview, which reads no scan. The design's TXST-JSON rule: a picked texture set whose EDID (as typed, as CS matches it) a project set's `cs_txst` block binds draws that set's in-memory `.csm` (built as the export builds it, `csm_export::csm_sets`: `entry_csm` from the set's PGPatcher entry, so borrowed slots count; unsaved edits included) in place of the name field, served from `MeshSpec.materials` ahead of the scan's copy of the same path (`csm_export::memory_source`, the source id a hash of the text, so an edit to the set reloads the view and an unchanged one does not); an EDID the export refuses (two sets bind it, or a bad file name) binds no set, a set that gets no `.csm` leaves the name field in force, and so does a blocked set (a needed texture missing, a schema rule broken, or no PGPatcher entry: the export then writes no texture-set file either, so the game keeps the name field), with its reason as a Problems row ending "the shape keeps the material its NIF names" (fix round 1 ruling). The app hands the sets' `.csm` over each frame (`MeshesModel::sync_txst_csms`) but builds them only when the sets' view key, the scan evidence revision or the shown mesh's TXST EDIDs change (Note FI18's rule: an idle frame builds nothing). The set's `.csm` names the project's output textures, which the scan has only once they are exported into a scanned mod; until then they are missing textures (magenta). `PBRTextureSets/*.json` files in other mods are not read (the scan does not index them). The Meshes view's edit preview (`Content::Parsed`, its `scan_db`) reads the scan the same way; the set preview passes none.

ARCHIVED (wording before the edit; superseded in part by Note IE28, 2026-11-16)
- **IE3 `pbr2::pbr_values` and `csm_from_values` bridge the `.csm` PBR keys to `PbrValues` (features, the 25 params with p21-p24 for thin surface, fuzz roughness -1 = base roughness); `PBRFoliageEnabled` is `feat::FOLIAGE`, and `PBRFuzzRoughnessTexture` is `feat::FUZZ_ROUGHNESS_TEXTURE` (1024), not a payload bit, so payload code masks with `feat::PAYLOAD_FEATURES`.** A written file holds the switches that are on, p0-p2 and each enabled feature's params (`clamp_param`), and with fuzz only `PBRFuzzRoughness` (`clamp_fuzz_roughness`, -1 to 1) and the roughness-texture switch. The TXST JSON key map is `pbr2::TXST_PARAMS`.

ARCHIVED (wording before the edit; superseded in part by Note IE27 and Note IE30, 2026-11-16)
- **IE6 A shape whose BSLightingShaderProperty name is a materials `.csm` path (`MaterialState.csm_path`, `path::canon_material`) takes its PBR material from that file in the preview: the renderer resolves it through the load's resolver (`csm::resolve`, cached per load and applied again to every edit and animation tick), sets `MaterialState.pbr` from `pbr_values` and replaces each slot a texture key names (`textures\<value>`; an empty value clears the slot) before the textures are resolved; the generic keys are not applied, so the NIF's values stand.** A file that does not resolve leaves the NIF's material (vanilla), sets `csm_error` and adds a load warning naming the file, and the next edit asks again for a file the resolver lacked; without `PBREnabled` a resolved file draws vanilla, and an absent key takes the spec default. `is_pbr2` means "has PBR values", drawn through `write_pbr2` with `FUZZ_ROUGHNESS_TEXTURE` setting `HasFuzzRoughnessTexture`, and a CS 2.0 PBR material counts as PBR for the MATO layer rule (`projected_eligible`), whatever its shader type.

ARCHIVED (wording before the edit; superseded in part by Note IE33 and Note IE34, 2026-11-16)
- **IE13 The set preview draws the followed set's in-memory `.csm` (`csm_export::view_csm`: built as the export builds it, unsaved edits included) on every shape pgpsim patches with the set's entry (the shapes it finds by the set's match key), through the 3D view's resolver: the shape's `csm_path` names the set's `materials/…` key and the scene's file map serves the `.csm` text and the game's copy of any texture only the `.csm` names (the set's outputs are its encodes); a PBR set with a texture-set block previews on its meshes, and only a PBR Landscape set is locked to the plane.** The `.csm` is rebuilt only when the project's view key (which holds the target), the scan evidence revision or the followed set changes, and a changed one is a new load (render3d reads a `.csm` once per load); a change to its warnings alone loads nothing. The set editor's warning area (its head, in 2D and 3D, and the 3D header of a selected member) shows, for every set that gets a `.csm`, "PGPatcher 2.3.1 writes v1 PBR, which CS 2.0 draws vanilla. A PGPatcher with .csm support is needed for these meshes in game."; a blocked set, or one with no PGPatcher entry, draws no `.csm`, and its reason ends "the shapes draw as PGPatcher 2.3.1 patches them (vanilla in CS 2.0)". "PGPatcher preview needs the CS target and a PBR set" shows only for a set that gets no `.csm`. A scanned mesh's own `.csm`, roots and textures are read through the scan (`Content::Parsed.scan_db`, its revision bumped by each mesh query; a `shortcut:` until it follows the completed-scan count), so the `.csm` rows of IE12 show here too. This session's encodes go into the scene's map last, so they win over an installed export of the set's outputs. A shape PGPatcher skips or does not match keeps its own material, `.csm` included. A PBR Landscape set's plane draws a preview-only `.csm` (`ViewCsm.preview_only`), built as a set's `.csm` from its PGPatcher entry and its texture-set block, the values its inline texture-set JSON holds; it is never exported and has no editor line, and without it the plane would draw PGPatcher's v1 output, vanilla since IE5. Supersedes ID45 and 1.x AD9's texture-set-block plane lock.

ARCHIVED (wording before the edit; superseded in part by Note IE38, 2026-11-16)
- **IE14 The NIF editor points a shape at its project set's `.csm` (`Op::SetMaterial`, "Use the set's .csm") and converts a v1 PBR shape (type 0 or 11 with SLSF2 23, `edit::is_v1`) in one atomic `Op::ConvertV1` (SLSF2 12, 23-27 and 30 cleared, type 0, slots kept, name field the `.csm` path); no edit writes v1 PBR any more: `apply` refuses (`V1_GONE`) a `SetField` or `SetBits` that turns bit 23 on and a shader type change that makes a shape with bit 23 v1, so an old saved edit that did reapplies as a refused row that blocks export.** The name field is replaced only when it is empty, a `.csm` path, or the user agreed in a dialog naming it (the op's `base` is that name: a renamed source is a Conflict, and Keep mine is the user's word on the name its row shows); off Community Shaders both actions are off ("A .csm is a Community Shaders file…", checked first), and the project set is the set that gets a `.csm` (`gets_csm`) whose diffuse is the shape's slot 1, else "Make a texture set for these textures first.". Convert copies only the `cs_txst`-owned values (`csm_actions::shape_block`) into a set that has none, asks nothing when they match at `.csm` precision, and otherwise shows the differences with "Use the set's values" (Enter) and "Replace the set's values" (red, never Enter, off while the sets cannot be edited or when the switches would block the set's `.csm`); the set is written first (the sets history), then the op (the NIF history), so the two undo separately. Supersedes ID13 (v1 PBR now means this for Convert to .csm).

ARCHIVED (wording before the edit; superseded in part by Note IE10, 2026-11-16)
- **IE15 The `.csm` warnings and preview of the NIF editor: a v1 shape's "Draws vanilla on CS 2.0" Warn carries the Convert to .csm button, and a shader controller on LSCF 9, 10 or LSCC 0 (the variables `var_label` renames on PBR) of a shape whose loaded material is CS 2.0 PBR gets "CS 2.0 may not animate values a .csm sets" (design §7), rebuilt only when the edit or the loaded shapes change.** The edit preview reads the project set's in-memory `.csm` for a shape that names it. A set that gets a `.csm` but has no PGPatcher entry is still a target: the path is the one its `.csm` gets once it has an entry, and its set editor's NO_ENTRY line (IE13) and the export's rows (IE11) already say no `.csm` is written this run.

ARCHIVED (wording before the edit; superseded in part by Note IE26 and Note IE29, 2026-11-16)
- **IE22 The rest of the design's §7 questions (IE20).** Whether CS 2.0 animates values a `.csm` sets is `csm_actions::CSM_CTRL`'s warning (IE15); whether its landscape path reads a `csm` key is `csm_export::gets_csm`, which leaves PBR Landscape inline (IE10); whether it still reads SLSF2 bit 23 is `material::drawn` and `validate::V1_VANILLA` (v1 draws vanilla, IE5); the final `$schema` URL is `nif::csm::SCHEMA_URL`, empty, so no `$schema` is written (IE1). Two owner questions ride with them: whether CS accepts our `PBRFoliageEnabled` key (IE1), and whether CS reads a `.csm` on a type-6 hair shape (IE17).

ARCHIVED (wording before the edit; superseded in part by Note IE24, 2026-11-16)
- **IE24 The cs2 shaders sample the projected normal and detail when `ProjectedUVParams3.w` > 0.5 (Lighting.hlsl:1920-1925), and `write_projected` writes w = 1, so the 3D preview draws them; GI16's result (dead code, X16) was measured on CS 1.9, and whether CS 2.0 draws them in game is not captured.** The Detail normal slot's hover (`model::sets`, "Community Shaders doesn't draw the projected detail normal in this build; it shows without CS.") still states the CS 1.9 result and is left to the final wave's code changes. The slots still export.
