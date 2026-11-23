//! The centre panel for a selected texture (M3a spec §2.4): the decisions it still needs, then its settings by group,
//! each with where its value came from. Drawing only: every edit is a call on the model, applied after drawing.
use super::columns::{AMBER, RED};
use super::status::code_hover;
use super::type_picker::TypeKind;
use crate::look::Combo036 as _;
use crate::model::bottom::{At, Level, Problem};
use crate::model::code_text::{
    alpha_confirm_text, alpha_label, change_line, code_line, not_a_cube,
};
use crate::model::fields::{
    auto_subtype_reason, bc3_pair, channel_text, encoding_options, filter_option_lock,
    format_choices, mips_on, provenance_text, weighting_option_lock, FieldView, BC1_ALPHA_HOVER,
    BC3_CHOICES, BC3_CUSTOM, BC3_SEARCH, EDGE_FIELD, EXR_STRAIGHT_HOVER, FILTER_FIELD,
    FILTER_OPTIONS, HELPER_HELP, LEVELS_FIELD, NOT_VALID, PARALLAX_HOVER, PERCEPTUAL_HOVER,
    WEIGHTING, WEIGHTING_OPTIONS,
};
use crate::model::items::{Decision, ItemKind, ItemStatus};
use crate::model::labels::{
    effort_label, type_hover, GREEN_TO_DIRECTX, GREEN_TO_OPENGL, RENORM_BAKE_HOVER,
    RENORM_BAKE_LABEL, UNORM_HOVER, UNORM_LABEL, VARIANCE_BAKE_HOVER,
};
use crate::model::nav::RevealField;
use crate::model::panel_groups::{self, GroupSummary, PanelGroup, SummaryExtra};
use crate::model::{Answer, AppModel, Selection, SetTarget};
use crate::ui::group::{collapsing_group, GroupHead};
use eframe::egui;
use engine::codes::{Code, Warning};
use engine::config::{
    AlphaPolicy, ColorTag, CubeLayout, CubeStyle, Effort, FaceFlip, Format, GreenConvention,
    MipFilter, Profile, Subtype, Target, ALPHA_THRESHOLD,
};
use engine::helper::HelperRole;
use engine::policy::{offered, Provenance, ResolvedJob};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// What a control asks of the model; applied once the drawing is over.
#[derive(Debug, PartialEq)]
pub enum Action {
    Set(&'static str, Value),
    Clear(&'static str),
    Decide(Answer),
    /// The single card's Confirm and likely-type buttons: decide, then select the next undecided texture in view.
    DecideAndAdvance(Answer),
    Select(String),
    Reencode(bool),
    /// Note HI4: a role picked under the ticked Helper box, or `None` (Reset) for the role guessed from the name.
    HelperRole(Option<HelperRole>),
    /// "Used by N meshes": a shape link; the app brings Meshes forward for it.
    ShowMesh(crate::model::used_by::MeshPick),
    /// "Show all in Meshes": the texture's scan path, as the Meshes texture filter takes it.
    ShowAllInMeshes(String),
    /// A group header's click: write the open state to `gui.toml`.
    SetGroupOpen(PanelGroup, bool),
    /// "+{n} more" on the first screen: open Problems on this texture's set (`Some`), or unfiltered (`None`).
    OpenProblems(Option<SetTarget>),
    /// A row took the waiting Go to's focus: it is done.
    RevealTaken,
}

/// Where the texture panel is drawn: the centre column, the 3D settings window, or the preview's 280 px strip.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Surface {
    Centre,
    Window,
    Strip,
}

impl Surface {
    /// The slot of this surface in each per-surface cache (`panel_cache`, `texture_problems`).
    fn slot(self) -> usize {
        match self {
            Surface::Centre => 0,
            Surface::Window => 1,
            Surface::Strip => 2,
        }
    }
}

/// Note HI4: an image's Helper row. `on` is the ticked box before a role is picked, held only in egui temp data (HI5);
/// `none` is a file the user unticked (`helper.role = "none"`).
struct HelperTick {
    on: bool,
    none: bool,
}

/// The groups the preview strip draws, in panel order.
const STRIP_GROUPS: [PanelGroup; 3] = [
    PanelGroup::Alpha,
    PanelGroup::Mipmaps,
    PanelGroup::Compression,
];
/// Note HI1: the label column's width, on the texture, helper and cubemap rows.
pub(super) const LABEL_W: f32 = 150.0;
/// Note HI1: every control's width on those rows (rule 3.5: `ComboBox::width` is only a minimum).
pub const CONTROL_W: f32 = 200.0;
/// Note JI3: the note column's least width (hint and ↺); the control column gives way first.
pub(super) const NOTE_MIN: f32 = 120.0;
/// Note JI3: the control column's least width before the label column shrinks (the set editor's `COMBO_W`).
pub(super) const CONTROL_MIN: f32 = super::set_editor::COMBO_W;
/// Note HI3: the controls' width in the preview's 280 px strip.
pub const COMPACT_CONTROL_W: f32 = 140.0;
/// A 4096 px side has 13 mip levels.
const MAX_LEVELS: u32 = 13;

/// An enum's options as (JSON value, label).
pub type Options = Vec<(Value, String)>;

fn opt(v: Value, label: &str) -> (Value, String) {
    (v, label.to_string())
}

pub fn subtype_label(st: Subtype) -> String {
    crate::model::labels::subtype_name(st)
}

/// The heading of the PBR group in every sub-type dropdown.
pub const PBR_HEADING: &str = "PBR (Community Shaders)";
/// The heading of the Advanced skin group, right after the PBR one.
pub const SKIN_HEADING: &str = "Advanced skin (Community Shaders)";
/// The heading of the last group: what no other group claims.
pub const OTHER_HEADING: &str = "Other";

/// The sub-types `keep` accepts, grouped for a dropdown: the PBR ones first (every `pbr_*`, `rmaos` and
/// `coat_normal`) under `PBR_HEADING`, then the Advanced skin ones (skin_albedo, skin_rfaos, skin_sk, skin_wet, skin_detail) under `SKIN_HEADING`, then the
/// rest by category, and `ui_sky` last under `OTHER_HEADING`. Empty groups are left out.
pub fn subtype_groups(keep: impl Fn(Subtype) -> bool) -> Vec<(&'static str, Vec<Subtype>)> {
    use engine::config::Category;
    let is_pbr = |s: Subtype| {
        s.id().starts_with("pbr_") || matches!(s, Subtype::Rmaos | Subtype::CoatNormal)
    };
    // The skin group reads in the order a set uses them, not in `Subtype::ALL`'s.
    let skin = [
        Subtype::SkinAlbedo,
        Subtype::SkinRfaos,
        Subtype::SkinSk,
        Subtype::SkinWet,
        Subtype::SkinDetail,
    ];
    let is_skin = |s: Subtype| skin.contains(&s);
    let is_other = |s: Subtype| matches!(s, Subtype::UiSky | Subtype::Cubemap);
    let claimed = |s: Subtype| is_pbr(s) || is_skin(s) || is_other(s);
    let kept: Vec<Subtype> = Subtype::ALL.into_iter().filter(|s| keep(*s)).collect();
    let pick = |f: &dyn Fn(Subtype) -> bool| -> Vec<Subtype> {
        kept.iter().copied().filter(|s| f(*s)).collect()
    };
    let mut groups: Vec<(&'static str, Vec<Subtype>)> = vec![
        (PBR_HEADING, pick(&is_pbr)),
        (
            SKIN_HEADING,
            skin.into_iter().filter(|s| keep(*s)).collect(),
        ),
    ];
    for (cat, name) in [
        (Category::Color, "Colour maps"),
        (Category::Normal, "Normal maps"),
        (Category::Grayscale, "Grayscale maps"),
        (Category::MixedPacked, "Packed and data"),
        (Category::Hdr, "HDR maps"),
    ] {
        groups.push((name, pick(&|s| !claimed(s) && s.category() == cat)));
    }
    groups.push((OTHER_HEADING, pick(&is_other)));
    groups.retain(|(_, v)| !v.is_empty());
    groups
}

/// The sub-type groups a picker offers under `target` (`engine::policy::offered_for`: `complex_grass` only in a
/// grass context, complex grass design §3.7, Note FC10).
pub fn offered_groups(target: Target, grass: bool) -> Vec<(&'static str, Vec<Subtype>)> {
    subtype_groups(|s| engine::policy::offered_for(target, s, grass))
}

const ALPHA_POLICIES: [AlphaPolicy; 5] = [
    AlphaPolicy::Remove,
    AlphaPolicy::Cutout,
    AlphaPolicy::Blend,
    AlphaPolicy::CutoutBlend,
    AlphaPolicy::Data,
];

pub fn alpha_options() -> Options {
    ALPHA_POLICIES
        .into_iter()
        .map(|p| (json!(p), alpha_label(p).into()))
        .collect()
}

/// Note FM35: the alpha question as one dropdown, open on `shown` (the resolved type). Picking an entry, the shown one
/// too, is the answer; each entry's hover says what it does. `None` while nothing is picked. `why` is the hover while
/// the dropdown is off. `place` starts the AccessKit name ("Problems alpha type: Cutout"), so the dropdowns of the
/// texture panel (`""`), Problems and Review, which can be on screen together, have distinct names (rule 8.4).
fn alpha_dropdown(
    ui: &mut egui::Ui,
    id: egui::Id,
    shown: AlphaPolicy,
    why: &str,
    place: &str,
) -> Option<AlphaPolicy> {
    let mut picked = None;
    let r = egui::ComboBox::from_id_salt(id)
        .selected_text(alpha_label(shown))
        .show_ui_036(ui, |ui| {
            for p in ALPHA_POLICIES {
                let r = ui.selectable_label(p == shown, alpha_label(p));
                if r.on_hover_text(alpha_hint(p)).clicked() {
                    picked = Some(p);
                }
            }
        })
        .response;
    let name = match place {
        "" => format!("Alpha type: {}", alpha_label(shown)),
        p => format!("{p} alpha type: {}", alpha_label(shown)),
    };
    r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, r.enabled(), &name));
    if !why.is_empty() {
        r.on_disabled_hover_text(why);
    }
    picked
}

/// Note HI7: the alpha question, [`alpha_dropdown`] and a Confirm button on one line, in reading order in either
/// layout direction (a Problems row is right to left). Confirm answers with the shown type, as picking it does. Its
/// accessible name is "Confirm alpha" after `place` ("Problems confirm alpha"), so the sub-type's "Confirm" stays unique.
pub fn alpha_question(
    ui: &mut egui::Ui,
    id: egui::Id,
    shown: AlphaPolicy,
    why: &str,
    place: &str,
) -> Option<AlphaPolicy> {
    if ui.layout().prefer_right_to_left() {
        let ok = alpha_confirm(ui, shown, why, place);
        alpha_dropdown(ui, id, shown, why, place).or(ok)
    } else {
        let picked = alpha_dropdown(ui, id, shown, why, place);
        picked.or(alpha_confirm(ui, shown, why, place))
    }
}

/// The Confirm button of an alpha question: answers with `shown`. Its accessible name is "Confirm alpha" after
/// `place` ("Problems confirm alpha"); `why` is its hover while it is off.
fn alpha_confirm(
    ui: &mut egui::Ui,
    shown: AlphaPolicy,
    why: &str,
    place: &str,
) -> Option<AlphaPolicy> {
    let r = ui.button("Confirm");
    let name = match place {
        "" => "Confirm alpha".to_string(),
        p => format!("{p} confirm alpha"),
    };
    r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, r.enabled(), &name));
    let r = r.on_hover_text(format!(
        "Use {} and close this question",
        alpha_label(shown)
    ));
    let r = match why {
        "" => r,
        w => r.on_disabled_hover_text(w),
    };
    r.clicked().then_some(shown)
}

/// Note JI5: the slot row's alpha type, left to right: the dropdown on `shown`, then Confirm while the type is still
/// the engine's proposal (`asked`). Picking the shown entry is no change and answers nothing; only Confirm answers
/// with it. The caller sets the dropdown's width (`spacing.combo_width`).
pub fn alpha_choice(
    ui: &mut egui::Ui,
    id: egui::Id,
    shown: AlphaPolicy,
    asked: bool,
    why: &str,
    place: &str,
) -> Option<AlphaPolicy> {
    let picked = alpha_dropdown(ui, id, shown, why, place).filter(|p| *p != shown);
    if asked {
        picked.or(alpha_confirm(ui, shown, why, place))
    } else {
        picked
    }
}

/// One line on what an alpha meaning does (Note FB8, review C12): the hover of each entry of the alpha dropdown.
pub fn alpha_hint(policy: AlphaPolicy) -> &'static str {
    match policy {
        AlphaPolicy::Remove => "Drops the alpha channel: the texture is fully solid",
        AlphaPolicy::Cutout => {
            "Alpha cuts holes: pixels over the threshold stay solid, the rest vanish (leaves, fences)"
        }
        AlphaPolicy::Blend => "Alpha fades smoothly: soft see-through edges (glass, hair)",
        AlphaPolicy::CutoutBlend => "Alpha cuts holes and also fades the edges",
        AlphaPolicy::Data => {
            "The alpha is kept as data (a mask or a height): it is written as it is, not as see-through"
        }
    }
}

