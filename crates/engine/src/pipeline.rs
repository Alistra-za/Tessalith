//! One `ResolvedJob` end to end (spec §5, carry order of Task 31):
//! load → preprocess → mips → coverage → quantise → encode → DDS bytes → validate the bytes → atomic write →
//! manifest record.
//!
//! The caller (`api::Engine::run`) has initialised the encoders with the batch BC1 model, admitted the file
//! against the memory budget and runs this on its rayon pool under `catch_unwind`; the encode stage's band
//! parallelism runs on the same pool. Nothing here depends on the thread count: every encoded block is a pure
//! function of its own texels, so the written bytes are identical for any `--jobs`.
use crate::api::{BatchRequest, CancelToken, LowDetailMode};
use crate::bake::NormalVariance;
use crate::config::{Format, Profile, Subtype};
use crate::coverage::{coverage_fraction, preserve_coverage, preserve_coverage_to};
use crate::dds::{dds_bytes, dds_bytes_cube, resolve_header, write_atomic, DdsDesc};
use crate::encode::{encode_level, EncodeCtx};
use crate::events::{EventKind, Reporter, Stage};
use crate::image::Rgba32F;
use crate::load::load_image_as;
use crate::manifest::{
    dispatched_isa, engine_commit, now_rfc3339, output_key, profile_sha256, recorded_sources,
    sha256_file, EncoderVersions, Manifest, ManifestRecord, PresetRef, BC1_ENCODE_ID,
    ENGINE_VERSION, MANIFEST_SCHEMA,
};
use crate::mips::build_chain_with;
use crate::ops::{preprocess_with, Domain, SourceTransfer};
use crate::pack::assemble::assemble;
use crate::policy::{ResolvedJob, SourceStamp};
use crate::quantize::{to_encoder_input, EncoderInput};
use crate::session::first_level_within;
use crate::validate::{validate, ValidationInput, ValidationReport};
use std::cell::Cell;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub enum FileError {
    /// Stopped by the cancel token before anything was written.
    Cancelled,
    Failed(Stage, String),
}

#[derive(Debug)]
pub struct FileOutcome {
    pub output: PathBuf,
    pub report: ValidationReport,
}

/// What the caller needs when `process_file` fails or panics: the last stage the file entered (the stage a panic
/// is reported at) and whether its output has been written (so the caller removes it; see `process_file`).
#[derive(Debug)]
pub struct FileTracker {
    pub stage: Cell<Stage>,
    pub written: Cell<bool>,
}

impl Default for FileTracker {
    fn default() -> Self {
        FileTracker {
            stage: Cell::new(Stage::Load),
            written: Cell::new(false),
        }
    }
}

fn fail(stage: Stage) -> impl Fn(String) -> FileError {
    move |e| FileError::Failed(stage, e)
}

/// The coverage step (spec §5.5), when coverage preservation applies: a stored alpha channel and `mips.coverage` on
/// (by default for `cutout` and `cutout_blend`). Returns the reference coverage, `Prepared::base_coverage`. A resized
/// texture is fitted, mip 0 included, to `source_coverage`, its coverage before the resize (Note HR6); otherwise
/// mip 0 is the reference and is left alone. The same predicate gates the coverage check in `validate`, so a
/// `remove`d channel (decoding to all-255) is never compared.
fn apply_coverage(
    chain: &mut [Rgba32F],
    p: &Profile,
    alpha_present: bool,
    source_coverage: Option<f64>,
) -> Option<f64> {
    if !(alpha_present && p.coverage_enabled()) {
        return None;
    }
    let (threshold, compare) = (p.alpha.threshold, p.alpha.compare);
    match source_coverage {
        Some(src) => {
            preserve_coverage_to(chain, src, threshold, compare);
            Some(src)
        }
        None => {
            let base =
                coverage_fraction(chain.first()?.px.iter().map(|q| q[3]), threshold, compare);
            preserve_coverage(chain, threshold, compare);
            Some(base)
        }
    }
}

/// Peak Rec. 709 luminance of the HDR base level as the encoder sees it (channels clamped to `[0, 65504]`,
/// matching the clamp in `quantize`), for the BC6H peak check. `None` for other formats.
fn source_peak(level: &Rgba32F, p: &Profile) -> Option<f32> {
    (p.format == Format::Bc6hUf16).then(|| {
        let c = crate::quantize::clamp_half;
        level
            .px
            .iter()
            .map(|q| 0.2126 * c(q[0]) + 0.7152 * c(q[1]) + 0.0722 * c(q[2]))
            .fold(0.0f32, f32::max)
    })
}

