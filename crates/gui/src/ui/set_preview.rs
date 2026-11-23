//! The set preview (P1-A, spec §2.3): the set's 3D view, in the centre pane under the set editor's header row while
//! the 2D | 3D switch is on 3D (AG78). Draws `SetPreview`; the frame, the CS toggle and the problem lists are the shared
//! `ui::viewport3d::show`. No logic beyond mapping input to model calls.
use crate::look::Combo036 as _;
use crate::model::set_preview::{
    badge_text, builtin_name, cap_text, count_text, Candidates, EncState, MeshChoice, Outcome,
    Pick, PreviewMode, SetPreview, BUILTINS, GRASS_APPROX, JSON_WINS, MATO_HOVER, MATO_TOGGLE,
    MODE_LABEL_ENCODED, MODE_LABEL_SOURCE, NEEDS_CS, NO_MATCH, TILING, WAITS_FOR_EXPORT,
};
use crate::model::sets::SetView;
use crate::model::AppModel;
use crate::ui::status::{AMBER, RED};
use eframe::egui;
use render3d::ShapeStatus;
use std::fmt::Write as _;

/// SLSF1.29 External Emittance: the preview draws the NIF colour (captures C1, frame6171: the engine sends exactly that
/// in an interior); outdoors the engine can substitute the cell or weather colour.
const EMISSIVE_APPROX: &str =
    "External emittance: in game the cell or weather can change this glow colour";
/// SLSF1 bit 29: External Emittance.
const SLSF1_EMISSIVE: u32 = 1 << 29;
/// The width the centre pane keeps at the least.
pub const CENTRE_MIN: f32 = 360.0;
/// The 3D settings window's width when it has no saved rect (it can be resized narrower; egui sets no minimum).
pub const SETTINGS_WINDOW_W: f32 = 460.0;
/// The tallest the shape warnings above the frame get.
const LIST_MAX: f32 = 80.0;

/// The switch is on 3D and a set is followed; the view draws in the centre in place of the set editor's body
/// while the set itself is selected (AG78).
pub fn wanted(m: &AppModel) -> bool {
    m.settings.set_view_3d && m.resolved_selection().target.is_some()
}

/// The 2D | 3D switch of a set: the slot cards, or this 3D preview. The TruePBR settings show in both.
pub fn view_switch(ui: &mut egui::Ui, m: &mut AppModel) {
    let mut three_d = m.settings.set_view_3d;
    if ui
        .selectable_label(!three_d, "2D")
        .on_hover_text("Show the set's slot rows")
        .clicked()
    {
        three_d = false;
    }
    if ui
        .selectable_label(three_d, "3D")
        .on_hover_text("Show the set on a mesh in 3D")
        .clicked()
    {
        three_d = true;
    }
    if three_d != m.settings.set_view_3d {
        m.settings.set_view_3d = three_d;
        m.save_settings();
    }
}