/// Options for the fields that pick from a list; `None` for the checkboxes and the slider.
pub(crate) fn options_of(field: &str, target: Target, p: &Profile) -> Option<Options> {
    Some(match field {
        "subtype" => subtype_groups(|s| s == p.subtype || offered(target, s))
            .into_iter()
            .flat_map(|(heading, members)| {
                std::iter::once((Value::Null, heading.to_string()))
                    .chain(members.into_iter().map(|s| (json!(s), subtype_label(s))))
            })
            .collect(),
        // Note IA2: named by the baker; the change it makes is in brackets.
        crate::model::fields::MSN_SOURCE => vec![
            opt(json!("skyrim"), "Skyrim MSN (no change)"),
            opt(json!("swap_gb"), "Blender object-space (swap G and B)"),
            opt(
                json!("invert_b"),
                "Marmoset right-handed or Substance (invert B)",
            ),
            opt(json!("invert_gb"), "Marmoset left-handed (invert G and B)"),
        ],
        "color.content" => vec![
            opt(json!("srgb"), "sRGB-encoded colour"),
            opt(json!("linear"), "Linear data"),
        ],
        // The tags the engine allows with the current format, and the current one marked when it is not allowed.
        "color.tag" => encoding_options(target, p)
            .into_iter()
            .filter(|((f, _), _)| *f == p.format)
            .filter_map(|((_, t), label)| {
                let name = match t {
                    ColorTag::Srgb => "_SRGB",
                    ColorTag::Unorm => UNORM_LABEL,
                    ColorTag::NotApplicable => return None,
                };
                let mark = if label.ends_with(NOT_VALID) {
                    NOT_VALID
                } else {
                    ""
                };
                Some((json!(t), format!("{name}{mark}")))
            })
            .collect(),
        "alpha.policy" => alpha_options(),
        "alpha.compare" => vec![
            opt(json!("gt"), "Greater than"),
            opt(json!("ge"), "Greater than or equal"),
        ],
        "mips.levels" => {
            let mut v = vec![
                opt(json!("full"), "Full chain"),
                opt(json!("none"), "None (base level only)"),
            ];
            v.extend((2..=MAX_LEVELS).map(|n| (json!(n), format!("{n} levels"))));
            v
        }
        "mips.normal" => vec![
            opt(json!("renormalize"), "Renormalise"),
            opt(json!("renormalize_bake"), RENORM_BAKE_LABEL),
        ],
        "mips.normal_base" => vec![
            opt(json!("renormalize"), "Renormalise to unit length"),
            opt(json!("keep"), "Keep as authored"),
        ],
        "mips.edge" => vec![opt(json!("wrap"), "Wrap"), opt(json!("clamp"), "Clamp")],
        "resize" => vec![
            opt(json!("none"), "None"),
            opt(json!("half"), "Half"),
            opt(json!("quarter"), "Quarter"),
            opt(json!("eighth"), "Eighth"),
        ],
        // Note FM44: own size, or a texture-set slot by the set editor's name for it (PBR names for a PBR map).
        "resize_relative_to" => {
            let pbr = target == Target::Cs
                && (p.subtype.is_pbr_color()
                    || matches!(
                        p.subtype,
                        Subtype::PbrNormal | Subtype::Rmaos | Subtype::CoatNormal
                    ));
            std::iter::once(opt(json!("self"), "Own size"))
                .chain((1..=8u8).map(|n| {
                    let name = crate::model::sets::slot_name(pbr, n);
                    (json!(format!("slot{n}")), format!("Slot {n} · {name}"))
                }))
                .collect()
        }
        // A hand-written floor outside the list (300) is listed too, so the combo reads "300 px".
        "resize_floor" => {
            let mut v = vec![64, 128, 256, 512, 1024, 2048];
            if !v.contains(&p.resize_floor) {
                v.push(p.resize_floor);
                v.sort_unstable();
            }
            v.into_iter()
                .map(|n| (json!(n), format!("{n} px")))
                .collect()
        }
        "mips.coverage" => vec![
            opt(json!("auto"), "Auto"),
            opt(json!("on"), "On"),
            opt(json!("off"), "Off"),
        ],
        // A (format, tag) pair, so a pick never leaves a tag the format cannot take; applied with `set_format`.
        // Note HI11: the name alone; the tag is the DDS tag row's.
        "format" => format_choices(target, p)
            .into_iter()
            .map(|(e, label)| (json!(e), label))
            .collect(),
        // The variants are listed by name, so a missing or misspelt level fails to compile; the id is the stored one.
        "encoder.effort" => [
            Effort::Ultrafast,
            Effort::Veryfast,
            Effort::Fast,
            Effort::Basic,
            Effort::Veryslow,
            Effort::Slowest,
            Effort::Max,
        ]
        .into_iter()
        .map(|e| {
            let id = json!(e);
            let label = effort_label(id.as_str().unwrap_or_default());
            (id, label.to_string())
        })
        .collect(),
        // Note JI4: the Effort row's choices on BC4 and BC5, BC1, and BC3 (both fields, `BC3_CHOICES`).
        "encoder.bc45_search_radius" => search_options(true),
        "encoder.bc1_search" => search_options(false),
        BC3_SEARCH => {
            let mut v = search_options(true);
            let (bc1, radius) = bc3_pair(p);
            if !BC3_CHOICES.iter().any(|c| c.1 == bc1 && c.2 == radius) {
                v.push((
                    json!(BC3_CUSTOM),
                    format!("Custom: search {bc1}, radius {radius}"),
                ));
            }
            v
        }
        "header" => vec![
            opt(json!("auto"), "Auto"),
            opt(json!("dx10"), "DX10 extended header"),
            opt(json!("legacy"), "Legacy FourCC"),
        ],
        "mips.filter" => FILTER_OPTIONS
            .iter()
            .map(|(m, l)| opt(json!(m), l))
            .collect(),
        // Cubemap (Note DT7). Auto is the unset layout, "none" the unset style.
        "cubemap.layout" => std::iter::once(opt(json!("auto"), "Auto"))
            // Faces is detected from a six-file set, never chosen.
            .chain(
                PICKABLE_LAYOUTS
                    .iter()
                    .map(|(l, label)| opt(json!(l), label)),
            )
            .collect(),
        "cubemap.style" => vec![
            opt(json!("none"), "None"),
            opt(json!(CubeStyle::VanillaMetal), "Vanilla metal"),
            opt(json!(CubeStyle::Room), "Room"),
            opt(json!(CubeStyle::Eye), "Eye"),
            opt(json!(CubeStyle::Hd), "HD"),
        ],
        "cubemap.face_size" => std::iter::once(opt(json!("auto"), "Auto"))
            .chain((2..=12).map(|n| (json!(1u32 << n), format!("{} px", 1u32 << n))))
            .collect(),
        "cubemap.face_rotate" => [0, 90, 180, 270]
            .map(|d| (json!(d), format!("{d}°")))
            .to_vec(),
        "cubemap.face_flip" => vec![
            opt(json!(FaceFlip::None), "No flip"),
            opt(json!(FaceFlip::H), "Flip H"),
            opt(json!(FaceFlip::V), "Flip V"),
        ],
        _ => return None,
    })
}

/// The label `options_of` gives `value` for `field`, or `None` for a field with no list or a value not in it. It builds
/// the whole list, so a caller runs it only when its inputs change (the panel's summary cache), never per frame.
pub(crate) fn option_label(
    field: &str,
    target: Target,
    p: &Profile,
    value: &Value,
) -> Option<String> {
    options_of(field, target, p)?
        .into_iter()
        .find(|(v, _)| v == value)
        .map(|(_, label)| label)
}

/// Note JI4, Note LI: Fast and Default, then (with `slow`) Slowest and Max, the search fields' choices in that order.
fn search_options(slow: bool) -> Options {
    let mut v = vec![opt(json!("fast"), "Fast"), opt(json!("default"), "Default")];
    if slow {
        v.push(opt(json!("slowest"), "Slowest"));
        v.push(opt(json!("max"), "Max"));
    }
    v
}

/// Note JI4: the hover of the BC3 Effort row's read-only entry for a pair no choice names.
const CUSTOM_HOVER: &str =
    "Mixed values from a preset or [files]; pick Fast, Default, Slowest or Max to set both";

/// The layouts a user can pick, in display order. `Faces` (a six-file set) is not chosen but detected.
const PICKABLE_LAYOUTS: [(CubeLayout, &str); 4] = [
    (CubeLayout::Hcross, "Horizontal cross"),
    (CubeLayout::Vcross, "Vertical cross"),
    (CubeLayout::Hstrip, "Horizontal strip"),
    (CubeLayout::Vstrip, "Vertical strip"),
];

