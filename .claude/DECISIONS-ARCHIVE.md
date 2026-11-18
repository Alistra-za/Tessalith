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

ARCHIVED (wording before the edit; narrowed by Note JA, 2026-11-14)
  - **API.** `encoders_sys::init()` is infallible and idempotent (`Once`) and selects Dual; `InitError` is gone. `Bc1Model` is `{ Nvidia = 1, Amd = 2 }`, decode models only. `Session::new()` and `BatchRequest` take no model; `decode_level` and `decode_level_rgba` take none. `engine::manifest::BC1_ENCODE_ID` (`"dual"`) is written to `ManifestRecord.bc1_model` for every record and `is_up_to_date` compares it for BC1 and BC3 outputs, so a 1.x record (a decoder model's name) is stale once. `versions().rgbcx_patch` is `bc4-round-to-nearest+bc4-search+bc1-dual`; BC1, BC3, BC4 and BC5 compare it, so their records go stale once too (BC4 and BC5 bytes do not change).

ARCHIVED (wording before the edit; narrowed by Note JA, 2026-11-14)
- **FM19 The BC3 alpha guard follows each texture's alpha-test cutoff (plan task 8, 2026-11-02).** The guard that keeps the BC4-style alpha search from flipping texels across the alpha test used the constant 128. It now uses the cutoff of the texture's resolved `alpha` settings, `engine::encode::alpha_cut`: `gt` t is t + 1 and `ge` t is t, so a texel passes iff alpha >= cutoff (threshold is 1..=254, so the cutoff is 1..=255). The value is already the per-mesh scan result when a mesh decides it (`mesh_alpha_policy`, provenance `mesh:alpha`), else the preset, `sdc.toml` or `--set` value, so the CLI needs no extra input and keeps 127/`gt` unless set. The default (127, `gt`) gives 128, so default output is byte-identical (`identity.json` is unchanged; `cov/textures/c/cutout_bc3_t200` is a new key at `alpha.threshold = 200`). The cutoff reaches the C++ as bits 8..15 of the `mode_flag` (0 means 128); `bc3_encode_block` takes it, and the `rgbcx_patch` tag gains `+bc4-guard-cut`, so BC1, BC3, BC4 and BC5 records go stale once. BC4 and BC5 have no alpha test and take no guard.

ARCHIVED (wording before the edit; narrowed by Note JA, 2026-11-14)
  - **The re-encode report is gone (supersedes FC7 in part; FM9's notice).** `manifest::recode_causes`, `RecodeCause`, `RecodeWhy`, `sdc convert`'s stderr lines, `PlanView.recode`, `AppModel.recode`, Review's Warning rows and the "First export" line are deleted, with `crates/engine/tests/recode_causes.rs`. The behaviour is unchanged: `IfChanged` still re-encodes a record whose encode version, profile hash or bc1 id is stale, and a 1.x record is a known, stale baseline (`a_1x_manifest_record_is_known_and_stale`). FC7's `sdc validate` cube-warning and its CN3/CN4 bullets stay.

ARCHIVED (wording before the edit; narrowed by Note JA, 2026-11-14)
  - **Presets.** `vanilla-compact` sets `encoder.perceptual = true` in its two BC1 colour `[[rules]]`, not in `[defaults]` (data types would hit `FMT-PERCEPTUAL-DATA`); Note FM43 adds the `diffuse`, `glow` and `skin_albedo` tables of `vanilla-quality` (inherited by `vanilla-compact`), so those are perceptual in BC3 and BC7 too. `cs-compact` and the other presets stay off. The preset version stays "2.0.0" (2.0 is unreleased); `docs/presets-changelog.md` has a line in its 2.0.0 entry. The `rgbcx_patch` tag gains `+bc1-perceptual`, so a 2.0 manifest from before this change re-encodes its BC1 to BC5 outputs once.

ARCHIVED (wording before the edit; narrowed by Note JA, 2026-11-14)
  - **Pins.** The arms are all "2.0.0" and `RELEASED_ENGINE_VERSION` is already "2.0.0", so `bumped()` accepts nothing this cycle; the stale keys were deleted and re-pinned by hand. `profile_hashes.json`: `vanilla-quality/*` and `vanilla-compact/*` for `cs` and `vanilla`, `diffuse`, `glow`, `skin_albedo` (12 keys); no BC6H key (the pin does not run resolve). `encode_inputs.json`: `textures/h/hdr_sky.dds` (effort) and `vanilla/textures/{h/diffuse,h/glow,pbr/rock,rnm/iron}.dds`. `identity.json`: those four vanilla files; `textures/h/hdr_sky.dds` keeps its bytes (the 64 px fixture encodes the same in both BC6H modes), so no BC6H identity key changed. Version stays 2.0.0. `if-changed` skips a file only when both the recorded `encode_version` arm and the SHA-256 of the resolved job profile match (`api::is_up_to_date`). The arms do not move, so what re-encodes is a file whose profile hash moved: a BC6H output (effort) or a vanilla diffuse, glow or skin albedo (perceptual) written by an earlier 2.0 build. A 1.x output has no `encode_version` and re-encodes on its first 2.0 run whatever this note does.

ARCHIVED (wording before the edit; superseded in part by Note JI7, 2026-11-15)
- **FM16 The `validation.*` thresholds are constants; project-wide fills are gone (plan task 6, controller rulings P10 and P13, 2026-11-02).** `ValidationSpec` keeps only `mips`; `Profile.validation` is `#[serde(default)]`, so `sdc-base.toml` no longer has a `validation` line. The other five fields are `pub const`s in `engine::validate`, with the values every preset set: `MIN_MIP_PSNR_DB` 30.0, `COVERAGE_TOLERANCE_PTS` 2.0, `COMPLEX_M_WARN_ALPHA` 247, `BC6H_MIN_PEAK_RATIO` 0.8, `PGPATCHER_NON_WHITE_SHARE` 0.5. `validate()` reads them directly; nothing else read the fields. They leave the `--set` field list. No encoded byte or validation verdict changes: `identity.json` is untouched, while `profile_hashes.json` changes because the profile lost five fields.
  - **Removed names.** `migrate::REMOVED` gains the five `validation.<key>` rows (any value; the hint gives the fixed value) and `defaults.packing.fill` (matches only under `[defaults]`, so a per-file, per-sub-type or `--set packing.fill.*` fill stays legal). The hint tells the user to delete the `[defaults]` line by hand and set fills per set, because a file the engine refuses opens read-only in the GUI and the Clear button below is gone.
  - **No engine fallback to delete.** `packing.fill.*` is an ordinary `Profile` field, merged like any other layer; nothing in the engine read `[defaults] packing.fill` apart from that merge. The removal is the `REMOVED` row alone.
  - **GUI.** The Advanced drawer loses the "Project-wide fills (legacy)" line and its Clear button (`legacy_fills`, `clear_legacy_fills`, `ClearLegacyFills`); `ProjectDoc::fill`, `set_fill` and their table helpers are deleted. No empty gap is left: the line was drawn only while a legacy fill existed.
  - Supersedes: R-fills-2 (in part: the legacy-line sentence). T5 and Y6 describe `validation.mips`, which stays; no ruling named the five thresholds as settings.

ARCHIVED (wording before the edit; superseded in part by Note JI1 and Note JI7, 2026-11-15)
- **FM25 The GUI settings sweep: worker threads, memory budget, preview size, Reduce motion and the Timings panel are out (plan task 14, 2026-11-03).**
  - **Batch and Interface.** The Advanced drawer's Batch section (Worker threads, Memory budget) and Interface section (Reduce motion) are gone, and its Preview section holds only Live encode. The GUI runs the engine with its defaults: one thread per core and half of the RAM (`BatchRequest.jobs = 0`, `mem_budget = 0`). The CLI flags are unchanged, and the drawer's copyable CLI command no longer carries `--jobs`.
  - **Preview size.** The preview-size combo and the 16384 tick are gone. The cap is `model::preview::PREVIEW_MAX` = 4096 on the largest side, lowered to the GPU's limit (`preview_limit`).
  - **Motion.** `ui/motion.rs` and every call site are gone. Toasts time out with no fade or rise. The Export button and dot take the accent at once. Progress bars show their value as it is. The reorder flash, the slot flash, the thumbnail press scale and the columns' fade-in on opening a folder are gone, and a scroll to a revealed set lands at once. egui's stock motion (dialog fades, collapsing headers) is left alone, and keyboard-opened overlays still open without a fade (CF3). Nothing here adds an animation.
  - **Timings panel.** The Set 3D view's Timings panel and the query, NIF and push times behind it (`model::set_preview`) are gone, and `main` no longer calls `render3d::perf_log::start()`. `render3d` is untouched.
  - **An old `gui.toml`.** The keys `jobs`, `mem_budget_mb`, `bc1_decoder`, `nvtt_path`, `reduce_motion`, `allow_16k_preview` and `preview_max` load without a message and are left out of the next save. They have no row in `REMOVED`, which is for project and preset keys.
  - **Tests.** `settings::a_1x_gui_toml_still_loads`. `advanced::advanced_has_no_batch_or_preview_size_rows` lists every label the drawer draws for one fixture (ruling P17), so a returning row fails it. `messages`: `a_new_toast_appears_whole_without_rising`, `a_toast_stays_whole_until_its_time_is_up`, `a_resting_toast_asks_for_no_frames_until_its_time_is_up`, `a_hovered_toast_asks_for_no_short_timer`. `set_order::a_reveal_lands_at_once`. `ui_smoke::opening_a_folder_shows_the_columns_at_full_opacity_at_once`. `review_tab::export_is_the_accent_once_ready_and_plain_when_not`.
  - Supersedes: CF1, CF2, CF11 to CF16 and CF18 (whole). In part: CF3, CF5, CF7, CF10 and CF19 (the motion wording and tests), N1 (the setting), AG112 (the fade), AG132 (Timings in Details), BS5 and BS8 (the flash), CL15 (after Interface), CL31 (Check all's threads and glide), FC9 (the slot flash), and the #19 row of Answers.

ARCHIVED (wording before the edit; superseded in part by Note JI1, 2026-11-15)
- **FM26 The preview loses Pin, Compare and the Layout setting; the Value range and Plastic look checks are out (plan task 15, controller rulings P17 and P18, 2026-11-03).** No encoded byte changes (`encode_versions` leaves `identity.json` alone).
  - **Preview.** "Pin this result", Unpin, the Compare toggle and its captions, the `Pinned:` readout lines, the pinned heatmap, the `P` key (`Action::Pin`), `model::preview::pin`, `Slot::Pinned` and the `app.rs` line that cleared it are gone. So are the Split layout, the Layout choice (the "Side by side" button was the other half of it, so it went with Split) and the `preview_layout` setting: the preview always shows the source beside the encode. The toolbar's exact control set is pinned by `preview_model::preview_offers_side_by_side_heatmap_lens_tile_and_exposure_only`, and `ui_smoke`'s `TOOLBAR_ORDER` has 20 entries.
  - **Checks.** `CHK-RANGE` and `CHK-PLASTIC` are removed with `RangeRule`, the Range overlay (the preview's "Range" toggle), dark metal and every threshold behind them. `ChecksSettings` is `{low_detail, green, parallax}`, so the Advanced Checks section has three boxes. The Z sign stays with Green direction, and a flat `_p` still raises `CHK-PARALLAX`. `CheckStats` lose the luma counts and the `low` and `high` bands, and `CodeHist` is four histograms.
  - **Names and old files.** `Code::parse` returns `None` for the two codes and `Code::ALL` has 93 (Note FM30 added `PACK-SOURCE-RESIZED`). `migrate::REMOVED` has an `allow` row for each, so a list naming one is an error that says "removed in 2.0" (`removed_check_codes_in_allow_lists_are_named`). An old `gui.toml` with `[checks] range`, `plastic` or `preview_layout` loads, ignores the keys and drops them on the next save (`removed_check_keys_in_gui_toml_are_ignored`, `window_layout`).
  - **Analysis cache.** `ANALYSIS_VERSION` stays 2 (2.0 is unreleased). An entry written with the removed fields still loads: serde ignores them (`check_stats_written_with_the_removed_counts_still_read`). `analysis_version.json` is re-pinned at the same version (its sha only).
  - **Tests.** Check sources that used the plastic `_n` now use a flat `_p` (`CHK-PARALLAX`) or `support::give_a_check` (a blue-0 `rock01_normal.png`, `CHK-Z-SIGN`). `preview_pin.rs` is deleted. `preview_model::no_toolbar_control_is_clipped_at_narrow_widths` pins that no toolbar control runs past the panel; the mip slider takes what is left of its row (40 to 100 pt).
  - Supersedes, whole: AG8, AG64 to AG71, CL9, CL13, CL37, CL38, CE18, FC13's Range overlay bullet. In part: AG14, AG17, AG21, AG54, AG57, AQ1, AQ3, CE13, CE14, CL1, CL2, CL4, CL5, CL7, CL8, CL10, CL15, CL16, CL28, CL29, CL31, CL42, CL44, DT17, CO74, GE16 and FC13's tests bullet.

ARCHIVED (wording before the edit; superseded in part by Note JI3, 2026-11-15)
- **FM29 Automatic power of two; `resize` is Half, Quarter or Eighth with a size floor (plan task 18, 2026-11-03).**
  - **The power of two.** `resize::pot_dims` sizes every texture with mips. With L the longer side and Lp its next power of two, L rounds up to Lp when L ≥ 0.95·Lp, else down to Lp/2. The shorter side S is scaled by the same factor first, Ss = S·L'/L (L' the rounded longer side), then rounds up to its next power of two when Ss ≥ 0.85 of it, else down; Ss < 1 gives 1. Examples (test `resize::pot_rule_examples`): 1000² → 1024², 1024×768 → 1024×512, 1000×600 → 1024×512, 48×40 → 32×16, 3×5 → 2×4 (then `DIM-SMALL`), 900×1 → 512×1. Integer arithmetic, no float rounding.
  - **Exempt textures.** `mips.levels = "none"` skips `pot_dims` (`resize::pot_exempt`): the texture keeps its size and gets `DIM-NPOT`, now a **Warning**, raised only there. `resize` still shrinks an exempt texture. A cube's source is never resized (`target_dims` returns it unchanged); its default face is `pot_dims((cell, cell))`, at most 4096, so a cell within 5 % of the next power of two now rounds up (a 1500×250 strip, cell 250 → face 256). An explicit `cubemap.face_size` is unchanged.
  - **`resize`** is `none`, `half`, `quarter` or `eighth`: `resize::shrink` halves both axes 1, 2 or 3 times (each at least 1 px), each time only while the longer side / 2 is still ≥ `resize_floor`. `resize_floor` is a new top-level `Profile` field, default 512 (`DEFAULT_RESIZE_FLOOR`), valid 1..=16384 (outside: `CFG-INVALID` "resize_floor N outside 1..=16384", test `policy_checks::a_resize_floor_out_of_range_is_cfg_invalid`; 0 would mean the same as 1, so it is refused rather than given a meaning), not serialised at its default, so existing profile hashes and identities hold where it is unset. `sdc-base` sets `[subtypes.height] resize_floor = 256`, so a 512² height map under `cs-compact` or `vanilla-compact` (`resize = "half"`) is written 256². `target_dims(dims, p)` = `shrink(pot_dims or dims, p.resize, p.resize_floor)` when `resize_relative_to` is `self` (with a slot, FM44's halving rule), used by the pipeline, the DIM checks, the bake plan, `estimated_bytes`, admission and the GUI's size estimate.
  - **Enlarging.** Tessalith's first upscale: `resize::enlarge` runs only on the axes that grow, by linear interpolation, texel-centre aligned, clamped edges; alpha-weighted colour is premultiplied, then divided by the interpolated alpha (plain RGB where it is 0). A roughness member grows on r² like the shrink. The existing down path (area-exact box, polyphase non-box) then shrinks the other axis, so `resize_work` is enlarge, then shrink when the size still differs. Admission (S-mips-11) charges 64 B per input texel when an axis grows, 32 when it only shrinks.
  - **Removed.** `resize = "pot-down"` and `drop_top_mip` (any value, ruling P21) are `migrate::REMOVED` rows: "the power of two is automatic; delete the key" and "use resize = \"half\"". `drop_top_mip` is gone from `Profile`, `FIELD_PATHS`, the pipeline and cube run (`chain.remove(0)`), the DIM block and the GUI (the Advanced *Half size* row, its lock and `set_field` special cases, the size estimate's dropped level). The texture panel's **Resize to a power of two** button is gone.
  - **DIM checks.** They read `target_dims`. `DIM-RESIZED` reads "{w}x{h} → {w'}x{h'}" plus " (resize = X)" when `resize` is set, data unchanged. Complex grass: `CFG-INVALID` when `shrink` would change the size (a non-POT grass is `GRASS-SHAPE`, as before).
  - **Low detail.** `checks::low_detail_eligible` needs `shrink(pot_dims(dims), Half, resize_floor) != pot_dims(dims)`; the `drop_top_mip` clause is gone (CL20's Eligibility bullet). `tests/low_detail_corpus.rs` judges eligibility on `target_dims`, the size `pipeline.rs` passes (`chain[0]`). Re-run on the owner's 10k set (2026-11-03, release): 10,000 jobs, 6,126 profile-eligible, 5,962 measured, 45 below 0.25, the same histogram as CL20 (the set is all 1024², where the power-of-two step changes nothing), so CL20's numbers stand.
  - **GUI.** Resize offers None, Half, Quarter, Eighth; a **Size floor** row (64 to 2048 px; a hand-written floor off that list, such as 300, is added to the combo so it reads "300 px", test `fields::the_resize_row_offers_the_halving_steps`) is an Advanced texture field (doc shot `advanced-texture-fields` on GUI-Advanced-Drawer) and a preset-editor row under *Resize & mips*. It is never locked, so Copy settings can carry `resize` and `resize_floor` together (a lock while Resize is None made the floor tick a no-op on targets whose resize was being set in the same copy). The `CHK-LOW-DETAIL` sentence ends "consider Resize: Half".
  - **Bake identity.** `BakeJob::identity_sha256` hashes `[resize, resize_floor]` only when the floor is not the default, else `resize` alone as before.
  - **Byte changes (encode versions).** Every arm is already 2.0.0 (FM9), so none moves. Outputs that change against 1.x and earlier 2.0 builds: NPOT sources with mips encode at the `pot_dims` size (before: `DIM-NPOT` blocked unless allowed, then written at the source size); 1024×768 becomes 1024×512; a 512 height map shrinks to 256 under the compact presets; enlarged axes use linear interpolation; exempt textures still take Half, Quarter and Eighth; cube faces from cells within 5 % of a power of two round up. `identity.json`: every existing key unchanged; new `cov/textures/c/npot_1000_bc1.dds` (1000² diffuse BC1 box → 1024²) and `cov/textures/c/height_half.dds` (512² height, `resize = "half"` → 256²). `encode_inputs.json` changes the height rows of cs and vanilla (profile only), `profile_hashes.json` 12 lines.
  - **Tests.** `resize::{pot_rule_examples, shrink_respects_the_floor, no_mip_textures_keep_their_size, enlarge_keeps_a_constant_image_constant, enlarge_is_linear_and_weighted_by_alpha}`; `golden::{npot_diffuse_converts_at_its_power_of_two_size, a_1000_square_diffuse_writes_1024, three_by_five_becomes_two_by_four_and_is_dim_small, no_mip_textures_are_written_at_their_own_size}`; `policy_checks::npot_with_mips_is_resized_and_without_mips_is_a_dim_npot_warning`; `low_detail::the_dim_checks_see_the_size_after_resize`; `presets::{pot_down_and_drop_top_mip_are_named, height_maps_shrink_to_a_256_floor}`; GUI `resize_field.rs` (was `drop_top_mip_field.rs`), `out_size::the_resized_size_agrees_in_all_three_places` (estimate, DIM-RESIZED and the written header), `model::an_npot_texture_is_resized_and_asks_no_question`. The GUI fixture's `odd.png` is 3 px (blocked by `DIM-SMALL`), since a 48 px NPOT is no longer blocked.
  - Supersedes, whole: T4 (`HALF_FLOOR`, no POT step), CL21 and CL23 (`drop_top_mip`), CL26 (the Advanced Half size row). In part: S-mips-6 (`pot-down`, owner question 3), S-mips-7 (`DIM-NPOT` after resize, the `pot-down` message), S-mips-10 (the Resize options and the `DIM-NPOT` button), the Deferred bullet of Note S (`pot-up`), CL20's Eligibility bullet, CL22, CL25, the CL cache-key bullet, CN19's Track C bullet, FC2 (the Half size tick and names), FC's `Half size has one meaning` and glossary bullets, and FM24's Half size bullet.

ARCHIVED (wording before the edit; superseded in part by Note JI5, 2026-11-15)
- **FM35 The alpha question is one dropdown on the resolved type (plan task 25, tune-up 4).**
  - **Which states ask.** Only a texture with no mesh evidence asks. Where a scan covers the texture, `mesh_alpha_policy` decides and `Provenance::MeshAlpha` counts as explicit, so neither `Need::AlphaMeaning` nor LOSS-ALPHA-DATA is raised and the texture is Ready; a non-remove mesh type is then written into the project at once ("Set 1 value automatically", `auto_record`), and a mesh `remove` stays `mesh:alpha` and GUI-only. A mesh-decided texture is not turned back into a question. Its panel shows the Alpha row's dropdown on the mesh's type, with "from the meshes that use this texture" beside it.
  - **One line.** A Problems or Review row that asks a texture's alpha shows `Alpha: <Type> (proposed) — confirm` (`code_text::alpha_confirm_text`; the type names are the five that the old buttons used), as does the preview's strip; the texture panel asks it as an Alpha row with Confirm (HI7, HI13). It replaces three lines: the Undecided sentence, the "carries data the format would drop" line with the engine's "non-constant alpha would be discarded", and the Acknowledge button. A row that asks only its alpha has no `Undecided:` prefix; a row that also asks something else keeps the prefix and joins the parts. Only "(proposed)" exists: a texture whose meshes decide its alpha is Ready and never asked, so there is no "(from mesh)" text.
  - **One dropdown.** `settings_panel::alpha_question` (its private `alpha_dropdown` and **Confirm**, HI7) is the control in the Problems row, Review's inline row and the settings box. It opens on the proposal (`items::alpha_asked`: `suggest_alpha` over the cached analysis, else the profile's own policy). Picking an entry, the shown one too, or Confirm records the answer (on the texture panel's Alpha row, picking the shown value writes nothing, HI14) with the old buttons' write (`alpha.policy` in the texture's own entry through `decide` or `answer_undecided_row`, nothing in `allow`). Enter or Space on a focused Problems row that asks only its alpha confirms the shown type (a row that also asks a sub-type ignores the key, so it never answers just one of the two); Accept all suggestions stays type-only. The `Decide…` menu and the five buttons are gone. The dropdown's accessible name is `Alpha type: <shown>`.
  - **Acknowledge.** While the alpha is asked, `decisions_of` offers no `Acknowledge(LossAlphaData)`: the type answers it (Remove is an explicit drop, any other type is kept), and the settings panel's Warnings list does not repeat it. The row's hover still names the code. After the type is chosen the code can return for a format that cannot store it; that is its own `Acknowledge` row and keeps `ack_label` ("Drop the alpha anyway"), because it records an `allow` entry that no type choice makes. Acknowledge for every other code is unchanged.
  - **Engine text.** The unconfirmed reason reads `alpha meaning is unconfirmed (proposed Cutout: …)` and the proposals no longer end in "the mesh decides", since no mesh did. Policy outcomes, codes, severities and output bytes do not change; `encode_versions` passes with `identity.json` unchanged.
  - **Tests.** `mesh_alpha::{a_mesh_resolved_cutout_shows_no_proposal_text, without_a_mesh_the_short_proposal_shows}`; `problems_rows::{the_problems_row_names_the_type, one_line_replaces_the_three_alpha_lines, picking_a_type_in_the_row_writes_what_the_old_buttons_wrote, a_mesh_remove_is_recorded_and_not_asked}`; `problems_accept::{enter_on_a_proposed_alpha_row_confirms_the_shown_type, enter_on_a_row_with_a_subtype_and_an_alpha_question_does_nothing}`; the old Decide… tests now pick from the dropdown (`ui_smoke`, `bottom_panel`, `no_codes`, `review_tab`). `the_decide_menu_starts_with_alpha_means` is deleted with the menu. No NIF-team file changed.
  - Supersedes: In part: CA11 (Decide… among the fix buttons), FB8 (the hover on the Decide… menu and Review's buttons), FB9 (Decide… rows stay menus), FC9 (the buttons stay Decide…), FI11 (the Decide… menu's lead line) and FI14 (the example row), FK4 (the alpha buttons in the wait test).

ARCHIVED (wording before the edit; superseded in part by Note JI3, 2026-11-15)
- **FM44 `resize` relative to a slot of the set (plan task 28B; owner 2026-11-03, amends FM29).**
  - **Why.** Under the compact presets a normal already exported at half its diffuse's size was halved again. The owner asked for the size to follow the set's diffuse, with the reference a choice of slot.
  - **Setting.** `resize_relative_to` (`ResizeRelativeTo`): `self` (default, not serialised, so every existing profile hash and identity holds) or `slot1` to `slot8`. It is in `FIELD_PATHS`, so rules, `[files]` and `--set` take it.
  - **Slot table.** `Subtype::txst_slot()`, an exhaustive match: 1 diffuse, landscape_diffuse, pbr_albedo, skin_albedo, complex_grass; 2 normal, landscape_normal, pbr_normal, msn; 3 pbr_emissive, glow, skin_sk, subsurface_tint; 4 height; 5 cubemap; 6 envmask, complex_material, rmaos, skin_rfaos; 7 coat_normal, pbr_fuzz; 8 pbr_subsurface, backlight_tint, msn_specular; none for the rest (ui_sky, flow, generic types, hdr_sky, skin_wet, projected_noise, detail_normal, gray). It is the engine's own table because the engine cannot read the GUI's `slot_kind` (controller ruling T28B-e). GUI test `resize_field::the_engine_slot_table_matches_the_slot_cards` holds it equal to `slot_kind` and `SlotKind::slot_number` both ways; the sub-types with no slot card (cubemap 5, envmask and complex_material 6, subsurface_tint 3, backlight_tint and msn_specular 8) are listed there by name.
  - **Halving rule.** own = `pot_dims(own)` (own when `pot_exempt`); cap = the longer side of `shrink(pot_dims(reference), resize, resize_floor)`. Both axes halve while the longer side is above the cap and its half is still ≥ `resize_floor`. The map never grows, keeps its aspect and stays a power of two. A 2048 diffuse with `half`: a 2048 normal → 1024, a 1024 or 512 normal keeps its size.
  - **No reference.** `resize = "none"` reads no reference and adds no code, so the compact BC1 colour rules' `resize = "none"` still means full size. A map whose own slot is the named slot is treated as `self`, with no code (controller ruling T28B-c: a slot-1 map "relative to slot1" is the flat half the owner asked for). A set with no map in the slot, more than one, or one whose size cannot be read falls back to `self`: `resize` applies to the map's own size, still stopped by the floor, and the map gets **`RESIZE-NO-REFERENCE`** (Info, data `{slot, maps}`, "…so this map was resized from its own size"). Controller ruling T28B-f: the compact presets must keep shrinking lone maps as 1.x did; the relative rule only protects a map that has a reference to compare with.
  - **Where the reference lives.** The planner writes it into the job's profile, `Profile::resize_reference: Option<(u32, u32)>` (`None` for `self`, `(0, 0)` for no single map, skipped when `None`; controller ruling T28B-d accepts this over a `target_dims` parameter), so every reader of `target_dims` (pipeline, DIM checks, bake plan and its `_n` profile copy, admission, `estimated_bytes`, the GUI size estimate and preview signatures) and the profile hash agree with no new parameter; a changed reference size moves the hash, so `if-changed` re-encodes. It is not a setting: a `[files]` entry, rule or `--set` naming it is refused as an unknown field, and a value in a preset table is overwritten by the planner for every job. `schemas::partial_profile_schema` drops it from the config and preset schemas (the partial profile and the by-product `Profile` definition there); the profile, manifest and event schemas keep it, since plans and records carry it.
  - **Planning.** `resize::plan_references` runs in `plan_sets` after the derive step, before shaders, bakes and `FilePlanned`. Sizes are pass 1's (`cube_facts`, else `input_dims`). A packed or derived map's own size, and its size as a reference, is its sources' common size (`api::job_dims_by`, the FM30 size before `target_dims`); a cube's is its source cell (`cube::layout::source_cell`). The pass recomputes `DIM-RESIZED` only for jobs with a reference (`policy::resized_warning`); a `self` job's warnings stay exactly as `check_profile` made them (unit test `resize::a_self_jobs_dim_resized_is_left_alone`). `DIM-SMALL` stays from `check_profile`, which saw the size without the reference; only a `resize_floor` below 4 could make it stale, since the relative size never falls below the floor.
  - **Presets.** `cs-compact` and `vanilla-compact` keep a plain `resize = "half"` on `diffuse`, `landscape_diffuse`, `pbr_albedo` and `skin_albedo` and add `resize_relative_to = "slot1"` to every other halved sub-type (25). A map in a set with no slot-1 job (a lone normal, `generic_data`, `flow`, a set whose diffuse is a skipped DDS) still halves its own size, as in 1.x, and gets `RESIZE-NO-REFERENCE`.
  - **GUI.** A **Relative to** Advanced texture field after Resize ("Own size", then "Slot N · <slot name>" by `sets::slot_name`), and a preset-editor row under *Resize & mips*. It is never locked, like Size floor (FM29), so Copy settings carries it with `resize`.
  - **Pins.** No arm moves (all 2.0.0). `identity.json`: two new keys, `cov/textures/c/relhalf.dds` (64 px diffuse) and `cov/textures/c/relhalf_n.dds` (128 px normal, `half`, floor 16, `slot1` → 32 px); no existing key changes. `profile_hashes.json`: the 25 sub-types under `cs-compact` and `vanilla-compact` for both targets (100 keys), re-pinned by hand (T28C-d).
  - **Tests.** `engine/tests/resize_relative.rs` (14: halves a full-size normal and writes it, keeps an already halved one, never enlarges, keeps the aspect, slot 4, no map and two maps in the slot fall back to own size, a lone compact normal halves 2048 → 1024 with the Info, own slot is self, `none` ignores the reference, `self` is byte-identical, `resize_reference` is not a setting, packed map sized from its sources, compact presets, slot words); `resize::a_self_jobs_dim_resized_is_left_alone`; `presets::compact_and_fast_preview_resolve_as_designed`; GUI `resize_field::{the_size_row_offers_relative_to_slots, the_engine_slot_table_matches_the_slot_cards}`, `advanced`. The lone-map compact tests (`golden::{compact_2048_normal_plans_1024, compact_1024x512_plans_512x256}`, `cli_convert::cs_compact_writes_bc1_colour_and_half_size_normals`) pass unchanged.

ARCHIVED (wording before the edit; superseded in part by Note JI7, 2026-11-15)
- **FM45 Final review fixes (main team, 2026-11-03; rulings FR-a to FR-e).**
  - **Removed keys, all at once (FR-a).** `migrate::removed_in` and `removed_set` list every hit, one per line in key order (an `allow` list gets one line per removed code; per key the first `REMOVED` row still wins, so `nvtt` names the value), at most 10 lines and then `and N more`; a single hit reads as before. `sdc` reports every bad `--set` and `--allow` in one error, and the GUI banner keeps the list one key per line. No auto-remove: some removed keys change output (`drop_top_mip`, `pot-down`), and nothing may change output silently. Tests `migrate::every_removed_key_is_listed_in_one_error` (20 `[files]` hits plus two other keys), `cli_basics::every_removed_set_and_allow_is_named_at_once`, GUI `project::every_removed_key_of_a_1x_project_is_named_at_once`.
  - **Skipped user presets (FR-b).** Every CFG-PRESET-SKIPPED file that is not an alias clash gets a Warning row "Preset file <name> was skipped: <reason>" with Open folder (the reason's first line; the hover has the file and all of it). When the project's missing preset is named after a skipped file (its stem, ASCII case folded), the Blocks row and Review's refusal read "Preset <id> was skipped: <reason>" instead of "is not installed"; Choose preset stays. `bottom::skipped_reason` strips the engine's `preset parse error: <file>: ` lead-in (a `shortcut:`, it parses the message). Test `presets_creator::a_skipped_user_preset_names_its_reason`.
  - **Cubes ignore `resize_relative_to` (FR-c).** `resize::plan_references` reads no slot for a `cubemap`, since `target_dims` leaves a cube to `cubemap.face_size`, so no RESIZE-NO-REFERENCE claims a resize that never happens. Test `cube::a_cube_ignores_resize_relative_to`. The GUI says "no effect on a cubemap" on a cube's Resize rows and "no effect while Resize is None" on Relative to and Size floor (`fields::no_effect_reason`), in place of the provenance; the rows stay unlocked (FM44).
  - **Pin record (FR-c).** `identity.json` key `cov/textures/c/cutout_bc3_t200.dds` was re-pinned by hand in ac5e4fff: its fixture changed from `rgba64()` to `rgba_near_cut()`, alpha near the cutoff, so threshold 200 changes its bytes (`t200_pins_the_cutoff`); no encoder byte changed. `encode_inputs.json` was rewritten in b9a8bb1c (all 61 keys): each key hashes its encode input, which lost the removed `validation.*` fields (FM16). Neither changes a 1.x output.
  - **GUI.** The preset editor has a Perceptual box for BC1, BC3 and BC7 with the texture panel's lock; the closed Relative to combo shows "Slot N", with the slot's name in the list and on hover (it held the Advanced drawer about 90 px past its edge); the 1-bit alpha box is named for AccessKit; the alpha dropdowns of Problems and Review are named "Problems alpha type: …" and "Review alpha type: …" (the texture panel keeps "Alpha type: …"); the FMT-BC1-ALPHA sentence offers 1-bit alpha only for see-through alpha.
  - **Schema.** `resize_floor` is `1..=16384` in the schemas, as the loader checks. `[defaults] packing.fill` still validates in the schema: the `[defaults]`, sub-type and `[files]` tables share one partial profile, so refusing it there needs a definition of its own. The loader refuses it (FM16).
  - **Docs.** CHANGELOG: the `--strict-cubemaps` and 2D cube-check lines go (a 2D `_e` file fails validation, as in 1.1.0), the power-of-two rule is the 95 % / 85 % one, the byte-change, vanilla perceptual and compact height lines give the final state, and the FM12 event change is listed. The Known issues page is a section of Troubleshooting. The broken `CHANGELOG.md#unreleased` anchor in `GUI-NIF-Edit.md` is the NIF team's (HANDOFF line, FR-d).

ARCHIVED (wording before the edit; superseded in part by Note JI7, 2026-11-15)
- **FM14 NVTT is removed everywhere; VAL-BC6H-PEAK advice reworded (plan task 4, 2026-11-02).** The optional external encoder backend (`nvcompress`) is gone: `crates/engine/src/nvtt.rs`, the `nvcompress` test fake, `Backend::Nvtt`, `EncoderVersions.nvtt`, `BatchRequest.nvtt`, `--nvtt` on `convert`, the three codes `ENC-NVTT-MISSING`, `ENC-NVTT-VERSION` and `ENC-NVTT-FAILED` (118 codes become 115), the GUI's NVTT path (Advanced drawer, `gui.toml` `nvtt_path`, the picker purpose), the Encoder row in the settings, preset and bulk panels, the Encoder filter in the Textures column (and the saved `filter_encoder`), the `NVTT-Backend` wiki page and the bug template's NVTT dropdown. The README charts and the Research tables keep NVTT 3 as a reference encoder (Note HA1). A preview now always encodes with the profile's own backend. `encoder.backend = "nvtt"` and an `allow` list naming one of the three codes are errors that name the removal (`migrate::REMOVED`, FM10); `--nvtt` is a usage error (exit 2) from clap. `identity.json` is unchanged: no built-in output moved. `VAL-BC6H-PEAK` advice no longer offers NVTT as the other way to keep the peak: it says to set `encoder.effort = "slowest"`. Supersedes: Y9, all of Note NV, AF3 (and its AF9 sub-bullet), BC9, BC12, BQ3, CV3 and CV15 (moved whole); AF10, AF11, AG40, AG42, AG46, AG111, AM5, BC13, BS3, CN33, CP4, CV21, DM4, DT2, FB8, FE7, FF6, FL2 and the BC7-exactness bullet of Note AH (the NVTT wording edited out, old wording archived). Code comments that cited NV, AF3 or BC9 went with the code they described.

ARCHIVED (wording before the edit; superseded in part by Note JI7, 2026-11-15)
- **FM15 The format alone picks the encoder; the rgbcx level is fixed; the mode-6 byte patch is gone (plan task 5, 2026-11-02).** `EncoderSpec` has no `backend` or `rgbcx_level`. `Backend`, `policy::backend_fits`, `default_backend`, `backend_follows_format`, the sources that tracked a pinned backend, and the `CFG-INVALID` checks for an encoder that cannot write the format and for a level above 18 are deleted. `encode_band` already chose by `p.format`: rgbcx for BC1, BC3, BC4 and BC5, bc7e for BC7, ISPC for BC6H. `bc1_encode_block` and `bc3_encode_block` take no level and pass `RGBCX_MAX_LEVEL` (18, the value every built-in preset set). No encoded byte changes: `identity.json` is untouched, while `profile_hashes.json` and `encode_inputs.json` change because the encoder spec lost two fields.
  - **Removed names.** `migrate::REMOVED` gains `encoder.backend` (any value, after FM14's `nvtt` row, so that value still names NVTT), `encoder.rgbcx_level` and `allow` = `IN-FLOAT-LDR`. `Code::parse_allow` reads the same table, so a config, a preset, `--set` and `--allow` all give the removal message. `IN-FLOAT-LDR` leaves the catalogue (114 codes); it was never emitted.
  - **Patches are required.** `build.rs` stops the build when `bc7e_ispc.h` lacks `m_exact_opaque_alpha` (patch 0001) or `m_endpoint_search` (patch 0002). `encode_band` loses its `native` parameter, `bc7_opaque_fix` its `mode6_patch` arm, and `bc7_fixup::force_opaque_mode6` goes with its tests. `check_profile` no longer refuses `encoder.effort = "max"`. The rotated mode 4/5 fix stays. The `BC7E_*_NATIVE` constants, their asserts, the `sdc_bc7e_patched` cfgs and function and the shim's `#if` branches are gone; the `build.rs` check is the only guard.
  - **Vendor header.** `vendor/bc7enc_rdo/rgbcx_table4_small.h` is deleted. `rgbcx.cpp` names it only inside the `RGBCX_USE_SMALLER_TABLES` branch, which `rgbcx.h` defaults to 0 and nothing sets, so it is never compiled; the vendored source is left as it is. `SHA256SUMS` and `README-VENDOR.md` follow.
  - **GUI.** The Advanced drawer loses the rgbcx level row, the pin summary loses its encoder word (it read "bc7e", "rgbcx" or "ispc (BC6H)"), and the preset creator no longer moves a backend with the format. Supersedes: Owner checks pending 8 (`IN-FLOAT-LDR` block), S2 and BY1 (archived); D, S4, BZ5, CB1, CP4, CP11 and FF1 (in part). The psdds items DC5, DC8, DC10 and DC11 describe the plug-in on branch `host-plugins`, which is not changed here.

ARCHIVED (wording before the edit; superseded in part by Note JL8, 2026-11-16)
- **FM31 Polish: window size, HDR tiles, helper-row keys, filled-analysis cache (plan task 20, 2026-11-03).**
  - **Window size.** egui 0.36.2 draws a `Window` bigger than the `default_rect` it was given (3 points with the default style: 1 plus twice the frame stroke, and more with some content), so a saved W×H came back (W+3)×(H+3) and was saved again. `ui::set_preview::settings_window_saved_rect`, which `layout::capture` reads, is the drawn rect less that excess. The excess is measured once per session, when the drawn size has held for 3 frames (60 at most); a difference over 16 points is a clamp to the window, not the margin, and takes nothing off. The excess depends a little on the content on show then (a selected item adds a few points), so a window whose content changes later saves off by that difference (a `shortcut:` in the code). A saved W×H is drawn W×H plus the excess and saved as W×H, so a start changes nothing. Test `set_preview_layout::a_saved_window_size_reopens_at_the_same_size`.
  - **HDR tiles.** `model::preview::dds_thumbnail` treats a BC6H, RGBA16F or RGBA32F DDS as float, so `make_thumbnail` maps each colour channel `x / (1 + x)` (a negative is 0 first) and encodes sRGB at 0 EV; alpha is untouched and an LDR tile is as stored. The main preview already did this on the GPU (`tonemap` in `preview.wgsl`; an RGBA32F file previews as RGBA16F); the shader now clamps a negative to 0 before the map, which turned -2 into +2. Test `preview_model::hdr_preview_tone_maps_values_above_one` (1, 4 and 16 give 188, 231 and 248; RGBA16F and RGBA32F; BGRA8 unchanged) and `a_bc6h_tile_is_tone_mapped`; the shader clamp is pinned by `render::exposure_scales_hdr_values_before_the_tone_map`.
  - **Helper rows.** `AppModel::row_move` (the Up and Down keys): from a helper row they step through the shown helper rows and stop at the ends; Up from the first returns to the last tile; from a tile in the last grid row (a partial one included), Down enters the first helper row; from any other tile, and with no helper rows, Up and Down are the grid row moves as before. Shift+arrow and Left/Right stay on the tiles (DY16's re-review). Enter and Space open the active row as before. The row is scrolled into view by the existing `reveal_item`. Tests `helpers_list::{helper_rows_move_with_arrow_keys, down_enters_the_helper_rows_only_from_the_last_grid_row, down_without_helper_rows_is_the_plain_grid_row_move, the_helper_row_the_keys_reach_stays_in_view}`.
  - **Filled analysis.** The fill step keeps each filled target's analysis and source info in the pack map under `session::fill_cache_key`: the target's path and stamp, the helper's `FillJob` (path, stamp, role, read, invert, assigned) and the EXR straight-alpha setting it was loaded under; hints are checked as for a pack. Memory only: the disk store's version pin does not run the fill. A re-plan with no change loads nothing there; a changed target or helper misses; `retain_plan` keeps the keys of the plan's filled jobs. If the straight-alpha loop ran twice, the first key is dropped after the plan and that first load repeats (EXR only). Test `helper_fill_run::filled_analysis_loads_once_across_replans`.
  - **Encode versions.** None moves: no output byte or verdict changes (`identity.json` keeps every key).
  - Supersedes: In part: CE21 (the creep), FC8 (CE21 as a kept shortcut). Whole: DY19's two shortcut bullets (the filled analysis is not cached; helper rows are mouse-only).

ARCHIVED (wording before the edit; superseded in part by Note JL1, 2026-11-16)
- **FM32 Sets column: keyboard and scroll (plan task 21, 2026-11-03).**
  - **Stand-ins.** A card outside the clip rect (CF7) still registers its widget id: a focus stop and an AccessKit node (Button, "Set {id}, position {n} of {N}", selected when the set is, the Focus and ScrollIntoView actions, the bounds of its allocated rect; the drawn card's own node carries the same name, role and selected state through `card_node`). The id is `ui.id().with(("set-card", key, id))`, and the drawn card's `scope_builder` takes the same one through `UiBuilder::id`, so the focus survives the swap from stand-in to card. Tab and Shift+Tab walk the cards in order. A stand-in that takes the focus this frame (the focus was elsewhere at the end of the last one: `DropState::focus`) or gets an AccessKit ScrollIntoView request (egui consumes that action only in a registered widget, so the stand-in reads it with `has_accesskit_action_request`; HP11) sets `nav.reveal_set`, or, for a created set with no key, calls `scroll_to_rect`; the next frame draws the card in view. A Go to set in the same frame wins: the card keeps its focus and the Tab asks for no scroll. A focused card the wheel scrolls off still loses focus (it is no widget that frame). The stand-in is registered by hand (`Memory::interested_in_focus` and `Context::accesskit_node_builder`), not with a `Response` per card: that cost 15% of a frame at 300 sets. Its node's parent is the window's root, not the list's container as for a drawn card: egui 0.36.2 sets a node's parent only through a `Ui` or `Ui::interact`, both of which register a widget rect per card, so it cannot be done at this cost.
  - **Cost.** 300 sets at 1500×900, measured once with a bare `egui::Context` (the benchmark was not kept): 294 µs a frame before, 311 µs after (+6%) with AccessKit off, which is how a window runs without a screen reader; 357 µs before and 430 µs after with it on. The widget rects registered stay 354. Off-screen cards are still not drawn.
  - **Heights.** A change of the column width of 0.5 pt or more (`DropState::card_w`) moves each remembered height by the chip rows it adds or removes times (chip + `DOCK_GAP_Y`): rows come from the set's slot row, missing entries and extra members at `CHIP` wide with the card's margins (`chip_rows`, `reflow_heights`). An id no longer in the views is forgotten. The estimate leaves out the name row, which wraps below the pill when the column is narrow; drawing a card measures it exactly.
  - **Anchor.** The first card in view is remembered with its top in the content, summed from the remembered heights and not from the laid-out rects (egui rounds each allocation, which drifted a pixel over 200 cards). The next frame sets the scroll offset so that card stays where it was on screen while the heights above it change (the mean of the measured ones moves, a width change re-flowed them). A card that comes into view with a height other than its estimate still moves the cards below it for that one frame. A frame where `model.sets_filter` differs from last frame's copy scrolls to the top instead, unless a reveal is pending (HP5); other list changes keep the anchor.
  - **Far jump.** A reveal of a card outside last frame's view lands it at the top of the view, 2 × the item spacing below the edge, not at the least scroll that shows it: an estimated height left it cut off and a second scroll moved it. Once that card has been laid out (`DropState::reveal_seen` holds its key; another target, or a frame with no reveal, takes the jump path again) the reveal scrolls to its measured rect and no longer re-aims from the heights, which moved each frame. Jumps stay instant.
  - **Tests.** `set_cards::{off_screen_set_cards_have_accesskit_nodes_and_tab_stops, card_heights_refresh_after_a_resize, a_far_jump_does_not_shift_the_scroll, a_go_to_set_in_the_same_frame_wins_over_a_tab_reveal}` (the first three failed before: no node for an off-screen card; 124 pt at 520 wide and at the minimum, where the drawn card is 481; the card moved 43 pt between the first frames). `docs/wiki/GUI-Sets-Column.md` drops the Tab limit.
  - **Step 1.** Live: every ruling. The three `shortcut:` comments in `sets_column.rs` are gone.
  - Supersedes: In part: CF7 (the Tab and screen-reader limit, the stale off-screen heights, the shift after a jump, the least-scroll jump) and FC8 (CF7 as a kept shortcut).

ARCHIVED (wording before the edit; superseded in part by Note JO, 2026-11-17)
- **FM24 Review keeps its gate; the half-size question, the warning ticks and the stale-outputs list are out (plan task 13, 2026-11-03).**
  - **The gate.** Review lists and gates on: Blockers (`export_blocks`, no export folder), Undecided textures, the Keep or Overwrite questions (an edited PGPatcher JSON, a NIF file Tessalith did not write) and the Also folders. `review_gate` is, in order: read-only `sdc.toml`, no export folder, an export running, a missing preset, a blocker, the Also gate, an unanswered overwrite question. A texture left undecided is skipped by the export, as before. An overwrite answer still stores the file's hash and is checked again at write time (CA7).
  - **Warnings are Problems rows.** `export_asks` (the PGPatcher set pitfalls, the CS PBR pitfalls) feed Problems as ordinary Warning rows (`carried`) and nothing else: no Warnings column in Review, no tick, no Acknowledge all, no `{n} warnings in Problems` link, no block. A pitfall alone does not open Review, so the header Export runs when nothing else needs an answer. A set with no match (V4) is one of these rows too. A NIF edit that cannot be written is listed in the Review footer under `NIF edits skipped (n)`, one amber `{path}: {reason}` row each, and never blocks.
  - **Half size.** There is no Review question, no `Review.half_size`, no `Measure whether full size adds anything` switch (an old `[checks] drop_top_mip` key in `gui.toml` is ignored on load) and no `LowDetailMode::Hold`, `Keep::Measure` or `Processed::UpToDate`: the engine writes a flagged texture at its planned size and `Warn` only adds the warning (`drop_top_mip` went in FM29). Encoded bytes and `identity.json` are unchanged.
  - **Stale outputs.** `engine::manifest::stale_outputs`, `StaleOutput`, the Review group `Stale outputs`, its Delete, the Problems row with `Review…`, `AppModel.stale` and `stale_json` are gone. An output the project no longer makes stays in the export folder and is left alone, as before FC11. The sidecar `_sdc/generated.json`, its reconcile on open and the hash records stay (FC11).
  - **Tests.** `review::{a_pgpatcher_pitfall_is_a_problems_row_not_a_gate, review_still_gates_on_blockers_and_overwrites, review_has_no_half_size_or_stale_group}`; the last lists every label the Review panel draws for one fixture, so a renamed or returning group fails it.
  - Supersedes: CA4 (whole), and the FC14 bullets on stale JSON rows, the half-size write and the half step. In part: CN62, CN64, CN66, CL (the Carried to Task 5 bullet), CL25, FL (the gate timing), the FC14 Review footer bullet, CA5, CL15, CL20 (Hold, the flagged-again bullet), CL26, FC2 (the Review question and the engine Hold pieces), FC5 (the half-size order, the Warnings row of a blocked NIF edit), FC11 (stale outputs in Review; the sidecar stays), FC12 (the portrait photo is a Problems row), the `Half size has one meaning` bullet of FC, the two FC bullets on `stale_outputs`, DS26, V4 (the ask before export), AA2, AD5, AG90, AT (Untick or orphan), DR9 and DV7 (the Review stale row). CL24, CL27 and CL43 were already archived by FC2.

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

ARCHIVED (wording before the edit; superseded in part by Note JO, 2026-11-17)
- The Problems list owns every fix button; `fix_off()` is the one definition of "fix buttons off". [CA11, CA12]

ARCHIVED (wording before the edit; superseded in part by Note JO, 2026-11-17)
- The set editor is one scrolling column; its room comes from Textures first, then Sets (HP6). [FB2, HP6]

ARCHIVED (wording before the edit; superseded in part by Note JO, 2026-11-17)
- The set editor's warnings block is one line "N problems in this set - Show in Problems"; a Textures-column "{n} undecided" count and a set's warning badge open Problems narrowed. [CA18]

ARCHIVED (wording before the edit; superseded in part by Note JQ, 2026-11-18)
- `packing.recipe` = `auto`, `rmaos` or `rfaos`; `rfaos` without fuzz uses the fuzz fill. [R-fills-1]

ARCHIVED (wording before the edit; superseded in part by Note JQ, 2026-11-18)
- A skin `_f` fills the RFAOS fuzz mask in a set already Advanced skin by other evidence; `_f` never makes a set Advanced skin by itself; a real `_fuzz`/`_sheen` wins G. [DR1, DR2, DR3, DR7]

ARCHIVED (wording before the edit; superseded in part by Note JQ, 2026-11-18)
- The packed suffix joins the set's suffixes before members resolve, predicted from the raw roles present. [M7]

ARCHIVED (wording before the edit; superseded in part by Note JQ, 2026-11-18)
- Skin detection uses the recipe actually chosen (a forced `rmaos` with fuzz is not skin). RFAOS is offered only where buildable (roughness, glossiness or AO source); `PACK-RECIPE-UNBUILDABLE` is attached only when `auto` would not build `_rfaos`. [I1, I2, BQ13]

ARCHIVED (wording before the edit; superseded in part by Note JQ, 2026-11-18)
- `_i` suggests `diffuse`; rule 12 picks `default`, never `pbr` without a PBR map. [S-shader-6]

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

ARCHIVED (wording before the edit; superseded in part by Note JI7, 2026-11-15)
- **FN2 A pack Fills box takes typed text once (lead 2).** `advanced::settled_drag` uses `DragValue::update_while_editing(false)` and settles on `drag_stopped || lost_focus || (changed && !dragged && !has_focus)`, and only when the value differs from what came in. Typing "200" is one edit and one undo step ("Fill value"), applied on Enter, Tab or a click away; Esc keeps the old value. Arrow keys on the focused box change the value at once (egui), and that value is held in egui memory while the box keeps focus, as a drag's is, so leaving the box writes it as one step. The box carries an AccessKit name ("AO fill" and so on). Tests `typing_into_a_fill_box_is_one_edit` (RED on the old rule: 255 was written on the first keystroke) and `an_arrow_key_on_a_fill_box_is_written_when_the_box_is_left` (fix round 1; RED before the hold: the arrowed value was lost).

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

ARCHIVED (wording before the edit; superseded in part by Note JI7, 2026-11-15)
- **FY1 One commit-once-per-gesture widget (L29).** `advanced::held` holds the in-flight and the sent value in egui memory (pending, sent, busy: R2) and settles on a drag end, on leaving the box (FN2: typed text and arrow keys on a number box are one edit), or on a click. `settled_drag` (Advanced fill boxes, busy), the cube section's `num` and `held_value` (busy false) wrap it; `colour` and `settings_panel::commit_slider` stay. A number box defers an arrow key to leaving the box. `held_value` also settles on `lost_focus` now, which changes nothing a caller sees: its boxes took an arrow key at once and still do. Tests in `ui/advanced.rs` and `cube_section.rs` (a typed value and an arrow key commit once in both; a slider click commits once in `advanced.rs`).

## Note HC — 2.1 bug batch (main team, 2026-11-05)

ARCHIVED (wording before the edit; superseded in part by Note GV1, 2026-11-07)
- **HC1 A specular fills the flat alpha of a pre-packed `_rmaos`/`_rfaos` image (owner, 2026-11-05).** Before, the image won under `packing.prefer = prepacked` and the specular read "not used", even with a hand-set `rmaos.a` fill. Now, when the set's only other loose map is a specular (a generated AO the image's empty B takes may join it) and the image's alpha is flat, the image joins the recipe as CO104 does: A from the specular, the other channels from the image. "Flat" is the alpha verdict `plan_fills` reads (`AlphaFacts::empty`), so any constant counts, not only 255. An alpha with content, or another loose map, keeps the M9 result. A pre-packed DDS is never joined (images only, as CO103). No `encode_version` arm moves: the recipe and its bytes for given sources are unchanged, and the new source list re-encodes that set by itself. Tests `generated_files::a_specular_fills_the_flat_alpha_of_a_prepacked_rmaos` and its neighbours.

ARCHIVED (wording before the edit; superseded in part by Note HL5, 2026-11-08)
- **HC5 A PBR pick without a PBR albedo is not called vanilla (owner's screenshot, 2026-11-05).** The shader pick reads any PBR map (a lone `_f`), while `SetView::pbr` and the JSON need a `pbr_albedo` diffuse, so the card said "PBR (auto)" over a "vanilla" pill. The pill now says "PBR, no JSON". Nothing else changes: the set still writes no JSON. Test `pbr_landscape::a_pbr_pick_without_a_pbr_albedo_is_not_called_vanilla`.

ARCHIVED (wording before the edit; superseded in part by Note JO, 2026-11-17)
- **HC2 A row about a missing or unused map says which (owner, 2026-11-05; amends FB8).** "Add the missing map" never named the map, and the name was only on the hover. `PACK-DEFAULTED`, `PACK-CONFLICT`, `PACK-UNUSED-INPUT`, `SHADER-EXTRA-MAP` and `SHADER-MISSING-MAP` show the engine's text, without its code: "x_rmaos.dds A: no specular source; neutral default 0.04". Every other code keeps its action sentence, and the table keeps these five for lines with no engine text (`code_line`, banners). Test `code_text::a_row_about_a_missing_map_names_the_map`.

ARCHIVED (wording before the edit; superseded in part by Note JO, 2026-11-17)
- **HC4 `Kinds ▾` hides kinds of Problems row (owner, 2026-11-05).** One missing specular filled the list. The menu ticks each kind (`Code`) with its row count; a hidden kind leaves the Problems list only, for this project, and `gui.toml` keeps it (`problems_hidden`, by project root). My calls: Warning and Info rows only, as a block or a question must stay in reach; the tab count, the ready counts and Review ignore it, as Review's safety rules read every warning (a hidden row is not acknowledged); no Clear button, since the rows come back on every plan. Tests `bottom_panel::a_hidden_kind_leaves_the_list_but_not_the_ready_count`, `a_question_is_no_kind_to_hide`.

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

ARCHIVED (wording before the edit; superseded in part by Note JH2, 2026-11-15)
- **HE4 No missing-map warning for a map a mesh or TXST already supplies (owner, 2026-11-06: "use existing: yes, and no need to confirm it's actually used").** `SHADER-MISSING-MAP` is dropped from a set's view (`SetView::set_warnings`, so the card badge, the pill and Problems follow) when a live mesh's texture set, or a winning TXST, that names the set's diffuse has that map's slot filled. `KnownPaths::supplies` (diffuse path with `.dds`, lower case, to 0-based slot bits 1..=7) is read from the scan with the conditions of HC6 (the winner and `owner_ok` joins) and of `LIVE_LIGHTING_TEXSET`; `sets::map_slot` gives each required role's slot (`_n`, `_n/_msn`, `_n (model-space)` 2; `_g` 3; `_p` 4; `_rmaos`, `skin_rfaos`, `_m complex` 6; `_i` 7; the diffuse-like roles none). The scan is not asked whether the game uses the map (the owner's word). `AppModel::known_rev` is part of `ViewKey`, so a scan landing after the plan removes the warning with no new plan. Agent rulings: GUI only, the CLI has no scan and still warns; the lookup is the name the set writes (`mesh_key` of the diffuse's game path and its output stem, as Used by does), because the meshes name the output, not the name the file is read as: an `output_name` or an exporter-named diffuse (`rock_albedo.png` writing `rock.dds`) is found by the written name, and an input name (alias) is not asked (final review I2, agent ruling); it is an exact path, not a stem or folder match; `PACK-DEFAULTED` stays, because it is about the channels of a file this set writes. Tests `a_map_a_mesh_supplies_raises_no_missing_map`, `an_aliased_set_looks_up_the_written_name`, `a_renamed_output_looks_up_the_written_name`, `no_scan_warns_as_before`, `every_required_role_has_a_slot` in `sets_model.rs`; `read_lists_the_slots_a_mesh_or_txst_fills_beside_a_diffuse` in `pgp_lock.rs`.

ARCHIVED (wording before the edit; superseded in part by Note JI7, 2026-11-15)
- **HE6 The helper panel's Fills rows (owner, 2026-11-06, on a screenshot: "this looks sloppy"; the layout and the naming are agent rulings).** The Fills block is one Grid row per line: `Fills` and the hint on the first line, one target per line with its ✕, then the Add combo with `Reset` (shown only when the list is hand-set). The ✕ and `Reset` sit in the same column as the Role row's `Reset`; the target name is as wide as the combos (`COMBO_W`), inset to the combo's text, and truncated with the full name on hover; the hint is truncated the same way. A target is named by the planned output that ends in its suffix (`bake_1001_rmaos R`, the name on the `Show …` button), else by the set's base name when no output landed (`helpers::target_label`); the Fills rows, their ✕ names, the Add options, the `Skipped …` line and the `fills to …` line all use it. Pinned by `fills_rows_line_up`, `the_hint_column_says_set_by_you_or_suggested_on_the_first_fills_line` and `a_packed_target_is_named_by_its_output` in `crates/gui/tests/helper_panel.rs`. At 1100 px the Advanced strip overlaps the centre's reported rect by 18 px (seen once the over-wide row no longer widens it); `panel_controls_inside_panel_at_1100` skips the strip and the overlap is left as it was.

## Note HG — Texture set records in the sets model (main team, 2026-11-06)

ARCHIVED (wording before the edit; superseded in part by Note HL4, 2026-11-08)
- **HG8 The picker, the record read and the record-change rows (Task 6; the key is the owner's, the rest agent rulings).** (a) **Key, owner (2026-11-06):** "Key is plugin + EDID, not FormID. Should probably have formid somewhere in there, written as 0x (no header, first two digits, no leading 0's)... edid within a plugin itself is not a forced unique." The settled reading is plugin + FormID with the EDID as a label: `0x05000814` shows as `0x814` (`formid_text`). It is computed from `rec.local`, never `rec.formid`, whose load-order digits move (a light plugin's `FE4CD814` would keep `4CD`). (b) **The defining plugin, agent:** the plugin in a record's key is `masters_json[local >> 24]`, or the winning plugin itself when the index is at or past the master count (`clamp_local`), so an override finds the same record when the winner changes; `TxstRecord.winner` is shown beside it when they differ. The read is one query over `rec` / `rec_winner` / `plugin` / `txst_slot` with `owner_ok = 1`, ordered by EDID; a record with no path is not listed; `txst_slot.slot` is converted by `tx_to_slot` there (HG1). Like `supplies` (K1) it sits behind `unwrap_or_default`, so a failing record query empties only the picker. (c) **The picker, agent:** **Map to texture set record…** is greyed with "Scan your mods first" when the scan holds no record, and with "PGPatcher PBR sets name their outputs in the JSON; use Rename outputs" for a PBR set on Community Shaders, the same set `record_entries` ignores (HG5); the scan reason comes first. The dialog writes nothing until **Map**; it has no Enter default, so Enter never maps; a click outside does not close it (it holds the typed words). The list is a `show_rows` list of one fixed row height, filtered again only when the words or the scan change (`record_hits`, kept in `Pending::MapRecord`). **Change record…** opens on the set's current record when the scan has it. (d) **Record changes, agent:** a mapped set whose stored slots differ from `record_of` (plugin + FormID, ASCII-folded, the winner not compared) gets a Warning row "Record {EDID} changed" with **Use new paths**, which copies the record's paths and EDID in as one undo step. A record missing from a non-empty scan gets an Info row (no fix, no Review block), and the mapping and stored paths stay. An empty scan says nothing, so the mapping survives a scan that has not run. A PBR set on Community Shaders gets neither row: its mapping is ignored. (e) **A supplied slot shows where the shader has a row for it, agent:** the set editor writes "From record: {file}" in an empty slot row the record supplies. The Advanced skin editor has no `_s` row, so a supplied slot 8 shows nowhere there; its warning is still dropped. The card's "Record:" line is small, not weak, and its hover lists every slot path, marking each the set does not write itself with "(from the record)". (f) **One rule for what a member writes, agent:** `member_write` says what a member of a mapped set writes: the output key, the record's path and where it goes under the export folder. `record_entries` (the plan) and the picker's preview both call it, so a packed output with a job on a record slot is previewed, and a kept DDS with no job, a refused path, a packing source and a helper say "keeps its own path". Path collisions are not predicted in the preview. (g) **A new record starts with every fill on, agent:** `map_record` to a different key (plugin ASCII-folded + FormID) and `unmap_record` clear the set's `no_fill` in the same edit, so one undo restores it; the same key (**Use new paths**, **Change record** to its own) keeps it. (h) **Arrow keys, agent:** Up and Down in the picker move the highlight (no highlight to the first row on Down) and scroll it into view; they never map, and Enter still does nothing. (i) **`record_ignored`, agent:** `AppModel::record_ignored(&SetView)` is the one test for "a PBR set on Community Shaders, whose mapping is ignored"; the plan, the preview, the menu and the Problems rows all use it.

ARCHIVED (wording before the edit; superseded in part by Note JI2, 2026-11-15)
- **HG4 `shared` is derived in the GUI; a shared set shows only its maps (agent ruling).** `SetView.shared` is set by `finish_view` (`sets::is_shared`): the pick is Advanced Skin, the set has members, and every filled slot and every member is Normal, `_sk`, `_wet` or `_detail_n` (a member in no slot, such as a kept DDS with no job, is judged by its name through the engine's `member_split`). That follows the engine's member rule (`group::mark_shared_skin`); the plan's narrower test (no Diffuse, no Rmaos) called a set shared after a hand-added `_p` that the engine had already unmarked, and hid that map's row. The statics choice ("PGPatcher entry for statics") on a shared set is disabled with the reason on hover and as a line, and its status line is not drawn: a shared set has no diffuse, so it writes no PGPatcher entry; the rest of its CS PBR records section and the Advanced tab stay. `TextureSet.shared_skin` is not carried over `SetResolved`, which is a serialised report. A lone `_rfaos` is not shared and keeps its missing-diffuse row. A shared set's card pill reads "shared" (hover "shared skin maps (no JSON)"); `shared_cells` keeps only Normal, `_sk`, `_wet` and `_detail_n` cards in the set editor, the set card and the card height. It writes no `.csskin.json` and raises no skin skip row (`cs_skin_json::build_plan`), takes a `_wet` or `_detail_n` drop (`choice_allowed`), and the name-tail readers (`member_tail`, `renamed_output`, the Rename dialog) read `_wet` and `_detail_n` as tails for it as for a skin set. `input_name.rs` reads a set's diffuse first, so a shared set never reaches it; it takes the same argument for consistency.

## Note HL — Target mod and in-development guard rails (builder, 2026-11-07)

ARCHIVED (wording before the edit; superseded in part by Note HQ, 2026-11-08)
- **HL5** Every PBR set gets its PGPatcher JSON entry on Community Shaders, also with no PBR albedo. `SetView::json` = `pbr`, or a set whose shader pick is `Shader::Pbr` and that is no skin or shared set and has no `pbr_albedo` diffuse; `pbr` keeps its meaning (the albedo set, the prefix owner) and the JSON readers (`json_entries`, blocks, asks, `is_json_set`, auto flags) read `json`. `derived_match`, and `match_from` (`MatchFrom::{Albedo, Mapping, BasePath}`), come from the first that exists: the mapping's base, a record's slot 1 (not for an albedo set), the albedo or diffuse, the set id's own path. A no-albedo set keeps its meshes' textures: `slot1` is the mapping's slot 1 else `textures\<derived path>.dds` (PGPatcher skips a shape whose slot 1 would point at a missing `textures\pbr` copy), every filled slot names its output path, an empty slot 2 or 6 takes the mapping's stored path else `lock_normal` / `lock_rmaos` is written; an explicit `slotN` or lock still wins. A filled slot no path names (a kept DDS with no job) is locked too (`lock_empty_slots`), so PGPatcher keeps the mesh's own. The slot paths are game paths: an output path gets `textures\` in front when the project root is itself a `textures` folder (`slots_of`), and the entry is left out, with its "mesh keeps" row, when a typed `match1` matches nothing. A set with a PBR map and no filled slot has nothing to write, so `json` also needs at least one filled slot (`finish_view` in `model/sets.rs`; an all-DDS set gets no entry). On a non-CS target such a set stays "PBR, no JSON", badge "PBR set (no JSON on this target)"; on CS a PBR pick with no filled slot has the badge `BADGE_PBR_NOTHING_TO_WRITE`, since the target is not the reason. Tests `target_mod::a_no_albedo_set_without_a_mapping_keys_on_its_base_path`, `a_mapped_no_albedo_set_keys_on_the_mapping`, `pgpsim_patches_the_written_no_albedo_entry`, `the_json_slots_name_where_the_files_were_written`.

## Note HF — Set groups, partial export and textures in no set (builder, 2026-11-08)

ARCHIVED (wording before the edit; superseded in part by Note HS13, 2026-11-10)
- **HF4 Set groups live in `[[groups]]` and the views decide what shows (owner's spec; agent rulings for the code).** `[[groups]]` holds `name`, `sets` (set ids, as `[project] order` does; a set a hand edit lists again, in its own or a later group, is read in its first group only, `SetsDoc::groups`) and `excluded` (left out when false; `SetsDoc::groups`). Membership is kept there whatever the cards do: what a group shows reads `views`, so a set with no card (Note GY5's waiting lone file, or no set) is neither shown nor counted in it, membership never keeps a card, and a group whose members all lack a card has no header (`AppModel::groups`, `group_of`) while its entry stays and the header comes back with a card. A group goes with its last set only when that member leaves the membership (Delete set, Move to group or No group, Ungroup), not when its cards hide. Names are unique ignoring case; Rename refuses a taken one with the reason, and a group's own name in another case is its own. `move_to_group`, `rename_group`, `ungroup` and `set_group_excluded` return `Result` (the read-only reason, or the refusal), are one undo step and do not re-plan. Delete set takes the set out of its group in its own undo step, so undo brings back `no_set`, the entry and the membership together. `rename_set` and `retarget_auto` carry membership, also for an automatic set that is only in a group and has no entry, and for one whose new id already has an entry (the entries stay put; membership still follows). A collapsed group is a `gui.toml` key by project and group name (`group_collapsed`, same key as `match_slots`), stored only while collapsed, with no undo step.

ARCHIVED (wording before the edit; superseded in part by Note JN5, 2026-11-17)
- **HF1 A texture can be in no set; any set can be deleted (owner's words in the set-groups spec; the code calls are agent rulings).** `[project] no_set` in `sdc-project.toml` lists `[files]` keys (`SetsDoc::no_set`, `set_no_set`). Remove member puts the file there in every case: automatic, moved in (its `add` line goes too) and the only member (supersedes the 1.x DU5 refusal; a packed output still refuses). Delete set works on every set: the files that are not packed outputs go to `no_set`, the entry and its order slot go, and the toast reads `Deleted set {id} ({n} textures now in no set)` (`1 texture` for one, no count for an empty entry), one undo step. Dropping a file on a card or on the empty area (`add_member`) and **Back to its automatic set** (item menu, `back_to_automatic_set`) take it out of `no_set`. An older `remove` line keeps meaning a one-file set that exports. A no-set file deleted from disk clears its entry with an Info line (`clear_deleted_lines`, as HE2). Tests `crates/gui/tests/no_set.rs`.

## Note IB — UV checker leaves the GUI; sdc uv and sdc dilate (NIF team for the owner, 2026-11-08)

ARCHIVED (wording before the edit; superseded in part by Note HS10, 2026-11-10)
- **IB11 The output is `<same file name>.png` in a `dilated` folder beside the input (`--out` overrides), same bit depth and channel layout.** The name stays so Tessalith still reads the suffix. An existing output is kept and reported unless `--overwrite`; an output that would replace its own input is refused. Exit codes as `sdc uv` (IB6): 0 all written; 1 an input wrote nothing (no mask, size mismatch, float, no valid texel, exists); 2 no image matches; 3 a write failed. One stdout line per file: path, kind, mask, filled texel count.

## Note HY — bcenc, rgbcx's block encoders in Rust (main team, 2026-11-13)

ARCHIVED (wording before the edit; superseded in part by Note HY9, 2026-11-13)
- **HY1 `bcenc` holds the Rust BC4-family search; the C++ is the reference.** `crates/bcenc` has no dependencies and no `unsafe`. `bcenc::bc4` is the BC4 search that lived in `encoders-sys/tests/support/bc4ref.rs` (moved; behaviour unchanged, branch counters added), and `bc4_encode_block`, `bc5_encode_block` and `bc4_encode_alpha` (BC3's alpha half, with the cut) wrap it. They are proven byte-equal to `encoders_sys` on generated block classes at radii 4, 5 and 10 (`crates/bcenc/tests/bc4.rs`) and, in the `#[ignore]` test, on 20.02 million blocks at radius 5 (57.9 s on 16 threads, 0 mismatches). The vendored C++ stays the specification and is never edited. A `hit!` counter marks each branch and compiles to nothing outside the crate's unit tests; `every_ported_branch_is_taken` fails if a generator stops reaching one.

ARCHIVED (wording before the edit; superseded in part by Note HY13, 2026-11-13)
- **HY1 `bcenc` holds the Rust BC4-family search; the C++ is the reference.** `crates/bcenc` has no dependencies, and `unsafe` only to call its two SSE2 kernels (HY9 for BC1, HY11 for BC4). `bcenc::bc4` is the BC4 search that lived in `encoders-sys/tests/support/bc4ref.rs` (moved; behaviour unchanged, branch counters added), and `bc4_encode_block`, `bc5_encode_block` and `bc4_encode_alpha` (BC3's alpha half, with the cut) wrap it. They are proven byte-equal to `encoders_sys` on generated block classes at radii 4, 5 and 10 (`crates/bcenc/tests/bc4.rs`) and, in the `#[ignore]` test, on 20.02 million blocks at radius 5 (57.9 s on 16 threads, 0 mismatches). The vendored C++ stays the specification and is never edited. A `hit!` counter marks each branch and compiles to nothing outside the crate's unit tests; `every_ported_branch_is_taken` fails if a generator stops reaching one.

ARCHIVED (wording before the edit; superseded in part by Note JI4, 2026-11-15)
- **HY17 `encoder.bc1_search` (`default` | `fast`) picks the BC1 search depth; `fast-preview` sets `fast`.** `default` is rgbcx level 18 with today's bytes; `fast` is HY16's level for BC1, BC3's colour half and the fully opaque blocks of one-bit-alpha BC1 (rgbcx's `encode_bc1_alpha` passes its level only there). Blocks with a transparent texel, BC4, BC5, BC7, BC6H and uncompressed formats ignore it without a warning, as BC1 ignores `bc45_search_radius`; the GUI locks the row there. Not serialised at `default`, so no profile hash, encode-input hash or `encode_version` arm moves; a project or preset that sets `fast` cannot be read by 3.0.0 (`deny_unknown_fields`). `fast-preview` profiles now carry it: their `profile_sha256` pins changed and nothing else did. This is the setting through which the engine reaches a level other than 18 (HY15's port of levels 10 to 15).

## Note HR — seven OPEN items closed (main team, 2026-11-09)

ARCHIVED (wording before the edit; narrowed by Note JA, 2026-11-14)
- **HR6 A resized cutout texture keeps its source's coverage (agent ruling, owner confirmed 2026-11-09).** When `resize::target_dims` changes the size (a `resize` mode or the power-of-two step) and coverage preservation applies, `preprocess_with` records the alpha-test coverage just before the resize (`Work::source_coverage`), and the pipeline fits every level, mip 0 included, to it (`coverage::preserve_coverage_to`); that is also the base the coverage check compares with. Under `if-changed` every output of those sub-types is written again once; only resized cutouts change. A texture that is not resized is unchanged byte for byte, and complex grass and cubemaps keep their path. Supersedes S-mips-8 (closes its OPEN line). The `encode_version` arms of `diffuse`, `pbr_albedo`, `ui_sky` and `skin_albedo` moved to the engine version: the sub-types a built-in layer can make a cutout (the mesh layer and alpha suggestion, `Subtype::alpha_from_mesh`, and the hair shader's diffuse); `complex_grass` is the other mesh sub-type and keeps its own path. A hand-written cutout on any other sub-type that is also resized keeps its old bytes under `if-changed` until that arm moves. No golden, `encode_inputs.json` or `identity.json` entry changed. Tests `coverage::preserve_coverage_to_fits_level_0`, `preserve_coverage_to_at_the_ends`, `ops::a_resize_records_the_source_coverage`, `pipeline::a_resized_cutout_keeps_its_source_coverage`, `encode::a_resized_cutout_export_keeps_the_source_coverage`.

## Note HW — Tessalith 3.0: version 3.0.0 and one licence (owner, 2026-11-11)

ARCHIVED (wording before the edit; narrowed by Note JA, 2026-11-14)
- **HW1 The workspace version is 3.0.0.** The 2.1 work never shipped on its own, so it ships in 3.0 (Note DM8: MAJOR, because v3 removes features). The `encode_version` arms that read `ENGINE_VERSION` (the 2.1.0 and HR6 arms) now name 3.0.0, so their types re-encode once against a 2.0.0 record, as before. `RELEASED_ENGINE_VERSION` stays 2.0.0 until the release commit sets it.

## Note HZ — AVX-512 ISPC target for BC7 and BC6H (main team, 2026-11-13)

ARCHIVED (wording before the edit; narrowed by Note JA, 2026-11-14)
- **HZ2 BC7 and BC6H bytes depend on whether the CPU has AVX-512 (owner, 2026-11-13: "yes that is fine, pinning tests to avx2 is the smart thing to do").** This extends CB3. Quality is equal (PSNR within 0.002 dB). A manifest record names the path in `isa` (`"avx512"`). `isa` is not part of the up-to-date check, so a project moved between CPUs is not re-encoded and one folder may hold both paths' bytes. No `encode_version` arm moves: bytes on every CPU without AVX-512 are unchanged, and a moved arm would re-encode every BC7 and BC6H output on every machine. This is an exception to DM1.

ARCHIVED (wording before the edit; superseded in part by Note JB, 2026-11-14)
- **HZ1 The ISPC objects have a fifth target, `avx512skx-x16`, and its two objects are built with ISPC 1.32.0.** Every other object and both headers are ISPC 1.31.0 builds, and `versions().ispc` stays `"1.31.0"`. ISPC 1.31.0 has a known AVX-512 miscompile (ISPC issue 3891); both compilers' AVX-512 objects wrote the same bytes in the measurement, and the fixed compiler's are shipped. `tools/regen-ispc.ps1` takes the second compiler as `-Ispc512`. ISPC's dispatcher picks the target on a CPU with AVX-512 (seen on a Ryzen 7 9800X3D, which has F, DQ, CD, BW and VL; `IspcTarget::Avx512.supported()` asks for those five). On that CPU it takes 0.61 to 0.81 of the AVX2 time: [docs/research/avx512-ispc.md](../docs/research/avx512-ispc.md). `avx512skx-x8` and `avx512icl-x16` write the same bytes and are not faster, so they are not built.
- **HZ3 Test builds encode on the AVX2 path at most.** The `test-hooks` feature of `encoders-sys` (forwarded by the `test-hooks` features of `engine` and `sdc-gui`; `sdc` reaches it through `engine`) caps `encoders_sys::ispc_target()` at AVX2; `SDC_TEST_AVX512=1` lifts the cap. A release build of `sdc` or `tessalith` has neither the feature nor the variable. A `cargo test --release` or a `cargo build --release` with `--tests`, `--examples` or `--all-targets` does turn the feature on and leaves a capped `sdc.exe` and `tessalith.exe` in the release folder, so `tools/package-release.ps1 -NoBuild` after one would ship a capped build. Two CLI tests (`sdc_encodes_on_avx2_unless_the_cap_is_lifted`, `a_rerun_on_the_other_path_keeps_every_output` in `crates/cli/tests/cli_convert.rs`) deliberately spawn a lifted `sdc convert`, so on an AVX-512 machine a default `cargo test` does encode that fixture on the AVX-512 path. The AVX-512 pins (`level_identity` row 4, `RECORDED_AVX512` in `tests/bc6h.rs`) are checked only with the cap lifted; `tools/gate.ps1` does that in its `avx512` step. `identity.json` never holds an `@avx512` field (FM1).
- **HZ5 HW4 is still open, and one of its four tests now checks more.** `bc6h::shipped_levels_are_byte_identical_to_the_recorded_objects` checked the AVX2 path only and skipped on a CPU without AVX2. It now also checks the forced SSE2, SSE4 and AVX paths against five hashes recorded on an AMD Zen 5 CPU, and a CPU without AVX2 checks its own paths. So if HW4's cause is the CPU vendor, this test can fail there in more rows. `level_identity` already checks its forced rows this way; it gains the AVX-512 row, read only with the cap lifted. The two engine tests encode on the same path as before on every CPU (the cap keeps them on the AVX2 path on a CPU with AVX-512). The new plain-call tests read no pin. The AVX-512 pins were recorded on an AMD Zen 5 CPU only; whether an Intel AVX-512 CPU writes the same bytes is not known.

ARCHIVED (superseded by Note JB2, 2026-11-14)
- **HZ4 The SSE2, SSE4, AVX and AVX2 objects did not change.** `tools/regen-ispc.ps1` printed MATCH for those eight files before the update, and every pin recorded on those paths is unchanged. Only the two dispatcher objects and the two generated headers changed (ISPC 1.31.0 builds; the CPU-detection code in them is the same as before), and two objects were added.

## Note FO — trace-debug fixes 2026-11-04 (main team, 2026-11-04)

ARCHIVED (wording before the edit; superseded in part by Note JI7, 2026-11-15)
- **FO3 Number boxes show a stored value as it is (finding 3).** Ruling V6: `clamp_existing_to_range(false)` on the CS PBR `drag` (every Num and Glint row), the skin card, the complex-material card and the PGPatcher RGB boxes, so drawing never rewrites an out-of-range stored value (a template's `densityRandomization` 300 stays 300). A drag or a typed value is still clamped. shortcut: an AccessKit SetValue skips the range; clamp what is written if that matters. `advanced.rs`, `presets.rs`, `marker_dialog.rs` and `viewport3d.rs` edit values that are not stored project values and keep the default. Test `cs_pbr_ui::a_draw_keeps_an_out_of_range_template_value`.

## Note FS — backlog batch B3: GUI input and keys (main team, 2026-11-04)

ARCHIVED (wording before the edit; superseded in part by Note JI3, 2026-11-15)
- **FS1** The Advanced drawer's Mipmaps and Advanced texture fields rows gate on `AppModel::fix_off` (read-only reason first, else "Wait for the plan to finish"), as the texture settings panel's rows do (FK), so a pick during a pending plan is not dropped as "unchanged" against the stale view. (Backlog T32)

ARCHIVED (wording before the edit; superseded in part by Note JI1, 2026-11-15)
- **FS5** The Preview key actions (`]`, `[`, 1-6, L, T...) are live only while the Selection is in front of the centre (`centre_front() == Selection`), as the Grid and Sets keys are (FB7); a docked Presets or JSON tab owns the keys. (Backlog T22)

## Note HI — Texture panel tidy (main team, 2026-11-09)

ARCHIVED (wording before the edit; superseded in part by Note JI3, 2026-11-15)
- **HI1 One label column, one control column (owner).** Every texture-panel row draws its label in a `LABEL_W` (150 px) cell, cut short with the whole text on hover, and its control in a `CONTROL_W` (200 px) cell (`settings_panel::field_label`, `control_slot`, `control_cell`). Where a grid is narrower than both columns plus ↺, a hint and a margin (the centre at 1100 px with the Advanced drawer open), `column_widths` scales both down alike, so no row runs under the drawer (rule 3.1; agent ruling, controller-approved. Cost if wrong: controls about 190 px at 1100, and narrower with the Advanced drawer open). A combo with no name of its own is named after its row (`labelled_by`), keeping its value. The Mipmaps tick sits in the control column, named "Mipmaps", with the level count or provenance in the hint column. The helper panel's rows and the cubemap rows use the same two widths; the helper's controls also give up the width of the Reset beside them where the row would not fit (`column_widths_beside`, agent ruling). Test `crates/gui/tests/texture_panel_layout.rs`.

ARCHIVED (wording before the edit; superseded in part by Note JI3, 2026-11-15)
- **HI3 The preview strip's controls are `COMPACT_CONTROL_W` (140 px) (agent ruling, owner to confirm).** Its labels take the room left (cut short, whole text on hover), so no control ends past the 280 px strip. The Advanced drawer's rows keep their content widths and their unnamed combos.

ARCHIVED (wording before the edit; superseded in part by Note JI5, 2026-11-15)
- **HI7 The alpha question has a Confirm button (owner).** `settings_panel::alpha_question` draws the dropdown on the proposal and **Confirm**, which answers with the shown type (the `Answer::Alpha` a pick gives); picking from the list still answers at once (FM35), and Enter on a Problems row is unchanged. Confirm's accessible name is "Confirm alpha" after the surface ("Problems confirm alpha"), so the sub-type's "Confirm" stays unique; it waits for the plan as the dropdown does.

ARCHIVED (wording before the edit; superseded in part by Note JI3, 2026-11-15)
- **HI9 A `?` and a tooltip on every settings row (owner).** Every `FIELDS` and `ADVANCED_FIELDS` row, Levels, Filter, Edge, Straight alpha, the Mipmaps tick and the Helper tick (texture and helper panels) has help: `fields::field_help` (or `MIPMAPS_HELP`, `HELPER_HELP`), shown on the label's hover, also while the row is off (`edit_off`, a lock). In the texture panel, the 3D settings window, the bulk panel and the helper panel's Helper row the label cell also ends with a weak `?` named "Help: {label}" with the same hover (not in the strip or the Advanced drawer, HI18). Aligned, the `?`s line up at the label column's right edge; in the bulk panel the `?` follows the label. The checkbox hovers (parallax, BC1 alpha, Perceptual, EXR straight) stay on the control and are those rows' help too.

ARCHIVED (wording before the edit; superseded in part by Note JI3, 2026-11-15)
- **HI14 While the alpha is asked, every row reads the proposal (agent ruling).** `draw` sets the proposal on its own copy of the job, so the Alpha row, Threshold, Comparison, Coverage and the filter locks show what Confirm will give, and the Alpha row's hint is "proposed: confirm above". Nothing is written, the document stays clean and the export gate still counts the texture undecided until an answer; a pick on the Alpha row is an answer, and picking the shown value there writes nothing (Confirm accepts it). The Advanced drawer shows the unconfirmed profile.

ARCHIVED (wording before the edit; superseded in part by Note JI3 and Note JI7, 2026-11-15)
- **HI18 The preview's settings strip and the Advanced drawer show no `?` (agent ruling).** The strip's labels are about 90 px and already cut short, and in the drawer at 280 px a `?` pushed every combo about 4 px past its edge; there the label's hover carries the help (with the full label where it is cut short).

## Note HV — analysis cache: compressed thumbnails and a limit the user sets (owner, 2026-11-10)

ARCHIVED (wording before the edit; superseded in part by Note JI1, 2026-11-15)
- **HV2 The cache limit is a GUI setting, 500 MB unless changed (owner).** `Session::with_disk_cache(dir, max_bytes)` takes the limit, and the GUI passes `GuiSettings::analysis_cache_bytes()`: `analysis_cache_mb` in `gui.toml`, 500 when unset (an unset limit is not written), one MB being 1024 × 1024 bytes. **Advanced → Cache** has one number box, *Analysis cache limit*, from 50 to 5,000 MB (agent ruling: the prune walks the whole folder before the window opens, and 5,000 MB is about 150,000 entries; its time at that size is not measured). A whole number typed into `gui.toml` outside that range is used and shown as it is, and drawing, entering or leaving the box does not change it; 0 keeps nothing between runs. Any other value is a file that does not load, as for every key. The prune still runs once, at start (DZ8), so a new limit applies the next time the app starts and the box's hover says so (agent ruling: lowering the limit deletes nothing while the app runs). It replaces DZ8's fixed 200 MB; HR7 is unchanged. With HV1, 500 MB holds about 15,000 entries, where 200 MB held about 3,100. Tests `analysis_disk_cache::a_session_prunes_to_the_given_size_at_start`, `app_no_local_store::with_analysis_cache_stores_under_the_given_folder_only`, `settings::the_analysis_cache_limit_is_500_mb_until_set`, `a_hand_edited_cache_limit_survives_the_next_save`, `advanced::the_cache_limit_box_saves_a_typed_limit`.

## Note JF — Review's file count and the Acknowledge offer (owner, 2026-11-15)

ARCHIVED (wording before the edit; superseded in part by Note JI1, 2026-11-15)
- **JF3 Acknowledge is offered only where it ends the block (owner, 2026-11-15: "3 yes").** The engine ignores `allow` for `FMT-CS-SKIN-LOADER`, and for `CFG-UNAVAILABLE` it only accepts a fallback sub-type (`policy.rs`, the `--allow` loop of `resolve`), so the GUI's Acknowledge wrote `allow` and the question stayed. `items::ack_clears` decides it: never for `FMT-CS-SKIN-LOADER`; for `CFG-UNAVAILABLE` only when `policy::availability` names a fallback (`pbr_albedo`, `pbr_emissive`, `pbr_normal` outside cs), where Acknowledge converts the file as the fallback and stays. Any other blocking code keeps Acknowledge. A block Acknowledge cannot clear is `Decision::Change(code)`, shown in Problems, in Review's Undecided column and in the texture panel's Undecided box:
  - `FMT-CS-SKIN-LOADER`: no button, and in practice no row. Note JG3 resets a format the type does not allow on every settled plan of a writable project, and every pair `format_rule::allowed` lists for `skin_rfaos`, `skin_wet` and `skin_detail` is one Community Shaders loads, so the question does not stay in the GUI. Where JG3 leaves a pair (a layer that still overrides the pin), the row has its sentence and no Acknowledge. A **Use BC7** button the owner first accepted ("3 yes") was dropped when this note met JG at the merge (agent ruling, 2026-11-15): the reset made it unreachable.
  - `CFG-UNAVAILABLE` with no fallback: the type picker (**Set type…** on a row, the sub-type dropdown and **Confirm** in the panel), and the sentence "This type isn't available in this mode — set another type, or change the mode in Presets…" (`code_text::change_line`). When the texture already asks for its type, that question is the answer and no second one is added.
  - The engine is unchanged, and `sdc.toml` files that hold such an `allow` entry are read as before. Tests in `crates/gui/tests/ack_offer.rs`.

ARCHIVED (wording before the edit; superseded in part by Note JO, 2026-11-17)
- **JF4 The `Kinds ▾` hover says what a hidden row still does.** It said "Export's review still lists every warning"; since FM24 Review lists none. It now ends "A hidden row still counts in the tab's number and in the ready counts", which is HC4's rule; the wiki line in GUI-Problems.md says the same. HC4's reason ("Review's safety rules read every warning") predates FM24 and is not the reason any more: the counts ignore the hiding because a hidden row is not fixed. Test `bottom_panel::the_kinds_hover_says_what_a_hidden_row_still_does`.

## Note JA — if-changed keeps outputs of an older encode version or a lower target; --ev re-encodes them (main team, 2026-11-14)

ARCHIVED (wording before the edit; superseded in part by Note JL6, 2026-11-16)
- **JA12 Re-encode every output is a one-shot item in Export to ▾ (controller ruling, Task 8 review).** It is a checkable item in the header's **Export to ▾** menu, below the Also export ticks, not a header checkbox: every user sees the default 1400 pt window at first launch, and a box in the header moved CD2's one-row threshold from about 1280 pt to about 1450 pt, so that window wrapped. The menu belongs to Export, which meets "beside Export"; the cost is one click to reach it. The header keeps CD1's Save/Export grouping and CD2's 1280 pt threshold. The item is off at start, never saved, greyed with no folder open, and the menu stays open on a tick (egui 0.36's `CloseOnClickOutside`, as for the Also export ticks). It turns itself off when an export pass starts with it ticked (one-shot, any export kind: **Export**, **Export set**, **Export group**), so a tick hidden in the closed menu never carries over to a later export. A tick during a running export applies to the next one.

## Note JI — Settings tab, and no Advanced drawer (owner, 2026-11-15)

ARCHIVED (wording before the edit; superseded in part by Note JO, 2026-11-17)
- **JI6 Unmatched `[files]` entries are a Problems row.** `CFG-FILE-UNMATCHED` entries from the last full plan are one Info row at the project, "{n} saved settings name no file in this mode ({mode})" (n = 1: "1 saved setting names no file in this mode ({mode})"), with every entry on hover, an `Entries ▾` menu that lists each entry still in `[files]` with its own **Remove** (accessible `Remove {key}`), and **Remove all**. Unmatched means in the mode of that plan (`model.target`), since some planning is cs-only. Like any Info row it counts in the tab number and takes nothing from "ready".
  - **Gates.** Each Remove waits on `fix_off()`, as every Problems row's fix does. Remove all is on when `project.target().is_none_or(|t| t == model.target)`, no full plan is pending and no partial plan is out, and its hover lists what it deletes. One entry is one step "Remove entry"; Remove all is one step "Remove {n} entries". While no entry is left in `[files]` (a read-only `sdc.toml`'s empty document), `Entries ▾` is off too, with Remove all's reason on hover.
  - **Code.** The Kinds ▾ name of the row is "Unmatched [files] entries" (FB8: no raw codes). `ProjectDoc::target()` reads the raw `target` key without a full parse, since the row asks every frame. `target_name` moved to `model/code_text.rs`.
  - It replaces the drawer's last section. Amends nothing: no live ruling described that section (FM16's drawer wording is edited under JI7). Tests in `crates/gui/tests/problems_rows.rs` (`unmatched_entries_show_as_one_info_row` and the tests after it).

## Note JO — Problems and Notes, Mark as intended, readiness (review E, plan 3a; controller for the owner, 2026-11-16)

ARCHIVED (wording before the edit; superseded in part by Note JP, 2026-11-17)
  - **A set with no files yet** (a created set with an empty key) reads "Not ready: this set has no files yet." with the amber dot and no reason buttons, as its card's hover does (`sets_column::NO_FILES_YET`); no row of another key-less set counts for it (R3).

ARCHIVED (wording before the edit; superseded in part by Note JP, 2026-11-17)
  - GRASS-DETECT-SOURCE has no button (the engine raises it only on a texture already tagged complex grass; overrules the plan table's **Tag as grass**, ledger) and stays a Problem, so **Mark as intended** clears it (final fix wave). Its sentence follows the job (`bottom::warning_text`, final fix wave A3): with the job's `mips.normal_base` renormalising, the row reads "This grass source's corner normal isn't unit length — the written file is renormalised, so Community Shaders still reads it as grass"; with `mips.normal_base = keep` the file is written as it is and the row reads "This grass source's corner normal isn't unit length, and its normal base level is kept as authored — Community Shaders won't read the written file as grass" (`code_text::GRASS_SOURCE_KEPT`). The row, its mark and the tile's count read the same sentence.

## Note JE — Advanced Skin re-types untouched recorded members (owner, 2026-11-15)

ARCHIVED (wording before the edit; superseded in part by Note JQ, 2026-11-18)
- **JE2 An untouched recorded `pbr_fuzz` is cleared.** In the same step `untouched_for_shader` clears it, so the `_f` packs into the `_rfaos` G channel as the skin fuzz mask (DR1, DR2) and is not exported alone; `sdc convert` on the saved text plans the same. Leaving Advanced Skin: the next settled plan records `pbr_fuzz` again (FC1). A pinned diffuse keeps the set from turning skin (DW1), so then nothing is cleared. shortcut: a set with a `_fuzz` or `_sheen` map keeps its `_f` as `pbr_fuzz` (DR7), so there the clear is followed by a recording step of its own; check the set's raw maps in `untouched_for_shader` if that shows up. Test `auto_accept::advanced_skin_packs_an_untouched_recorded_fuzz_into_rfaos`.
