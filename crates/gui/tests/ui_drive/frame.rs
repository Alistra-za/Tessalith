#![allow(
    dead_code,
    reason = "shared by the ui_drive and ui_proof tests; ui_drive uses a subset of its fields and methods"
)]
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
    /// The AccessKit selected state (`Some(false)` is a node that can be selected and is not).
    pub selected: Option<bool>,
    /// The node offers the AccessKit ScrollIntoView action (a set card does: `sets_column::card_node`).
    pub scroll_into_view: bool,
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
    /// Every filled rect the frame paints, with its fill's alpha, its clip and (with `track_paint`) its layer: where a
    /// scroll bar's track and handle show.
    pub filled: Vec<Filled>,
    /// Every filled circle the frame paints, with its clip and layer: where a status dot shows.
    pub discs: Vec<Disc>,
    /// Whether every painted shape above has its layer: true only when `track_paint` was installed before the frame and
    /// the layers it read hold exactly the shapes egui output. False means `layer` is `None` everywhere.
    pub paint_layers_known: bool,
    /// The layers whose paint egui moves by a transform other than identity when it drains the pass (their shapes, and
    /// their nodes' rects, are then not in window coordinates).
    pub transformed_layers: Vec<String>,
    /// Where the pointer is, when egui has one in the window; `None` is a frame with no pointer (a bar at rest).
    pub pointer: Option<Pos2>,
    /// The outer rects of the named panels the frame built (`ui::panel_ids`), and the centre column as "centre": what
    /// names the area a scroll bar belongs to.
    pub panels: Vec<(String, Rect)>,
    /// The layer of the topmost open modal dialog (egui's `Memory::top_modal_layer`, the `Debug` text a node's `layer`
    /// has), when one is open.
    pub modal: Option<String>,
}

impl Snapshot {
    /// The rect of the named panel (see `panels`).
    pub fn panel(&self, name: &str) -> Option<Rect> {
        self.panels.iter().find(|(n, _)| n == name).map(|(_, r)| *r)
    }

    /// The open modal dialog, named by its layer and the first named node in it (its title, as `ui::dialogs` draws one).
    pub fn modal_name(&self) -> Option<String> {
        let layer = self.modal.as_ref()?;
        let title = self
            .nodes
            .iter()
            .find(|n| n.layer.as_ref() == Some(layer) && !n.label.is_empty())
            .map_or("no named node", |n| n.label.as_str());
        Some(format!("{title:?} in {layer}"))
    }
}

/// The panels `Snapshot::panels` looks for (`ui::panel_ids` salts); "centre" is read from `ui::centre_rect`.
const PANEL_NAMES: [&str; 24] = [
    "header",
    "status",
    "textures",
    "textures-strip",
    "textures-scan",
    "sets",
    "sets-strip",
    "bottom",
    "bottom-strip",
    "bottom-review",
    "review-footer",
    "preview-settings",
    "preview-settings-strip",
    "preview-toolbar",
    "preview-notes",
    "meshes-bar",
    "meshes-tree",
    "meshes-edit",
    "meshes-edit-export",
    "meshes-3d-anim",
    "meshes-3d-bar",
    "meshes-3d-details",
    "set-3d-bar",
    "set-3d-details",
];

/// A painted filled rect.
#[derive(Clone, Debug)]
pub struct Filled {
    pub rect: Rect,
    pub alpha: u8,
    /// The clip rect the shape was painted with.
    pub clip: Rect,
    /// The egui layer that painted it (the `Debug` text a node's `layer` has), when known.
    pub layer: Option<String>,
}

/// A painted filled circle.
#[derive(Clone, Debug)]
pub struct Disc {
    pub center: Pos2,
    pub radius: f32,
    pub fill: egui::Color32,
    pub clip: Rect,
    pub layer: Option<String>,
}

fn discs_in(s: &Shape, clip: Rect, layer: &Option<String>, out: &mut Vec<Disc>) {
    match s {
        Shape::Vec(v) => v.iter().for_each(|s| discs_in(s, clip, layer, out)),
        Shape::Circle(c) if c.fill.a() > 0 => out.push(Disc {
            center: c.center,
            radius: c.radius,
            fill: c.fill,
            clip,
            layer: layer.clone(),
        }),
        _ => {}
    }
}

