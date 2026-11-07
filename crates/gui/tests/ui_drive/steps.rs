#![allow(dead_code)]
//! The step format of the headless UI driver and the code that runs a UI step on any `Harness`.
//! Steps are JSON lines; `Pick`, `Expect` and `Check` need the app and are run elsewhere.

use crate::frame::{names, snapshot, visible_centre, Node, Snapshot};
use eframe::egui::{self, Modifiers, PointerButton, Pos2, Rect};
use egui_kittest::kittest::NodeT;
use egui_kittest::Harness;
use serde::Deserialize;
use serde_json::Value;

fn one() -> u32 {
    1
}
fn four() -> usize {
    4
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Target {
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub starts: Option<String>,
    #[serde(default)]
    pub placeholder: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub index: Option<usize>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ScrollDir {
    Up,
    Down,
    Left,
    Right,
    IntoView,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Step {
    Click {
        at: Target,
        #[serde(default)]
        mods: Option<String>,
    },
    DoubleClick {
        at: Target,
    },
    RightClick {
        at: Target,
    },
    Hover {
        at: Target,
    },
    Type {
        text: String,
        #[serde(default)]
        at: Option<Target>,
    },
    Key {
        key: String,
    },
    Drag {
        from: Target,
        to: Target,
    },
    Scroll {
        at: Target,
        dir: ScrollDir,
        #[serde(default = "one")]
        times: u32,
    },
    Resize {
        w: f32,
        h: f32,
    },
    Settle {
        #[serde(default = "four")]
        frames: usize,
    },
    Pick {
        path: String,
    },
    Expect {
        fact: String,
        #[serde(default)]
        equals: Option<Value>,
        #[serde(default)]
        contains: Option<Value>,
        #[serde(default)]
        absent: bool,
    },
    Check {
        name: String,
    },
}

impl Step {
    fn targets(&self) -> Vec<&Target> {
        match self {
            Step::Click { at, .. }
            | Step::DoubleClick { at }
            | Step::RightClick { at }
            | Step::Hover { at }
            | Step::Scroll { at, .. } => vec![at],
            Step::Type { at, .. } => at.iter().collect(),
            Step::Drag { from, to } => vec![from, to],
            _ => vec![],
        }
    }
}

/// One step per non-blank line, with the line's text kept for reports.
pub fn parse_script(text: &str) -> Result<Vec<(String, Step)>, String> {
    let mut out = vec![];
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let n = i + 1;
        let step: Step = serde_json::from_str(line).map_err(|e| format!("line {n}: {e}"))?;
        for t in step.targets() {
            let named = [&t.label, &t.starts, &t.placeholder]
                .iter()
                .filter(|o| o.is_some())
                .count();
            if named != 1 {
                return Err(format!(
                    "line {n}: a target needs exactly one of label, starts, placeholder"
                ));
            }
        }
        out.push((line.to_string(), step));
    }
    Ok(out)
}

fn modifier(name: &str) -> Result<Modifiers, String> {
    match name {
        "Ctrl" => Ok(Modifiers::COMMAND),
        "Shift" => Ok(Modifiers::SHIFT),
        "Alt" => Ok(Modifiers::ALT),
        other => Err(format!("unknown modifier \"{other}\" (Ctrl, Shift, Alt)")),
    }
}

fn parse_mods<'a>(parts: impl Iterator<Item = &'a str>) -> Result<Modifiers, String> {
    let mut m = Modifiers::NONE;
    for p in parts {
        m |= modifier(p)?;
    }
    Ok(m)
}

/// "Enter", "Ctrl+K", "Ctrl+Shift+Z": modifiers, then one key by egui's name.
pub fn parse_key(s: &str) -> Result<(Modifiers, egui::Key), String> {
    let parts: Vec<&str> = s.split('+').collect();
    let (name, mods) = parts.split_last().expect("split yields one part");
    let key = egui::Key::from_name(name).ok_or_else(|| format!("unknown key \"{name}\""))?;
    Ok((parse_mods(mods.iter().copied())?, key))
}

fn describe(t: &Target) -> String {
    match (&t.label, &t.starts, &t.placeholder) {
        (Some(l), ..) => format!("labelled \"{l}\""),
        (_, Some(s), _) => format!("starting \"{s}\""),
        (_, _, Some(p)) => format!("with placeholder \"{p}\""),
        _ => "with no name".into(),
    }
}

fn reading_order(a: &Node, b: &Node) -> std::cmp::Ordering {
    a.rect
        .min
        .y
        .total_cmp(&b.rect.min.y)
        .then(a.rect.min.x.total_cmp(&b.rect.min.x))
}

fn at(n: &Node) -> String {
    let r = n.rect;
    format!(
        "{}@({},{} {}x{})",
        n.role,
        r.min.x.round(),
        r.min.y.round(),
        r.width().round(),
        r.height().round()
    )
}

fn words(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

fn quoted(labels: Vec<&str>) -> String {
    labels
        .iter()
        .map(|l| format!("\"{l}\""))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The node a target names; several matches need an `index` (reading order: top, then left) or a `role`.
pub fn find<'a>(s: &'a Snapshot, t: &Target) -> Result<&'a Node, String> {
    let wanted = t
        .label
        .as_deref()
        .or(t.starts.as_deref())
        .or(t.placeholder.as_deref())
        .unwrap_or_default();
    let named = |n: &Node| {
        if let Some(l) = &t.label {
            n.label == *l
        } else if let Some(p) = &t.starts {
            n.label.starts_with(p.as_str())
        } else {
            n.placeholder.as_deref() == t.placeholder.as_deref()
        }
    };
    let mut hits: Vec<&Node> = s
        .nodes
        .iter()
        .filter(|n| n.rect != Rect::NOTHING && named(n))
        .filter(|n| t.role.as_ref().is_none_or(|r| n.role == *r))
        .collect();
    hits.sort_by(|a, b| reading_order(a, b));
    if let Some(i) = t.index.filter(|_| !hits.is_empty()) {
        return hits
            .get(i)
            .copied()
            .ok_or_else(|| format!("index {i} but only {} nodes {}", hits.len(), describe(t)));
    }
    match hits.len() {
        1 => Ok(hits[0]),
        0 => {
            let mut all: Vec<&Node> = s
                .nodes
                .iter()
                .filter(|n| n.rect != Rect::NOTHING && !n.label.is_empty())
                .collect();
            all.sort_by(|a, b| reading_order(a, b));
            let w = words(wanted);
            let mut sharing: Vec<&str> = vec![];
            for n in &all {
                if words(&n.label).iter().any(|x| w.contains(x))
                    && !sharing.contains(&n.label.as_str())
                {
                    sharing.push(&n.label);
                }
            }
            sharing.truncate(20);
            let hint = if sharing.is_empty() {
                let mut inter: Vec<&str> = vec![];
                for n in all.iter().filter(|n| n.interactive) {
                    if !inter.contains(&n.label.as_str()) {
                        inter.push(&n.label);
                    }
                }
                inter.truncate(40);
                format!("interactive labels: {}", quoted(inter))
            } else {
                format!("labels sharing a word: {}", quoted(sharing))
            };
            Err(format!("no node {}; {hint}", describe(t)))
        }
        n => Err(format!(
            "{n} nodes {}: {}; add \"index\" (0 = first in reading order) or \"role\"",
            describe(t),
            hits.iter().map(|h| at(h)).collect::<Vec<_>>().join(", ")
        )),
    }
}

/// The kittest node (the one AccessKit actions go to) with the same role, label and rect as `n`.
fn with_kit<S, R>(
    h: &Harness<'static, S>,
    n: &Node,
    f: impl FnOnce(&egui_kittest::Node<'_>) -> R,
) -> Result<R, String> {
    let root = h.root();
    let k = root.children_recursive().find(|k| {
        let a = k.accesskit_node();
        a.bounding_box().is_some()
            && format!("{:?}", a.role()) == n.role
            && names(&a).0 == n.label
            && k.rect() == n.rect
    });
    k.map(|k| f(&k))
        .ok_or_else(|| format!("{} is gone from the tree", at(n)))
}

fn on_screen(s: &Snapshot, n: &Node) -> bool {
    let c = visible_centre(n);
    Rect::from_min_size(Pos2::ZERO, s.size).contains(c) && n.clip.is_none_or(|k| k.contains(c))
}

/// Find the target; if its centre is hidden, scroll it into view once and find it again.
fn aim<S>(h: &mut Harness<'static, S>, t: &Target) -> Result<(Snapshot, Node), String> {
    let s = snapshot(h);
    let n = find(&s, t)?.clone();
    if on_screen(&s, &n) {
        return Ok((s, n));
    }
    with_kit(h, &n, |k| k.scroll_to_me())?;
    h.run_steps(4);
    let s = snapshot(h);
    let n = find(&s, t)?.clone();
    if on_screen(&s, &n) {
        Ok((s, n))
    } else {
        Err(format!(
            "\"{}\" is not on screen after scrolling it into view",
            n.label
        ))
    }
}

fn press(pos: Pos2, button: PointerButton, pressed: bool, modifiers: Modifiers) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers,
    }
}