/// Test hook (Task 44): when `SDC_TEST_FAIL_VALIDATION` names the input's file stem, add one `VAL-HEADER` error to
/// its validation report, so a written output fails validation (exit 4). No setting reaches an error-severity
/// `VAL-*` code, so the exit-4 path has no other deterministic trigger. Compiled only with the `test-hooks` cargo
/// feature (off in release builds).
#[cfg(feature = "test-hooks")]
fn force_validation_failure_for_test(job: &ResolvedJob, report: &mut ValidationReport) {
    if std::env::var_os("SDC_TEST_FAIL_VALIDATION")
        .is_some_and(|v| job.input.file_stem() == Some(v.as_os_str()))
    {
        report.warnings.push(crate::codes::Warning::new(
            crate::codes::Code::ValHeader,
            "test hook: SDC_TEST_FAIL_VALIDATION",
        ));
        report.summary.passed = false;
    }
}

/// Test hook (Note DM): SHA-256 of everything that decides a file's encoded bytes and validation verdict, so the
/// encode-version guard pins it per sub-type without depending on ISA-specific encoder output. Covers the header,
/// each level's quantised encoder input (length-prefixed), the resolved profile and the verdict.
#[cfg(feature = "test-hooks")]
fn encode_input_sha256(
    header: &[u8],
    inputs: &[EncoderInput],
    p: &Profile,
    passed: bool,
) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(header);
    for level in inputs {
        let bytes: Vec<u8> = match level {
            EncoderInput::Ldr(px) => px.iter().flatten().copied().collect(),
            EncoderInput::Hdr(px) => px.iter().flatten().flat_map(|v| v.to_le_bytes()).collect(),
        };
        h.update((bytes.len() as u64).to_le_bytes());
        h.update(&bytes);
    }
    h.update(crate::manifest::sha256_json(p).expect("profile json"));
    h.update([passed as u8]);
    hex::encode(h.finalize())
}

/// What `prepare` keeps of the chain (M3a spec §6.5). `Encode` is the export: every mip is quantised, none of the
/// float chain is kept. `Preview` and `SourceOnly` start at the first mip within `max_size`; `SourceOnly` stops
/// before quantising.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Keep {
    Encode,
    Preview {
        max_size: u32,
    },
    /// `face` picks which of a six-face cube's processed chains is kept (0 for everything else).
    SourceOnly {
        max_size: u32,
        face: usize,
    },
}

/// The stages up to quantise. `chain` and `inputs` start at mip `first_level`; `dims` covers every mip. `chain` is
/// empty for `Keep::Encode`, `inputs` for `Keep::SourceOnly`.
pub(crate) struct Prepared {
    pub first_level: usize,
    pub dims: Vec<(u32, u32)>,
    pub chain: Vec<Rgba32F>,
    /// What `chain`'s values mean (the working domain, not the stored values).
    pub domain: Domain,
    pub inputs: Vec<EncoderInput>,
    pub base_coverage: Option<f64>,
    pub source_peak: Option<f32>,
    /// Check 3's ratio, when measuring was asked for and the texture is eligible.
    pub detail_ratio: Option<f64>,
    /// 6 for an encode or measure of a cube job (`dims` and `inputs` face-major), else 1.
    pub faces: usize,
}

/// Source stamps (final review I7): the plan's decisions (alpha policy, sub-type facts, ...) hold only for the
/// file it analysed; a packed job checks every source, and a baking roughness member its `_n` too (M4 carry).
fn check_stamps(job: &ResolvedJob) -> Result<(), FileError> {
    let mut stamps: Vec<(&Path, Option<SourceStamp>)> = match (&job.pack, &job.derive) {
        (Some(pk), _) => pk
            .sources
            .iter()
            .map(|s| (s.path.as_path(), s.stamp))
            .collect(),
        (None, Some(dj)) => dj
            .sources
            .iter()
            .map(|s| (s.path.as_path(), s.stamp))
            .collect(),
        (None, None) => vec![(job.input.as_path(), job.source)],
    };
    // DECISIONS DS8: a six-file cube checks every face (its first is `job.input`, already in the list).
    let faces = job.cube.as_ref().filter(|c| c.sources.len() > 1);
    for s in faces.iter().flat_map(|c| c.sources.iter().skip(1)) {
        stamps.push((s.path.as_path(), s.stamp));
    }
    if let Some(b) = &job.bake {
        stamps.push((b.normal.as_path(), b.stamp));
    }
    if let Some(f) = &job.fill {
        stamps.push((f.source.path.as_path(), f.source.stamp));
    }
    let named = job.pack.is_some()
        || job.derive.is_some()
        || faces.is_some()
        || job.bake.is_some()
        || job.fill.is_some();
    for (src, planned) in stamps {
        if let Some(planned) = planned {
            if SourceStamp::of(src) != Some(planned) {
                let what = if named {
                    format!("{}: ", src.display())
                } else {
                    String::new()
                };
                return Err(fail(Stage::Load)(format!(
                    "{what}source changed since planning; re-run"
                )));
            }
        }
    }
    Ok(())
}

