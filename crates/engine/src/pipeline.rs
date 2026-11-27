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
    sha256_file, EncoderVersions, Manifest, ManifestEntry, ManifestRecord, PresetRef,
    BC1_ENCODE_ID, ENGINE_VERSION, MANIFEST_SCHEMA,
};
use crate::mips::build_chain_with;
use crate::ops::{preprocess_with, Domain, SourceTransfer};
use crate::pack::assemble::assemble;
use crate::policy::{ResolvedJob, SourceStamp};
use crate::profile::{self, Stage as Timing};
use crate::quantize::{to_encoder_input, EncoderInput};
use crate::session::first_level_within;
use crate::validate::{
    always_validated, validate, ValidationInput, ValidationReport, ValidationSummary,
};
use std::cell::Cell;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub enum FileError {
    /// Stopped by the cancel token before anything was written.
    Cancelled,
    Failed(Stage, String),
}

/// The message alone, as the export's Problems row words a `FileFailed`, so a preview names a failure the same way.
impl std::fmt::Display for FileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileError::Cancelled => f.write_str("cancelled"),
            FileError::Failed(_, e) => f.write_str(e),
        }
    }
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
    #[cfg(test)]
    pub alpha_present: bool,
    #[cfg(test)]
    pub weighted: bool,
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
    {
        let _span = profile::span(Timing::StampCheck);
        check_stamps(job)?
    };
    let extra = {
        let _span = profile::span(Timing::HashSource);
        after_stamps()?
    };
    // DECISIONS DS10: a cube job builds its faces and mips in direction space.
    if let Some(cj) = &job.cube {
        return Ok((
            {
                let _span = profile::span(Timing::CubePrepare);
                crate::cube::run::prepare(job, cj, keep, cancel, tracker)?
            },
            extra,
        ));
    }
    // Spec §6.2–6.3 (DECISIONS R4): a roughness member decodes its `_n` first and keeps only the footprint lengths,
    // so the two images are never held together.
    let lengths = match &job.bake {
        // `load_image`'s errors already name the `_n`.
        Some(b) => Some(
            {
                let _span = profile::span(Timing::BakeLoad);
                b.load_lengths()
            }
            .map_err(fail(Stage::Load))?,
        ),
        None => None,
    };
    stop()?;
    let (mut img, mut info) = match (&job.pack, &job.derive) {
        (Some(pk), _) => {
            // The gray source fixed at plan time (Note M6), not `p.channels.grayscale_source` of the final profile.
            let _span = profile::span(Timing::PackAssemble);
            let asm = assemble(pk, pk.gray).map_err(|e| fail(Stage::Load)(e.to_string()))?;
            (asm.image, asm.info)
        }
        // Note CN11: the recipe's image, encoded as any other.
        (None, Some(dj)) => {
            let _span = profile::span(Timing::DeriveAssemble);
            let asm = crate::derive::assemble::assemble_derived(dj)
                .map_err(|e| fail(Stage::Load)(e.to_string()))?;
            (asm.image, asm.info)
        }
        (None, None) => {
            let _span = profile::span(Timing::Decode);
            load_image_as(&job.input, p.alpha.exr_straight)
        }
        .map_err(|e| fail(Stage::Load)(e.to_string()))?,
    };
    // Note DY: the helper's channel becomes the alpha, as the plan analysed it (Ruling 11: a cube returned above).
    if let Some(f) = &job.fill {
        let _span = profile::span(Timing::Fill);
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
        let _span = profile::span(Timing::Preprocess);
        let (w, c) = crate::grass::build(img, p, transfer).map_err(fail(Stage::Preprocess))?;
        (w, Some(c))
    } else {
        let _span = profile::span(Timing::Preprocess);
        (preprocess_with(img, p, transfer), None)
    };
    stop()?;

    enter(Stage::Mipmaps);
    let mut chain = match grass_chain {
        Some(c) => c,
        None => {
            let _span = profile::span(Timing::Mips);
            build_chain_with(&work, p, bake.as_ref())
        }
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
        let _span = profile::span(Timing::Coverage);
        apply_coverage(&mut chain, p, alpha_present, work.source_coverage)
    };
    // Check 3 runs on the finished chain. An eligible job has `resize = none`, so `chain[0]` is the power-of-two size.
    let detail_ratio = (detail
        && crate::checks::low_detail_eligible(p, (chain[0].width, chain[0].height)))
    .then(|| {
        let _span = profile::span(Timing::DetailCheck);
        crate::checks::detail_ratio(&chain)
    })
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
        _ => {
            let _span = profile::span(Timing::Quantize);
            chain[first_level..]
                .iter()
                .map(|l| to_encoder_input(l, &work, p))
                .collect()
        }
    };
    let chain = match keep {
        Keep::Encode => {
            drop(chain);
            vec![]
        }
        _ => chain.split_off(first_level),
    };
    let domain = work.domain;
    #[cfg(test)]
    let weighted = crate::mips::weighted_rgb(&work, p);
    drop(work);
    stop()?;
    Ok((
        Prepared {
            #[cfg(test)]
            alpha_present,
            #[cfg(test)]
            weighted,
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

/// Publish a complete new DDS without replacing a destination that appeared during encoding.
/// Only a collision at the final move is a skip. Staging and other move errors fail the job.
fn write_new_dds(path: &Path, bytes: &[u8]) -> io::Result<bool> {
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    // Note LV1: tempfile's Windows persist hands both paths to the Win32 calls as they are, so a path that reaches
    // MAX_PATH needs the long form that std::fs adds for its own calls. Below that the paths stay as given, so an
    // error names the path the user knows.
    let (dir, path) = if needs_long_form(dir, path)? {
        (long_form(dir)?, long_form(path)?)
    } else {
        (dir.to_path_buf(), path.to_path_buf())
    };
    let mut builder = tempfile::Builder::new();
    builder.prefix(".sdc-").suffix(".tmp");
    #[cfg(unix)]
    builder.permissions(std::os::unix::fs::PermissionsExt::from_mode(0o666));
    let mut temp = builder.tempfile_in(&dir)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    match temp.persist_noclobber(&path) {
        Ok(_) => Ok(true),
        Err(e) if e.error.kind() == io::ErrorKind::AlreadyExists => Ok(false),
        Err(e) => Err(e.error),
    }
}

/// Whether the output `path` in `dir` needs the long form: Win32 MAX_PATH (260, counting the NUL) for the file calls
/// tempfile makes, 248 for folders (std's own legacy limit), checked for the folder, the output, and the longest temp
/// file tempfile makes in the folder (`.sdc-` + 6 random characters + `.tmp`, and a separator). Each length is the
/// larger of the raw path and the absolute path, in bytes of the OS string, which is never fewer than the UTF-16 units
/// Win32 counts, so a non-ASCII name converts a little early, never late, and a raw path with `..` that is longer than
/// its normalised form still converts (Note LV1).
fn needs_long_form(dir: &Path, path: &Path) -> io::Result<bool> {
    const DIR_LIMIT: usize = 248;
    const FILE_LIMIT: usize = 260;
    const TEMP_NAME: usize = 1 + ".sdc-".len() + 6 + ".tmp".len();
    let dir = dir
        .as_os_str()
        .len()
        .max(std::path::absolute(dir)?.as_os_str().len());
    let file = path
        .as_os_str()
        .len()
        .max(std::path::absolute(path)?.as_os_str().len());
    Ok(dir >= DIR_LIMIT || dir + TEMP_NAME >= FILE_LIMIT || file >= FILE_LIMIT)
}

/// `p` as an absolute verbatim (`\\?\`) path, the form Win32 file calls accept past MAX_PATH without a long-path
/// manifest (Note LV1). `std::path::absolute` first applies the normalisation a verbatim path skips (separators, `.`
/// and `..`), as std does before adding the prefix for its own calls; a UNC path takes `\\?\UNC\`, and a path that is
/// already verbatim stays as it is. A device path (`\\.\`) also stays as it is and is still capped at MAX_PATH: Win32
/// normalises it again, so an output root given in that form does not work past 260 characters.
#[cfg(windows)]
fn long_form(p: &Path) -> io::Result<PathBuf> {
    use std::path::{Component, Prefix};
    let abs = std::path::absolute(p)?;
    let mut parts = abs.components();
    let Some(Component::Prefix(prefix)) = parts.next() else {
        return Ok(abs);
    };
    let mut out = match prefix.kind() {
        Prefix::Disk(_) => std::ffi::OsString::from(r"\\?\"),
        Prefix::UNC(server, share) => {
            let mut s = std::ffi::OsString::from(r"\\?\UNC\");
            s.push(server);
            s.push(r"\");
            s.push(share);
            s.push(parts.as_path());
            return Ok(s.into());
        }
        _ => return Ok(abs),
    };
    out.push(&abs);
    Ok(out.into())
}

/// Elsewhere paths have no MAX_PATH form to add.
#[cfg(not(windows))]
fn long_form(p: &Path) -> io::Result<PathBuf> {
    Ok(p.to_path_buf())
}

/// Convert one file and append its manifest record. Returns `None` when a late unrecorded output was kept.
/// Cancellation is checked between stages and encode bands, last just before the write.
/// A cancelled file never leaves an output behind. Everything that can fail
/// before the output exists (validation of the in-memory DDS bytes, the profile hash, the record)
/// runs before the write; only the manifest append comes after it. When that fails,
/// or anything panics, `tracker.written` tells the caller to remove the output, so an output on disk has its
/// record, except after a second Ctrl-C: that aborts the process (exit 3) and can leave `.sdc-*.tmp` files, or an
/// output without a record (`OUT-UNKNOWN-EXISTING` on the next run). A failed validation (`passed = false`) is not a failure here: the output is written and recorded.
/// With `req.validate` off the output is not decoded again and its record says `validation.skipped`, unless its
/// sub-type is always validated (`validate::always_validated`, Note LG9).
///
/// Check 3 (`req.low_detail`): a flagged texture is written in full and carries the warning in its report under
/// `Warn`.
#[allow(
    clippy::too_many_arguments,
    reason = "one file's whole conversion: the job, request, preset, cancel token, reporter, manifest and replace flag from the batch worker, and the tracker its caller reads after a panic, each with a different owner"
)]
pub fn process_file(
    job: &ResolvedJob,
    req: &BatchRequest,
    preset: &PresetRef,
    cancel: &CancelToken,
    rep: &Reporter,
    manifest: &Manifest,
    tracker: &FileTracker,
    replace_existing: bool,
) -> Result<Option<FileOutcome>, FileError> {
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
    let levels = {
        let _span = profile::span(Timing::Encode);
        encode_levels(&prep, p, cancel, rep, &key, set, path)?
    };
    let total = levels_per_face(&prep);

    enter(Stage::Write);
    let srgb = p.srgb_tag();
    let desc = DdsDesc {
        width,
        height,
        format: p.format,
        srgb,
        header: resolve_header(p.format, srgb, p.header),
    };
    let bytes = {
        let _span = profile::span(Timing::DdsAssemble);
        if prep.faces == 6 {
            dds_bytes_cube(&desc, &levels)
        } else {
            dds_bytes(&desc, &levels)
        }
    }
    .map_err(|e| fail(Stage::Write)(e.to_string()))?;
    drop(levels);

    // Validate the exact bytes about to be written: `validate` parses them with the engine's DDS reader (spec §5.8).
    // Skipped, it only reads, so the bytes written are the same.
    let validated = req.validate || always_validated(p.subtype);
    let mut report = if validated {
        enter(Stage::Validate);
        let _span = profile::span(Timing::Validate);
        validate_prepared(&bytes, p, &prep)
    } else {
        ValidationReport {
            summary: ValidationSummary::not_run(total),
            warnings: vec![],
        }
    };
    // Test hook (Note DM, `test-hooks` only): record this output's encode-input hash for the encode-version guard.
    #[cfg(feature = "test-hooks")]
    if let Some(dir) = std::env::var_os("SDC_TEST_ENCODE_INPUT_DIR") {
        let header = crate::dds::header_bytes_with(&desc, total, prep.faces == 6).expect("header");
        let sha256 = encode_input_sha256(&header, &prep.inputs, p, report.summary.passed);
        let json = serde_json::json!({ "subtype": p.subtype.id(), "sha256": sha256 });
        let file = Path::new(&dir).join(format!("{}.json", key.replace('/', "~")));
        std::fs::write(file, json.to_string()).expect("encode input hook");
    }
    profile::note_job(|| profile::JobNote {
        input: job.input.clone(),
        subtype: p.subtype.id().into(),
        format: p.format.id().into(),
        // The serde name (Note LH4 removed the generic `serde_id`; `AlphaPolicy` has no `id`); profiling only, so the
        // allocation is off the plain path.
        alpha_policy: serde_json::to_value(p.alpha.policy)
            .expect("unit enum serialises as a string")
            .as_str()
            .expect("unit enum serialises as a string")
            .to_owned(),
        packed: job.pack.is_some(),
        baked: job.bake.is_some(),
        width,
        height,
        levels: total,
    });
    drop(prep);
    // Note K: the hook fails a validation, so it runs only when one ran.
    #[cfg(feature = "test-hooks")]
    if validated {
        force_validation_failure_for_test(job, &mut report);
    }
    enter(Stage::Write);
    let mut record = {
        let _span = profile::span(Timing::ManifestRecord);
        ManifestRecord {
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
        }
    };
    stop()?;

    let write_error = |e: io::Error| fail(Stage::Write)(e.to_string());
    {
        let _span = profile::span(Timing::Write);
        if replace_existing {
            write_atomic(&job.output, &bytes).map_err(write_error)?;
        } else if !write_new_dds(&job.output, &bytes).map_err(write_error)? {
            return Ok(None);
        }
    }
    tracker.written.set(true);
    drop(bytes);
    // Note Y1: taken after the rename, so `if-changed` notices a replaced output. Any failure from here on removes
    // the output (`run_one`, `tracker.written`).
    record.output_stamp = SourceStamp::of(&job.output);
    {
        let _span = profile::span(Timing::ManifestAppend);
        manifest.append(&record)
    }
    .map_err(|e| fail(Stage::Write)(format!("manifest {}: {e}", manifest.path().display())))?;
    // `Warn`: listed with the file's warnings only; the record and its validation summary do not carry it.
    report.warnings.extend(low_detail);
    Ok(Some(FileOutcome {
        output: job.output.clone(),
        report,
    }))
}

/// Mips per face: the header's count, and the level count validation expects, on the encode and the check path alike.
fn levels_per_face(prep: &Prepared) -> u32 {
    (prep.inputs.len() / prep.faces) as u32
}

fn validate_prepared(bytes: &[u8], profile: &Profile, prep: &Prepared) -> ValidationReport {
    validate(&ValidationInput {
        bytes,
        profile,
        encoder_input: Some(&prep.inputs),
        base_coverage: prep.base_coverage,
        source_peak: prep.source_peak,
        expected_levels: Some(levels_per_face(prep)),
    })
}

/// Rebuild the encoder input for a current DDS and append its verdict without writing the DDS.
pub(crate) fn check_in_place(
    job: &ResolvedJob,
    entry: &ManifestEntry,
    cancel: &CancelToken,
    manifest: &Manifest,
) -> Result<ValidationReport, FileError> {
    if cancel.is_cancelled() {
        return Err(FileError::Cancelled);
    }
    let bytes = std::fs::read(&job.output).map_err(|e| fail(Stage::Validate)(e.to_string()))?;
    check_output_stamp(job, entry)?;
    let prep =
        prepare(job, cancel, &FileTracker::default(), Keep::Encode).map_err(|e| match e {
            FileError::Failed(_, msg) => FileError::Failed(Stage::Validate, msg),
            FileError::Cancelled => FileError::Cancelled,
        })?;
    #[cfg_attr(
        not(feature = "test-hooks"),
        allow(
            unused_mut,
            reason = "only the test hook (force_validation_failure_for_test) mutates the report"
        )
    )]
    let mut report = validate_prepared(&bytes, &job.profile, &prep);
    #[cfg(feature = "test-hooks")]
    force_validation_failure_for_test(job, &mut report);
    commit_check(job, entry, &report, cancel, manifest)?;
    Ok(report)
}

fn check_output_stamp(job: &ResolvedJob, entry: &ManifestEntry) -> Result<(), FileError> {
    if SourceStamp::of(&job.output) != entry.output_stamp {
        return Err(FileError::Failed(
            Stage::Validate,
            "the output changed during the run".into(),
        ));
    }
    Ok(())
}

pub(crate) fn commit_check(
    job: &ResolvedJob,
    entry: &ManifestEntry,
    report: &ValidationReport,
    cancel: &CancelToken,
    manifest: &Manifest,
) -> Result<(), FileError> {
    let mut record: ManifestRecord = serde_json::from_value(entry.raw.clone())
        .map_err(|e| fail(Stage::Validate)(e.to_string()))?;
    record.validation = report.summary.clone();
    record.ts = now_rfc3339();
    if cancel.is_cancelled() {
        return Err(FileError::Cancelled);
    }
    check_output_stamp(job, entry)?;
    manifest
        .append(&record)
        .map_err(|e| fail(Stage::Validate)(format!("manifest {}: {e}", manifest.path().display())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AlphaCompare, ResizeMode};
    use crate::coverage::passes;
    use crate::ops::tests::{cutout_profile, sparse_cutout};

    fn unchecked_output(dir: &Path) -> (ResolvedJob, Manifest, ManifestEntry) {
        use crate::api::Engine;
        use crate::config::Target;
        use crate::events::VecSink;
        let input = dir.join("textures/a/stone.png");
        std::fs::create_dir_all(input.parent().unwrap()).unwrap();
        ::image::RgbImage::new(32, 32).save(&input).unwrap();
        let mut req = BatchRequest::new(vec![input], dir.join("out"), Target::Cs);
        req.overrides
            .push(crate::overrides::parse_set("subtype=diffuse").unwrap());
        let rep = Reporter::new(std::sync::Arc::new(VecSink::default()), "check-guards");
        let engine = Engine::new().unwrap();
        let cancel = CancelToken::default();
        let plan = engine.plan(req, &rep, &cancel).unwrap();
        let job = plan.jobs[0].clone();
        let manifest = Manifest::at(&plan.request.output_root);
        assert_eq!(engine.run(plan, &rep, &cancel).unwrap().converted, 1);
        let entry = manifest.read().unwrap().into_values().next().unwrap();
        (job, manifest, entry)
    }

    fn line_count(manifest: &Manifest) -> usize {
        std::fs::read_to_string(manifest.path())
            .unwrap()
            .lines()
            .count()
    }

    #[test]
    fn check_in_place_stops_at_its_entry_guards() {
        let dir = tempfile::tempdir().unwrap();
        let (job, manifest, mut entry) = unchecked_output(dir.path());
        let lines = line_count(&manifest);
        entry.output_stamp.as_mut().unwrap().size += 1;
        // Preparing would now fail, so the error identifies the entry guard specifically.
        std::fs::remove_file(&job.input).unwrap();
        let cancel = CancelToken::default();
        assert_eq!(
            check_in_place(&job, &entry, &cancel, &manifest).unwrap_err(),
            FileError::Failed(Stage::Validate, "the output changed during the run".into())
        );
        assert_eq!(line_count(&manifest), lines);
        cancel.cancel();
        std::fs::remove_file(&job.output).unwrap();
        assert_eq!(
            check_in_place(&job, &entry, &cancel, &manifest).unwrap_err(),
            FileError::Cancelled
        );
        assert_eq!(line_count(&manifest), lines);
    }

    #[test]
    fn the_final_guards_stop_the_append() {
        let dir = tempfile::tempdir().unwrap();
        let (job, manifest, entry) = unchecked_output(dir.path());
        let cancel = CancelToken::default();
        let bytes = std::fs::read(&job.output).unwrap();
        let modified = std::fs::metadata(&job.output).unwrap().modified().unwrap();
        let prep = prepare(&job, &cancel, &FileTracker::default(), Keep::Encode).unwrap();
        let report = validate_prepared(&bytes, &job.profile, &prep);
        let lines = line_count(&manifest);
        std::fs::write(&job.output, b"changed").unwrap();
        let changed = commit_check(&job, &entry, &report, &cancel, &manifest);
        assert_eq!(line_count(&manifest), lines, "changed output");
        assert_eq!(
            changed,
            Err(FileError::Failed(
                Stage::Validate,
                "the output changed during the run".into()
            ))
        );
        std::fs::write(&job.output, bytes).unwrap();
        std::fs::File::options()
            .write(true)
            .open(&job.output)
            .unwrap()
            .set_modified(modified)
            .unwrap();
        assert_eq!(SourceStamp::of(&job.output), entry.output_stamp);
        let stopped = CancelToken::default();
        stopped.cancel();
        let result = commit_check(&job, &entry, &report, &stopped, &manifest);
        assert_eq!(line_count(&manifest), lines, "cancelled");
        assert_eq!(result, Err(FileError::Cancelled));
        commit_check(&job, &entry, &report, &cancel, &manifest).unwrap();
        assert_eq!(line_count(&manifest), lines + 1);
    }

    /// QA2: a failure reads as its message alone, as the export's Problems row shows `FileFailed`.
    #[test]
    fn file_error_reads_as_text() {
        assert_eq!(
            FileError::Failed(Stage::Load, r"C:\mods\a.png: bad".into()).to_string(),
            r"C:\mods\a.png: bad"
        );
        assert_eq!(FileError::Cancelled.to_string(), "cancelled");
    }

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

    /// A folder under `root` deep enough that a file in it has a full path over MAX_PATH (260 characters), built
    /// from 60-character parts so the length does not depend on TEMP (Note LC3).
    fn past_max_path(root: &Path) -> PathBuf {
        let mut dir = std::path::absolute(root).unwrap();
        while dir.as_os_str().len() < 265 {
            dir.push("x".repeat(60));
        }
        dir
    }

    /// Note LV1: the long form normalises a drive path before its `\\?\`, gives a UNC path `\\?\UNC\`, keeps a
    /// verbatim or device path, and makes a relative path absolute.
    #[cfg(windows)]
    #[test]
    fn the_long_form_keeps_each_kind_of_path() {
        let long = |p: &str| long_form(Path::new(p)).unwrap();
        assert_eq!(long("C:/out/./a/../b.dds"), Path::new(r"\\?\C:\out\b.dds"));
        assert_eq!(
            long(r"\\server\share\out\b.dds"),
            Path::new(r"\\?\UNC\server\share\out\b.dds")
        );
        assert_eq!(long(r"\\?\C:\out\b.dds"), Path::new(r"\\?\C:\out\b.dds"));
        assert_eq!(long(r"\\.\pipe\x"), Path::new(r"\\.\pipe\x"));
        // A relative path is made absolute from the current directory, whatever form that has.
        let rel = long("b.dds");
        let want = long_form(&std::env::current_dir().unwrap().join("b.dds")).unwrap();
        assert_eq!(rel, want);
        let cwd = std::env::current_dir().unwrap();
        if matches!(cwd.components().next(), Some(std::path::Component::Prefix(p)) if matches!(p.kind(), std::path::Prefix::Disk(_)))
        {
            assert_eq!(rel, Path::new(&format!(r"\\?\{}\b.dds", cwd.display())));
        } else {
            eprintln!("skipped the drive-form check: the current directory is not a drive path");
        }
    }

    /// A folder of exactly `len` characters under `root` (parts of at most 60 characters).
    fn dir_of_len(root: &Path, len: usize) -> PathBuf {
        let mut dir = std::path::absolute(root).unwrap();
        assert!(dir.as_os_str().len() + 2 <= len, "{}", dir.display());
        while dir.as_os_str().len() < len {
            // Each part is at least 1 character: never leave a gap of 1 that only a separator could fill.
            let left = len - dir.as_os_str().len();
            let part = if left == 62 { 59 } else { (left - 1).min(60) };
            dir.push("x".repeat(part));
        }
        assert_eq!(dir.as_os_str().len(), len);
        dir
    }

    /// Note LV1: the long form is added only where the folder, the output or its temp file reaches the legacy limit.
    #[test]
    fn the_long_form_is_only_for_paths_near_the_limit() {
        let d = tempfile::tempdir().unwrap();
        let short = d.path().join("out");
        assert!(!needs_long_form(&short, &short.join("a.dds")).unwrap());
        // A folder of 243: the temp (243 + 16 = 259) still fits.
        let dir = dir_of_len(d.path(), 243);
        assert!(!needs_long_form(&dir, &dir.join("a.dds")).unwrap());
        // 244: the output is 250, but the temp is 260.
        let dir = dir_of_len(d.path(), 244);
        assert!(dir.join("a.dds").as_os_str().len() < 260);
        assert!(needs_long_form(&dir, &dir.join("a.dds")).unwrap());
        // A long file name in a short folder reaches the file limit.
        let name = "n".repeat(250);
        assert!(needs_long_form(&short, &short.join(name)).unwrap());
    }

    /// Note LV1: an output under MAX_PATH whose temp file in the same folder reaches it is written, and leaves no temp.
    #[test]
    fn a_new_dds_whose_temp_path_reaches_max_path_is_written() {
        let d = tempfile::tempdir().unwrap();
        let dir = dir_of_len(d.path(), 245);
        let path = dir.join("a.dds");
        assert!(path.as_os_str().len() < 260, "{}", path.display());
        assert!(write_new_dds(&path, b"first").unwrap());
        assert_eq!(std::fs::read(&path).unwrap(), b"first");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
    }

    /// Note LV1: a write that fails on a short path names the path as given, never in the `\\?\` form.
    #[test]
    fn a_failed_write_on_a_short_path_names_no_verbatim_path() {
        // The temp cannot be created in the output folder: everyone is denied creating files there (the error
        // names the temp's path).
        let d = tempfile::tempdir().unwrap();
        let out = d.path().join("out");
        std::fs::create_dir(&out).unwrap();
        let icacls = |flag: &str, rule: &str| {
            std::process::Command::new("icacls")
                .arg(&out)
                .arg(flag)
                .arg(rule)
                .output()
                .is_ok_and(|o| o.status.success())
        };
        if !icacls("/deny", "*S-1-1-0:(WD,AD)") {
            eprintln!("skipped: icacls could not deny file creation");
            return;
        }
        let r = write_new_dds(&out.join("a.dds"), b"x");
        icacls("/remove:d", "*S-1-1-0");
        let e = r.expect_err("the denied folder takes no new file");
        assert!(!e.to_string().contains(r"\\?\"), "{e}");
        // The message prints the path with its backslashes escaped, as `Path`'s Debug does.
        let given = format!("{out:?}");
        assert!(e.to_string().contains(given.trim_matches('"')), "{e}");
    }

    /// Note LV1: a new DDS whose full path is over MAX_PATH is written, a second write to it is still a skip that
    /// keeps the first bytes, and no temp file is left.
    #[test]
    fn a_new_dds_past_max_path_is_written_and_never_replaced() {
        let d = tempfile::tempdir().unwrap();
        let path = past_max_path(d.path()).join("stone.dds");
        assert!(path.as_os_str().len() > 260, "{}", path.display());
        assert!(write_new_dds(&path, b"first").unwrap());
        assert_eq!(std::fs::read(&path).unwrap(), b"first");
        assert!(!write_new_dds(&path, b"second").unwrap());
        assert_eq!(std::fs::read(&path).unwrap(), b"first");
        let names: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, [std::ffi::OsString::from("stone.dds")]);
    }

    /// Note LV1: a convert from a short input writes its output and manifest record under an output folder whose
    /// files are over MAX_PATH, and a changed source then replaces that output.
    #[test]
    fn a_convert_writes_and_replaces_an_output_past_max_path() {
        use crate::api::Engine;
        use crate::config::Target;
        use crate::events::{EventKind, VecSink};
        let d = tempfile::tempdir().unwrap();
        let input = d.path().join("stone.png");
        ::image::RgbImage::new(32, 32).save(&input).unwrap();
        let out = past_max_path(d.path());
        let run = || {
            let mut req = BatchRequest::new(vec![input.clone()], out.clone(), Target::Cs);
            req.overrides
                .push(crate::overrides::parse_set("subtype=diffuse").unwrap());
            let sink = std::sync::Arc::new(VecSink::default());
            let rep = Reporter::new(sink.clone(), "long-paths");
            let engine = Engine::new().unwrap();
            let cancel = CancelToken::default();
            let plan = engine.plan(req, &rep, &cancel).unwrap();
            let output = plan.jobs[0].output.clone();
            let summary = engine.run(plan, &rep, &cancel).unwrap();
            let failures: Vec<String> = sink
                .take()
                .into_iter()
                .filter_map(|e| match e.kind {
                    EventKind::FileFailed { error, .. } => Some(error),
                    _ => None,
                })
                .collect();
            assert_eq!(summary.converted, 1, "{failures:?}");
            output
        };
        let output = run();
        assert!(output.as_os_str().len() > 260, "{}", output.display());
        let first = std::fs::read(&output).unwrap();
        ::image::RgbImage::from_pixel(32, 32, ::image::Rgb([200, 40, 10]))
            .save(&input)
            .unwrap();
        assert_eq!(run(), output);
        assert_ne!(std::fs::read(&output).unwrap(), first);
        assert_eq!(Manifest::at(&out).load().unwrap().len(), 1);
    }
}