fn display_value(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// A combo over `options`; the new value when one is picked.
pub(super) fn combo(
    ui: &mut egui::Ui,
    id: egui::Id,
    current: &Value,
    options: &Options,
    mixed: bool,
) -> (Option<Value>, egui::Response) {
    combo_for("", ui, id, current, options, mixed)
}

/// [`combo`] for `field`: the Alpha field's five options carry `alpha_hint` as their hover (Note FB8, review C12).
pub(super) fn combo_for(
    field: &str,
    ui: &mut egui::Ui,
    id: egui::Id,
    current: &Value,
    options: &Options,
    mixed: bool,
) -> (Option<Value>, egui::Response) {
    // Note FB4: "— mixed" and "— not set" (no value, nothing to mix) are drawn weak, never the word `null`.
    let shown = if mixed {
        egui::RichText::new("— mixed").weak()
    } else if current.is_null() {
        // A null option is a group heading, so a null value never matches one.
        egui::RichText::new("— not set").weak()
    } else {
        match options.iter().find(|(v, _)| v == current) {
            // Rule 3.5: "Slot 8 · Back-light / specular" held its panel ~90 px past its edge, so the closed combo
            // shows "Slot 8"; the list and the hover keep the slot's name.
            Some((_, l)) if field == "resize_relative_to" => {
                egui::RichText::new(l.split(" · ").next().unwrap_or(l))
            }
            Some((_, l)) => egui::RichText::new(l),
            None => egui::RichText::new(display_value(current)),
        }
    };
    // Rule 3.4: the BC3 row's Custom entry is longer than the combo, so the closed combo shows it whole on hover.
    let custom = field == BC3_SEARCH && *current == json!(BC3_CUSTOM);
    let full = (field == "resize_relative_to" || custom)
        .then(|| options.iter().find(|(v, _)| v == current))
        .flatten()
        .map(|(_, l)| l.clone());
    let mut picked = None;
    let r = egui::ComboBox::from_id_salt(id)
        .selected_text(shown)
        .show_ui_036(ui, |ui| {
            for (v, label) in options {
                // A null value is a non-selectable group heading.
                if v.is_null() {
                    ui.label(egui::RichText::new(label).weak());
                } else if field == BC3_SEARCH && *v == json!(BC3_CUSTOM) {
                    // Note JI4: the mixed pair is shown, never picked.
                    ui.add_enabled(false, egui::Button::selectable(v == current, label))
                        .on_disabled_hover_text(CUSTOM_HOVER);
                } else {
                    let mut r = ui.selectable_label(v == current, label);
                    if field == "alpha.policy" {
                        if let Ok(p) = serde_json::from_value::<AlphaPolicy>(v.clone()) {
                            r = r.on_hover_text(alpha_hint(p));
                        }
                    } else if field == "color.tag" && *v == json!(ColorTag::Unorm) {
                        r = r.on_hover_text(UNORM_HOVER);
                    } else if field == "mips.normal" && *v == json!("renormalize_bake") {
                        r = r.on_hover_text(RENORM_BAKE_HOVER);
                    }
                    if r.clicked() {
                        picked = Some(v.clone());
                    }
                }
            }
        })
        .response;
    let r = match full {
        Some(l) => r.on_hover_text(l),
        None => r,
    };
    (picked.filter(|v| v != current), r)
}

/// A slider keeps its value locally while it is dragged and commits on release: every commit is a full re-plan.
/// `committed` marks a value already sent, shown until the new plan lands.
#[derive(Clone, Copy)]
struct Pending {
    value: u32,
    committed: bool,
}

/// A slider over `range` that returns a value only when the drag ends (or a click or key press settles it).
/// `id` keeps the in-progress value between frames; `name` is the accessible name.
pub fn commit_slider(
    ui: &mut egui::Ui,
    model_busy: bool,
    id: egui::Id,
    current: u32,
    range: std::ops::RangeInclusive<u32>,
    name: &str,
    mixed: bool,
) -> (Option<u32>, egui::Response) {
    let mut pending: Option<Pending> = ui.data(|d| d.get_temp(id));
    if let Some(p) = pending {
        if (!mixed && p.value == current) || (p.committed && !model_busy) {
            pending = None;
        }
    }
    let mut value = pending.map_or(current, |p| p.value);
    let mut slider = egui::Slider::new(&mut value, range);
    // A mixed value shows as text until the user drags; only a drag writes.
    if mixed && pending.is_none() {
        slider = slider.custom_formatter(|_, _| "— mixed".to_string());
    }
    let r = ui.add(slider);
    r.widget_info(|| egui::WidgetInfo::slider(r.enabled(), f64::from(value), name));
    if r.changed() {
        pending = Some(Pending {
            value,
            committed: false,
        });
    }
    let released = r.drag_stopped()
        || r.lost_focus()
        || r.clicked()
        || (r.changed() && !r.dragged() && !r.is_pointer_button_down_on());
    let mut out = None;
    if released {
        if let Some(Pending {
            value,
            committed: false,
        }) = pending
        {
            if mixed || value != current {
                out = Some(value);
                pending = Some(Pending {
                    value,
                    committed: true,
                });
            }
        }
    }
    ui.data_mut(|d| match pending {
        Some(p) => {
            d.insert_temp(id, p);
        }
        None => {
            d.remove::<Pending>(id);
        }
    });
    (out, r)
}

fn threshold(
    ui: &mut egui::Ui,
    model_busy: bool,
    key: &str,
    current: u8,
    mixed: bool,
) -> (Option<Value>, egui::Response) {
    let id = egui::Id::unique(("threshold", key));
    let (out, r) = commit_slider(
        ui,
        model_busy,
        id,
        u32::from(current),
        u32::from(*ALPHA_THRESHOLD.start())..=u32::from(*ALPHA_THRESHOLD.end()),
        THRESHOLD_NAME,
        mixed,
    );
    (out.map(|v| json!(v)), r)
}

/// The threshold slider's own name: it says more than its row's "Threshold" label.
const THRESHOLD_NAME: &str = "Alpha threshold";

/// E28: an [`editor`] linked to its row's label with `labelled_by`, which in egui 0.37 clears the control's own name
/// (`response.rs:1065-1071`; 0.36.2 kept it, and a screen reader reads a control's own name first). Call it after the
/// link: it names again the one editor whose own name says more than the label, the threshold slider, as 0.36.2
/// announced it. Every other editor's own name is the label's words (a tick box, BC7 weighting) or nothing (a combo,
/// whose text is its value), so the label names it.
pub(super) fn own_name_after_link(r: egui::Response, field: &str) -> egui::Response {
    match field {
        "alpha.threshold" => super::name_input(r, THRESHOLD_NAME),
        _ => r,
    }
}

/// The BC7 weighting dropdown (DECISIONS Note CP): every option with one line of hover text; Perceptual disabled with
/// its reason where the old Perceptual row was locked.
fn weighting_combo(
    ui: &mut egui::Ui,
    id: egui::Id,
    target: Target,
    job: &ResolvedJob,
    f: &FieldView,
    mixed: bool,
) -> (Option<Value>, egui::Response) {
    let current = f.value.as_str().unwrap_or("linear");
    let shown = if mixed {
        "— mixed"
    } else {
        WEIGHTING_OPTIONS
            .iter()
            .find(|o| o.0 == current)
            .map_or(current, |o| o.1)
    };
    let mut picked = None;
    let r = egui::ComboBox::from_id_salt(id)
        .selected_text(shown)
        .show_ui_036(ui, |ui| {
            for (v, label, hover) in WEIGHTING_OPTIONS {
                let lock = weighting_option_lock(target, &job.profile, v);
                let r = ui.add_enabled(
                    lock.is_none(),
                    egui::Button::selectable(!mixed && v == current, label),
                );
                match lock {
                    Some(why) => {
                        r.on_disabled_hover_text(format!("{hover} Not here: {why}."));
                    }
                    None => {
                        if r.on_hover_text(hover).clicked() && (mixed || v != current) {
                            picked = Some(json!(v));
                        }
                    }
                }
            }
        })
        .response;
    // M4: named for a screen reader, with the shown option as its value.
    r.widget_info(|| {
        let mut i = egui::WidgetInfo::labeled(egui::Role::ComboBox, r.enabled(), "BC7 weighting");
        i.current_text_value = Some(shown.to_string());
        i
    });
    (picked, r)
}

/// Shown on an entry no selected texture can take.
const NO_REACH_HOVER: &str = "None of the selected textures can take this";

/// E14: the bulk combo of a value-dependent row (filter, BC7 weighting, Format, DDS tag). Each entry reads "{option}
/// ({k} of {total})" and is enabled when a write of it reaches some texture (`BulkField::option_reach`); a pick
/// dispatches when it differs from the row's one value (a mixed row has none). The closed combo shows the bare option.
pub(super) fn reach_combo(
    ui: &mut egui::Ui,
    id: egui::Id,
    field: &str,
    current: &Value,
    reach: &[(String, Value, usize)],
    total: usize,
    (mixed, busy): (bool, bool),
) -> (Option<Value>, egui::Response) {
    let shown = if mixed {
        egui::RichText::new("— mixed").weak()
    } else if current.is_null() {
        egui::RichText::new("— not set").weak()
    } else {
        match reach.iter().find(|(_, v, _)| v == current) {
            Some((l, _, _)) => egui::RichText::new(l),
            None => egui::RichText::new(display_value(current)),
        }
    };
    let mut picked = None;
    let r = egui::ComboBox::from_id_salt(id)
        .selected_text(shown)
        .show_ui_036(ui, |ui| {
            for (label, v, k) in reach {
                let on = *k > 0 && !busy;
                let text = format!("{label} ({k} of {total})");
                let r = ui.add_enabled(on, egui::Button::selectable(!mixed && v == current, text));
                let hover = if field == WEIGHTING {
                    WEIGHTING_OPTIONS
                        .iter()
                        .find(|o| v.as_str() == Some(o.0))
                        .map(|o| o.2)
                } else if field == "color.tag" && *v == json!(ColorTag::Unorm) {
                    Some(UNORM_HOVER)
                } else {
                    None
                };
                // An entry that reaches nothing stays off after the plan too, so waiting is no reason for it.
                let r = match (on, hover) {
                    (false, _) if *k == 0 => r.on_disabled_hover_text(NO_REACH_HOVER),
                    (false, _) => r.on_disabled_hover_text("Wait for the plan to finish"),
                    (true, Some(h)) => r.on_hover_text(h),
                    (true, None) => r,
                };
                if on && r.clicked() && v != current {
                    picked = Some(v.clone());
                }
            }
        })
        .response;
    (picked, r)
}

/// The checkbox fields, each with its hover (`None` inside: a row with no hover); `None` for any other field.
fn checkbox_hover(field: &str) -> Option<Option<&'static str>> {
    Some(match field {
        "alpha.parallax" => Some(PARALLAX_HOVER),
        "encoder.bc1_alpha" => Some(BC1_ALPHA_HOVER),
        "encoder.perceptual" => Some(PERCEPTUAL_HOVER),
        "alpha.exr_straight" => Some(EXR_STRAIGHT_HOVER),
        "alpha.exact_opaque" => None,
        _ => return None,
    })
}

pub(super) fn editor(
    ui: &mut egui::Ui,
    key: &str,
    target: Target,
    job: &ResolvedJob,
    busy: bool,
    f: &FieldView,
    mixed: bool,
) -> (Option<Value>, egui::Response) {
    match f.field {
        "alpha.threshold" => {
            let cur = f.value.as_u64().unwrap_or(128).clamp(
                u64::from(*ALPHA_THRESHOLD.start()),
                u64::from(*ALPHA_THRESHOLD.end()),
            ) as u8;
            threshold(ui, busy, key, cur, mixed)
        }
        WEIGHTING => weighting_combo(ui, egui::Id::unique((key, f.field)), target, job, f, mixed),
        field if checkbox_hover(field).is_some() => {
            let hover = checkbox_hover(field).flatten();
            let mut b = f.value.as_bool().unwrap_or(false);
            let mut r = ui.add(egui::Checkbox::new(&mut b, "").indeterminate(mixed));
            if let Some(h) = hover {
                r = r.on_hover_text(h);
            }
            // The box has no text of its own, so it takes the row's label as its AccessKit name (FM42, Note HP M1).
            r.widget_info(|| {
                egui::WidgetInfo::selected(egui::Role::CheckBox, r.enabled(), b, f.label)
            });
            (r.changed().then(|| json!(b)), r)
        }
        field => {
            let options = options_of(field, target, &job.profile).unwrap_or_default();
            let current = if field == "format" {
                json!((job.profile.format, job.profile.color.tag))
            } else {
                f.value.clone()
            };
            combo_for(
                field,
                ui,
                egui::Id::unique((key, field)),
                &current,
                &options,
                mixed,
            )
        }
    }
}

/// Everything a group's rows need besides the group name.
pub struct Ctx<'a> {
    pub key: &'a str,
    pub job: &'a ResolvedJob,
    pub target: Target,
    pub busy: bool,
    /// Note HI1, HI17: off (the texture panel in the centre and in the 3D settings window), the rows stand in a
    /// `LABEL_W` label column and a `CONTROL_W` control column; on in the strip.
    pub compact: bool,
    /// Note HI14: the alpha question is open, so `job` holds the proposal (display only) and the Alpha row says so.
    pub alpha_asked: bool,
    /// Why the rows' edits are off, shown on their disabled hover: `sdc.toml` is read-only, or (the texture panel,
    /// `AppModel::fix_off`, Note FK) a plan is in flight.
    pub edit_off: Option<String>,
    /// The item's warnings (the Compression group shows the HDR reason).
    pub warnings: &'a [Warning],
    /// Where this panel is drawn; `compact` is `surface == Surface::Strip`.
    pub surface: Surface,
    /// `model.settings.texture_groups`, read by `panel_groups::is_open_in`.
    pub groups: &'a BTreeMap<String, bool>,
    /// Groups opened for a problem or a Go to, until the user's click (`model.panel_forced`, for the selected item).
    pub forced: &'a BTreeSet<PanelGroup>,
    /// Groups that hold a field one of this texture's problems points at: each header's dot (`texture_problems`).
    /// and the worst level of each (`panel_groups::problem_levels`).
    pub problem_groups: &'a BTreeMap<PanelGroup, Level>,
    /// The header summaries and marks, built with `views` (`panel_cache`).
    pub summaries: &'a [(PanelGroup, GroupSummary)],
    /// A Go to waiting for a row of this texture: only the centre holds one, so the window and the strip never take
    /// the focus (`model.nav.reveal_field`).
    pub reveal: Option<&'a RevealField>,
}

/// What a field row asks of the model.
pub enum RowEdit {
    Set(Value),
    /// Use the inherited value again.
    Clear,
}

/// A row's label cell (Note HI1): with `aligned`, exactly the label column's width (`LABEL_W` where the row fits) and
/// cut short with the whole text on hover (rule 3.4); otherwise a plain label. With `help` (Note HI9), a `?` ends the
/// cell and the label shows the same help on hover.
pub(super) fn field_label(
    ui: &mut egui::Ui,
    aligned: bool,
    label: &str,
    help: Option<&str>,
) -> egui::Response {
    if !aligned {
        let Some(t) = help else {
            return ui.label(label);
        };
        return ui
            .horizontal(|ui| {
                let l = ui.label(label);
                help_mark(ui, label, t);
                help_hover(l, t)
            })
            .inner;
    }
    let w = column_widths(ui).0;
    label_slot(ui, w, label, help)
}

/// An exactly `w` wide label cell, cut short; with `help`, a `?` right-aligned at its end (Note HI9).
fn label_slot(ui: &mut egui::Ui, w: f32, label: &str, help: Option<&str>) -> egui::Response {
    control_slot(ui, w, |ui| match help {
        None => ui.add(egui::Label::new(label).truncate()),
        Some(t) => {
            let (l, _) = egui::Sides::new().shrink_left().truncate().show(
                ui,
                |ui| ui.add(egui::Label::new(label).truncate()),
                |ui| help_mark(ui, label, t),
            );
            help_hover(l, t)
        }
    })
}

/// `t` on hover, also while the row is off (`edit_off`, a lock): a disabled widget shows only its disabled hover.
fn help_hover(r: egui::Response, t: &str) -> egui::Response {
    r.on_hover_text(t).on_disabled_hover_text(t)
}

/// A weak `?` in a small ring that shows `text` on hover (Note HI9), named "Help: {name}" for a screen reader.
pub(super) fn help_mark(ui: &mut egui::Ui, name: &str, text: &str) -> egui::Response {
    const D: f32 = 14.0;
    let (rect, r) = ui.allocate_exact_size(egui::vec2(D, D), egui::Sense::hover());
    let color = ui.visuals().weak_text_color();
    let p = ui.painter();
    p.circle_stroke(rect.center(), D / 2.0 - 0.5, egui::Stroke::new(1.0, color));
    p.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        "?",
        egui::FontId::proportional(10.0),
        color,
    );
    r.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::Label, r.enabled(), format!("Help: {name}"))
    });
    help_hover(r, text)
}