/// Stamp check → load → preprocess → mips → coverage → quantise, shared by the export and the GUI preview.
pub(crate) fn prepare(
    job: &ResolvedJob,
    cancel: &CancelToken,
    tracker: &FileTracker,
    keep: Keep,
) -> Result<Prepared, FileError> {
    prepare_then(job, cancel, tracker, keep, false, || Ok(())).map(|(prep, ())| prep)
}

/// `prepare`, running `after_stamps` once the stamp check has passed and before anything is decoded (the export
/// hashes its sources there, so a source that cannot be hashed still fails before it is loaded). `detail`
/// measures check 3's ratio on an eligible chain.
fn prepare_then<T>(
    job: &ResolvedJob,
    cancel: &CancelToken,
    tracker: &FileTracker,
    keep: Keep,
    detail: bool,
    after_stamps: impl FnOnce() -> Result<T, FileError>,
) -> Result<(Prepared, T), FileError> {
    let enter = |st: Stage| tracker.stage.set(st);
    let p = &job.profile;
    let stop = || {
        if cancel.is_cancelled() {
            Err(FileError::Cancelled)
        } else {
            Ok(())
        }
    };
    enter(Stage::Load);
    check_stamps(job)?;
    let extra = after_stamps()?;
    // DECISIONS DS10: a cube job builds its faces and mips in direction space.
    if let Some(cj) = &job.cube {
        return Ok((
            crate::cube::run::prepare(job, cj, keep, cancel, tracker)?,
            extra,
        ));
    }
    // Spec §6.2–6.3 (DECISIONS R4): a roughness member decodes its `_n` first and keeps only the footprint lengths,
    // so the two images are never held together.
    let lengths = match &job.bake {
        // `load_image`'s errors already name the `_n`.
        Some(b) => Some(b.load_lengths().map_err(fail(Stage::Load))?),
        None => None,
    };
    stop()?;
    let (mut img, mut info) = match (&job.pack, &job.derive) {
        (Some(pk), _) => {
            // The gray source fixed at plan time (Note M6), not `p.channels.grayscale_source` of the final profile.
            let asm = assemble(pk, pk.gray).map_err(|e| fail(Stage::Load)(e.to_string()))?;
            (asm.image, asm.info)
        }
        // Note CN11: the recipe's image, encoded as any other.
        (None, Some(dj)) => {
            let asm = crate::derive::assemble::assemble_derived(dj)
                .map_err(|e| fail(Stage::Load)(e.to_string()))?;
            (asm.image, asm.info)
        }
        (None, None) => load_image_as(&job.input, p.alpha.exr_straight)
            .map_err(|e| fail(Stage::Load)(e.to_string()))?,
    };
    // Note DY: the helper's channel becomes the alpha, as the plan analysed it (Ruling 11: a cube returned above).
    if let Some(f) = &job.fill {
        crate::helper::apply_fill(&mut img, &mut info, f).map_err(fail(Stage::Load))?;
    }
    let transfer = SourceTransfer::of(&info);
    if crate::ops::complex_alpha_zero(p, info.has_alpha) {
        // Constant, so every mip stays 0.
        for px in &mut img.px {
            px[3] = 0.0;
        }
    }
    let bake = match (lengths, &job.bake) {
        (Some(l), Some(b)) => {
            let member = crate::resize::target_dims((img.width, img.height), p);
            let nv = NormalVariance::new(l, member, p.mips.bake_clamp).map_err(|e| {
                fail(Stage::Load)(format!("variance bake with {}: {e}", b.normal.display()))
            })?;
            if nv.shift() != b.shift {
                return Err(fail(Stage::Load)(format!(
                    "{}: size changed since planning; re-run",
                    b.normal.display()
                )));
            }
            Some(nv)
        }
        _ => None,
    };
    stop()?;

    enter(Stage::Preprocess);
    // Complex grass builds each half's chain on its own (design §3.4); the stacked chain comes with its Work.
    let grass = p.subtype == Subtype::ComplexGrass;
    let (mut work, grass_chain) = if grass {
        let (w, c) = crate::grass::build(img, p, transfer).map_err(fail(Stage::Preprocess))?;
        (w, Some(c))
    } else {
        (preprocess_with(img, p, transfer), None)
    };
    stop()?;

    enter(Stage::Mipmaps);
    let mut chain = match grass_chain {
        Some(c) => c,
        None => build_chain_with(&work, p, bake.as_ref()),
    };
    drop(bake);
    // Audit 21: mip 0 is in `chain` now; nothing below reads `work.img` (`to_encoder_input` reads only
    // `alpha_present` and `domain`), so free the source-sized copy before quantise, the peak.
    work.img = Rgba32F::new(0, 0, [0.0; 4]);
    let alpha_present = work.alpha_present;
    // Spec §5.5 (Task 30): coverage runs after the mips and before quantise; for `cutout_blend` it rewrites the
    // only alpha channel. Complex grass preserved its top half already; its bottom half's alpha is specular data,
    // so coverage measured on the stacked image is not checked either (Note BC, audit #4).
    let base_coverage = if grass {
        None
    } else {
        apply_coverage(&mut chain, p, alpha_present, work.source_coverage)
    };
    // Check 3 runs on the finished chain. An eligible job has `resize = none`, so `chain[0]` is the power-of-two size.
    let detail_ratio = (detail
        && crate::checks::low_detail_eligible(p, (chain[0].width, chain[0].height)))
    .then(|| crate::checks::detail_ratio(&chain))
    .flatten();
    let dims: Vec<(u32, u32)> = chain.iter().map(|l| (l.width, l.height)).collect();
    let first_level = match keep {
        Keep::Encode => 0,
        Keep::Preview { max_size } | Keep::SourceOnly { max_size, .. } => {
            first_level_within(&dims, max_size)
        }
    };
    // The BC6H peak check compares the first decoded level with this peak, so it is measured on the first level kept
    // (mip 0 for an export). `base_coverage` stays the reference (mip 0's, or the source's after a resize, Note HR6):
    // coverage is compared with it on every level.
    let source_peak = source_peak(&chain[first_level], p);
    // Quantise clamps mip 0 (Task 31). These exact levels are encoded and handed to Validate (Task 35).
    let inputs: Vec<EncoderInput> = match keep {
        Keep::SourceOnly { .. } => vec![],
        _ => chain[first_level..]
            .iter()
            .map(|l| to_encoder_input(l, &work, p))
            .collect(),
    };
    let chain = match keep {
        Keep::Encode => {
            drop(chain);
            vec![]
        }
        _ => chain.split_off(first_level),
    };
    let domain = work.domain;
    drop(work);
    stop()?;
    Ok((
        Prepared {
            first_level,
            dims,
            chain,
            domain,
            inputs,
            base_coverage,
            source_peak,
            detail_ratio,
            faces: 1,
        },
        extra,
    ))
}