pub fn draw(ui: &mut egui::Ui, m: &mut AppModel, s: &mut SetPreview) {
    s.shown = true;
    // Edits arrive as keys, text or clicks; pointer moves and drags (the orbit) keep the throttle.
    if ui.ctx().input(|i| {
        i.events.iter().any(|e| {
            matches!(
                e,
                egui::Event::Key { .. }
                    | egui::Event::Text(_)
                    | egui::Event::Paste(_)
                    | egui::Event::PointerButton { .. }
            )
        })
    }) {
        s.input = true;
    }
    // AG132: the set's name and path are the set editor's header row above this view; no heading of its own.
    let active = m.resolved_selection();
    if active.target.is_none() {
        ui.label("Select a set to preview it in 3D.");
        return;
    }
    let Some((simulated, landscape, grass)) =
        s.fp.as_ref()
            .filter(|fp| Some(&fp.set_key) == active.target.as_ref())
            .map(|fp| (fp.entry.is_some(), fp.landscape, fp.grass))
    else {
        ui.label(PREPARING);
        return;
    };
    let view = active.view();
    crate::ui::viewport3d::prelude(ui.ctx(), &mut s.v3);
    // One toolbar line, then what needs attention; the frame takes the rest above the collapsed Details.
    super::panel_ids::top(ui, "set-3d-bar")
        .frame(egui::Frame::new())
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                mode_toggle(ui, s);
                if landscape {
                    // Note BM (review M4): short, so the toolbar stays one line at 1024 pt (BH13).
                    ui.label("Mesh: Plane").on_hover_text(
                        "A PBR texture set or PBR Landscape set previews on the plane",
                    );
                    let mut t = s.tiling;
                    if ui
                        .add(
                            egui::Slider::new(&mut t, 1.0..=3.0)
                                .step_by(0.5)
                                .text(TILING),
                        )
                        .changed()
                    {
                        s.set_tiling(t);
                    }
                } else {
                    mesh_picker(ui, s);
                }
                if s.mato_available() {
                    mato_toggle(ui, s);
                }
                crate::ui::viewport3d::toolbar(ui, &mut s.v3);
            });
            for line in s.mato_lines() {
                ui.add(egui::Label::new(egui::RichText::new(&line).weak()).truncate())
                    .on_hover_text(&line);
            }
            if !simulated {
                ui.colored_label(AMBER, NEEDS_CS);
            }
            if grass {
                ui.colored_label(AMBER, GRASS_APPROX);
            }
            if s.enc.paused {
                ui.colored_label(AMBER, WAITS_FOR_EXPORT);
            }
            if let Candidates::Error(e) = &s.candidates {
                ui.colored_label(RED, e);
            }
            members(ui, s, view, true);
            shapes(ui, s, true);
            for n in &s.notices {
                ui.colored_label(AMBER, n);
            }
            crate::ui::viewport3d::warnings(ui, &mut s.v3);
        });
    super::panel_ids::bottom(ui, "set-3d-details")
        .frame(egui::Frame::new())
        .resizable(false)
        .show(ui, |ui| {
            crate::ui::viewport3d::details_footer(ui, crate::ui::viewport3d::SET_DETAILS, |ui| {
                if simulated {
                    ui.label(JSON_WINS);
                }
                ui.label(egui::RichText::new(cap_text(s.enc.cap)).small());
                mesh_line(ui, s, simulated);
                members(ui, s, view, false);
                shapes(ui, s, false);
                if emissive_shown(s) {
                    ui.label(EMISSIVE_APPROX);
                }
                crate::ui::viewport3d::details(ui, &mut s.v3);
            });
        });
    if !crate::ui::viewport3d::le_mesh(&s.v3) {
        crate::ui::viewport3d::image(ui, &mut s.v3);
    }
    if s.busy() || s.v3.busy() {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(16));
    }
}

/// The mesh combo, on the toolbar line: "Mesh: Auto (Sphere)" and its list.
fn mesh_picker(ui: &mut egui::Ui, s: &mut SetPreview) {
    let pick = s.pick();
    let mut chosen = None;
    let pick_name = match &pick {
        Pick::Nif(c) => c.label(),
        Pick::Builtin(b) => builtin_name(*b).to_string(),
    };
    let current = match s.choice() {
        MeshChoice::Auto => format!("Mesh: Auto ({pick_name})"),
        _ => format!("Mesh: {pick_name}"),
    };
    // Note JJ1 (ui-rules 3.5): an exact-width slot. The combo lays itself out in a child `horizontal`, which the
    // toolbar's wrapped row places before its width is known, so it ran past the window instead of taking the next
    // row. The slot's width is the whole text's, so the row wraps first; a row narrower than that truncates the text,
    // with the whole of it on hover (and in the accessible value).
    let sp = ui.spacing();
    let need = egui::WidgetText::from(current.as_str())
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Extend),
            f32::INFINITY,
            egui::TextStyle::Button,
        )
        .size()
        .x
        + sp.icon_spacing
        + sp.icon_width
        + 2.0 * sp.button_padding.x;
    let width = need.min(ui.max_rect().width());
    let slot = egui::vec2(width, ui.spacing().interact_size.y);
    ui.allocate_ui(slot, |ui| {
        let cut = width < need;
        if cut {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
        }
        let r = egui::ComboBox::from_id_salt("set-3d-mesh")
            .width(width)
            .selected_text(current.as_str())
            .show_ui_036(ui, |ui| {
                if ui
                    .selectable_label(s.choice() == MeshChoice::Auto, "Auto")
                    .clicked()
                {
                    chosen = Some(MeshChoice::Auto);
                }
                if let Candidates::Ready(list) = &s.candidates {
                    for c in list {
                        let sel = matches!(&pick, Pick::Nif(p) if p.load_copy_id == c.load_copy_id);
                        let r = ui
                            .add_enabled_ui(!c.le_mesh, |ui| ui.selectable_label(sel, c.label()))
                            .inner;
                        if r.clicked() {
                            chosen = Some(MeshChoice::Nif(c.load_copy_id));
                        }
                    }
                }
                ui.separator();
                for (b, name) in BUILTINS {
                    let sel = matches!(pick, Pick::Builtin(x) if x == b);
                    if ui.selectable_label(sel, name).clicked() {
                        chosen = Some(MeshChoice::Builtin(b));
                    }
                }
            });
        let r = super::name_input(r.response, "Preview mesh");
        if cut {
            r.on_hover_text(current.as_str());
        }
    });
    if let Some(c) = chosen {
        s.choose(c);
    }
}