/// The label and control columns' widths: `LABEL_W` and `CONTROL_W`, both scaled down alike where a row would run
/// past the visible edge (a narrow centre; rules 3.1, 3.8). The rest of the row keeps three gaps, ↺, an elided hint
/// and a margin (rule 3.7): a row as wide as the centre widens it past the window.
/// Measured from the grid's left edge, so every row of a grid gets the same two widths.
pub(super) fn column_widths(ui: &egui::Ui) -> (f32, f32) {
    column_widths_beside(ui, 0.0)
}

/// `column_widths` for a grid whose control cells also hold a `trail` wide button (the helper panel's ✕ and Reset):
/// the label column is the same, the control gives up what the button needs where the row would not fit.
pub(super) fn column_widths_beside(ui: &egui::Ui, trail: f32) -> (f32, f32) {
    column_widths_at(ui, ui.max_rect().left(), trail)
}

/// `column_widths_beside` for a grid whose left edge is `left` (the Undecided box lines up with the groups below it).
///
/// Note JI3, HI1: the note column (hint and ↺) keeps `NOTE_MIN`, so as the room shrinks the control gives way first,
/// from `CONTROL_W` to `CONTROL_MIN`, then the label, from `LABEL_W` to `interact_size.x`; only below that does the
/// note give way. One rule for every caller, so controls keep one x and one width across groups.
fn column_widths_at(ui: &egui::Ui, left: f32, trail: f32) -> (f32, f32) {
    let s = ui.spacing();
    let room = ui.clip_rect().right() - left - 2.0 * s.item_spacing.x - NOTE_MIN;
    let control = (room - LABEL_W - trail).clamp(CONTROL_MIN, CONTROL_W);
    let label = (room - control - trail).min(LABEL_W).max(s.interact_size.x);
    (label, control)
}

/// An exactly `w` wide cell (rule 3.5): text in it is cut short, and a combo in it is `w` wide. A slider's rail leaves
/// room for its value box.
pub(super) fn control_slot<R>(
    ui: &mut egui::Ui,
    w: f32,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let h = ui.spacing().interact_size.y;
    let layout = egui::Layout::left_to_right(egui::Align::Center);
    ui.allocate_ui_with_layout(egui::vec2(w, h), layout, |ui| {
        ui.set_min_size(egui::vec2(w, h));
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
        let s = ui.spacing_mut();
        s.combo_width = w;
        s.slider_width = w - s.interact_size.x - s.item_spacing.x;
        add(ui)
    })
    .inner
}

/// A row's label and control cells (Note HI1, HI3). The control is `CONTROL_W` wide; in the strip,
/// `COMPACT_CONTROL_W`. A control with no name of its own (a combo's text is its value) is named after the row's
/// label.
fn control_cell(
    ui: &mut egui::Ui,
    cx: &Ctx,
    label: &str,
    help: Option<&str>,
    add: impl FnOnce(&mut egui::Ui) -> (Option<Value>, egui::Response),
) -> (Option<Value>, egui::Response) {
    let l = if !cx.compact {
        field_label(ui, true, label, help)
    } else {
        // The strip's labels take the room the control and ↺ leave (bounded by the visible edge, rule 3.8), cut
        // short with the whole text on hover, so a 140 px control fits the 280 px strip.
        let s = ui.spacing();
        let room = ui.clip_rect().right() - ui.cursor().min.x;
        let w = (room - COMPACT_CONTROL_W - 2.0 * s.item_spacing.x - s.interact_size.y)
            .max(s.interact_size.x);
        let l = label_slot(ui, w, label, None);
        // Note HI18: the strip is narrow, so no `?` (it cut the strip's labels); the label carries the help.
        match help {
            Some(t) => help_hover(l, t),
            None => l,
        }
    };
    let w = if cx.compact {
        COMPACT_CONTROL_W
    } else {
        column_widths(ui).1
    };
    let (v, r) = control_slot(ui, w, add);
    (v, r.labelled_by(l.id))
}

/// One grid row of a field: its label, its editor (disabled with the reason when locked), where the value came from
/// (or the lock's reason), and a reset when the texture's own entry sets it. `compact` leaves the provenance blank. The
/// row's edit goes to `actions`, and a waiting Go to for this field takes the focus here.
pub fn field_row(ui: &mut egui::Ui, cx: &Ctx, f: &FieldView, actions: &mut Vec<Action>) {
    let mut edit = None;
    let help = crate::model::fields::field_help(f.field);
    let (set, r) = control_cell(ui, cx, f.label, help, |ui| {
        ui.scope(|ui| {
            if f.lock.is_some() {
                ui.disable();
            }
            if f.field == "subtype" {
                return subtype_field(ui, cx, f);
            }
            editor(ui, cx.key, cx.target, cx.job, cx.busy, f, false)
        })
        .inner
    });
    let r = own_name_after_link(r, f.field);
    take_field_focus(cx, f.field, &r, actions);
    // Note CL40: on a `normal` in a complex material set, the bake is the opt-in gloss bake, so its hover shows
    // before the choice. Elsewhere the hover (and the variance line) belongs to the active bake only.
    let bake_active = f.field == "mips.normal" && f.value == json!("renormalize_bake");
    let gloss_opt_in = f.field == "mips.normal"
        && cx.job.profile.subtype == Subtype::Normal
        && cx
            .job
            .shader
            .as_ref()
            .is_some_and(|s| s.shader == engine::shader::Shader::ComplexMaterial);
    let r = if bake_active {
        r.on_hover_text(format!("{RENORM_BAKE_HOVER}\n{VARIANCE_BAKE_HOVER}"))
    } else if gloss_opt_in {
        r.on_hover_text(RENORM_BAKE_HOVER)
    } else {
        r
    };
    if let Some(reason) = f.lock.map(str::to_string).or(cx.edit_off.clone()) {
        r.on_disabled_hover_text(reason);
    }
    if let Some(v) = set {
        edit = Some(RowEdit::Set(v));
    }
    let hint = |ui: &mut egui::Ui| {
        if cx.compact {
            ui.label("");
        } else if let Some(reason) = f.lock {
            hint_label(ui, reason);
        } else if let Some(reason) =
            crate::model::fields::no_effect_reason(&cx.job.profile, f.field)
        {
            hint_label(ui, reason);
        } else if cx.alpha_asked && f.field == "alpha.policy" {
            hint_label(ui, "proposed: confirm above");
        } else if let Some(why) = (f.field == "subtype")
            .then(|| auto_subtype_reason(cx.job))
            .flatten()
        {
            // M6 Track E (Note U4): a sub-type an accepted High suggestion decided; hover says why.
            ui.label(egui::RichText::new("(auto)").small().weak())
                .on_hover_text(why);
        } else {
            hint_label(ui, &provenance_text(&f.provenance));
        }
    };
    if hint_and_reset(ui, f.edited, hint) {
        edit = Some(RowEdit::Clear);
    }
    ui.end_row();
    push_edit(actions, f.field, edit);
}

/// A Go to waiting for `field` (only the centre's `Ctx` holds one): focus its control, scroll it to the middle and tell
/// the model it was taken. An `Option` test and one `&str` compare per row per frame.
pub(super) fn take_field_focus(
    cx: &Ctx,
    field: &str,
    r: &egui::Response,
    actions: &mut Vec<Action>,
) {
    if focus_if(cx.reveal, field, r) {
        actions.push(Action::RevealTaken);
    }
}

/// The one rule every row that can be a `FIX_FIELDS` target follows (the field rows, the Filter row, the cube rows):
/// when the waiting Go to names `field`, focus `r` (a disabled control keeps no focus; the group is open and the row
/// scrolled to all the same) and scroll it to the middle. True when the request was for this row, so it is taken.
pub(super) fn focus_if(reveal: Option<&RevealField>, field: &str, r: &egui::Response) -> bool {
    let hit = reveal.is_some_and(|q| q.field == field);
    if hit {
        r.request_focus();
        r.scroll_to_me(Some(egui::Align::Center));
    }
    hit
}

/// A row's hint, cut to the room left with the whole text on one tooltip (Note FI12, rule 3.4).
fn hint_label(ui: &mut egui::Ui, text: &str) {
    ui.add(
        egui::Label::new(egui::RichText::new(text).small().weak())
            .truncate()
            .show_tooltip_when_elided(false),
    )
    .on_hover_text(text);
}

/// The last cell of a field row: ↺ (when `edited`) is laid out first on the right, the hint takes what is left
/// (Note FI12, rule 3.3), so the reset never leaves the panel. Returns true when ↺ is clicked.
fn hint_and_reset(ui: &mut egui::Ui, edited: bool, hint: impl FnOnce(&mut egui::Ui)) -> bool {
    // The panel's scroll bar (floating, thin in the centre: rule 3.2) senses its full width, which reaches over the
    // content's right edge, and with Levels, Size and DDS header (Note JI3) the panel scrolls at 800 px high: ↺ keeps
    // clear of it, or its click goes to the bar.
    let s = &ui.spacing().scroll;
    let bar = s.bar_width + s.bar_outer_margin;
    ui.scope(|ui| {
        ui.set_max_width((ui.available_width() - bar).max(0.0));
        egui::Sides::new()
            .shrink_left()
            .truncate()
            .show(ui, hint, |ui| {
                edited
                    && ui
                        .small_button("↺")
                        .on_hover_text("Use the inherited value")
                        .clicked()
            })
            .1
    })
    .inner
}

/// The Sub-type field (Note FB9, owner amendment J): a button showing the type that opens the type picker, the
/// engine's suggestion pinned on top. Named and valued as the combo it replaces.
fn subtype_field(ui: &mut egui::Ui, cx: &Ctx, f: &FieldView) -> (Option<Value>, egui::Response) {
    let current = serde_json::from_value::<Subtype>(f.value.clone()).ok();
    let shown = current.map_or_else(|| display_value(&f.value), subtype_label);
    // It fills its cell, as the combos do (Note HI1); capped at CONTROL_W in case a cell is ever unbounded.
    let fill = egui::vec2(ui.available_width().min(CONTROL_W), 0.0);
    let r = ui.add(egui::Button::new(format!("{shown} \u{25BE}")).min_size(fill));
    r.widget_info(|| {
        let mut i = egui::WidgetInfo::labeled(egui::Role::ComboBox, r.enabled(), f.label);
        i.current_text_value = Some(shown.clone());
        i
    });
    let id = egui::Id::unique(("sub-type-picker", cx.key));
    let suggested = cx.job.suggestions.first().map(|s| s.subtype);
    let grass = engine::policy::grass_context(cx.job);
    let kind = TypeKind::SubType;
    let picked = super::type_picker::show(ui, id, &r, current, suggested, kind, cx.target, grass);
    (picked.filter(|s| Some(*s) != current).map(|s| json!(s)), r)
}

/// The basic Mipmaps rows: the on/off tick box, the level count (Note JI3; not in the strip), then the filter.
fn mipmaps_rows(ui: &mut egui::Ui, cx: &Ctx, views: &[FieldView], actions: &mut Vec<Action>) {
    // Each row is drawn when `panel_views` carries it (a profile may hide one); the cached views were read once.
    let find = |field: &str| views.iter().find(|v| v.field == field);
    if let Some(f) = find(LEVELS_FIELD.0) {
        mips_tick_and_levels(ui, cx, f, actions);
    }
    if let Some(filter) = find(FILTER_FIELD.0) {
        filter_row(ui, cx, filter, actions);
    }
    if let Some(e) = find(EDGE_FIELD.0) {
        field_row(ui, cx, e, actions);
    }
}

/// The Mipmaps tick row and, outside the strip, the Levels row under it.
fn mips_tick_and_levels(ui: &mut egui::Ui, cx: &Ctx, f: &FieldView, actions: &mut Vec<Action>) {
    let mut on = mips_on(&f.value);
    // Note HI1: "Mipmaps" in the label column, the tick in the control column, named for the tests and a screen reader.
    let help = Some(crate::model::fields::MIPMAPS_HELP);
    let (_, ticked) = control_cell(ui, cx, "Mipmaps", help, |ui| {
        let r = ui.add_enabled(cx.edit_off.is_none(), egui::Checkbox::new(&mut on, ""));
        let on = on;
        r.widget_info(|| {
            egui::WidgetInfo::selected(egui::Role::CheckBox, r.enabled(), on, "Mipmaps")
        });
        (None, r)
    });
    let ticked = match &cx.edit_off {
        Some(why) => ticked.on_disabled_hover_text(why),
        None => ticked,
    };
    if ticked.changed() {
        actions.push(Action::Set(
            f.field,
            json!(if on { "full" } else { "none" }),
        ));
    }
    // Note JI3: the Levels row below carries the count's note and ↺; only the strip, which has no Levels row, puts the
    // ↺ on the tick row.
    let hint = |ui: &mut egui::Ui| {
        ui.label("");
    };
    if hint_and_reset(ui, cx.compact && f.edited, hint) {
        actions.push(Action::Clear(f.field));
    }
    ui.end_row();
    if !cx.compact {
        field_row(ui, cx, f, actions);
    }
}

