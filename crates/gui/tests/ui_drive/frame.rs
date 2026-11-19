#![allow(dead_code)]
//! A frame read as data: the AccessKit tree joined to egui's widget list, plus the text actually painted.
//! Works on any `Harness`, so it is tested on tiny UIs.

use eframe::egui::{self, Pos2, Rect, Shape, Vec2};
use egui_kittest::kittest::NodeT;
use egui_kittest::Harness;
use std::collections::HashMap;
use std::fmt::Write as _;

#[derive(Clone, Debug)]
pub struct Node {
    pub parent: Option<usize>,
    pub depth: usize,
    pub role: String,
    pub label: String,
    pub value: Option<String>,
    pub placeholder: Option<String>,
    /// The AccessKit description: what `on_hover_text` copies into a node.
    pub description: Option<String>,
    pub rect: Rect,
    pub clip: Option<Rect>,
    pub layer: Option<String>,
    pub interactive: bool,
    pub focused: bool,
    pub disabled: bool,
    pub hovered: bool,
}

#[derive(Clone, Debug)]
pub struct Text {
    pub text: String,
    pub rect: Rect,
    pub clip: Rect,
    /// The painted galley was cut short by egui (`Galley::elided`); `source` is then the full text of the galley.
    pub elided: bool,
    /// The galley's text exactly as painted, untrimmed (`text` has its trailing ellipses removed): `head……` stays
    /// `head……`. The cut-text invariant reads this and `elided` only.
    pub source: String,
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub size: Vec2,
    pub nodes: Vec<Node>,
    pub texts: Vec<Text>,
    /// Painted texts that are nothing but ellipses ("…"): `texts` leaves them out (its `text` is empty after the trim),
    /// so existing readers see no change; the cut-text invariant reads them beside `texts`.
    pub bare_ellipses: Vec<Text>,
    /// The right edge of the Sets column's panel (`ui::panel_ids::panel_rect`), when the frame built it: the set
    /// editor is the area right of it.
    pub sets_right: Option<f32>,
    /// Every filled rect the frame paints, with its fill's alpha: where a scroll bar's track and handle show.
    pub filled: Vec<Filled>,
    /// Every filled circle the frame paints: where a status dot shows.
    pub discs: Vec<Disc>,
    /// Where the pointer is, when egui has one in the window; `None` is a frame with no pointer (a bar at rest).
    pub pointer: Option<Pos2>,
}

/// A painted filled rect.
#[derive(Clone, Debug)]
pub struct Filled {
    pub rect: Rect,
    pub alpha: u8,
}

/// A painted filled circle.
#[derive(Clone, Debug)]
pub struct Disc {
    pub center: Pos2,
    pub radius: f32,
    pub fill: egui::Color32,
}

fn discs_in(s: &Shape, out: &mut Vec<Disc>) {
    match s {
        Shape::Vec(v) => v.iter().for_each(|s| discs_in(s, out)),
        Shape::Circle(c) if c.fill.a() > 0 => out.push(Disc {
            center: c.center,
            radius: c.radius,
            fill: c.fill,
        }),
        _ => {}
    }
}

const INTERACTIVE_ROLES: [&str; 10] = [
    "Button",
    "CheckBox",
    "ComboBox",
    "TextInput",
    "Slider",
    "SpinButton",
    "RadioButton",
    "Tab",
    "MenuItem",
    "Link",
];

/// What the walk keeps of an egui WidgetRect (the type itself is not public).
struct Joined {
    id: egui::Id,
    rect: Rect,
    clip: Rect,
    layer: String,
    interactive: bool,
}

/// A node's name and value. egui names a Label by its text in `value`, not in `label`; `with_kit` in steps.rs
/// matches kittest nodes with this too.
pub fn names(a: &egui_kittest::kittest::AccessKitNode<'_>) -> (String, Option<String>) {
    match (a.label(), a.value()) {
        (None, Some(v)) if format!("{:?}", a.role()) == "Label" => (v, None),
        (l, v) => (l.unwrap_or_default(), v),
    }
}

