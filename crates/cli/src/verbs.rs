//! The `inspect` and `validate` verbs (spec §8): read-only analysis of inputs and of existing DDS files.
//!
//! Both are result verbs (DECISIONS Note J): their report goes to stdout. With `--json`, stdout carries only
//! JSON (`inspect`: the plan's events and one `dds_info` object per DDS; `validate`: one `dds_validation`
//! object per file) and `validate`'s human report moves to stderr.
use crate::args::*;
use crate::human::{skip_line, warning_line, TeeSink};
use engine::api::{CancelToken, Engine, EngineError};
use engine::codes::{Code, Warning};
use engine::config::*;
use engine::dds::{check_header, read_dds, DdsFile, DdsPixelFormat};
use engine::decode::{decode_level, Decoded};
use engine::discover::{discover, game_path, InputKind};
use engine::events::{
    new_batch_id, EngineEvent, EventKind, EventSink, JsonLinesSink, Reporter, SkipKind, VecSink,
};
use engine::group::split_suffix_with;
use engine::manifest::output_key;
use engine::policy::{availability, Availability, JobStatus, ResolvedJob};
use engine::presets::{default_preset_for, AlphaContent};
use engine::reports::{
    DdsHeaderInfo, DdsInfo, DdsInfoKind, DdsMipInfo, DdsUnreadable, DdsValidation,
    DdsValidationKind, DdsValidationUnreadable, DdsVerdict, REPORT_SCHEMA,
};
use engine::schemas::SchemaKind;
use engine::suggest::{suggest, SuggestContext};
use engine::validate::alpha::alpha_stats;
use engine::validate::{validate, ValidationReport, ValidationSummary};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// `println!` to stdout that ignores write errors: a reader that closes the pipe early (`| head -n1`) is not a
/// crash (audit REPORT #1, Note BC). The engine's JSON sink ignores them the same way.
macro_rules! out {
    ($($t:tt)*) => {{
        use std::io::Write as _;
        let _ = writeln!(std::io::stdout().lock(), $($t)*);
    }};
}

/// [`out!`] without the newline (`print!`).
macro_rules! out_raw {
    ($($t:tt)*) => {{
        use std::io::Write as _;
        let _ = write!(std::io::stdout().lock(), $($t)*);
    }};
}

/// Formats whose decoded RGBA carries a meaningful alpha channel.
fn carries_alpha(f: DdsPixelFormat) -> bool {
    use DdsPixelFormat::*;
    matches!(
        f,
        Bc1 { .. }
            | Bc2 { .. }
            | Bc3 { .. }
            | Bc7 { .. }
            | Rgba8 { .. }
            | Bgra8 { .. }
            | Rgba16f
            | Rgba32f
    )
}

/// `("Bc7", Some(false))`: the format's name and, where it has one, its `_SRGB` tag.
fn format_name(f: DdsPixelFormat) -> (String, Option<bool>) {
    match serde_json::to_value(f) {
        Ok(Value::String(s)) => (s, None),
        Ok(Value::Object(m)) => m.into_iter().next().map_or_else(
            || (format!("{f:?}"), None),
            |(k, v)| (k, v.get("srgb").and_then(Value::as_bool)),
        ),
        _ => (format!("{f:?}"), None),
    }
}

fn read_file(path: &Path) -> Result<(Vec<u8>, DdsFile), String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let file = read_dds(&bytes).map_err(|e| e.to_string())?;
    Ok((bytes, file))
}

/// The `dds_info` object for one file: header facts and per-mip decoded-alpha statistics.
fn dds_info(path: &Path) -> DdsInfo {
    let path_s = path.to_string_lossy().replace('\\', "/");
    let file = match read_file(path) {
        Ok((_, f)) => f,
        Err(error) => {
            return DdsInfo::Unreadable(DdsUnreadable {
                kind: DdsInfoKind::DdsInfo,
                schema: REPORT_SCHEMA,
                path: path_s,
                error,
            })
        }
    };
    let mips: Vec<DdsMipInfo> = file
        .levels
        .iter()
        .map(|(w, h, r)| {
            let mut m = DdsMipInfo {
                width: *w,
                height: *h,
                alpha: None,
                decode_error: None,
            };
            if carries_alpha(file.format) {
                match decode_level(file.format, *w, *h, &file.data[r.clone()]) {
                    Ok(Decoded::Ldr(px)) => m.alpha = Some(alpha_stats(&px)),
                    Ok(Decoded::Hdr(_)) => {}
                    Err(e) => m.decode_error = Some(e.to_string()),
                }
            }
            m
        })
        .collect();
    let (format, srgb) = format_name(file.format);
    DdsInfo::Info(DdsHeaderInfo {
        kind: DdsInfoKind::DdsInfo,
        schema: REPORT_SCHEMA,
        path: path_s.clone(),
        format,
        srgb,
        header: file.header_kind,
        width: file.width,
        height: file.height,
        mip_count: file.mip_count,
        cubemap: file.is_cubemap,
        header_issues: check_header(&file)
            .into_iter()
            .chain(engine::cube::checks::inspect_issues(&path_s, &file))
            .collect(),
        mips,
    })
}