/// The live Filter combo: every option is listed, the ones S-mips-2 bars for this profile disabled with the reason.
fn filter_row(ui: &mut egui::Ui, cx: &Ctx, f: &FieldView, actions: &mut Vec<Action>) {
    const OPTIONS: [(MipFilter, &str); 4] = FILTER_OPTIONS;
    let current = cx.job.profile.mips.filter;
    let locks = OPTIONS.map(|(m, _)| filter_option_lock(&cx.job.profile, m));
    let mut picked = None;
    let help = crate::model::fields::field_help(f.field);
    let (_, r) = control_cell(ui, cx, f.label, help, |ui| {
        let r = egui::ComboBox::from_id_salt((cx.key, f.field))
            .selected_text(OPTIONS.iter().find(|o| o.0 == current).map_or("", |o| o.1))
            .show_ui_036(ui, |ui| {
                for ((m, label), lock) in OPTIONS.into_iter().zip(locks) {
                    let r = ui.add_enabled(
                        lock.is_none(),
                        egui::Button::selectable(m == current, label),
                    );
                    match lock {
                        Some(why) => {
                            r.on_disabled_hover_text(why);
                        }
                        None if r.clicked() && m != current => picked = Some(m),
                        None => {}
                    }
                }
            })
            .response;
        (None, r)
    });
    take_field_focus(cx, f.field, &r, actions);
    // Only Box is open: say why on the combo itself.
    let r = match locks[1..]
        .iter()
        .all(Option::is_some)
        .then_some(locks[1])
        .flatten()
    {
        Some(why) => r.on_hover_text(why),
        None => r,
    };
    if let Some(why) = &cx.edit_off {
        r.on_disabled_hover_text(why);
    }
    if let Some(m) = picked {
        actions.push(Action::Set(f.field, json!(m)));
    }
    let hint = |ui: &mut egui::Ui| {
        if cx.compact {
            ui.label("");
        } else {
            hint_label(ui, &provenance_text(&f.provenance));
        }
    };
    if hint_and_reset(ui, f.edited, hint) {
        actions.push(Action::Clear(f.field));
    }
    ui.end_row();
}

/// "Helper role [Pick a role…]" in the Sub-type row's place while the Helper box is ticked (Note HI4, HI5).
fn helper_role_row(
    ui: &mut egui::Ui,
    key: &str,
    edit_off: &Option<String>,
    actions: &mut Vec<Action>,
) {
    field_label(ui, true, "Helper role", None);
    let w = column_widths(ui).1;
    if let Some(role) = super::helper_panel::role_combo(ui, key, None, false, edit_off, w) {
        actions.push(Action::HelperRole(Some(role)));
    }
    hint_label(ui, "pick a role to make this a helper");
    ui.end_row();
}

/// The Helper row under Sub-type (Note HI4): the tick only swaps the row above, nothing is written (HI5). A file the
/// user unticked says so, with the way back to the guessed role.
fn helper_tick_row(
    ui: &mut egui::Ui,
    key: &str,
    t: &HelperTick,
    edit_off: &Option<String>,
    actions: &mut Vec<Action>,
) {
    field_label(ui, true, "Helper", Some(HELPER_HELP));
    let w = column_widths(ui).1;
    let r = control_slot(ui, w, |ui| {
        super::helper_panel::helper_tick(ui, t.on, 0, edit_off)
    });
    if r.changed() {
        let id = super::helper_panel::tick_id(key);
        ui.data_mut(|d| {
            if t.on {
                d.remove_temp::<bool>(id);
            } else {
                d.insert_temp(id, true);
            }
        });
    }
    let none = t.none && !t.on;
    let hint = |ui: &mut egui::Ui| match none {
        true => hint_label(ui, "set by you"),
        false => {
            ui.label("");
        }
    };
    let reset = egui::Sides::new()
        .shrink_left()
        .truncate()
        .show(ui, hint, |ui| {
            none && super::helper_panel::reset(
                ui,
                "Reset role",
                "Use the role guessed from the name",
                edit_off,
            )
        })
        .1;
    if reset {
        actions.push(Action::HelperRole(None));
    }
    ui.end_row();
}

/// What a row edit asks of the model, as an action.
fn push_edit(actions: &mut Vec<Action>, field: &'static str, edit: Option<RowEdit>) {
    match edit {
        Some(RowEdit::Set(v)) => actions.push(Action::Set(field, v)),
        Some(RowEdit::Clear) => actions.push(Action::Clear(field)),
        None => {}
    }
}

/// A click on a group header (`Action::SetGroupOpen`), on this panel and on the bulk panel: store the state and save it.
pub(super) fn set_group_open(model: &mut AppModel, g: PanelGroup, open: bool) {
    panel_groups::set_open(&mut model.settings, g, open);
    // Rule 4: the user's click ends a forced open of this group, so a closed group stays closed.
    if !open {
        model.panel_forced.groups.remove(&g);
    }
    model.save_settings();
}

/// Whether group `g` is drawn open: the user's saved state, or forced open (a problem or a Go to).
pub(super) fn group_open(cx: &Ctx, g: PanelGroup) -> bool {
    open_with(cx.groups, cx.forced, g)
}

/// [`group_open`] over its two inputs, for the header drawn after `Ctx` is gone (Sources).
fn open_with(
    groups: &BTreeMap<String, bool>,
    forced: &BTreeSet<PanelGroup>,
    g: PanelGroup,
) -> bool {
    panel_groups::is_open_in(groups, g) || forced.contains(&g)
}

/// The no-group set a draw of a key other than the selected item reads as `Ctx::forced`.
static NO_GROUPS: BTreeSet<PanelGroup> = BTreeSet::new();

/// The header of group `g` over its summary and marks, from the panel's cache.
pub(super) fn group_head<'a>(cx: &'a Ctx, g: PanelGroup) -> GroupHead<'a> {
    let s = cx.summaries.iter().find(|(x, _)| *x == g).map(|(_, s)| s);
    GroupHead {
        title: g.title(),
        summary: s.map_or("", |s| s.text.as_str()),
        summary_hover: s.and_then(|s| s.hover.as_deref()),
        changed: s.is_some_and(|s| s.changed),
        problem: cx
            .problem_groups
            .get(&g)
            .map(|l| super::problems::status_of(*l)),
    }
}

/// A group of the panel: its header, then `body` under it, disabled with the rows while `edit_off` is set (Note FK3:
/// the header above stays clickable, so it is drawn outside the disabled scope). A click on the header is an action.
fn grouped(
    ui: &mut egui::Ui,
    cx: &Ctx,
    g: PanelGroup,
    actions: &mut Vec<Action>,
    body: impl FnOnce(&mut egui::Ui, &mut Vec<Action>),
) {
    let open = group_open(cx, g);
    let r = collapsing_group(
        ui,
        g.id(),
        &group_head(cx, g),
        open,
        cx.forced.contains(&g),
        |ui| {
            if cx.edit_off.is_some() {
                ui.disable();
            }
            body(ui, actions);
        },
    );
    if r.toggled {
        actions.push(Action::SetGroupOpen(g, !open));
    }
}

/// One group's rows, from `views` (`panel_cache`, built once per input change). The Cubemap, Channels and Sources
/// groups are drawn by `cube_section`, `channels` and `inspector_section`.
pub fn group_section(
    ui: &mut egui::Ui,
    cx: &Ctx,
    g: PanelGroup,
    views: &[FieldView],
    actions: &mut Vec<Action>,
) {
    grouped(ui, cx, g, actions, |ui, actions| {
        egui::Grid::new((cx.key, g.id()))
            .num_columns(3)
            .show(ui, |ui| {
                // Mipmaps draws its own three rows first; any other field routed to it takes the generic loop.
                let own = [LEVELS_FIELD.0, FILTER_FIELD.0, EDGE_FIELD.0];
                if g == PanelGroup::Mipmaps {
                    mipmaps_rows(ui, cx, views, actions);
                }
                for f in views
                    .iter()
                    .filter(|f| panel_groups::panel_group(f.field) == Some(g))
                    .filter(|f| !(g == PanelGroup::Mipmaps && own.contains(&f.field)))
                    // Note HI2: the strip keeps the rows it showed, so not the colour rows Compression now holds;
                    // nor DDS header (Note JI3).
                    .filter(|f| {
                        !(cx.compact && (f.field.starts_with("color.") || f.field == "header"))
                    })
                {
                    field_row(ui, cx, f, actions);
                }
            });
        if g == PanelGroup::Compression {
            if let Some(w) = cx.warnings.iter().find(|w| w.code == Code::FmtHdrSource) {
                ui.weak(&w.message);
            }
        }
    });
}

/// The rows under the texture's name, before any group (Task 4): Texture type and the Helper rows (not in the strip),
/// then Format, in `FIRST_SCREEN` order. While the Undecided card asks the type (`show_type` off) it has its own picker,
/// so the Texture type row is left out and the Helper rows stay (the card has no Helper tick; TPC-T4-1).
fn first_screen(
    ui: &mut egui::Ui,
    cx: &Ctx,
    views: &[FieldView],
    tick: Option<&HelperTick>,
    show_type: bool,
    actions: &mut Vec<Action>,
) {
    egui::Grid::new((cx.key, "first-screen"))
        .num_columns(3)
        .show(ui, |ui| {
            // Note FK3: the rows wait.
            if cx.edit_off.is_some() {
                ui.disable();
            }
            for f in panel_groups::FIRST_SCREEN
                .iter()
                .filter_map(|name| views.iter().find(|v| v.field == *name))
            {
                if f.field != "subtype" {
                    field_row(ui, cx, f, actions);
                    continue;
                }
                if cx.compact {
                    continue;
                }
                // Note HI4: an image's Helper row under Sub-type; ticked, the role list takes Sub-type's place.
                if tick.is_some_and(|t| t.on) {
                    helper_role_row(ui, cx.key, &cx.edit_off, actions);
                } else if show_type {
                    field_row(ui, cx, f, actions);
                }
                if let Some(t) = tick {
                    helper_tick_row(ui, cx.key, t, &cx.edit_off, actions);
                }
            }
        });
}

/// How many of this texture's problems the first screen lists before "+{n} more".
const PROBLEM_ROWS: usize = 3;

/// This texture's problems on the first screen: up to `PROBLEM_ROWS` one-line rows (the full text and the code on
/// hover), then a "+{n} more" button that opens Problems on the texture's set (`set`, or unfiltered with none), then a
/// weak line for the `notes` the panel does not list.
pub fn first_screen_problems(
    ui: &mut egui::Ui,
    rows: &[Problem],
    notes: usize,
    set: Option<&SetTarget>,
    actions: &mut Vec<Action>,
) {
    for p in rows.iter().take(PROBLEM_ROWS) {
        let text = p.shown();
        let r = ui.add(
            egui::Label::new(egui::RichText::new(&*text))
                .truncate()
                .show_tooltip_when_elided(false),
        );
        r.on_hover_ui(|ui| {
            ui.label(&*text);
            if let Some(codes) = p.codes_text() {
                ui.weak(codes);
            }
        });
    }
    if let Some(more) = rows.len().checked_sub(PROBLEM_ROWS).filter(|n| *n > 0) {
        let r = ui.small_button(format!("+{more} more"));
        // Built only while AccessKit asks for it (Note LH7).
        r.widget_info(|| {
            let plural = if more == 1 { "problem" } else { "problems" };
            egui::WidgetInfo::labeled(
                egui::Role::Button,
                true,
                format!("{more} more {plural}: open Problems"),
            )
        });
        let hover = if set.is_some() {
            "Show every problem of this texture's set in Problems."
        } else {
            "Show every problem in Problems."
        };
        if r.on_hover_text(hover).clicked() {
            actions.push(Action::OpenProblems(set.cloned()));
        }
    }
    if notes > 0 {
        let plural = if notes == 1 { "note" } else { "notes" };
        ui.weak(format!("{notes} {plural} in Problems"));
    }
}

/// Everything the panel reads per frame that only an input change moves: the rows' views and the eight header
/// summaries (`panel_cache`).
struct PanelCache {
    views: Vec<FieldView>,
    summaries: Vec<(PanelGroup, GroupSummary)>,
}

/// A cache slot's inputs beside its value (rule 9.1): the drawn key and `AppModel::panel_cache_inputs`.
struct PanelSlot {
    key: String,
    inputs: crate::model::bulk::PanelInputs,
    /// The alpha proposal `draw` writes into the job it passes (display only): the builder reads it, so it is keyed.
    proposal: Option<AlphaPolicy>,
    cache: Arc<PanelCache>,
}

/// The ids of a per-surface cache slot, hashed once (`Id::unique` records a debug name for every call).
fn slot_id(cache: &'static str, surface: Surface) -> egui::Id {
    static IDS: std::sync::OnceLock<[[egui::Id; 3]; 2]> = std::sync::OnceLock::new();
    let ids = IDS.get_or_init(|| {
        ["panel-cache", "panel-problems"]
            .map(|c| [0, 1, 2].map(|s| egui::Id::unique(("texture-panel-slot", c, s))))
    });
    ids[usize::from(cache == "panel-problems")][surface.slot()]
}