fn flatten(s: &Shape, clip: Rect, out: &mut Vec<Text>, bare: &mut Vec<Text>) {
    match s {
        Shape::Vec(v) => v.iter().for_each(|s| flatten(s, clip, out, bare)),
        Shape::Text(t) => {
            let text = t.galley.text().trim_end_matches('…').to_string();
            let rect = t.galley.rect.translate(t.pos.to_vec2());
            if rect.intersects(clip) && !t.galley.text().trim().is_empty() {
                let painted = Text {
                    text,
                    rect,
                    clip,
                    elided: t.galley.elided,
                    source: t.galley.text().to_string(),
                };
                if painted.text.trim().is_empty() {
                    bare.push(painted);
                } else {
                    out.push(painted);
                }
            }
        }
        _ => {}
    }
}

pub fn snapshot<S>(h: &Harness<'_, S>) -> Snapshot {
    // The widget list keyed by the id AccessKit uses as an action target.
    let widgets: HashMap<_, _> = h.ctx.viewport(|v| {
        v.this_pass
            .widgets
            .layers()
            .flat_map(|(l, ws)| {
                ws.iter().map(move |w| {
                    (
                        w.id.accesskit_id(),
                        Joined {
                            id: w.id,
                            rect: w.rect,
                            clip: w.interact_rect,
                            layer: format!("{l:?}"),
                            interactive: w.sense.interactive(),
                        },
                    )
                })
            })
            .collect()
    });
    let mut nodes = Vec::new();
    fn walk<'t, N: NodeT<'t> + 't>(
        n: &N,
        parent: Option<usize>,
        depth: usize,
        rect_of: &dyn Fn(&N) -> Rect,
        h: &HashMap<egui::accesskit::NodeId, Joined>,
        ctx: &egui::Context,
        out: &mut Vec<Node>,
    ) {
        let a = n.accesskit_node();
        let role = format!("{:?}", a.role());
        if role == "TextRun" {
            return; // the glyph run under a Label; its text is already the Label's value
        }
        let joined = h.get(&a.locate().0);
        let (label, value) = names(&a);
        let r = if a.bounding_box().is_some() {
            rect_of(n)
        } else {
            Rect::NOTHING
        };
        let idx = out.len();
        out.push(Node {
            parent,
            depth,
            interactive: joined.map_or(INTERACTIVE_ROLES.contains(&role.as_str()), |w| {
                w.interactive
            }),
            role,
            label,
            value,
            placeholder: a.placeholder().map(str::to_owned),
            description: a.description(),
            rect: r,
            // egui registers a Label in a ScrollArea at its unscrolled place; a clip for another place is not this node's
            clip: joined
                .filter(|w| (w.rect.min - r.min).abs().max_elem() < 1.0)
                .map(|w| w.clip),
            layer: joined.map(|w| w.layer.clone()),
            focused: a.is_focused(),
            disabled: a.is_disabled(),
            hovered: joined.is_some_and(|w| ctx.read_response(w.id).is_some_and(|r| r.hovered())),
        });
        for c in n.children() {
            walk(&c, Some(idx), depth + 1, rect_of, h, ctx, out);
        }
    }
    walk(
        &h.root(),
        None,
        0,
        &|n: &egui_kittest::Node<'_>| n.rect(),
        &widgets,
        &h.ctx,
        &mut nodes,
    );
    let mut texts = Vec::new();
    let mut bare_ellipses = Vec::new();
    for c in &h.output().shapes {
        flatten(&c.shape, c.clip_rect, &mut texts, &mut bare_ellipses);
    }
    let mut discs = Vec::new();
    for c in &h.output().shapes {
        discs_in(&c.shape, &mut discs);
    }
    Snapshot {
        size: h.ctx.content_rect().size(),
        nodes,
        texts,
        bare_ellipses,
        sets_right: sdc_gui::ui::panel_ids::panel_rect(&h.ctx, "sets").map(|r| r.max.x),
        filled: crate::support::filled_rects(&h.output().shapes)
            .into_iter()
            .map(|(rect, alpha)| Filled { rect, alpha })
            .collect(),
        discs,
        pointer: h.ctx.input(|i| i.pointer.latest_pos()),
    }
}