fn pct(n: u64, of: u64) -> String {
    format!(
        "{:.1}%",
        if of == 0 {
            0.0
        } else {
            n as f64 * 100.0 / of as f64
        }
    )
}

fn print_dds_info(info: &DdsInfo) {
    let h = match info {
        DdsInfo::Unreadable(u) => {
            out!("{}", u.path);
            out!("  cannot be read: {}", u.error);
            return;
        }
        DdsInfo::Info(h) => h,
    };
    out!("{}", h.path);
    let srgb = match h.srgb {
        Some(true) => " srgb",
        Some(false) => " unorm",
        None => "",
    };
    out!(
        "  format {}{srgb}  {}x{}  {} mips  {:?} header  cubemap {}",
        h.format,
        h.width,
        h.height,
        h.mip_count,
        h.header,
        h.cubemap
    );
    for issue in &h.header_issues {
        out!("  header issue: {issue}");
    }
    for (i, m) in h.mips.iter().enumerate() {
        let alpha = match (&m.alpha, &m.decode_error) {
            (Some(a), _) => format!(
                "  alpha min {} =255 {} =254 {} <254 {}",
                a.min,
                pct(a.eq255, a.pixels),
                pct(a.eq254, a.pixels),
                pct(a.below254, a.pixels)
            ),
            (None, Some(e)) => format!("  decode failed: {e}"),
            (None, None) => String::new(),
        };
        out!("  mip {i} {}x{}{alpha}", m.width, m.height);
    }
}

/// The serde string form of an enum value (`"bc7"`, `"high"`), as the JSON events carry it.
macro_rules! tag {
    ($v:expr) => {
        match serde_json::to_value($v) {
            Ok(Value::String(t)) => t,
            _ => "-".to_string(),
        }
    };
}

/// One compact line per RNM layer of a base normal: file, resolved strength, mask and AO source when used.
fn print_rnm(job: &ResolvedJob) {
    let Some(r) = job.active_rnm() else { return };
    let name = |f: &engine::rnm::RnmFile| {
        f.path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    };
    let spec = &job.profile.rnm;
    let inputs = r.inputs(spec);
    for (f, role) in inputs.iter().filter(|(_, k)| !k.contains('.')) {
        let Some(l) = r.layers.iter().find(|l| l.id.id() == *role) else {
            continue;
        };
        let mask = inputs.iter().find(|(_, k)| *k == format!("{role}.mask"));
        let ao = inputs.iter().find(|(_, k)| k.starts_with("bent.ao"));
        let ao = ao.filter(|_| *role == "bent");
        out!(
            "  rnm {role} {} strength {}{}{}",
            name(f),
            spec.layer(l.id).strength,
            mask.map(|(m, _)| format!(", mask {}", name(m)))
                .unwrap_or_default(),
            ao.map(|(a, _)| format!(", AO {}", name(a)))
                .unwrap_or_default(),
        );
    }
}

fn print_planned(job: &ResolvedJob) {
    out!("{}", job.input.display());
    match job.suggestions.first() {
        Some(g) => out!(
            "  suggested {} ({} confidence: {})",
            g.subtype.id(),
            tag!(&g.confidence),
            g.reason
        ),
        None => out!("  no suggestion"),
    }
    out!(
        "  sub-type {}  format {}  tag {}  alpha {}  output {}",
        job.profile.subtype.id(),
        tag!(&job.profile.format),
        tag!(&job.profile.color.tag),
        tag!(&job.profile.alpha.policy),
        job.output.display()
    );
    let detail: Vec<String> = match &job.status {
        JobStatus::Unresolved { reasons, .. } => reasons.clone(),
        JobStatus::Blocked { codes } | JobStatus::Error { codes } => {
            codes.iter().map(|c| c.as_str().to_string()).collect()
        }
        JobStatus::Ready | JobStatus::PackingInput { .. } | JobStatus::NormalLayer { .. } => vec![],
    };
    let state = serde_json::to_value(&job.status)
        .ok()
        .and_then(|v| v["state"].as_str().map(str::to_owned))
        .unwrap_or_else(|| "-".into());
    out!("  status {state} {}", detail.join("; "));
    print_rnm(job);
    for w in &job.warnings {
        out!("  {}", warning_line(None, w));
    }
}