/// Where the mesh list comes from, in Details (an error is a warning above the frame instead).
fn mesh_line(ui: &mut egui::Ui, s: &SetPreview, simulated: bool) {
    match &s.candidates {
        Candidates::NoStore(t) => {
            ui.label(t);
        }
        Candidates::Querying => {
            ui.label("Looking for meshes that use this diffuse…");
        }
        Candidates::Ready(v) if v.is_empty() && simulated => {
            ui.label(NO_MATCH);
        }
        Candidates::Ready(v) if !v.is_empty() => {
            ui.label(count_text(v.len()));
        }
        _ => {}
    }
}

/// "Slot: state" for one member; the slot reads the set's own name for it (an RFAOS on a skin set, Note HE1).
pub fn member_text(view: Option<&SetView>, m: &crate::model::set_preview::Member) -> String {
    let slot = view.map_or_else(|| m.slot.kind.label(), |v| v.slot_text(m.slot.kind));
    format!("{slot}: {}", m.state.label())
}

/// The maps' encode states: with `attention`, the failed, stale and blocked ones (above the frame); else the others
/// (Details).
fn members(ui: &mut egui::Ui, s: &SetPreview, view: Option<&SetView>, attention: bool) {
    let shown: Vec<_> = s
        .enc
        .members
        .values()
        .filter(|m| {
            attention
                == matches!(
                    m.state,
                    EncState::Failed(_) | EncState::Stale(_) | EncState::Blocked(_)
                )
        })
        .collect();
    if shown.is_empty() {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        for m in shown {
            let mut text = member_text(view, m);
            // The time of the bytes on show, so the two modes can be compared (Fresh already says it).
            if let (false, Some(e)) = (matches!(m.state, EncState::Fresh(_)), m.state.shown()) {
                let _ = write!(text, " · last {:.0} ms", e.ms);
            }
            match &m.state {
                EncState::Failed(_) => ui.colored_label(RED, text),
                EncState::Stale(_) | EncState::Blocked(_) => ui.colored_label(AMBER, text),
                _ => ui.label(text),
            };
        }
    });
}

fn mode_toggle(ui: &mut egui::Ui, s: &mut SetPreview) {
    let mut mode = s.mode();
    ui.radio_value(&mut mode, PreviewMode::Encoded, MODE_LABEL_ENCODED);
    ui.radio_value(&mut mode, PreviewMode::Source, MODE_LABEL_SOURCE);
    if mode != s.mode() {
        s.set_mode(mode);
    }
}

/// Note GG: the snow/ash layer of the followed set's `cs_mato` block, drawn over the PBR shapes. Session only.
fn mato_toggle(ui: &mut egui::Ui, s: &mut SetPreview) {
    let mut on = s.mato_layer;
    if ui
        .checkbox(&mut on, MATO_TOGGLE)
        .on_hover_text(MATO_HOVER)
        .changed()
    {
        s.set_mato_layer(on);
    }
}

/// Any shape on show has an emissive colour (SLSF1 bit 29): the panel's rows, or the viewport's loaded shapes.
fn emissive_shown(s: &SetPreview) -> bool {
    let on = |m: &nif::material::MaterialState| !m.hidden && m.slsf1 & SLSF1_EMISSIVE != 0;
    s.rows.iter().any(|r| on(&r.material))
        || s.v3
            .shapes
            .iter()
            .any(|i| i.status != ShapeStatus::Hidden && on(&i.material))
}