/// Encode `prep.inputs[i]` at `prep.dims[prep.first_level + i]`, emitting a `Compress` progress event per mip
/// (`done` 1..=n, carrying `job_key`).
pub(crate) fn encode_levels(
    prep: &Prepared,
    p: &Profile,
    cancel: &CancelToken,
    rep: &Reporter,
    job_key: &str,
    set: Option<&str>,
    path: Option<&Path>,
) -> Result<Vec<Vec<u8>>, FileError> {
    let ectx = EncodeCtx {
        cancel: Some(cancel.flag()),
    };
    let total = prep.inputs.len() as u32;
    let mut levels = Vec::with_capacity(prep.inputs.len());
    for (i, (inp, &(w, h))) in prep
        .inputs
        .iter()
        // A six-face preview repeats one chain's dims per face.
        .zip(prep.dims[prep.first_level..].iter().cycle())
        .enumerate()
    {
        let level = encode_level(inp, w, h, p, &ectx).map_err(|_| FileError::Cancelled)?;
        levels.push(level);
        rep.emit_job(
            Some(job_key),
            set,
            path,
            EventKind::Progress {
                stage: Stage::Compress,
                done: i as u32 + 1,
                total,
            },
        );
    }
    Ok(levels)
}

/// Convert one file and append its manifest record. Cancellation is checked between stages and between encode
/// bands, last just before the write, so a cancelled file never leaves an output behind. Everything that can fail
/// before the output exists (validation of the in-memory DDS bytes, the profile hash, the record)
/// runs before the write; only the manifest append comes after it. When that fails,
/// or anything panics, `tracker.written` tells the caller to remove the output, so an output on disk has its
/// record, except after a second Ctrl-C: that aborts the process (exit 3) and can leave `.sdc-*.tmp` files, or an
/// output without a record (`OUT-UNKNOWN-EXISTING` on the next run). A failed validation (`passed = false`) is not a failure here: the output is written and recorded.
///
/// Check 3 (`req.low_detail`): a flagged texture is written in full and carries the warning in its report under
/// `Warn`.
#[allow(clippy::too_many_arguments)]
pub fn process_file(
    job: &ResolvedJob,
    req: &BatchRequest,
    preset: &PresetRef,
    cancel: &CancelToken,
    rep: &Reporter,
    manifest: &Manifest,
    tracker: &FileTracker,
) -> Result<FileOutcome, FileError> {
    let enter = |st: Stage| tracker.stage.set(st);
    enter(Stage::Load);
    // Test hook (Task 44, `test-hooks` feature only): panic on the named input file to prove per-file panic isolation.
    #[cfg(feature = "test-hooks")]
    if std::env::var_os("SDC_TEST_PANIC_ON")
        .is_some_and(|v| job.input.file_name() == Some(v.as_os_str()))
    {
        panic!("test hook: SDC_TEST_PANIC_ON");
    }
    let p = &job.profile;
    let set = Some(job.set_key.as_str());
    let path = Some(job.input.as_path());
    let key = output_key(&req.output_root, &job.output);
    let stop = || {
        if cancel.is_cancelled() {
            Err(FileError::Cancelled)
        } else {
            Ok(())
        }
    };

    // Only a ratio below the threshold acts; its warning names the base size.
    let flagged = |prep: &Prepared| {
        prep.detail_ratio
            .filter(|r| crate::checks::is_low_detail(*r))
            .map(|r| {
                // dims[0] is the source size only because an eligible job has resize none and no dropped mip.
                let (w, h) = prep.dims[0];
                crate::codes::Warning::new(
                    crate::codes::Code::ChkLowDetail,
                    format!(
                        "mip 0 adds little detail over mip 1 ({r:.2}); a {w}x{h} texture carries about {}x{} of detail",
                        w / 2,
                        h / 2
                    ),
                )
                .with_data(serde_json::json!({ "width": w, "height": h, "ratio": r }))
            })
    };
    let detail = req.low_detail != LowDetailMode::Off;
    // The hashes sit between the stamp check and the load, as they always have: a source that cannot be hashed
    // fails here, before it is decoded.
    let (prep, (source_sha256, sources)) =
        prepare_then(job, cancel, tracker, Keep::Encode, detail, || {
            let source_sha256 =
                sha256_file(&job.input).map_err(|e| fail(Stage::Load)(e.to_string()))?;
            let sources = recorded_sources(job).map_err(fail(Stage::Load))?;
            Ok((source_sha256, sources))
        })?;
    let low_detail = flagged(&prep);
    let (width, height) = prep.dims[0];

    enter(Stage::Compress);
    let levels = encode_levels(&prep, p, cancel, rep, &key, set, path)?;
    // Mips per face: the header's count.
    let total = (prep.inputs.len() / prep.faces) as u32;

    enter(Stage::Write);
    let srgb = p.srgb_tag();
    let desc = DdsDesc {
        width,
        height,
        format: p.format,
        srgb,
        header: resolve_header(p.format, srgb, p.header),
    };
    let bytes = if prep.faces == 6 {
        dds_bytes_cube(&desc, &levels)
    } else {
        dds_bytes(&desc, &levels)
    }
    .map_err(|e| fail(Stage::Write)(e.to_string()))?;
    drop(levels);

    // Validate the exact bytes about to be written: `validate` parses them with the engine's DDS reader (spec §5.8).
    enter(Stage::Validate);
    #[cfg_attr(not(feature = "test-hooks"), allow(unused_mut))]
    // only the test hook mutates the report
    let mut report = validate(&ValidationInput {
        bytes: &bytes,
        profile: p,
        encoder_input: Some(&prep.inputs),
        base_coverage: prep.base_coverage,
        source_peak: prep.source_peak,
        expected_levels: Some(total),
    });
    // Test hook (Note DM, `test-hooks` only): record this output's encode-input hash for the encode-version guard.
    #[cfg(feature = "test-hooks")]
    if let Some(dir) = std::env::var_os("SDC_TEST_ENCODE_INPUT_DIR") {
        let header = crate::dds::header_bytes_with(&desc, total, prep.faces == 6).expect("header");
        let sha256 = encode_input_sha256(&header, &prep.inputs, p, report.summary.passed);
        let json = serde_json::json!({ "subtype": p.subtype.id(), "sha256": sha256 });
        let file = Path::new(&dir).join(format!("{}.json", key.replace('/', "~")));
        std::fs::write(file, json.to_string()).expect("encode input hook");
    }
    drop(prep);
    #[cfg(feature = "test-hooks")]
    force_validation_failure_for_test(job, &mut report);
    enter(Stage::Write);
    let mut record = ManifestRecord {
        schema: MANIFEST_SCHEMA,
        ts: now_rfc3339(),
        engine_version: ENGINE_VERSION.into(),
        encode_version: Some(crate::manifest::encode_version(p.subtype).into()),
        engine_commit: engine_commit().into(),
        encoders: EncoderVersions::current(),
        isa: dispatched_isa().into(),
        preset: preset.clone(),
        profile: p.clone(),
        profile_sha256: profile_sha256(p).map_err(|e| fail(Stage::Write)(e.to_string()))?,
        provenance: job.provenance.clone(),
        source: job.input.to_string_lossy().replace('\\', "/"),
        source_sha256,
        sources,
        output: output_key(&req.output_root, &job.output),
        validation: report.summary.clone(),
        bc1_model: Some(BC1_ENCODE_ID.into()),
        shader: job.shader.clone(),
        output_stamp: None,
        copied_from: None,
    };
    stop()?;

    write_atomic(&job.output, &bytes).map_err(|e| fail(Stage::Write)(e.to_string()))?;
    tracker.written.set(true);
    drop(bytes);
    // Note Y1: taken after the rename, so `if-changed` notices a replaced output. Any failure from here on removes
    // the output (`run_one`, `tracker.written`).
    record.output_stamp = SourceStamp::of(&job.output);
    manifest
        .append(&record)
        .map_err(|e| fail(Stage::Write)(format!("manifest {}: {e}", manifest.path().display())))?;
    // `Warn`: listed with the file's warnings only; the record and its validation summary do not carry it.
    report.warnings.extend(low_detail);
    Ok(FileOutcome {
        output: job.output.clone(),
        report,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AlphaCompare, ResizeMode};
    use crate::coverage::passes;
    use crate::ops::tests::{cutout_profile, sparse_cutout};

    /// Note HR6: a resized cutout's chain, mip 0 included, is fitted to the source's coverage; without a resize mip 0
    /// is untouched.
    #[test]
    fn a_resized_cutout_keeps_its_source_coverage() {
        let mut p = cutout_profile();
        p.resize = ResizeMode::Half;
        let work = preprocess_with(sparse_cutout(), &p, SourceTransfer::Stored);
        let mut chain = build_chain_with(&work, &p, None);
        let base = apply_coverage(&mut chain, &p, work.alpha_present, work.source_coverage);
        assert_eq!(base, Some(0.125));
        assert_eq!(chain[0].px.len(), 16);
        let pass = chain[0]
            .px
            .iter()
            .filter(|q| passes(crate::analyse::code8(q[3]), 128, AlphaCompare::Ge))
            .count();
        assert_eq!(pass, 2);

        let p = cutout_profile();
        let mut work = preprocess_with(sparse_cutout(), &p, SourceTransfer::Stored);
        // A failing alpha inside the threshold margin (code 126 of 128): a fit of level 0 would push it clear.
        work.img.at_mut(1, 1)[3] = 126.0 / 255.0;
        let mut chain = build_chain_with(&work, &p, None);
        let base = apply_coverage(&mut chain, &p, work.alpha_present, work.source_coverage);
        assert_eq!(base, Some(0.125));
        assert_eq!(chain[0], work.img);
    }
}
