//! Concise human log on stderr (spec §8: human output on stderr; `--json` keeps stdout for JSON Lines only).
use engine::codes::{Code, Severity, Warning};
use engine::events::{EngineEvent, EventKind, EventSink, SkipKind};
use std::io::Write;
use std::sync::{Arc, Mutex};

pub struct HumanSink {
    err: Mutex<std::io::Stderr>,
    /// Note JA6: the counts of the run-level `OUT-KEPT-OLDER-ENCODE` and `OUT-KEPT-LOWER-TARGET` events, which come
    /// before `batch_done` (it carries no kept counts); the done line reads and clears them, once per phase.
    kept: Mutex<(u64, u64)>,
    /// Whether the run validates (`sdc convert --validate`); the done line names the validation count accordingly.
    validated: bool,
}

impl HumanSink {
    pub fn new() -> Self {
        HumanSink {
            err: Mutex::new(std::io::stderr()),
            kept: Mutex::new((0, 0)),
            validated: false,
        }
    }

    /// Sets whether the run validates its outputs (`new()` means not validating).
    pub fn validating(mut self, on: bool) -> Self {
        self.validated = on;
        self
    }
}

/// The done line's validation clause: the count when the run validates; `validation off` when it does not, or the
/// count of the always-validated types (complex material and grass) that failed.
fn validation_clause(validated: bool, failed: u32) -> String {
    if validated {
        format!("{failed} failed validation")
    } else if failed > 0 {
        format!("{failed} failed validation (complex types only)")
    } else {
        "validation off".to_string()
    }
}

/// `, {k} kept from an older encode version (--ev re-encodes), {t} kept at a higher target`, naming only the counts
/// above 0 (nothing when both are 0).
fn kept_clause((older, lower): (u64, u64)) -> String {
    let mut s = String::new();
    if older > 0 {
        s += &format!(", {older} kept from an older encode version (--ev re-encodes)");
    }
    if lower > 0 {
        s += &format!(", {lower} kept at a higher target");
    }
    s
}

impl Default for HumanSink {
    fn default() -> Self {
        Self::new()
    }
}

/// `severity CODE message`; with a `label` (a file or set) the severity is padded and the label leads:
/// `warning label: CODE message`.
pub fn warning_line(label: Option<&str>, w: &Warning) -> String {
    // The serde form, as the JSON events carry it.
    let sev = match serde_json::to_value(w.severity) {
        Ok(serde_json::Value::String(s)) => s,
        _ => "-".to_string(),
    };
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
        let kept = || self.kept.lock().unwrap_or_else(|e| e.into_inner());
        let line = match &ev.kind {
            EventKind::Warning(w)
                if matches!(w.code, Code::OutKeptOlderEncode | Code::OutKeptLowerTarget) =>
            {
                let n = w.data["count"].as_u64().unwrap_or(0);
                let mut k = kept();
                if w.code == Code::OutKeptOlderEncode {
                    k.0 += n;
                } else {
                    k.1 += n;
                }
                None
            }
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
                checked,
                checked_failed,
                cancelled,
            } => Some(format!(
                "done: {converted} converted, {up_to_date} up to date, {skipped} skipped ({skipped_intentional} on purpose), {failed} failed, {}{}, {} cancelled{}",
                validation_clause(self.validated, *validation_failed),
                if *checked > 0 { format!(", {checked} checked in place ({checked_failed} failed)") } else { String::new() },
                cancelled.len(),
                kept_clause(std::mem::take(&mut *kept()))
            )),
            _ => None,
        };
        if let Some(l) = line {
            let _ = writeln!(self.err.lock().unwrap_or_else(|e| e.into_inner()), "{l}");
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

#[cfg(test)]
mod tests {
    use super::validation_clause;

    #[test]
    fn the_done_line_validation_clause_has_three_cases() {
        assert_eq!(validation_clause(true, 2), "2 failed validation");
        assert_eq!(validation_clause(false, 0), "validation off");
        assert_eq!(
            validation_clause(false, 1),
            "1 failed validation (complex types only)"
        );
    }
}