/// The shapes: with `attention`, the failed shaders, the skipped shapes and the patch badges (above the frame); else
/// the rest, compiling and patched lines (Details).
fn shapes(ui: &mut egui::Ui, s: &SetPreview, attention: bool) {
    // Spec §1 criterion 4: a shader that failed to compile is named, and one still compiling says so.
    for i in &s.v3.shapes {
        match &i.status {
            ShapeStatus::Failed {
                error,
                showing_previous,
            } if attention => {
                let kept = if *showing_previous {
                    " (previous look kept)"
                } else {
                    ""
                };
                ui.colored_label(RED, format!("{}: shader failed: {error}{kept}", i.name));
            }
            ShapeStatus::Pending { .. } if !attention => {
                ui.label(format!("{}: compiling…", i.name));
            }
            _ => {}
        }
    }
    let needs = |o: &Outcome| match o {
        Outcome::Skip(_) | Outcome::Crash(_) => true,
        Outcome::Patched { badges } => !badges.is_empty(),
        Outcome::NoMatch | Outcome::Plain => false,
    };
    let rows: Vec<_> = s
        .rows
        .iter()
        .filter(|r| needs(&r.outcome) == attention)
        .collect();
    if rows.is_empty() {
        return;
    }
    let list = |ui: &mut egui::Ui| {
        for r in rows {
            match &r.outcome {
                Outcome::Skip(_) | Outcome::Crash(_) => {
                    ui.colored_label(RED, r.text());
                }
                Outcome::Patched { badges } => {
                    ui.label(r.text());
                    for b in badges {
                        ui.colored_label(AMBER, badge_text(b));
                    }
                }
                Outcome::NoMatch | Outcome::Plain => {
                    ui.label(r.text());
                }
            }
        }
    };
    // Above the frame the list is capped; Details scrolls as a whole.
    if attention {
        egui::ScrollArea::vertical()
            .id_salt("set-3d-shapes")
            .max_height(LIST_MAX)
            .show(ui, list);
    } else {
        list(ui);
    }
}

/// What the 3D view says until its first fingerprint is ready.
pub const PREPARING: &str = "Preparing the preview…";
/// The header-row button that shows or hides the 3D view's settings window (AG132).
pub const SETTINGS_BUTTON: &str = "TruePBR settings…";
/// The settings window's id: one window whatever it shows, so its place and size carry over.
pub fn settings_window_id() -> egui::Id {
    egui::Id::unique("set-3d-settings-window")
}

/// What the settings window's size has been doing since it was first drawn this session (Note FM31).
#[derive(Clone, Copy)]
struct SizeFit {
    /// The size asked for (`default_rect`), which egui only reads when it makes the window.
    asked: egui::Vec2,
    last: egui::Vec2,
    frames: u32,
    still: u32,
    /// What egui drew over `asked` once the size held, or None while it is still moving.
    extra: Option<egui::Vec2>,
}

/// Frames the drawn size must hold before the extra is taken, and the most it may take.
const FIT_STILL: u32 = 3;
const FIT_MAX_FRAMES: u32 = 60;
/// An excess past this is not taken off, and the window is saved as drawn. On egui 0.37 that is a saved width far
/// under the content's minimum width, a clamp to the window, or the user's resize.
const FIT_MAX_EXTRA: f32 = 16.0;

/// egui can draw a `Window` bigger than `default_rect` asks, and then a saved size would grow at every start. On egui
/// 0.37 with the pinned look the excess is the content's minimum width: a size at least that wide is drawn as asked,
/// and a narrower one is drawn at the content's width (262.1 points for a set's TruePBR settings; both measured in the
/// kittest harness). This is the drawn rect less that excess, which is what `default_rect` takes back: save and
/// restore are the identity while no content wider than the first is shown and the window is not dragged (the
/// `shortcut:` below says what those two cases save). None until the size has settled.
// shortcut: the excess is measured once per session, from the content shown first. The content already changes with
// the selection (a texture's own settings, the JSON panel, skin, CS and complex material cards), though in the kittest
// fixture its minimum width stayed 262.1 for the set, each member and other sets' textures. When later content is
// wider, egui widens the window and never narrows it (`Resize` keeps the largest size), so the wider width less the
// first excess is saved: a saved width can step up to the widest content's minimum, never past it, and holds there.
// A drag in a session that measured an excess is saved that excess (at most `FIT_MAX_EXTRA`) narrower. Upgrade when
// a saved width is seen to pass the widest content's minimum, or a dragged width must come back exactly: then save the
// size the user dragged the window to, kept apart from the drawn rect (as `column_panel` keeps a chosen width), so no
// excess is measured.
pub fn settings_window_saved_rect(ctx: &egui::Context) -> Option<egui::Rect> {
    let id = settings_window_id();
    let r = ctx.memory(|m| m.area_rect(id))?;
    let extra = ctx.data(|d| d.get_temp::<SizeFit>(id))?.extra?;
    Some(egui::Rect::from_min_size(r.min, r.size() - extra))
}

