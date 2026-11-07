//! The headless UI driver: runs the real app on a generated project, replays a JSON-lines step script and writes a
//! screenshot and the AccessKit tree after every step. It opens no window: file dialogs are answered by a fake
//! backend and Explorer requests are recorded (Note HM3). Every run starts from a fresh fixture (Note HM2).
//!
//! Usage:
//!   cargo test --offline -p sdc-gui --test ui_drive -- --fixture <name> --script <steps.jsonl> --out <dir>
//!       [--size <W>x<H>] [--theme dark|light]
//!   cargo test --offline -p sdc-gui --test ui_drive -- --list      fixture names
//!   Any other arguments (cargo test's own, e.g. --skip render_): prints one line and exits 0.
//! Exit 0: every step ran. 1: a step failed (its frame and run.json are still written). 2: bad arguments or fixture,
//! or a frame could not be written (run.json is still written, with that step marked failed).
//!
//! Steps, one JSON object per line (`op` picks the kind; a target names exactly one of `label`, `starts`,
//! `placeholder`, plus optional `role` and `index`, 0 = first in reading order):
//!   {"op":"click","at":{"label":"Open"},"mods":"Ctrl+Shift"}   also double_click, right_click, hover
//!   {"op":"type","text":"abc","at":{"placeholder":"Search"}}   `at` clicks the field first
//!   {"op":"key","key":"Ctrl+K"}        {"op":"drag","from":{..},"to":{..}}
//!   {"op":"scroll","at":{..},"dir":"up|down|left|right|into_view","times":3}
//!   {"op":"resize","w":1100,"h":700}   {"op":"settle","frames":4}
//!   {"op":"pick","path":"textures"}    queue the answer of the next file dialog (a path inside the fixture folder;
//!                                      no answer queued = Cancel)
//!
//! Output in --out: `00` is the settled start, `NN` follows step NN. Per frame `NN.tree.txt` (the AccessKit tree, with
//! focused/disabled/hovered/clipped/offscreen marks) and `NN.png`. `steps.jsonl` is the script; `run.json` holds
//! {fixture, size, theme, adapter, comparable, ms_fixture, ms_total, steps: [{n, line, ok, ms, error, note}],
//! dialogs, shell: [{step, request}]}. `ms_fixture` is the fixture build, app start and first settle. Screenshots are comparable with the doc shots only when the adapter is the
//! one in shots.toml; the driver says so when it is not.

#[path = "../app_harness/mod.rs"]
mod app_harness;
mod fixtures;
mod frame;
#[allow(dead_code)] // fields only the doc shots read
#[path = "../doc_shots/manifest.rs"]
mod manifest;
mod steps;
#[path = "../support/mod.rs"]
mod support;

use app_harness::{adapter_name, redirect_app_data, settle, H};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::time::Instant;

struct Args {
    fixture: String,
    script: PathBuf,
    out: PathBuf,
    size: Option<(f32, f32)>,
    light: Option<bool>,
}

enum Parsed {
    Passthrough,
    List,
    Run(Args),
}

fn parse_size(s: &str) -> Result<(f32, f32), String> {
    let (w, h) = s
        .split_once('x')
        .ok_or_else(|| format!("--size \"{s}\" is not <W>x<H>"))?;
    let n = |v: &str| {
        v.parse::<f32>()
            .ok()
            .filter(|v| *v >= 200.0)
            .ok_or_else(|| format!("--size \"{s}\" is not <W>x<H>"))
    };
    Ok((n(w)?, n(h)?))
}