/// Plan-time events as human text. Warnings are listed under their `file_planned`; skips and failures are
/// the lines `convert` would report for the same files.
fn print_event(ev: &EngineEvent) {
    let path = ev.path.as_deref().unwrap_or("");
    match &ev.kind {
        EventKind::FilePlanned { job } => print_planned(job),
        EventKind::FileSkipped {
            kind: SkipKind::PackedSource,
            ..
        } => {}
        EventKind::FileSkipped { code, reason, .. } => {
            out!("{path}\n  skipped {}", skip_line(*code, reason))
        }
        EventKind::FileFailed { error, .. } => out!("{path}\n  cannot be read: {error}"),
        // A batch-level warning (CFG-PRESET-SKIPPED).
        EventKind::Warning(w) => out!("{}", warning_line(None, w)),
        EventKind::SetResolved {
            recipes,
            shader,
            warnings,
            ..
        } => {
            let shown = shader
                .as_ref()
                .map(|p| format!("  shader {} ({})", p.shader.id(), p.provenance))
                .unwrap_or_default();
            out!("set {}{shown}", ev.set.as_deref().unwrap_or(""));
            for w in warnings {
                out!("  {}", warning_line(None, w));
            }
            for r in recipes {
                let status = serde_json::to_value(r.status)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_string))
                    .unwrap_or_default();
                let reason = r
                    .reason
                    .as_deref()
                    .map(|x| format!(": {x}"))
                    .unwrap_or_default();
                out!("  pack {} ({}) {status}{reason}", r.output, r.subtype.id());
                let channels: Vec<String> = ["R", "G", "B", "A"]
                    .iter()
                    .zip(&r.channels)
                    .filter(|(_, p)| !p.is_empty())
                    .map(|(c, p)| format!("{c} {p}"))
                    .collect();
                out!("    {}", channels.join("  "));
            }
        }
        _ => {}
    }
}

/// Strips the mapped root off the front of `s`, so a path inside it reads relative to it. `root` is
/// '/'-separated and lower-case; `set` keys are lower-case already, `output` paths keep their case.
fn strip_root(s: &str, root: &str) -> Option<String> {
    let norm = s.replace('\\', "/");
    // `root` is the lower-cased form, which can be longer than the original ('İ' lowers to two chars), so
    // lower-case `norm` char by char until it reaches the root's length, then slice at that char.
    let mut head = String::new();
    let mut end = 0;
    for c in norm.chars() {
        if head.len() >= root.len() {
            break;
        }
        head.extend(c.to_lowercase());
        end += c.len_utf8();
    }
    if head != root {
        return None;
    }
    let rest = &norm[end..];
    // Only whole components: `sdc-inspect-abc` is not under `sdc-inspect-ab`.
    if !rest.is_empty() && !rest.starts_with('/') {
        return None;
    }
    Some(rest.trim_start_matches('/').to_string())
}