/// Watches the drawn size after `settings_window` has shown the window, until it holds still.
fn measure_window_extra(ctx: &egui::Context, asked: egui::Vec2) {
    let id = settings_window_id();
    let Some(drawn) = ctx.memory(|m| m.area_rect(id)).map(|r| r.size()) else {
        return;
    };
    let settled = ctx.data_mut(|d| {
        let fit = d.get_temp_mut_or_insert_with(id, || SizeFit {
            asked,
            last: egui::Vec2::ZERO,
            frames: 0,
            still: 0,
            extra: None,
        });
        if fit.extra.is_some() {
            return true;
        }
        fit.frames += 1;
        fit.still = if drawn == fit.last { fit.still + 1 } else { 0 };
        fit.last = drawn;
        if fit.still >= FIT_STILL || fit.frames >= FIT_MAX_FRAMES {
            let extra = drawn - fit.asked;
            fit.extra = Some(if extra.abs().max_elem() <= FIT_MAX_EXTRA {
                extra
            } else {
                egui::Vec2::ZERO
            });
        }
        fit.extra.is_some()
    });
    if !settled {
        ctx.request_repaint();
    }
}

/// The settings toggle, beside the input name box (or the texture's name) in the 3D header row.
pub fn settings_toggle(ui: &mut egui::Ui, m: &mut AppModel) {
    let mut on = m.settings.set_settings_open;
    if ui
        .toggle_value(&mut on, SETTINGS_BUTTON)
        .on_hover_text(
            "A window with the set's TruePBR settings (and a selected texture's own settings); edits show in 3D",
        )
        .changed()
    {
        m.settings.set_settings_open = on;
        m.save_settings();
    }
}

/// AG132: the 3D view's settings in a movable, resizable window: a selected member's own settings, then the set's
/// TruePBR settings (coat group, NiAlphaProperty and the rest) and, for an Advanced skin set, the skin card. It follows
/// the selection; a selection with none of them closes it. Nothing shows while the switch is on 2D.
pub fn settings_window(ctx: &egui::Context, m: &mut AppModel, three_d: bool) {
    if !three_d || !m.settings.set_settings_open {
        return;
    }
    let active = m.resolved_selection();
    let item = match &active.selection {
        crate::model::Selection::Item(k) => Some(k),
        _ => None,
    };
    let set = active.view();
    let json = set.is_some_and(|v| crate::model::sets::shows_json_panel(v, m.target));
    let skin = set.is_some_and(|v| v.skin);
    // The CS PBR section (CS target) and the complex material card show here too, as they do in 2D.
    let cs = set.is_some() && m.target == engine::config::Target::Cs;
    if item.is_none() && !json && !skin && !cs && !set.is_some_and(|v| v.shows_cm_card) {
        m.settings.set_settings_open = false;
        m.save_settings();
        return;
    }
    let title = match item {
        Some(k) => format!(
            "{} settings",
            m.item(k).map_or(k.as_str(), |i| i.name.as_str())
        ),
        None => "TruePBR settings".to_string(),
    };
    let rect = m.settings.layout.settings_window.map_or(
        egui::Rect::from_min_size(
            egui::pos2(120.0, 120.0),
            egui::vec2(SETTINGS_WINDOW_W, 620.0),
        ),
        |[x, y, w, h]| egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, h)),
    );
    let mut open = true;
    egui::Window::new(title)
        .id(settings_window_id())
        .open(&mut open)
        .resizable(true)
        .default_rect(rect)
        .constrain(true)
        .scroll([false, true])
        .show(ctx, |ui| {
            if let Some(k) = item {
                crate::ui::settings_panel::draw(ui, m, k, false);
            }
            let current = m.resolved_selection();
            if let Some(v) = current.view() {
                let json = crate::model::sets::shows_json_panel(v, m.target);
                if item.is_some()
                    && (json || v.skin || m.target == engine::config::Target::Cs || v.shows_cm_card)
                {
                    ui.separator();
                    ui.strong(format!("Set {}", v.id));
                }
                if json {
                    crate::ui::cs_pbr::pgp_panel(ui, m, v);
                }
                if v.skin {
                    crate::ui::skin_card::card(ui, m, v);
                }
                crate::ui::cs_pbr::card(ui, m, v);
                crate::ui::cs_pbr::section(ui, m, v);
                crate::ui::cm_card::section(ui, m, v);
            }
        });
    measure_window_extra(ctx, rect.size());
    if !open {
        m.settings.set_settings_open = false;
        m.save_settings();
    }
}