/// The views and summaries of `key`'s panel, kept in one slot per `Surface` and rebuilt only when the drawn key or
/// one of `AppModel::panel_cache_inputs` moves. A hit clones one `Arc` and compares a `String` with a `&str` and six
/// plain values: no allocation.
fn panel_cache(
    ctx: &egui::Context,
    surface: Surface,
    model: &AppModel,
    key: &str,
    job: &ResolvedJob,
    proposal: Option<AlphaPolicy>,
) -> Arc<PanelCache> {
    let id = slot_id("panel-cache", surface);
    let inputs = model.panel_cache_inputs();
    if let Some(slot) = ctx.data(|d| d.get_temp::<Arc<PanelSlot>>(id)) {
        if slot.key == key && slot.inputs == inputs && slot.proposal == proposal {
            return slot.cache.clone();
        }
    }
    let views = panel_groups::panel_views(model.target, job);
    // The Channels lines are read-only text no field carries: one per packed channel, from `pack.r` .. `pack.a`.
    let lines = ["r", "g", "b", "a"].map(|ch| {
        job.provenance
            .get(&format!("pack.{ch}"))
            .map(|p| channel_text(&p.to_string()))
    });
    let channels = lines
        .iter()
        .any(Option::is_some)
        .then(|| lines.each_ref().map(|l| l.as_deref().unwrap_or("")));
    let extra = SummaryExtra { channels };
    let summaries = PanelGroup::ALL
        .into_iter()
        .map(|g| {
            (
                g,
                panel_groups::summary(g, &views, model.target, &job.profile, &extra),
            )
        })
        .collect();
    let cache = Arc::new(PanelCache { views, summaries });
    let slot = Arc::new(PanelSlot {
        key: key.to_owned(),
        inputs,
        proposal,
        cache: cache.clone(),
    });
    ctx.data_mut(|d| d.insert_temp(id, slot));
    cache
}

/// A `texture_problems` slot: what it was built from beside what it holds.
struct ProblemSlot {
    key: String,
    /// The whole list the rows were taken from; holding it keeps its address from being reused for a new list.
    source: Arc<Vec<Problem>>,
    rows: Arc<Vec<Problem>>,
    notes: usize,
    /// `panel_groups::problem_levels(rows)`, built with them: a header reads it every frame.
    groups: Arc<BTreeMap<PanelGroup, Level>>,
}

/// The codes whose first-screen widget is not the problem row: the card's low-confidence line, the green line and the
/// Compression HDR line.
const SAID_ELSEWHERE: [Code; 4] = [
    Code::SugLowConfidence,
    Code::InGreenUnstated,
    Code::ChkGreenGuess,
    Code::FmtHdrSource,
];

/// This texture's problem rows (`panel_groups::is_panel_problem`, less what another widget says), its note count and
/// the groups those rows point at with their worst level (`problem_levels`, for the header dots and the forced-open
/// fill), in the
/// surface's slot. The slot is used while its key is the drawn one and its source list is
/// `bottom::problems(model)` itself (`Arc::ptr_eq`: `problems` hands out the cached `Arc` until its key moves), so a
/// frame does not rescan the list. `alpha_asked` is read only on a rebuild: it is a function of the item, and the list
/// is rebuilt whenever an item changes (the list's key holds `items_rev`).
fn texture_problems(
    ctx: &egui::Context,
    surface: Surface,
    model: &AppModel,
    key: &str,
) -> (Arc<Vec<Problem>>, usize, Arc<BTreeMap<PanelGroup, Level>>) {
    let all = crate::model::bottom::problems(model);
    let id = slot_id("panel-problems", surface);
    if let Some(slot) = ctx.data(|d| d.get_temp::<Arc<ProblemSlot>>(id)) {
        if slot.key == key && Arc::ptr_eq(&slot.source, &all) {
            return (slot.rows.clone(), slot.notes, slot.groups.clone());
        }
    }
    let alpha_asked = model
        .item(key)
        .is_some_and(|i| crate::model::items::alpha_asked(i).is_some());
    let (rows, notes) = panel_problem_rows(&all, key, alpha_asked);
    let groups = Arc::new(panel_groups::problem_levels(&rows));
    let rows = Arc::new(rows);
    let slot = Arc::new(ProblemSlot {
        key: key.to_owned(),
        source: all,
        rows: rows.clone(),
        notes,
        groups: groups.clone(),
    });
    ctx.data_mut(|d| d.insert_temp(id, slot));
    (rows, notes, groups)
}

/// The first screen's rows of texture `key` out of the whole problem list, and its note count. Left out, besides notes
/// (`is_panel_problem`): rows another widget of the first screen already says (TPC-T4-2, as `other_warnings` left them
/// out), namely the Decide row (the Undecided card is it) and the `SAID_ELSEWHERE` codes; and, while the alpha question
/// is open (`alpha_asked`), a LOSS-ALPHA-DATA row, which the question answers (Note FM35).
pub fn panel_problem_rows(all: &[Problem], key: &str, alpha_asked: bool) -> (Vec<Problem>, usize) {
    let has = |p: &Problem, c: Code| p.code == Some(c) || p.also.contains(&c);
    let mut rows = vec![];
    let mut notes = 0;
    for p in all
        .iter()
        .filter(|p| matches!(&p.at, At::Texture(k) if k == key))
    {
        if !panel_groups::is_panel_problem(p) {
            notes += 1;
        } else if p.level != Level::Decide
            && !SAID_ELSEWHERE.iter().any(|c| has(p, *c))
            && !(alpha_asked && has(p, Code::LossAlphaData))
        {
            rows.push(p.clone());
        }
    }
    (rows, notes)
}

/// The set `+{n} more` opens Problems on: the selected item's set view (a key-less set by its id), or another drawn
/// item's `set_key`; `None` when the texture is in no set.
pub fn problems_target(model: &AppModel, key: &str) -> Option<SetTarget> {
    let active = model.resolved_selection();
    if matches!(&active.selection, crate::model::Selection::Item(k) if k == key) {
        return active.view().map(SetTarget::view);
    }
    model
        .item(key)
        .and_then(|i| i.set_key.as_deref())
        .map(SetTarget::from)
}

fn message_of(warnings: &[Warning], code: Code) -> Option<&str> {
    warnings
        .iter()
        .find(|w| w.code == code)
        .map(|w| w.message.as_str())
}

/// A small edit button that is off, with `off`'s reason on hover, while `off` is set (Note FK).
pub(super) fn waiting_button(
    ui: &mut egui::Ui,
    off: &Option<String>,
    text: &str,
) -> egui::Response {
    let r = ui.add_enabled(off.is_none(), egui::Button::new(text).small());
    match off {
        Some(why) => r.on_disabled_hover_text(why),
        None => r,
    }
}

/// The amber "Undecided" box: one control per decision, each writing an explicit answer.
fn decision_box(
    ui: &mut egui::Ui,
    cx: &Ctx,
    decisions: &[Decision],
    warnings: &[Warning],
    alpha: Option<AlphaPolicy>,
    likely: &[(Subtype, String)],
    actions: &mut Vec<Action>,
) {
    // Note HI13: where the groups' grids start (a group's body is not indented, as the first screen's rows are not),
    // so the box's rows line up with theirs.
    let left = (!cx.compact).then(|| ui.max_rect().left());
    let frame = egui::Frame::new()
        .stroke(egui::Stroke::new(2.0, AMBER))
        .corner_radius(4.0)
        .inner_margin(8.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.colored_label(AMBER, egui::RichText::new("Undecided").strong());
            // Note FK2: the answers wait for the plan; the reason is the box's hover, so nothing moves.
            if cx.edit_off.is_some() {
                ui.disable();
            }
            for d in decisions {
                match d {
                    Decision::Subtype { suggested } => {
                        subtype_decision(ui, cx, left, *suggested, likely, actions)
                    }
                    // Note HI7, HI13: the dropdown on the resolved type and Confirm, in the Alpha row's columns (the
                    // strip keeps FM35's line); the engine's reason is the label's hover.
                    Decision::AlphaMeaning => {
                        if let Some(shown) = alpha {
                            let id = egui::Id::unique(("alpha-decision", cx.key));
                            let mut picked = None;
                            let label = match left {
                                Some(left) => decision_row(ui, left, "Alpha", |ui, _| {
                                    picked = alpha_question(ui, id, shown, "", "");
                                }),
                                None => {
                                    let line = ui.label(alpha_confirm_text(shown));
                                    ui.horizontal(|ui| {
                                        picked = alpha_question(ui, id, shown, "", "");
                                    });
                                    line
                                }
                            };
                            if let Some(m) = message_of(warnings, Code::SugLowConfidence) {
                                label.on_hover_text(m);
                            }
                            if let Some(p) = picked {
                                actions.push(Action::Decide(Answer::Alpha(p)));
                            }
                        }
                    }
                    // Note GU2: no layout answer is left; the sub-type picker above is the way out.
                    Decision::NotACubeSource => {
                        ui.label(not_a_cube(Some(cx.job)));
                    }
                    Decision::Acknowledge(code) => {
                        // Note FB8: the sentence, with the code on its hover.
                        ui.label(egui::RichText::new(code_line(*code)).strong())
                            .on_hover_text(code.as_str());
                        if let Some(m) = message_of(warnings, *code) {
                            ui.label(m);
                        }
                        if ui
                            .button(crate::model::code_text::ack_label_for_texture(*code))
                            .on_hover_text(code_line(*code))
                            .clicked()
                        {
                            actions.push(Action::Decide(Answer::Acknowledge(*code)));
                        }
                    }
                    // Note JF3: a block Acknowledge cannot clear: the sentence, the engine's message and, for a type
                    // the mode does not have, the type picker. Any other such code names its fix in the sentence.
                    Decision::Change(code) => {
                        ui.label(egui::RichText::new(change_line(*code)).strong())
                            .on_hover_text(code.as_str());
                        if let Some(m) = message_of(warnings, *code) {
                            ui.label(m);
                        }
                        if *code == Code::CfgUnavailable {
                            // No likely-type buttons: this type is decided but blocked, not undecided (3d-T11-2).
                            subtype_decision(ui, cx, left, None, &[], actions);
                        }
                    }
                }
                ui.add_space(4.0);
            }
        });
    if let Some(why) = &cx.edit_off {
        // Over every control of the box: a disabled widget shows no hover of its own here.
        ui.interact(
            frame.response.rect,
            ui.scope_id().with(("decision-why", cx.key)),
            egui::Sense::hover(),
        )
        .on_hover_text(why);
    }
    ui.add_space(6.0);
}

/// Note HI13: a row of the Undecided box in the groups' two columns: `label` in the label column (`left` is the groups'
/// left edge, so the box's frame inset cancels out), then the controls; `add` gets the control column's width, which
/// is also the row's `combo_width`. Where the box is too narrow for that width and a Confirm button (1100 px), the
/// dropdown gives up the difference, so Confirm is never cut short (rule 3.1). Returns the label.
fn decision_row(
    ui: &mut egui::Ui,
    left: f32,
    label: &str,
    add: impl FnOnce(&mut egui::Ui, f32),
) -> egui::Response {
    ui.horizontal(|ui| {
        let (lw, cw) = column_widths_at(ui, left, 0.0);
        let s = ui.spacing();
        let confirm = egui::WidgetText::from("Confirm")
            .into_galley(ui, None, f32::INFINITY, egui::TextStyle::Button)
            .size()
            .x
            + 2.0 * s.button_padding.x;
        let room = ui.max_rect().right() - (left + lw) - 2.0 * s.item_spacing.x - confirm;
        let cw = cw.min(room).max(s.interact_size.x);
        let w = (left + lw - ui.cursor().min.x).max(0.0);
        let l = control_slot(ui, w, |ui| ui.add(egui::Label::new(label).truncate()));
        ui.spacing_mut().combo_width = cw;
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
        add(ui, cw);
        l
    })
    .inner
}

/// The likely types of one texture, built again only when the items, the document or the target change (the pairing
/// walks the `[files]` keys, so it is not run every frame).
fn cached_likely(ui: &egui::Ui, model: &AppModel, key: &str) -> Vec<(Subtype, String)> {
    let id = ui.scope_id().with(("likely-types", key));
    let stamp = (
        model.items_rev(),
        model.project.as_ref().map_or(0, |p| p.rev()),
        model.target,
    );
    if let Some((s, v)) =
        ui.data(|d| d.get_temp::<((u64, u64, Target), Vec<(Subtype, String)>)>(id))
    {
        if s == stamp {
            return v;
        }
    }
    let v = crate::model::likely::likely_types(model, &[key.to_string()]);
    ui.data_mut(|d| d.insert_temp(id, (stamp, v.clone())));
    v
}