fn fills_in(s: &Shape, clip: Rect, layer: &Option<String>, out: &mut Vec<Filled>) {
    let mut found = vec![];
    crate::support::fill_rects_of(s, &mut found);
    out.extend(found.into_iter().map(|(rect, alpha)| Filled {
        rect,
        alpha,
        clip,
        layer: layer.clone(),
    }));
}

/// What `PaintLayers` read at the end of the last pass: the shapes of every layer, each with its layer.
#[derive(Default)]
pub struct PaintLayers {
    filled: Vec<Filled>,
    discs: Vec<Disc>,
    /// How many shapes the layers held: equals `FullOutput::shapes.len()` when no layer was missed.
    total: usize,
    transformed: Vec<String>,
}

impl egui::Plugin for PaintLayers {
    fn debug_name(&self) -> &'static str {
        "PaintLayers"
    }

    fn on_end_pass(&mut self, ui: &mut egui::Ui) {
        // egui's output drops which layer painted a shape; the layers' paint lists still hold it until the pass ends
        let ctx = ui.ctx().clone();
        let mut layers: Vec<egui::LayerId> =
            ctx.memory(|m| m.areas().visible_layer_ids().into_iter().collect());
        layers.push(egui::LayerId::background());
        layers.sort_by_key(|l| format!("{l:?}"));
        layers.dedup();
        self.filled.clear();
        self.discs.clear();
        self.total = 0;
        self.transformed = layers
            .iter()
            .filter(|l| ctx.layer_transform_to_global(**l).is_some())
            .map(|l| format!("{l:?}"))
            .collect();
        ctx.graphics(|g| {
            for l in &layers {
                let Some(list) = g.get(*l) else { continue };
                let name = Some(format!("{l:?}"));
                for cs in list.all_entries() {
                    self.total += 1;
                    fills_in(&cs.shape, cs.clip_rect, &name, &mut self.filled);
                    discs_in(&cs.shape, cs.clip_rect, &name, &mut self.discs);
                }
            }
        });
    }
}

/// Installs `PaintLayers` on `ctx` (once; a second call does nothing). A `Snapshot` of a frame run after this has the
/// paint's layers.
pub fn track_paint(ctx: &egui::Context) {
    ctx.add_plugin(PaintLayers::default());
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
            selected: a.data().is_selected(),
            scroll_into_view: a
                .data()
                .supports_action(egui::accesskit::Action::ScrollIntoView),
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
    // The paint with its layers when `track_paint` saw this frame and held every shape of the output; else the output's
    // shapes alone, with no layer.
    let tracked = h.ctx.plugin_opt::<PaintLayers>().and_then(|p| {
        let p = p.lock();
        (p.total == h.output().shapes.len())
            .then(|| (p.filled.clone(), p.discs.clone(), p.transformed.clone()))
    });
    let paint_layers_known = tracked.is_some();
    let (filled, discs, transformed_layers) = tracked.unwrap_or_else(|| {
        let (mut filled, mut discs) = (vec![], vec![]);
        for c in &h.output().shapes {
            fills_in(&c.shape, c.clip_rect, &None, &mut filled);
            discs_in(&c.shape, c.clip_rect, &None, &mut discs);
        }
        (filled, discs, vec![])
    });
    let mut panels: Vec<(String, Rect)> = PANEL_NAMES
        .iter()
        .filter_map(|n| sdc_gui::ui::panel_ids::panel_rect(&h.ctx, n).map(|r| (n.to_string(), r)))
        .collect();
    if let Some(r) = sdc_gui::ui::centre_rect(&h.ctx) {
        panels.push(("centre".to_string(), r));
    }
    Snapshot {
        size: h.ctx.content_rect().size(),
        nodes,
        texts,
        bare_ellipses,
        sets_right: sdc_gui::ui::panel_ids::panel_rect(&h.ctx, "sets").map(|r| r.max.x),
        filled,
        discs,
        paint_layers_known,
        transformed_layers,
        pointer: h.ctx.input(|i| i.pointer.latest_pos()),
        panels,
        modal: h
            .ctx
            .memory(|m| m.top_modal_layer())
            .map(|l| format!("{l:?}")),
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