/// Where a pointer should aim: the part of the rect that is not clipped away.
pub fn visible_centre(n: &Node) -> Pos2 {
    match n.clip {
        Some(c) if c.intersect(n.rect).is_positive() => c.intersect(n.rect).center(),
        _ => n.rect.center(),
    }
}

fn flags(n: &Node) -> String {
    let mut s = String::new();
    if n.focused {
        s += " focused";
    }
    if n.disabled {
        s += " disabled";
    }
    if n.hovered {
        s += " hovered";
    }
    if let Some(c) = n.clip {
        let r = n.rect;
        if !c.intersect(r).is_positive() {
            s += " offscreen";
        } else {
            let i = c.intersect(r);
            if i.width() < r.width() - 0.5 || i.height() < r.height() - 0.5 {
                s += " clipped";
            }
        }
    }
    s
}

/// Printed children per level; the Sets column alone holds more than 50 nodes.
const MAX_CHILDREN: usize = 400;

/// One line per named node, indented by printed depth; unnamed containers fold into their parent.
pub fn tree_text(s: &Snapshot) -> String {
    let mut kids: Vec<Vec<usize>> = vec![vec![]; s.nodes.len()];
    for (i, n) in s.nodes.iter().enumerate() {
        if let Some(p) = n.parent {
            kids[p].push(i);
        }
    }
    let unnamed = |n: &Node| n.role == "GenericContainer" && n.label.is_empty();
    // Printed children of `i`: its children, with unnamed containers replaced by theirs.
    fn printed(
        i: usize,
        s: &Snapshot,
        kids: &[Vec<usize>],
        unnamed: &dyn Fn(&Node) -> bool,
        out: &mut Vec<usize>,
    ) {
        for &c in &kids[i] {
            if unnamed(&s.nodes[c]) {
                printed(c, s, kids, unnamed, out);
            } else {
                out.push(c);
            }
        }
    }
    fn emit(
        i: usize,
        depth: usize,
        s: &Snapshot,
        kids: &[Vec<usize>],
        unnamed: &dyn Fn(&Node) -> bool,
        out: &mut String,
    ) {
        let n = &s.nodes[i];
        let r = n.rect;
        let _ = write!(out, "{}{} {:?}", "  ".repeat(depth), n.role, n.label);
        if let Some(v) = &n.value {
            let _ = write!(out, " ={v}");
        }
        if let Some(p) = &n.placeholder {
            let _ = write!(out, " ~{p:?}");
        }
        let _ = writeln!(
            out,
            " @{},{} {}x{}{}",
            r.min.x.round(),
            r.min.y.round(),
            r.width().round(),
            r.height().round(),
            flags(n)
        );
        let mut ch = vec![];
        printed(i, s, kids, unnamed, &mut ch);
        for &c in ch.iter().take(MAX_CHILDREN) {
            emit(c, depth + 1, s, kids, unnamed, out);
        }
        if ch.len() > MAX_CHILDREN {
            let _ = writeln!(
                out,
                "{}… {} more",
                "  ".repeat(depth + 1),
                ch.len() - MAX_CHILDREN
            );
        }
    }
    let mut out = String::new();
    // The root itself is printed when named; otherwise its printed children are the top level.
    if s.nodes.is_empty() {
        return out;
    }
    if unnamed(&s.nodes[0]) {
        let mut top = vec![];
        printed(0, s, &kids, &unnamed, &mut top);
        for &c in top.iter().take(MAX_CHILDREN) {
            emit(c, 0, s, &kids, &unnamed, &mut out);
        }
        if top.len() > MAX_CHILDREN {
            let _ = writeln!(out, "… {} more", top.len() - MAX_CHILDREN);
        }
    } else {
        emit(0, 0, s, &kids, &unnamed, &mut out);
    }
    out
}
