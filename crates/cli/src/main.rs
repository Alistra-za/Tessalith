//! `sdc` command line (spec §8). Human output goes to stderr; with `--json`, stdout carries only JSON Lines.
//! Exit codes, highest precedence first: 3 fatal > 2 usage/config > 1 partial > 4 validation failed > 0.
/// `eprintln!` that ignores write errors: a closed stderr is not a crash, and a panic while reporting one would end
/// the process with 101 outside `catch_unwind` (trace-debug #18, Note BQ).
macro_rules! err {
    ($($t:tt)*) => {{
        use std::io::Write as _;
        let _ = writeln!(std::io::stderr().lock(), $($t)*);
    }};
}

mod args;
mod cancel;
mod dilate;
mod human;
mod uv;
mod verbs;

use args::*;
use clap::Parser;
use engine::also::{run_also, AlsoSpec};
use engine::api::{CancelToken, Engine, EngineError, EvScope};
use engine::events::{new_batch_id, EventSink, JsonLinesSink, Reporter};
use engine::session::AnalysisCache;
use std::process::ExitCode;
use std::sync::Arc;

/// Human output on stderr, plus JSON Lines on stdout with `--json`. `run_also` takes this and makes its own reporters.
fn tee_sink(json: bool) -> Arc<dyn EventSink> {
    let mut sinks: Vec<Arc<dyn EventSink>> = vec![Arc::new(human::HumanSink::new())];
    if json {
        sinks.push(Arc::new(JsonLinesSink::new(std::io::stdout())));
    }
    Arc::new(human::TeeSink(sinks))
}

fn reporter(json: bool) -> Reporter {
    Reporter::new(tee_sink(json), new_batch_id())
}

/// `SDC_PROFILE`'s folder: `None` when unset or empty; a missing folder is created. A path that is not a folder, or
/// cannot be created, is an error (a usage error: nothing has been written).
fn profile_dir(v: Option<std::ffi::OsString>) -> Result<Option<std::path::PathBuf>, String> {
    let Some(v) = v.filter(|v| !v.is_empty()) else {
        return Ok(None);
    };
    let path = std::path::PathBuf::from(v);
    let fail = |why: String| format!("SDC_PROFILE must name a folder: {}: {why}", path.display());
    if path.exists() {
        if !path.is_dir() {
            return Err(fail("not a folder".into()));
        }
    } else {
        std::fs::create_dir_all(&path).map_err(|e| fail(e.to_string()))?;
    }
    Ok(Some(path))
}

/// Write the report after a batch; a failure is a warning and never changes the exit code.
fn write_profile(rec: &engine::profile::Recorder, dir: &std::path::Path, batch: &str) {
    match rec.write_report(dir, batch, std::env::args().collect()) {
        Ok(p) => err!("profile: {}", p.display()),
        Err(e) => err!("warning: profile report not written: {e}"),
    }
}

fn convert(a: ConvertArgs) -> Result<i32, EngineError> {
    let profile_out = profile_dir(std::env::var_os("SDC_PROFILE")).map_err(EngineError::Usage)?;
    let out = a
        .output
        .clone()
        .unwrap_or_else(|| std::path::PathBuf::from("sdc-out"));
    let mut req = build_request(&a.batch, out.clone()).map_err(EngineError::Usage)?;
    req.jobs = a.jobs;
    req.mem_budget = a
        .mem_budget
        .as_deref()
        .map(parse_bytes)
        .transpose()
        .map_err(EngineError::Usage)?
        .unwrap_or(0);
    req.on_existing = a.on_existing.into();
    // Note JA7: clap's `requires` can't refuse `--ev` with a mode that has a default, so the check is here.
    if a.ev && !matches!(a.on_existing, OnExistingArg::IfChanged) {
        return Err(EngineError::Usage(
            "--ev applies only with --on-existing if-changed".into(),
        ));
    }
    req.ev = match (a.ev, a.property.is_empty()) {
        (false, _) => EvScope::Off,
        (true, true) => EvScope::All,
        (true, false) => EvScope::matching(a.property).map_err(EngineError::Usage)?,
    };
    req.dry_run = a.dry_run;
    let profile = profile_out
        .as_deref()
        .map(|dir| (dir, engine::profile::Recorder::new()));
    req.profile = profile.as_ref().map(|(_, r)| r.clone());
    // Before planning, so a Ctrl-C during a long scan is not lost. A failed install is not fatal, but it is
    // reported: without the handler Ctrl-C ends the process at once, as for any console program.
    let cancel = CancelToken::default();
    if let Err(e) = cancel::install(cancel.clone()) {
        err!(
            "warning: could not install the Ctrl-C handler ({e}); Ctrl-C will end sdc immediately"
        );
    }
    let engine = Engine::new()?;
    // Track A (Note CN): `[export] also` in the project `sdc.toml` writes the three folders under `-o`. A bad
    // `[export.legacy]` value is an error on this path only (O2): `run_also` checks it, `export_config` does not.
    let cfg = engine.export_config(&req)?;
    if !cfg.also.is_empty() {
        let spec = AlsoSpec {
            cfg,
            // The export folder itself, not the request's output root below it (Note HQ1).
            location: out,
            // The CLI reads no scan (TA13, TA18), so no env `_m` is written.
            scanned: false,
        };
        // `run_also` returns Ok even when the PBR phase failed (CN56), so the exit code is the summary's.
        let sum = run_also(
            &engine,
            &AnalysisCache::default(),
            req,
            &spec,
            tee_sink(a.batch.json),
            &cancel,
        );
        if let Some((dir, rec)) = &profile {
            write_profile(rec, dir, &new_batch_id());
        }
        let sum = sum?;
        // Plain lines on stderr (stdout carries the JSON lines): the files this export did not write, and the sets left out.
        for n in &sum.notes {
            err!("{n}");
        }
        for n in &sum.sets {
            err!("{}: {}", n.id, n.why);
        }
        return Ok(sum.exit_code());
    }
    let rep = reporter(a.batch.json);
    let result = engine
        .plan(req, &rep, &cancel)
        .and_then(|plan| engine.run(plan, &rep, &cancel));
    if let Some((dir, rec)) = &profile {
        write_profile(rec, dir, &rep.batch);
    }
    Ok(result?.exit_code())
}

fn run(cli: Cli) -> Result<i32, EngineError> {
    match cli.cmd {
        Command::Convert(a) => convert(a),
        Command::Inspect(a) => verbs::inspect(a),
        Command::Validate(a) => verbs::validate_files(a),
        Command::Presets(c) => verbs::presets(c),
        Command::CubemapMarker(a) => verbs::cubemap_marker(a),
        Command::Uv(a) => uv::run(a),
        Command::Dilate(a) => dilate::run(a),
    }
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) => {
            let code = if e.use_stderr() { 2 } else { 0 };
            let _ = e.print();
            return ExitCode::from(code);
        }
    };
    // An internal error (panic) is a fatal exit 3, not Rust's 101 (spec §8).
    match std::panic::catch_unwind(|| run(cli)) {
        Ok(Ok(code)) => ExitCode::from(code as u8),
        Ok(Err(e)) => {
            err!("sdc: {e}");
            ExitCode::from(e.exit_code() as u8)
        }
        Err(_) => {
            err!("sdc: internal error");
            ExitCode::from(3)
        }
    }
}
