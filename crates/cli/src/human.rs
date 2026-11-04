//! Concise human log on stderr (spec §8: human output on stderr; `--json` keeps stdout for JSON Lines only).
use engine::codes::{Code, Severity, Warning};
use engine::events::{EngineEvent, EventKind, EventSink, SkipKind};
use std::io::Write;
use std::sync::{Arc, Mutex};

pub struct HumanSink(Mutex<std::io::Stderr>);

impl HumanSink {
    pub fn new() -> Self {
        HumanSink(Mutex::new(std::io::stderr()))
    }
}

impl Default for HumanSink {
    fn default() -> Self {
        Self::new()
    }
}

/// `severity CODE message`; with a `label` (a file or set) the severity is padded and the label leads:
/// `warning label: CODE message`.
pub fn warning_line(label: Option<&str>, w: &Warning) -> String {
    let sev = format!("{:?}", w.severity).to_lowercase();
    match label {
        Some(l) => format!("{sev:<5} {l}: {} {}", w.code.as_str(), w.message),
        None => format!("{sev} {} {}", w.code.as_str(), w.message),
    }
}

/// `[CODE ]reason`, the tail of a skip line.
pub fn skip_line(code: Option<Code>, reason: &str) -> String {
    format!(
        "{}{reason}",
        code.map(|c| format!("{} ", c.as_str())).unwrap_or_default()
    )
}

impl EventSink for HumanSink {
    fn emit(&self, ev: &EngineEvent) {
        let path = ev.path.as_deref().unwrap_or_default();
        let line = match &ev.kind {
            EventKind::FileDone { output, validation } => Some(format!(
                "ok    {path} -> {output}{}",
                if validation.passed {
                    ""
                } else {
                    "  [VALIDATION FAILED]"
                }
            )),
            // A raw map consumed by a recipe is reported by its packed output; the log stays as it was.
            EventKind::FileSkipped {
                kind: SkipKind::PackedSource,
                ..
            } => None,
            EventKind::FileSkipped { code, reason, .. } => {
                Some(format!("skip  {path}: {}", skip_line(*code, reason)))
            }
            EventKind::FileFailed { stage, error } => {
                Some(format!("FAIL  {path} at {stage:?}: {error}"))
            }
            // S-mips-13: the one Info the log shows, so an unscoped filter that a texture ignores is not silent.
            EventKind::Warning(w)
                if w.severity >= Severity::Warning || w.code == Code::CfgFilterIgnored =>
            {
                Some(warning_line(Some(path), w))
            }
            // A set's SHADER-* warnings, as `inspect` lists them; the set takes the file's place.
            EventKind::SetResolved { warnings, .. } => {
                let set = ev.set.as_deref().unwrap_or("");
                let lines: Vec<String> = warnings
                    .iter()
                    .filter(|w| w.severity >= Severity::Warning)
                    .map(|w| warning_line(Some(set), w))
                    .collect();
                (!lines.is_empty()).then(|| lines.join("\n"))
            }
            EventKind::BatchDone {
                converted,
                up_to_date,
                skipped,
                skipped_intentional,
                failed,
                validation_failed,
                cancelled,
            } => Some(format!(
                "done: {converted} converted, {up_to_date} up to date, {skipped} skipped ({skipped_intentional} on purpose), {failed} failed, {validation_failed} failed validation, {} cancelled",
                cancelled.len()
            )),
            _ => None,
        };
        if let Some(l) = line {
            let _ = writeln!(self.0.lock().unwrap_or_else(|e| e.into_inner()), "{l}");
        }
    }
}

/// Fans one event out to several sinks.
pub struct TeeSink(pub Vec<Arc<dyn EventSink>>);

impl EventSink for TeeSink {
    fn emit(&self, ev: &EngineEvent) {
        for s in &self.0 {
            s.emit(ev);
        }
    }
}