fn relativise(v: &mut Value, root: &str) {
    match v {
        Value::String(s) => {
            if let Some(r) = strip_root(s, root) {
                *s = r;
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| relativise(x, root)),
        Value::Object(m) => m.values_mut().for_each(|x| relativise(x, root)),
        _ => {}
    }
}

/// Passes events on with every path under the throw-away output root rewritten relative to it, so the
/// output of `inspect` does not depend on the temp folder it happened to use.
struct RelativeRoot {
    root: String,
    inner: Arc<dyn EventSink>,
}

impl EventSink for RelativeRoot {
    fn emit(&self, ev: &EngineEvent) {
        let rewritten = serde_json::to_value(ev)
            .ok()
            .map(|mut v| {
                relativise(&mut v, &self.root);
                v
            })
            .and_then(|v| serde_json::from_value::<EngineEvent>(v).ok());
        self.inner.emit(rewritten.as_ref().unwrap_or(ev));
    }
}

/// `sdc inspect`: DDS headers and mip alpha for `.dds` inputs; a dry-run plan for images. Exit 0 unless the
/// command line or a config is invalid (2), or an input could not be read (1, spec §8).
pub fn inspect(a: InspectArgs) -> Result<i32, EngineError> {
    let json = a.batch.json;
    if let Some(p) = a.batch.inputs.iter().find(|p| !p.exists()) {
        return Err(EngineError::Usage(format!(
            "input not found: {}",
            p.display()
        )));
    }
    // The plan maps outputs under a root nothing is written to; a fresh temp folder keeps it unique and is
    // removed when `root` drops.
    let root = tempfile::Builder::new()
        .prefix("sdc-inspect-")
        .tempdir()
        .map_err(|e| EngineError::Fatal(format!("temp folder: {e}")))?;
    // Codex-CLI 2: the shared batch options and every config are checked before any output, even when only DDS
    // files (or nothing) are found and no plan runs.
    let mut req = build_request(&a.batch, root.path().to_path_buf()).map_err(EngineError::Usage)?;
    // A DDS is reported once, by its header below, never also as a re-encode job (DECISIONS S6): drop every
    // command-line `reencode`; the unscoped `false` then outranks any `[files]` opt-in.
    req.overrides
        .retain(|o| o.field != engine::overrides::REENCODE_FIELD);
    req.overrides.push(engine::overrides::SetOverride {
        scope: engine::overrides::Scope::All,
        field: engine::overrides::REENCODE_FIELD.into(),
        value: false.into(),
    });
    let engine = Engine::new()?;
    let preset_skips = engine.preflight(&req)?;
    // rgbcx's mode is global state; set it before any decode.
    encoders_sys::init();
    let found = discover(&a.batch.inputs, root.path(), a.batch.recursive)
        .map_err(|e| EngineError::Usage(format!("discover: {e}")))?;
    let mut unreadable = 0u32;
    for f in found.iter().filter(|f| f.kind == InputKind::Dds) {
        let info = dds_info(&f.path);
        unreadable += u32::from(matches!(info, DdsInfo::Unreadable(_)));
        if json {
            out!("{}", serde_json::to_value(&info).unwrap_or(Value::Null));
        } else {
            print_dds_info(&info);
        }
    }
    if !found.iter().any(|f| f.kind == InputKind::Image) {
        // No plan runs here, so report the skipped user presets ourselves (a plan reports them itself).
        // One reporter, so every skip shares one batch id.
        let sink = Arc::new(VecSink::default());
        let rep = if json {
            Reporter::new(
                Arc::new(JsonLinesSink::new(std::io::stdout())),
                new_batch_id(),
            )
        } else {
            Reporter::new(sink.clone(), new_batch_id())
        };
        for w in preset_skips {
            rep.emit(None, None, EventKind::Warning(w));
        }
        sink.take().iter().for_each(print_event);
        return Ok(i32::from(unreadable > 0));
    }

    req.dry_run = true;
    let events = Arc::new(VecSink::default());
    let mut sinks: Vec<Arc<dyn EventSink>> = vec![events.clone()];
    if json {
        sinks.push(Arc::new(JsonLinesSink::new(std::io::stdout())));
    }
    let sink = RelativeRoot {
        root: root
            .path()
            .to_string_lossy()
            .replace('\\', "/")
            .to_lowercase(),
        inner: Arc::new(TeeSink(sinks)),
    };
    let rep = Reporter::new(Arc::new(sink), new_batch_id());
    let plan = engine.plan(req, &rep, &CancelToken::default())?;
    // What `run` would report for the files `plan` set aside. DDS files are inspected above, not skipped.
    for sk in plan
        .skipped
        .iter()
        .filter(|sk| sk.code != Some(Code::InDdsSkipped))
    {
        rep.emit_job(
            Some(&output_key(&plan.request.output_root, &sk.output)),
            sk.set.as_deref(),
            Some(&sk.path),
            EventKind::FileSkipped {
                kind: sk.kind(),
                code: sk.code,
                reason: sk.reason.clone(),
            },
        );
    }
    for f in &plan.failed {
        rep.emit_job(
            Some(&output_key(&plan.request.output_root, &f.output)),
            None,
            Some(&f.path),
            EventKind::FileFailed {
                stage: f.stage,
                error: f.error.clone(),
            },
        );
    }
    if !json {
        for ev in events.take() {
            print_event(&ev);
        }
    }
    Ok(i32::from(unreadable > 0 || !plan.failed.is_empty()))
}

/// Mip-0 decoded alpha below this is "varying" for sub-type inference. DECISIONS Note L: alpha whose minimum
/// is 251 or more is opaque (BC encode noise, Community Shaders' 251 opacity gate), so BC7's 254 is not varying.
const VARYING_ALPHA_BELOW: u8 = engine::analyse::CS_COMPLEX_GATE_CODE;

/// Mip-0 decoded alpha at or above this is flat opaque: the file carries no alpha worth keeping, so it is
/// held to the `remove` guarantee (spec §5.8: decoded alpha >= 254 for BC7, 255 otherwise). It is the
/// guarantee's own bound, so a file that passes it is never judged as `data` by mistake, and one that stores
/// even a single lower texel is treated as storing alpha.
const FLAT_ALPHA_MIN: u8 = 254;

/// Mip-0 decoded alpha: its minimum, and whether the texels below the opaque cutoff are only stray noise (the
/// engine's rule, DECISIONS Note FM34), so such a file is opaque for sub-type inference as its analysis would be.
#[derive(Clone, Copy)]
struct Mip0Alpha {
    min: u8,
    speck: bool,
}

/// Mip-0 alpha, for formats that carry alpha (`None` for others and on decode errors).
fn mip0_alpha(file: &DdsFile) -> Option<Mip0Alpha> {
    if !carries_alpha(file.format) {
        return None;
    }
    let (w, h, r) = file.levels.first()?;
    match decode_level(file.format, *w, *h, &file.data[r.clone()]) {
        Ok(Decoded::Ldr(px)) => {
            let low = |i: usize| engine::analyse::is_low_alpha(f32::from(px[i][3]) / 255.0);
            let n_low = (0..px.len()).filter(|&i| low(i)).count() as u64;
            let (w, h) = (*w as usize, *h as usize);
            Some(Mip0Alpha {
                min: alpha_stats(&px).min,
                speck: n_low > 0
                    && px.len() == w * h
                    && engine::analyse::low_alpha_is_speck_noise(w, h, n_low, low),
            })
        }
        _ => None,
    }
}

/// The sub-type a DDS is meant to be when `--subtype` is not given: the top suggestion for its name, path and
/// (for `_m`) stored alpha, else `generic_data`.
fn infer_subtype(path: &Path, file: &DdsFile, target: Target, alpha: Option<Mip0Alpha>) -> Subtype {
    let stem = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    // No user presets are loaded, so there are no suffix aliases to resolve.
    let suffix = split_suffix_with(&stem, &BTreeMap::new()).suffix;
    let abs = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let rel = PathBuf::from(path.file_name().unwrap_or_default());
    if file.is_cubemap {
        return Subtype::Cubemap;
    }
    let alpha_class = if !carries_alpha(file.format) {
        AlphaContent::None
    } else if alpha.is_some_and(|a| a.min < VARYING_ALPHA_BELOW && !a.speck) {
        AlphaContent::Varying
    } else {
        AlphaContent::Constant255
    };
    let set = BTreeSet::new();
    let ctx = SuggestContext {
        target,
        suffix: suffix.as_deref(),
        game_path: &game_path(&abs, &rel),
        set_suffixes: &set,
        alpha: alpha_class,
        is_float: matches!(
            file.format,
            DdsPixelFormat::Rgba16f | DdsPixelFormat::Rgba32f | DdsPixelFormat::Bc6hUf16
        ),
        advanced_skin: false,
    };
    suggest(&ctx)
        .first()
        .map_or(Subtype::GenericData, |g| g.subtype)
}

/// An inferred sub-type that the target does not offer is judged as its fallback (as Resolve's
/// `--allow CFG-UNAVAILABLE` would), or as `generic_data` when it has none; the second value says so.
fn available_or_fallback(st: Subtype, target: Target) -> (Subtype, Option<String>) {
    match availability(target, st) {
        Availability::Available => (st, None),
        Availability::Unavailable { fallback } => {
            let (to, how) = match fallback {
                Some(fb) => (fb, format!("its fallback {}", fb.id())),
                None => (Subtype::GenericData, "generic_data (no fallback)".into()),
            };
            let note = format!(
                "inferred {} is unavailable under {target:?}; judged as {how}",
                st.id()
            );
            (to, Some(note))
        }
    }
}

/// The alpha policy to judge a file by when `--alpha-policy` is not given. A file whose mip-0 alpha is flat
/// opaque (>= `FLAT_ALPHA_MIN`) is `remove`; one that stores alpha keeps the sub-type's own policy when that
/// keeps alpha, else `data`; a file without alpha keeps the sub-type's policy.
fn infer_alpha_policy(alpha_min: Option<u8>, subtype_default: AlphaPolicy) -> AlphaPolicy {
    match alpha_min {
        Some(m) if m >= FLAT_ALPHA_MIN => AlphaPolicy::Remove,
        Some(_) if subtype_default == AlphaPolicy::Remove => AlphaPolicy::Data,
        _ => subtype_default,
    }
}

/// The report for a file in a format sdc never writes (BC2, signed BC6H, uncompressed): there is no profile
/// format to judge it against, so only its header is checked.
fn header_only_report(file: &DdsFile) -> ValidationReport {
    let warnings: Vec<Warning> = check_header(file)
        .into_iter()
        .map(|m| Warning::new(Code::ValHeader, m))
        .collect();
    ValidationReport {
        summary: ValidationSummary {
            passed: warnings.is_empty(),
            levels: file.mip_count,
            ..Default::default()
        },
        warnings,
    }
}

/// One report object as a single JSON line.
fn json_line(v: &DdsValidation) -> String {
    serde_json::to_string(v).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

/// Result-verb output: stdout, or stderr when stdout is reserved for JSON.
fn say(json: bool, line: String) {
    if json {
        err!("{line}");
    } else {
        out!("{line}");
    }
}

/// `sdc validate`: the spec §5.8 checks on existing DDS files. The profile comes from the sub-type and the
/// target, but its format, `_SRGB` tag and alpha policy from the file, so third-party files are judged on the
/// header rules and the alpha rules rather than on sdc's preferred format. `alpha.exact_opaque` is off unless
/// `--exact-opaque` is given. A format sdc never writes only has its header checked. Exit 1 if a file cannot
/// be read, else 4 if one fails validation, else 0.
pub fn validate_files(a: ValidateArgs) -> Result<i32, EngineError> {
    if let Some(p) = a.dds.iter().find(|p| p.is_dir()) {
        return Err(EngineError::Usage(format!(
            "{} is a folder; give .dds files",
            p.display()
        )));
    }
    let target = a.target.map_or(Target::Cs, Target::from);
    let json = a.json;
    // rgbcx's mode is global state; set it before any decode.
    encoders_sys::init();
    let reg = Engine::new()?.registry().clone();
    let explicit: Option<Subtype> = a
        .subtype
        .as_ref()
        .map(|id| {
            serde_json::from_value(Value::String(id.clone()))
                .map_err(|_| EngineError::Usage(format!("--subtype {id}: unknown sub-type")))
        })
        .transpose()?;
    if let Some(st) =
        explicit.filter(|&st| !matches!(availability(target, st), Availability::Available))
    {
        return Err(EngineError::Usage(format!(
            "--subtype {} is unavailable under {target:?} (CFG-UNAVAILABLE)",
            st.id()
        )));
    }
    let explicit_policy: Option<AlphaPolicy> = a
        .alpha_policy
        .as_ref()
        .map(|id| {
            serde_json::from_value(Value::String(id.clone()))
                .map_err(|_| EngineError::Usage(format!("--alpha-policy {id}: unknown policy")))
        })
        .transpose()?;
    let (mut unreadable, mut failed, mut not_judged) = (0u32, 0u32, 0u32);
    let total = a.dds.len();
    for path in &a.dds {
        let path_s = path.to_string_lossy().replace('\\', "/");
        let (bytes, file) = match read_file(path) {
            Ok(r) => r,
            Err(e) => {
                say(json, format!("FAIL  {path_s}: cannot be read: {e}"));
                if json {
                    let rec = DdsValidation::Unreadable(DdsValidationUnreadable {
                        kind: DdsValidationKind::DdsValidation,
                        schema: REPORT_SCHEMA,
                        path: path_s,
                        readable: false,
                        passed: false,
                        error: e,
                    });
                    out!("{}", json_line(&rec));
                }
                unreadable += 1;
                continue;
            }
        };
        let alpha = mip0_alpha(&file);
        let alpha_min = alpha.map(|a| a.min);
        let (st, mut note) = match explicit {
            Some(st) => (st, None),
            None => available_or_fallback(infer_subtype(path, &file, target, alpha), target),
        };
        let mut policy = None;
        let mut judged = true;
        let mut report = match file.format.as_format() {
            None => {
                judged = false;
                let (name, _) = format_name(file.format);
                note = Some(format!("not judged: format not produced by sdc ({name})"));
                header_only_report(&file)
            }
            Some((format, srgb)) => {
                let mut p = reg
                    .profile(default_preset_for(target), target, st)
                    .map_err(|e| EngineError::Fatal(e.to_string()))?;
                p.format = format;
                p.color.tag = match (format, srgb) {
                    (f, _) if f.is_hdr() => ColorTag::NotApplicable,
                    (_, true) => ColorTag::Srgb,
                    (_, false) => ColorTag::Unorm,
                };
                p.alpha.policy = explicit_policy
                    .unwrap_or_else(|| infer_alpha_policy(alpha_min, p.alpha.policy));
                // Codex-CLI 3: exact-opaque is a promise of sdc's own encoder (on by default for `envmask` and
                // `complex_material`), not of the file, so it is not inherited from the sub-type's preset.
                p.alpha.exact_opaque = a.exact_opaque;
                policy = Some(p.alpha.policy);
                validate(&engine::validate::ValidationInput {
                    bytes: &bytes,
                    profile: &p,
                    encoder_input: None,
                    base_coverage: None,
                    source_peak: None,
                    expected_levels: None,
                })
            }
        };
        // A 2D file where a cube is expected fails, as a cube's own issues do (FM).
        for m in engine::cube::checks::inspect_issues(&path_s, &file) {
            report.summary.passed = false;
            report.warnings.push(Warning::new(Code::ValHeader, m));
        }
        let passed = report.summary.passed;
        let alpha_part = match policy {
            Some(p) if carries_alpha(file.format) => format!(", alpha {}", policy_id(p)),
            _ => String::new(),
        };
        let word = match (passed, judged) {
            (false, _) => "FAIL ",
            (true, true) => "ok   ",
            (true, false) => "skip ",
        };
        say(json, format!("{word} {path_s} ({}{alpha_part})", st.id()));
        if let Some(n) = &note {
            say(json, format!("  note: {n}"));
        }
        for w in &report.warnings {
            say(
                json,
                format!(
                    "  {} {} {}",
                    format!("{:?}", w.severity).to_lowercase(),
                    w.code.as_str(),
                    w.message
                ),
            );
        }
        if json {
            let rec = DdsValidation::Validated(DdsVerdict {
                kind: DdsValidationKind::DdsValidation,
                schema: REPORT_SCHEMA,
                path: path_s,
                readable: true,
                subtype: st,
                alpha_policy: policy,
                judged,
                note,
                passed,
                summary: report.summary,
                warnings: report.warnings,
            });
            out!("{}", json_line(&rec));
        }
        failed += u32::from(!passed);
        not_judged += u32::from(passed && !judged);
    }
    let ok = total as u32 - failed - unreadable - not_judged;
    let skipped = if not_judged > 0 {
        format!(", {not_judged} not judged")
    } else {
        String::new()
    };
    say(
        json,
        format!("validated {total}: {ok} ok, {failed} failed validation, {unreadable} unreadable{skipped}"),
    );
    Ok(if unreadable > 0 {
        1
    } else if failed > 0 {
        4
    } else {
        0
    })
}

fn policy_id(p: AlphaPolicy) -> String {
    match serde_json::to_value(p) {
        Ok(Value::String(s)) => s,
        _ => format!("{p:?}"),
    }
}

/// `sdc presets …`. Result verb: the report goes to stdout (DECISIONS Note J); skipped user presets go to stderr.
pub fn presets(args: PresetsArgs) -> Result<i32, EngineError> {
    let engine = Engine::new()?;
    let mut reg = engine.registry().clone();
    if let Some(dir) = engine::presets::user_presets_dir() {
        for w in reg.add_dir(&dir) {
            err!("{}: {}", w.code.as_str(), w.message);
        }
    }
    for f in &args.preset_file {
        reg.add_file(f)
            .map_err(|e| EngineError::Config(e.to_string()))?;
    }
    let reg = &reg;
    match args.cmd {
        PresetsCmd::List { json } => {
            for p in reg.selectable() {
                if json {
                    out!(
                        "{}",
                        json!({
                            "id": p.id, "version": p.version, "target": p.target,
                            "description": p.description, "suffix_aliases": p.suffix_aliases,
                            "source": reg.source(&p.id).unwrap_or("builtin"),
                        })
                    );
                } else {
                    let from = match reg.source(&p.id) {
                        Some(s) if s != engine::presets::SOURCE_BUILTIN => format!("  from {s}"),
                        _ => String::new(),
                    };
                    let aliases = alias_list(&p.suffix_aliases)
                        .map(|a| format!("  aliases: {a}"))
                        .unwrap_or_default();
                    out!(
                        "{:<18} {:<8} {:<8} {}{aliases}{from}",
                        p.id,
                        p.version,
                        p.target
                            .map(|t| format!("{t:?}").to_lowercase())
                            .unwrap_or_default(),
                        p.description
                    );
                }
            }
            Ok(0)
        }
        PresetsCmd::Show {
            id,
            subtype,
            target,
        } => {
            let usage = |e: engine::presets::PresetError| EngineError::Usage(e.to_string());
            reg.chain(&id).map_err(usage)?;
            let preset = reg
                .get(&id)
                .ok_or_else(|| EngineError::Usage(format!("unknown preset '{id}'")))?;
            let target = target
                .map(Target::from)
                .or(preset.target)
                .unwrap_or(Target::Cs);
            let subtypes: Vec<Subtype> = match subtype {
                Some(s) => vec![serde_json::from_value(Value::String(s.clone()))
                    .map_err(|_| EngineError::Usage(format!("--subtype {s}: unknown sub-type")))?],
                None => Subtype::ALL.to_vec(),
            };
            if let Some(a) = alias_list(&preset.suffix_aliases) {
                out!("suffix aliases: {a}");
            }
            for st in subtypes {
                // The effort a BC6H sub-type encodes at (Note FM43), with the reason in its provenance.
                let (profile, bc6h_default) =
                    reg.effective_profile(&id, target, st).map_err(usage)?;
                let mut prov = BTreeMap::new();
                for (source, patch) in reg.layers(&id, st).map_err(usage)? {
                    let mut paths = vec![];
                    engine::presets::leaf_paths(&patch, "", &mut paths);
                    for p in paths {
                        prov.insert(p, source.to_string());
                    }
                }
                if bc6h_default {
                    prov.insert("encoder.effort".into(), "builtin (BC6H default)".into());
                }
                let shown = json!({ "subtype": st, "profile": profile, "provenance": prov });
                out!(
                    "{}",
                    serde_json::to_string_pretty(&shown).unwrap_or_default()
                );
            }
            Ok(0)
        }
        PresetsCmd::Schema { which, out } => {
            match (SchemaKind::from(which), out) {
                (SchemaKind::All, Some(dir)) => {
                    engine::schemas::write_all(&dir)
                        .map_err(|e| EngineError::Fatal(format!("{}: {e}", dir.display())))?;
                    out!(
                        "wrote {} schemas to {}",
                        SchemaKind::FILES.len(),
                        dir.display()
                    );
                }
                (SchemaKind::All, None) => {
                    return Err(EngineError::Usage("schema all needs --out <dir>".into()))
                }
                (kind, Some(dir)) => {
                    std::fs::create_dir_all(&dir)
                        .and_then(|()| {
                            std::fs::write(
                                dir.join(kind.file_name()),
                                engine::schemas::schema_json(kind),
                            )
                        })
                        .map_err(|e| EngineError::Fatal(format!("{}: {e}", dir.display())))?;
                    out!("wrote {} to {}", kind.file_name(), dir.display());
                }
                (kind, None) => out_raw!("{}", engine::schemas::schema_json(kind)),
            }
            Ok(0)
        }
    }
}

/// `alias→canonical, ...`, or `None` when there are no aliases.
fn alias_list(aliases: &BTreeMap<String, String>) -> Option<String> {
    (!aliases.is_empty()).then(|| {
        aliases
            .iter()
            .map(|(a, c)| format!("{a}\u{2192}{c}"))
            .collect::<Vec<_>>()
            .join(", ")
    })
}

/// `sdc cubemap-marker`: 0 written, 1 kept (a file Tessalith did not write), 2 usage, 3 write failed.
pub fn cubemap_marker(a: MarkerArgs) -> Result<i32, EngineError> {
    use engine::cube::marker::{alpha_of, write_marker};
    let alpha = alpha_of(a.roughness).map_err(EngineError::Usage)?;
    match write_marker(&a.out, a.color, alpha).map_err(EngineError::Fatal)? {
        Ok(()) => {
            out!("wrote {}", a.out.display());
            Ok(0)
        }
        Err(note) => {
            out!("{note}");
            Ok(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_name_splits_the_srgb_tag() {
        assert_eq!(
            format_name(DdsPixelFormat::Bc7 { srgb: true }),
            ("Bc7".into(), Some(true))
        );
        assert_eq!(format_name(DdsPixelFormat::Bc5), ("Bc5".into(), None));
        assert_eq!(
            format_name(DdsPixelFormat::Bgra8 { srgb: false }),
            ("Bgra8".into(), Some(false))
        );
    }

    #[test]
    fn alpha_policy_is_inferred_from_the_stored_alpha() {
        use AlphaPolicy::*;
        // Flat opaque (BC7's 254 included) is `remove`, whatever the sub-type's default.
        assert_eq!(infer_alpha_policy(Some(255), Data), Remove);
        assert_eq!(infer_alpha_policy(Some(254), Blend), Remove);
        // A single texel below 254 means the file stores alpha: `data` where the default would remove it.
        assert_eq!(infer_alpha_policy(Some(239), Remove), Data);
        assert_eq!(infer_alpha_policy(Some(253), Remove), Data);
        assert_eq!(infer_alpha_policy(Some(0), Cutout), Cutout);
        assert_eq!(infer_alpha_policy(Some(0), CutoutBlend), CutoutBlend);
        // No alpha in the format: the sub-type's own policy.
        assert_eq!(infer_alpha_policy(None, Remove), Remove);
        assert_eq!(infer_alpha_policy(None, Data), Data);
    }

    #[test]
    fn strip_root_handles_case_separators_and_foreign_paths() {
        let root = "c:/users/x/temp/sdc-inspect-ab";
        let s = |v| strip_root(v, root);
        assert_eq!(
            s("C:\\Users\\X\\Temp\\sdc-inspect-AB\\a\\Rock_N.dds").as_deref(),
            Some("a/Rock_N.dds")
        );
        assert_eq!(
            s("c:/users/x/temp/sdc-inspect-ab/a/rock_n").as_deref(),
            Some("a/rock_n")
        );
        // 'İ' lowers to two chars, so the root is longer than the path's own prefix.
        let root_i = "c:/users/İbrahim/t".to_lowercase();
        assert_eq!(
            strip_root("C:\\Users\\İbrahim\\t\\a.dds", &root_i).as_deref(),
            Some("a.dds")
        );
        assert_eq!(s("in/rock_n.png"), None);
        assert_eq!(s("c:/users/x/temp/sdc-inspect-abc/a"), None);
    }
}