/// Task 11: the likely types of an undecided card as buttons (the label, the reason on hover, "Use {label}" as the
/// name). Returns the one clicked; the caller acts on it (the single card writes it and moves on, the bulk card puts
/// it in its combo).
pub(super) fn likely_buttons(ui: &mut egui::Ui, likely: &[(Subtype, String)]) -> Option<Subtype> {
    if likely.is_empty() {
        return None;
    }
    let mut picked = None;
    ui.horizontal_wrapped(|ui| {
        for (st, why) in likely {
            let label = subtype_label(*st);
            let r = ui.button(&label).on_hover_text(why);
            super::helper_panel::named(&r, egui::Role::Button, &format!("Use {label}"), None);
            if r.clicked() {
                picked = Some(*st);
            }
        }
    });
    picked
}

/// A combo of the sub-types available under the target, the suggestion preselected, and a Confirm button. Aligned,
/// they stand in the control column under the sentence (Note HI13). `likely` are the buttons above the combo; a click
/// on one answers as Confirm does (ruling 3d-T12-1).
fn subtype_decision(
    ui: &mut egui::Ui,
    cx: &Ctx,
    left: Option<f32>,
    suggested: Option<Subtype>,
    likely: &[(Subtype, String)],
    actions: &mut Vec<Action>,
) {
    ui.label("What kind of texture is this?");
    let id = egui::Id::unique(("confirm-subtype", cx.key));
    let mut chosen: Subtype = ui
        .data(|d| d.get_temp(id))
        .unwrap_or_else(|| suggested.unwrap_or(cx.job.profile.subtype));
    if let Some(st) = likely_buttons(ui, likely) {
        chosen = st;
        actions.push(Action::DecideAndAdvance(Answer::Subtype(st)));
    }
    let mut controls = |ui: &mut egui::Ui| {
        let r = egui::ComboBox::from_id_salt(id)
            .selected_text(subtype_label(chosen))
            .show_ui_036(ui, |ui| {
                let grass = engine::policy::grass_context(cx.job);
                for (heading, members) in offered_groups(cx.target, grass) {
                    ui.label(egui::RichText::new(heading).weak());
                    for st in members {
                        ui.selectable_value(&mut chosen, st, subtype_label(st))
                            .on_hover_text(type_hover(st));
                    }
                }
            })
            .response;
        super::helper_panel::named(
            &r,
            egui::Role::ComboBox,
            "Type for this texture",
            Some(&subtype_label(chosen)),
        );
    };
    let confirm = match left {
        Some(left) => {
            let mut ok = false;
            decision_row(ui, left, "", |ui, w| {
                control_slot(ui, w, &mut controls);
                ok = ui.button("Confirm").clicked();
            });
            ok
        }
        None => {
            ui.horizontal(|ui| {
                controls(ui);
                ui.button("Confirm").clicked()
            })
            .inner
        }
    };
    if confirm {
        actions.push(Action::DecideAndAdvance(Answer::Subtype(chosen)));
    }
    ui.data_mut(|d| d.insert_temp(id, chosen));
}

/// One line per packed channel, from the job's `pack.r` … `pack.a` provenance, under the Channels header.
fn channels(ui: &mut egui::Ui, job: &ResolvedJob) {
    for (ch, name) in [("r", "R"), ("g", "G"), ("b", "B"), ("a", "A")] {
        if let Some(p) = job.provenance.get(&format!("pack.{ch}")) {
            ui.label(format!("{name}: {}", channel_text(&p.to_string())));
        }
    }
}

/// A failed texture's line (Note FB8: the sentences, not the codes; the codes are the label's hover).
fn error_text(codes: &[Code]) -> String {
    format!(
        "Error: {}",
        codes
            .iter()
            .map(|c| code_line(*c))
            .collect::<Vec<_>>()
            .join("; ")
    )
}

fn error_codes(codes: &[Code]) -> String {
    codes
        .iter()
        .map(|c| c.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Why `sdc.toml` cannot take an edit, when its file is read-only.
/// Item 9: the meshes that use this texture, winner first, from the scan database (read off the UI thread).
fn used_by(ui: &mut egui::Ui, model: &mut AppModel, key: &str, actions: &mut Vec<Action>) {
    let Some(u) = model.used_by_for(key) else {
        return;
    };
    // A read still outstanding (or a scan since the answer) leaves the answer on screen for context, but nothing
    // that acts on it: its copy ids may not exist any more, in this store or in the one Meshes reads.
    // While a scan is unsettled the store is being rewritten and no read runs: the links say so, not "reading".
    let unsettled = model.scan_store_unsettled;
    let reading = !unsettled
        && (model.used_by.pending.is_some() || model.used_by.stale || model.meshes_rows_loading);
    // `MeshesModel::store_path` and `AppModel::scan_db` are the one value (`SdcApp::new` copies it, nothing sets
    // either again), so a plain comparison cannot differ by spelling.
    let other_store = u
        .as_ref()
        .is_some_and(|u| model.meshes_store.as_ref().is_some_and(|s| *s != u.store));
    let why_off = if unsettled {
        Some(USED_BY_SCAN_WHY)
    } else if reading {
        Some(USED_BY_READING_WHY)
    } else if other_store {
        Some(USED_BY_OTHER_STORE_WHY)
    } else {
        None
    };
    let title = match &u {
        Some(u) => crate::model::used_by::header_line(u.total_copies),
        None => "Used by meshes".to_string(),
    };
    // The list's open state is not a setting: egui temp data, closed at start (planner's call, plan Task 4).
    let open_id = used_by_open_id();
    let open = ui.data(|d| d.get_temp::<bool>(open_id)).unwrap_or(false);
    let head = GroupHead {
        title: &title,
        summary: "",
        summary_hover: None,
        changed: false,
        problem: None,
    };
    let r = collapsing_group(ui, "used-by-meshes", &head, open, false, |ui| {
        let Some(u) = u else {
            ui.weak("Reading the scan database…");
            return;
        };
        if let Some(e) = &u.error {
            ui.colored_label(RED, e);
        }
        if u.rows.is_empty() && u.error.is_none() {
            ui.weak(format!("No mesh of the latest scan uses {}.", u.tex));
        }
        for r in &u.rows {
            used_by_group(ui, r, &u.store, why_off, actions);
        }
        if u.more > 0 {
            ui.weak(crate::model::used_by::more_line(u.more));
        }
        // The row facts are per copy above; this is the texture-wide filter.
        if reading {
            let link = ui.add_enabled(false, egui::Link::new("Reading meshes..."));
            link.on_disabled_hover_text(USED_BY_READING_WHY);
        } else if !u.rows.is_empty() {
            let link = ui.add_enabled(why_off.is_none(), egui::Link::new("Show all in Meshes"));
            match why_off {
                Some(why) => {
                    link.on_disabled_hover_text(why);
                }
                None => {
                    if link.clicked() {
                        // Note GC: app.rs switches the view
                        actions.push(Action::ShowAllInMeshes(u.tex.clone()));
                    }
                }
            }
        }
    });
    if r.toggled {
        ui.data_mut(|d| d.insert_temp(open_id, !open));
    }
}

/// Where the Used by header keeps its open flag.
fn used_by_open_id() -> egui::Id {
    static ID: std::sync::OnceLock<egui::Id> = std::sync::OnceLock::new();
    *ID.get_or_init(|| egui::Id::unique("used-by-meshes"))
}

/// Why they are off while a scan runs, or after one that did not finish.
const USED_BY_SCAN_WHY: &str =
    "A scan is running or did not finish, so this list may change. These links work once a scan has finished.";
/// Why the Used-by links are off while the list is being read.
const USED_BY_READING_WHY: &str =
    "The mesh list is still being read. These links work once it has finished.";
/// Why they are off when Meshes reads another scan database than this list.
const USED_BY_OTHER_STORE_WHY: &str =
    "Meshes shows another scan database than this list was read from.";

/// One NIF in one mod name: its path (cut in the middle when it does not fit, the whole path on hover and in the
/// accessible name), then each copy with its own facts and its own shape links. A group of several copies shows no
/// row-level priority or rank: its copies come from different origins.
fn used_by_group(
    ui: &mut egui::Ui,
    r: &crate::model::used_by::MeshUser,
    store: &std::path::Path,
    why_off: Option<&str>,
    actions: &mut Vec<Action>,
) {
    use crate::model::used_by::{user_line, DECIDED_ALPHA, DECIDED_ALPHA_HOVER};
    let glyph = ui.fonts_mut(|f| f.glyph_width(&egui::TextStyle::Body.resolve(ui.style()), '0'));
    let chars = (ui.available_width() / glyph).floor().max(12.0) as usize;
    let cut = super::header::elide_middle(&r.nif, chars);
    let elided = cut != r.nif;
    let label = ui.add(
        egui::Label::new(&cut)
            .truncate()
            .show_tooltip_when_elided(!elided),
    );
    // The whole path is the name whether or not our cut applied: egui may still end-truncate a path it did not cut.
    label.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, &r.nif));
    if elided {
        label.on_hover_text(&r.nif);
    }
    for c in &r.copies {
        ui.horizontal_wrapped(|ui| {
            ui.weak(user_line(&r.mod_name, c.priority, c.bsa));
            if let Some(by) = &c.overridden_by {
                // The same mod name in another root reads as nonsense bare: name the root as well.
                match c.overridden_root.as_ref().filter(|_| *by == r.mod_name) {
                    Some(root) => {
                        let cut = super::header::elide_middle(root, 40);
                        let shown = format!("(overridden by {by} at {cut})");
                        let full = format!("(overridden by {by} at {root})");
                        let l = ui.weak(&shown);
                        if shown != full {
                            l.widget_info(|| {
                                egui::WidgetInfo::labeled(egui::Role::Label, true, &full)
                            });
                            l.on_hover_text(&full);
                        }
                    }
                    None => {
                        ui.weak(format!("(overridden by {by})"));
                    }
                }
            }
            if c.decided_alpha {
                ui.colored_label(AMBER, DECIDED_ALPHA)
                    .on_hover_text(DECIDED_ALPHA_HOVER);
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.add_space(12.0);
            for (block, name) in &c.shapes {
                let shown = name.clone().unwrap_or_else(|| format!("Shape {block}"));
                let accessible = format!(
                    "Open {} at {shown} (copy {}, block {block})",
                    r.nif, c.copy_id
                );
                let on = why_off.is_none();
                let link = ui.add_enabled(on, egui::Link::new(&shown));
                link.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Link, on, &accessible));
                match why_off {
                    Some(why) => {
                        link.on_disabled_hover_text(why);
                    }
                    None => {
                        let clicked = link.clicked();
                        link.on_hover_text(format!(
                            "Open this mesh in Meshes with {shown} selected."
                        ));
                        if clicked {
                            actions.push(Action::ShowMesh(crate::model::used_by::MeshPick {
                                store: store.to_path_buf(),
                                copy_id: c.copy_id,
                                block: *block,
                                shape: name.clone(),
                            }));
                        }
                    }
                }
            }
        });
    }
}