fn click_at<S>(
    h: &mut Harness<'static, S>,
    p: Pos2,
    button: PointerButton,
    pairs: usize,
    mods: Modifiers,
) {
    h.event(egui::Event::PointerMoved(p));
    h.run_steps(1);
    h.event(egui::Event::ModifiersChanged(mods));
    h.run_steps(1);
    for _ in 0..pairs {
        h.event(press(p, button, true, mods));
        h.event(press(p, button, false, mods));
    }
    h.run_steps(1);
    h.event(egui::Event::ModifiersChanged(Modifiers::NONE));
}

pub fn ui_step<S>(h: &mut Harness<'static, S>, step: &Step) -> Result<(), String> {
    match step {
        Step::Click { at, mods } => {
            let m = match mods {
                Some(m) => parse_mods(m.split('+'))?,
                None => Modifiers::NONE,
            };
            let (_, n) = aim(h, at)?;
            click_at(h, visible_centre(&n), PointerButton::Primary, 1, m);
        }
        Step::DoubleClick { at } => {
            let (_, n) = aim(h, at)?;
            click_at(
                h,
                visible_centre(&n),
                PointerButton::Primary,
                2,
                Modifiers::NONE,
            );
        }
        Step::RightClick { at } => {
            let (_, n) = aim(h, at)?;
            click_at(
                h,
                visible_centre(&n),
                PointerButton::Secondary,
                1,
                Modifiers::NONE,
            );
        }
        Step::Hover { at } => {
            let (_, n) = aim(h, at)?;
            h.event(egui::Event::PointerMoved(visible_centre(&n)));
            // 4 steps of 0.25 s pass egui's 0.5 s tooltip delay
            h.run_steps(4);
        }
        Step::Type { text, at } => {
            if let Some(t) = at {
                ui_step(
                    h,
                    &Step::Click {
                        at: t.clone(),
                        mods: None,
                    },
                )?;
            }
            let s = snapshot(h);
            if !s.nodes.iter().any(|n| n.role == "TextInput" && n.focused) {
                return Err(
                    "no text field has focus; give the step an \"at\" or click one first".into(),
                );
            }
            h.event(egui::Event::Text(text.clone()));
        }
        Step::Key { key } => {
            let (m, k) = parse_key(key)?;
            if m == Modifiers::NONE {
                h.key_press(k);
            } else {
                h.key_press_modifiers(m, k);
            }
        }
        Step::Drag { from, to } => {
            let (_, a) = aim(h, from)?;
            let (_, b) = aim(h, to)?;
            let (p, q) = (visible_centre(&a), visible_centre(&b));
            // the house drag (sets_filter.rs): hover, press, 8 interpolated moves a frame each, release
            h.event(egui::Event::PointerMoved(p));
            h.run_steps(1);
            h.event(press(p, PointerButton::Primary, true, Modifiers::NONE));
            h.run_steps(1);
            for i in 1..=8 {
                h.event(egui::Event::PointerMoved(p + (q - p) * (i as f32 / 8.0)));
                h.run_steps(1);
            }
            h.event(press(q, PointerButton::Primary, false, Modifiers::NONE));
            h.run_steps(3);
        }
        Step::Scroll { at, dir, times } => {
            // one action per frame: a scroll moves the node, so it is found again each time
            for _ in 0..if *dir == ScrollDir::IntoView {
                1
            } else {
                *times
            } {
                let s = snapshot(h);
                let n = find(&s, at)?.clone();
                with_kit(h, &n, |k| match dir {
                    ScrollDir::IntoView => k.scroll_to_me(),
                    ScrollDir::Up => k.scroll_up(),
                    ScrollDir::Down => k.scroll_down(),
                    ScrollDir::Left => k.scroll_left(),
                    ScrollDir::Right => k.scroll_right(),
                })?;
                h.run_steps(1);
            }
        }
        Step::Resize { w, h: height } => {
            h.set_size(egui::vec2(*w, *height));
        }
        Step::Settle { frames } => h.run_steps(*frames),
        Step::Pick { .. } | Step::Expect { .. } | Step::Check { .. } => {
            return Err("needs the app run".into())
        }
    }
    h.run_steps(2);
    Ok(())
}