fn parse_args(args: &[String]) -> Result<Parsed, String> {
    const OURS: [&str; 6] = [
        "--fixture",
        "--script",
        "--out",
        "--size",
        "--theme",
        "--list",
    ];
    if !args.iter().any(|a| OURS.contains(&a.as_str())) {
        return Ok(Parsed::Passthrough);
    }
    if args.iter().any(|a| a == "--list") {
        return Ok(Parsed::List);
    }
    let (mut fixture, mut script, mut out, mut size, mut light) = (None, None, None, None, None);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = || {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{a} needs a value"))
        };
        match a.as_str() {
            "--fixture" => fixture = Some(value()?),
            "--script" => script = Some(PathBuf::from(value()?)),
            "--out" => out = Some(PathBuf::from(value()?)),
            "--size" => size = Some(parse_size(&value()?)?),
            "--theme" => {
                light = Some(match value()?.as_str() {
                    "light" => true,
                    "dark" => false,
                    t => return Err(format!("--theme \"{t}\" is not dark or light")),
                })
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    fn need<T>(o: Option<T>, n: &str) -> Result<T, String> {
        o.ok_or_else(|| format!("{n} is required"))
    }
    Ok(Parsed::Run(Args {
        fixture: need(fixture, "--fixture")?,
        script: need(script, "--script")?,
        out: need(out, "--out")?,
        size,
        light,
    }))
}

fn write_frame(h: &mut H, out: &Path, n: usize) -> Result<(), String> {
    let text = frame::tree_text(&frame::snapshot(h));
    std::fs::write(out.join(format!("{n:02}.tree.txt")), text).map_err(|e| e.to_string())?;
    h.render()?
        .save(out.join(format!("{n:02}.png")))
        .map_err(|e| e.to_string())
}

/// Stale frames and the last run.json go; anything else in `out` is the caller's.
fn clear_out(out: &Path) {
    let Ok(rd) = std::fs::read_dir(out) else {
        return;
    };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let b = name.as_bytes();
        if name == "run.json"
            || (b.len() > 2 && b[0].is_ascii_digit() && b[1].is_ascii_digit() && b[2] == b'.')
        {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

fn run(a: Args) -> Result<i32, String> {
    let text = std::fs::read_to_string(&a.script)
        .map_err(|e| format!("cannot read {}: {e}", a.script.display()))?;
    let script = steps::parse_script(&text)?;
    fixtures::lookup(&a.fixture)?;
    std::fs::create_dir_all(&a.out).map_err(|e| e.to_string())?;
    clear_out(&a.out);
    let copy = a.out.join("steps.jsonl");
    if std::fs::canonicalize(&a.script).ok() != std::fs::canonicalize(&copy).ok() {
        std::fs::write(&copy, &text).map_err(|e| e.to_string())?;
    }

    let t0 = Instant::now();
    let tmp = tempfile::tempdir().map_err(|e| e.to_string())?;
    redirect_app_data(tmp.path());
    let dir = tmp.path().join("fixture");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let (mut h, run) = fixtures::start(&a.fixture, &dir, true)?;
    let ms_fixture = t0.elapsed().as_millis();
    if let Some((w, hh)) = a.size {
        h.set_size(eframe::egui::vec2(w, hh));
        settle(&mut h, 4);
    }
    let render = manifest::load(&manifest::repo_root().join("docs/img/gui/shots.toml")).render;
    let light = a.light.unwrap_or(render.theme == "light");
    if a.light.is_some() {
        h.ctx.set_theme(if light {
            eframe::egui::Theme::Light
        } else {
            eframe::egui::Theme::Dark
        });
        settle(&mut h, 2);
    }
    let adapter = adapter_name();
    let comparable = adapter == render.adapter;
    if !comparable {
        println!("screenshots not comparable: adapter {adapter}");
    }
    let (mut rows, mut shell, mut code) = (vec![], vec![], 0);
    let mut start_error = write_frame(&mut h, &a.out, 0).err();
    if start_error.is_some() {
        code = 2;
    }
    for (i, (line, step)) in script.iter().enumerate() {
        if code != 0 {
            break;
        }
        let n = i + 1;
        let t = Instant::now();
        let res = fixtures::app_step(&mut h, &run, step);
        let ms = t.elapsed().as_millis();
        let log = h.state_mut().shell_log.as_mut().map(std::mem::take);
        for r in log.into_iter().flatten() {
            shell.push(json!({"step": n, "request": format!("{r:?}")}));
        }
        let written = write_frame(&mut h, &a.out, n);
        let (mut ok, mut error, note) = match res {
            Ok(note) => (true, None, note),
            Err(e) => (false, Some(e), None),
        };
        if !ok {
            code = 1;
        }
        if let Err(e) = written {
            // exit 2 outranks a failed step: the frame, not the script, is what broke
            ok = false;
            code = 2;
            error = Some(match error {
                Some(s) => format!("{s}; frame not written: {e}"),
                None => format!("frame not written: {e}"),
            });
        }
        println!("{n:02} {} {line}", if ok { "ok" } else { "FAIL" });
        if let Some(e) = &error {
            println!("    {e}");
        }
        rows.push(json!({"n": n, "line": line, "ok": ok, "ms": ms, "error": error, "note": note}));
        if !ok {
            break;
        }
    }
    if let Some(e) = &start_error {
        eprintln!("ui_drive: frame 00 not written: {e}");
    }
    let s = h.ctx.content_rect().size();
    let summary = json!({
        "fixture": a.fixture,
        "size": [s.x, s.y],
        "theme": if light { "light" } else { "dark" },
        "adapter": adapter,
        "comparable": comparable,
        "ms_fixture": ms_fixture,
        "ms_total": t0.elapsed().as_millis(),
        "steps": rows,
        "dialogs": *run.picker.asked.lock().unwrap(),
        "shell": shell,
        "error": start_error.take(),
    });
    std::fs::write(
        a.out.join("run.json"),
        serde_json::to_string_pretty(&summary).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    Ok(code)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match parse_args(&args) {
        Ok(Parsed::Passthrough) => {
            println!("ui_drive: no --fixture given, nothing to run (see the header of tests/ui_drive/main.rs)");
            0
        }
        Ok(Parsed::List) => {
            for (n, _) in fixtures::FIXTURES {
                println!("{n}");
            }
            0
        }
        Ok(Parsed::Run(a)) => run(a).unwrap_or_else(|e| {
            eprintln!("ui_drive: {e}");
            2
        }),
        Err(e) => {
            eprintln!("ui_drive: {e}");
            2
        }
    };
    std::process::exit(code);
}