/// Draw texture `key`'s settings on `surface`. The preview strip (`Surface::Strip`) keeps the decision box, the Format
/// row and the Alpha, Mipmaps and Compression groups, without provenance text.
pub fn draw(ui: &mut egui::Ui, model: &mut AppModel, key: &str, surface: Surface) {
    let compact = surface == Surface::Strip;
    // `panel_forced` rule 1: a new selection starts a new latch. Only the model's selection resets it, whichever key
    // this call draws (the centre and the strip draw two items in one frame). `draw_with_scan` observes it too, every
    // frame; this call covers a caller that draws the panel alone.
    model.observe_panel_selection();
    // The latch holds groups for this draw only when it is the selected item's (read once: every header uses it).
    let forced_here = matches!(&model.selection, Selection::Item(k) if k == key)
        && model.panel_forced.item == key;
    let Some(item) = model.item(key) else {
        ui.weak("This texture is no longer in the plan.");
        return;
    };
    if let Some(h) = item.helper.clone() {
        return super::helper_panel::draw(ui, model, key, &h);
    }
    let (name, job_key, kind, status) = (
        item.name.clone(),
        item.job_key.clone(),
        item.kind,
        item.status.clone(),
    );
    // The texture's own list shows its check results, Info included (preflight K5, X3).
    let (decisions, warnings, mut job) = (
        item.decisions.clone(),
        model.item_messages(key),
        item.job.clone(),
    );
    let alpha = crate::model::items::alpha_asked(item);
    // Note HI14: while the alpha is asked, every row reads as Confirm would leave it. This clone is display only.
    if let (Some(j), Some(p)) = (job.as_mut(), alpha) {
        j.profile.alpha.policy = p;
    }
    let packed_output = match &status {
        ItemStatus::PackedInto(j) => model
            .items
            .iter()
            .find(|i| i.kind == ItemKind::Packed && &i.job_key == j)
            .map(|i| i.key.clone()),
        _ => None,
    };
    let (target, busy) = (model.target, model.edit_busy());
    let read_only = model.project.as_ref().is_some_and(|p| p.read_only());
    // Note FK: the panel shows the last plan; until the edit's partial plan lands (Note FL: or its full plan, when it
    // has no partial) every edit control is off, as Problems' fix buttons are (CA12), so a click is not lost to a
    // stale view. A read-only `sdc.toml` gives its own reason first.
    let edit_off = model.fix_off();
    // Note FI3: why the re-encode button is off (read-only first), and whether it would write over its source. Only a
    // skipped DDS draws that button, so nothing else pays for the scan or the file-system check each frame.
    let skipped_dds = kind == ItemKind::Dds && job.is_none();
    let reencode_off = edit_off
        .clone()
        .or_else(|| skipped_dds.then(|| model.reencode_blocker(key)).flatten());
    let into_source = skipped_dds
        && engine::dds::same_file(
            &model.item(key).map(|i| i.input.clone()).unwrap_or_default(),
            &model.sets_root().join(&job_key),
        );
    let reencoding = model.reencode_on(key);
    let mut actions = vec![];
    if !compact {
        let active = model.resolved_selection();
        let set_key = model.item(key).and_then(|i| i.set_key.as_deref());
        let owner = if matches!(&active.selection, crate::model::Selection::Item(k) if k == key) {
            active
                .view()
                .map(|v| v.key.as_str())
                .filter(|k| Some(*k) == set_key)
        } else {
            set_key.filter(|k| crate::model::sets::has_card(model, k))
        };
        if let Some(sk) = owner {
            let label = super::columns::set_label(&super::columns::export_prefix(model), sk);
            if ui.link(format!("← {label}")).clicked() {
                let key = sk.to_owned();
                model.select_set(&key);
            }
        }
        ui.heading(&name);
        ui.weak(if job_key.is_empty() { key } else { &job_key });
        // Note FI3: a DDS re-encoded by `sdc.toml` (it has a job, or failed at plan) offers the way back.
        if reencoding {
            let r = ui
                .add_enabled(edit_off.is_none(), egui::Button::new("Stop re-encoding"))
                .on_hover_text(
                    "Leave the DDS file as it is again: it is not written to the export.",
                );
            let r = match &edit_off {
                Some(why) => r.on_disabled_hover_text(why),
                None => r,
            };
            if r.clicked() {
                actions.push(Action::Reencode(false));
            }
        }
        // Review E20: under the heading, above the type row, so it is on the first screen.
        used_by(ui, model, key, &mut actions);
        ui.add_space(4.0);
    }
    // Note HI4 (DY17): any image can be made a helper from its Helper tick, one with no helper suffix too (HI15: one in
    // no set as well). A file the user unticked (`helper.role = "none"`) reads unticked, "set by you".
    let tick = (kind == ItemKind::Image && !compact).then(|| HelperTick {
        on: ui
            .data(|d| d.get_temp::<bool>(super::helper_panel::tick_id(key)))
            .unwrap_or(false),
        none: model
            .project
            .as_ref()
            .and_then(|p| p.file_value(key, engine::helper::ROLE_FIELD))
            .is_some_and(|v| v == json!("none")),
    });
    let mut cube_edits: super::cube_section::Edits = vec![];
    // Note FK3: with a job, each edit control waits on `edit_off` itself, so the headers stay usable read-only.
    ui.add_enabled_ui(!read_only || job.is_some(), |ui| match &job {
        None => {
            match &status {
                ItemStatus::Skipped(r) => ui.label(format!("Skipped: {r}")),
                ItemStatus::Failed(e) => ui.label(format!("Failed: {e}")),
                ItemStatus::PackedInto(j) => ui.label(format!("This map is packed into {j}.")),
                ItemStatus::Error(c) => ui.label(error_text(c)).on_hover_text(error_codes(c)),
                ItemStatus::NeedsDecision | ItemStatus::Ready => ui.label(""),
            };
            if kind == ItemKind::Dds {
                ui.label(
                    "Already a DDS file: it is left as it is and not written to the export.",
                );
                // Note FI3: the S6 `reencode` selector from a button, not a hand edit.
                let mut hover = String::from("Decode it and encode it again with this project's settings. Block-compressed files lose some quality.");
                if into_source {
                    hover.push_str(" It cannot be written into the folder it comes from.");
                }
                let r = ui
                    .add_enabled(reencode_off.is_none(), egui::Button::new("Re-encode this file"))
                    .on_hover_text(hover);
                let r = match &reencode_off {
                    Some(why) => r.on_disabled_hover_text(why),
                    None => r,
                };
                if r.clicked() {
                    actions.push(Action::Reencode(true));
                }
            }
            if let Some(out) = packed_output {
                if ui.button("Show packed output").clicked() {
                    actions.push(Action::Select(out));
                }
            }
            // Note HI4: no Type group, so the Helper row stands alone and stays reachable (with the roles once ticked).
            if let Some(t) = &tick {
                egui::Grid::new(("helper-jobless", key))
                    .num_columns(3)
                    .show(ui, |ui| {
                        if edit_off.is_some() {
                            ui.disable();
                        }
                        if t.on {
                            helper_role_row(ui, key, &edit_off, &mut actions);
                        }
                        helper_tick_row(ui, key, t, &edit_off, &mut actions);
                    });
            }
        }
        Some(job) => {
            // Once per input change, not once per frame: every row, header and summary below reads this.
            let pc = panel_cache(ui.ctx(), surface, model, key, job, alpha);
            // `size_of` needs `&mut model`, which `cx` below keeps shared: ask now, draw the line in its place.
            let size = (!compact).then(|| model.size_of(key));
            let (rows, notes, problem_groups) = texture_problems(ui.ctx(), surface, model, key);
            // Rule 2: the selected item's first draw with a job adds its problems' groups, once.
            if forced_here && !model.panel_forced.filled {
                model
                    .panel_forced
                    .groups
                    .extend(problem_groups.keys().copied());
                model.panel_forced.filled = true;
            }
            let forced = if forced_here {
                &model.panel_forced.groups
            } else {
                &NO_GROUPS
            };
            let cx = Ctx {
                key,
                job,
                target,
                busy,
                compact,
                alpha_asked: alpha.is_some(),
                edit_off: edit_off.clone(),
                warnings: &warnings,
                surface,
                groups: &model.settings.texture_groups,
                forced,
                problem_groups: &problem_groups,
                summaries: &pc.summaries,
                reveal: (surface == Surface::Centre)
                    .then_some(model.nav.reveal_field.as_ref())
                    .flatten()
                    .filter(|q| q.item == key),
            };
            match &status {
                ItemStatus::Skipped(r) => {
                    ui.label(format!("Skipped: {r}"));
                }
                ItemStatus::Error(c) => {
                    ui.colored_label(RED, error_text(c))
                        .on_hover_text(error_codes(c));
                }
                _ => {}
            }
            let guess = message_of(&warnings, Code::ChkGreenGuess);
            if let Some(m) = message_of(&warnings, Code::InGreenUnstated) {
                // Note AW: an Info line, not a question; the OpenGL answer stays one click away.
                let hint = if guess.is_some() {
                    " Looks OpenGL (curl test)."
                } else {
                    ""
                };
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new(format!("{m}{hint}")).small())
                        .on_hover_ui(|ui| code_hover(ui, m, Code::InGreenUnstated.as_str()));
                    if waiting_button(ui, &edit_off, GREEN_TO_OPENGL).clicked() {
                        actions.push(Action::Decide(Answer::Green(GreenConvention::Opengl)));
                    }
                });
            } else if let Some(m) = guess {
                // No AW question was asked: the check's own line, same button, never an auto-flip.
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new(m).small())
                        .on_hover_ui(|ui| code_hover(ui, m, Code::ChkGreenGuess.as_str()));
                    if waiting_button(ui, &edit_off, GREEN_TO_OPENGL).clicked() {
                        actions.push(Action::Decide(Answer::Green(GreenConvention::Opengl)));
                    }
                });
            } else if job.profile.channels.green_convention == Some(GreenConvention::Opengl) {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Green: OpenGL (flipped)");
                    if matches!(
                        job.provenance.get("channels.green_convention"),
                        Some(Provenance::ProjectFile | Provenance::User)
                    ) && waiting_button(ui, &edit_off, GREEN_TO_DIRECTX).clicked()
                    {
                        actions.push(Action::Clear("channels.green_convention"));
                    }
                });
            }
            if !decisions.is_empty() {
                let likely = if decisions
                    .iter()
                    .any(|d| matches!(d, Decision::Subtype { .. }))
                {
                    cached_likely(ui, model, key)
                } else {
                    vec![]
                };
                decision_box(ui, &cx, &decisions, &warnings, alpha, &likely, &mut actions);
            }
            let asks_type = decisions
                .iter()
                .any(|d| matches!(d, Decision::Subtype { .. }));
            first_screen(ui, &cx, &pc.views, tick.as_ref(), !asks_type, &mut actions);
            if let Some(size) = size {
                size_line(ui, size);
                let set = (rows.len() > PROBLEM_ROWS)
                    .then(|| problems_target(model, key))
                    .flatten();
                first_screen_problems(ui, &rows, notes, set.as_ref(), &mut actions);
            }
            // Panel order is `PanelGroup::ALL`; Sources is drawn last, after this closure (a jobless texture has it too).
            for g in PanelGroup::ALL {
                match g {
                    PanelGroup::Sources => {}
                    PanelGroup::Cubemap if !compact => {
                        // Draws nothing unless the item is a cube map (`cube_section`'s own check).
                        let open = group_open(&cx, g);
                        let (edits, toggled, taken) = super::cube_section::cube_section(
                            ui,
                            model,
                            key,
                            &group_head(&cx, g),
                            open,
                            cx.forced.contains(&g),
                            cx.reveal,
                        );
                        if toggled {
                            actions.push(Action::SetGroupOpen(g, !open));
                        }
                        if taken {
                            actions.push(Action::RevealTaken);
                        }
                        cube_edits = edits;
                    }
                    PanelGroup::Channels if !compact && kind == ItemKind::Packed => {
                        grouped(ui, &cx, g, &mut actions, |ui, _| channels(ui, job));
                    }
                    PanelGroup::Cubemap | PanelGroup::Channels => {}
                    g if !compact || STRIP_GROUPS.contains(&g) => {
                        group_section(ui, &cx, g, &pc.views, &mut actions);
                    }
                    _ => {}
                }
            }
        }
    });
    for (k, f, v) in cube_edits {
        match v {
            Some(v) => model.set_field(&k, &f, v),
            None => model.clear_field(&k, &f),
        }
    }
    if !compact {
        // Note JI3: the preset inspector, folded by default, last.
        let g = PanelGroup::Sources;
        // `Ctx` is gone here, so the header reads the same two inputs through `open_with`, the rule `group_open` is.
        let forced = if forced_here {
            &model.panel_forced.groups
        } else {
            &NO_GROUPS
        };
        let open = open_with(&model.settings.texture_groups, forced, g);
        let instant = forced.contains(&g);
        let head = GroupHead {
            title: g.title(),
            summary: panel_groups::SOURCES_SUMMARY,
            summary_hover: None,
            changed: false,
            problem: None,
        };
        if super::inspector_section::section(ui, model, key, &head, open, instant) {
            actions.push(Action::SetGroupOpen(g, !open));
        }
    }
    for a in actions {
        match a {
            Action::Set("format", v) => {
                if let Ok((f, t)) = serde_json::from_value::<(Format, ColorTag)>(v) {
                    model.set_format(key, f, t);
                }
            }
            Action::Set(f, v) => model.set_field(key, f, v),
            Action::Clear(f) => model.clear_field(key, f),
            Action::Decide(a) => model.decide(key, a),
            Action::DecideAndAdvance(a) => model.decide_and_advance(key, a),
            Action::Select(k) => model.select_item(&k),
            Action::ShowMesh(p) => model.show_mesh = Some(p),
            Action::ShowAllInMeshes(t) => model.show_in_meshes = Some(t),
            Action::Reencode(on) => model.set_reencode(&[key.to_string()], on),
            Action::HelperRole(r) => {
                ui.data_mut(|d| d.remove_temp::<bool>(super::helper_panel::tick_id(key)));
                model.set_helper_role(key, r);
            }
            Action::SetGroupOpen(g, open) => set_group_open(model, g, open),
            Action::OpenProblems(Some(target)) => model.open_set_problems(target),
            Action::OpenProblems(None) => model.open_problems(None),
            Action::RevealTaken => model.nav.reveal_field = None,
        }
    }
}

/// Item 22 (DECISIONS AG118): the DDS this texture writes: format, size after any resize, mips, bytes. A compressed
/// size is also its video memory.
fn size_line(ui: &mut egui::Ui, size: Option<crate::model::out_size::SizeEst>) {
    use crate::model::{fields::format_name, out_size::human};
    match size {
        Some(e) => {
            ui.label(format!(
                "Output: {} · {}×{} · {} mips · {} ≈ VRAM",
                format_name(e.format),
                e.width,
                e.height,
                e.mips,
                human(e.chain_bytes)
            ))
            .on_hover_text(format!(
                "{} bytes of mip data + {} bytes of header = {} bytes on disk",
                e.chain_bytes,
                e.header_bytes,
                e.file_bytes()
            ));
        }
        None => {
            ui.weak("Output size unknown: the source size could not be read.");
        }
    }
}
