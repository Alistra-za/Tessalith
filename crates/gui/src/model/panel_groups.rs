//! The texture panel's collapsible groups, which group each field sits in, and the open state kept in `gui.toml`
//! (texture-panel-compact plan, Task 1). No egui types.
use super::bottom::{Level, Problem};
use super::bulk::BulkField;
use super::code_text::{row_class, RowClass};
use super::fields::{
    cube_views, edge_view, exr_straight_view, field_hidden, field_views, filter_view, levels_view,
    mips_on, no_effect_reason, FieldView, BC3_SEARCH, CHANNEL_GUARD, CUBE_FIELDS, EDGE_FIELD,
    EFFORT_FIELDS, EXR_STRAIGHT_FIELD, FIELDS, FILTER_FIELD, GUARD_FIELDS, LEVELS_FIELD,
    MSN_SOURCE, SEARCH_FIELDS, WEIGHTING,
};
use crate::settings::GuiSettings;
use crate::ui::settings_panel::option_label;
use engine::config::{Profile, Subtype, Target};
use engine::policy::ResolvedJob;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PanelGroup {
    Alpha,
    Mipmaps,
    NormalMap,
    Size,
    Compression,
    Cubemap,
    Channels,
    Sources,
}

impl PanelGroup {
    /// In panel order.
    pub const ALL: [PanelGroup; 8] = [
        PanelGroup::Alpha,
        PanelGroup::Mipmaps,
        PanelGroup::NormalMap,
        PanelGroup::Size,
        PanelGroup::Compression,
        PanelGroup::Cubemap,
        PanelGroup::Channels,
        PanelGroup::Sources,
    ];

    /// The key in `[texture_groups]` and the salt of the header's id.
    pub fn id(self) -> &'static str {
        match self {
            PanelGroup::Alpha => "alpha",
            PanelGroup::Mipmaps => "mipmaps",
            PanelGroup::NormalMap => "normal-map",
            PanelGroup::Size => "size",
            PanelGroup::Compression => "compression",
            PanelGroup::Cubemap => "cubemap",
            PanelGroup::Channels => "channels",
            PanelGroup::Sources => "sources",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            PanelGroup::Alpha => "Alpha",
            PanelGroup::Mipmaps => "Mipmaps",
            PanelGroup::NormalMap => "Normal map",
            PanelGroup::Size => "Size",
            PanelGroup::Compression => "Compression",
            PanelGroup::Cubemap => "Cubemap",
            PanelGroup::Channels => "Channels",
            PanelGroup::Sources => "Where these settings come from",
        }
    }
}

/// Whether a group is open when `gui.toml` says nothing about it.
pub fn default_open(_g: PanelGroup) -> bool {
    false
}

/// The rows that stay on the first screen, outside any group.
pub const FIRST_SCREEN: [&str; 2] = ["subtype", "format"];

/// The group a panel field sits in; `None` for a first-screen row or a key no panel lists.
pub fn panel_group(field: &str) -> Option<PanelGroup> {
    let listed = |list: &[(&str, &str)]| list.iter().any(|(k, _)| *k == field);
    if FIRST_SCREEN.contains(&field) {
        return None;
    }
    if listed(&CUBE_FIELDS) {
        return Some(PanelGroup::Cubemap);
    }
    if listed(&[LEVELS_FIELD, FILTER_FIELD, EDGE_FIELD]) {
        return Some(PanelGroup::Mipmaps);
    }
    if listed(&[EXR_STRAIGHT_FIELD]) {
        return Some(PanelGroup::Alpha);
    }
    // The Effort row's search fields and the BC3 Effort row are Compression rows no `FIELDS` entry lists.
    // The Channel guard row's four fields, the guard's A (`alpha.exact_opaque`) included, sit in its row in
    // Compression (Note LR, LR8); no `FIELDS` entry lists them, the row itself is `CHANNEL_GUARD` below.
    if listed(&SEARCH_FIELDS) || listed(&GUARD_FIELDS) || field == BC3_SEARCH {
        return Some(PanelGroup::Compression);
    }
    // Only a key `FIELDS` lists is placed, so a field added there without a line here has no group and the table
    // test in `tests/texture_groups.rs` fails. Channels holds read-only lines, never a field.
    if !listed(&FIELDS) {
        return None;
    }
    match field {
        MSN_SOURCE | "mips.normal" | "mips.normal_base" => Some(PanelGroup::NormalMap),
        "alpha.policy" | "alpha.threshold" | "alpha.compare" | "mips.coverage"
        | "encoder.bc1_alpha" | "alpha.parallax" => Some(PanelGroup::Alpha),
        "resize" | "resize_relative_to" | "resize_floor" => Some(PanelGroup::Size),
        "color.tag" | "color.content" | "encoder.effort" | WEIGHTING | CHANNEL_GUARD
        | "encoder.perceptual" | "header" => Some(PanelGroup::Compression),
        _ => None,
    }
}

/// One group's header text: the summary, an optional hover, and the two marks. Built once per input change, never per
/// frame (Task 4 caches it).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupSummary {
    pub text: String,
    /// Why the group is not used, when every shown row is locked.
    pub hover: Option<String>,
    /// A row of the group holds a value this texture's own entry set.
    pub changed: bool,
    /// Every shown row is locked (`FieldView.lock`, never `edit_off`).
    pub all_locked: bool,
}

/// What `summary` reads besides the views: the Channels lines are read-only text no field carries.
pub struct SummaryExtra<'a> {
    /// The R, G, B and A sources of a packed texture.
    pub channels: Option<[&'a str; 4]>,
}

/// Every row the panel's groups show for `job`: `field_views`, the Levels, Filter and Edge views, the EXR Straight
/// alpha view, and `cube_views` on a cube map, without the rows `fields::hidden_reason` leaves out.
pub fn panel_views(target: Target, job: &ResolvedJob) -> Vec<FieldView> {
    let mut views = field_views(target, job);
    views.push(levels_view(target, job));
    views.push(filter_view(target, job));
    views.push(edge_view(target, job));
    views.extend(exr_straight_view(target, job));
    if job.profile.subtype == Subtype::Cubemap {
        views.extend(cube_views(target, job));
    }
    views.retain(|v| !field_hidden(&job.profile, v.field));
    views
}

const NOT_USED: &str = "not used for this texture";
const NOT_USED_BULK: &str = "not used for these textures";

/// The Sources header's fixed summary, also drawn for a texture with no job (so no `summary` call).
pub const SOURCES_SUMMARY: &str = "How each value was chosen";

fn shown(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// The header summary of `g` over its rows in `views` (the texture panel's one-line value; the table is the plan's
/// "Header summaries"). A locked row is left out; a group whose every row is locked reads "not used for this texture".
/// `changed` (LQ4): some row of the group shows its ↺ (`edited`), a locked one included, as `field_row` draws it.
pub fn summary(
    g: PanelGroup,
    views: &[FieldView],
    target: Target,
    p: &Profile,
    extra: &SummaryExtra<'_>,
) -> GroupSummary {
    let fixed = |text: String| GroupSummary {
        text,
        hover: None,
        changed: false,
        all_locked: false,
    };
    match g {
        PanelGroup::Sources => return fixed(SOURCES_SUMMARY.into()),
        PanelGroup::Channels => {
            return fixed(extra.channels.map_or_else(String::new, |[r, g, b, a]| {
                format!("R {r}, G {g}, B {b}, A {a}")
            }));
        }
        _ => {}
    }
    let rows: Vec<&FieldView> = views
        .iter()
        .filter(|v| panel_group(v.field) == Some(g))
        .collect();
    let changed = rows.iter().any(|v| v.edited);
    if !rows.is_empty() && rows.iter().all(|v| v.lock.is_some()) {
        let first = rows[0].lock;
        let hover = if rows.iter().all(|v| v.lock == first) {
            format!("Every row here: {}.", first.unwrap_or_default())
        } else {
            "Each row below says why it is not used.".to_string()
        };
        return GroupSummary {
            text: NOT_USED.into(),
            hover: Some(hover),
            changed,
            all_locked: true,
        };
    }
    let usable: Vec<SumRow<'_>> = rows
        .iter()
        .filter(|v| v.lock.is_none())
        .map(|v| SumRow {
            field: v.field,
            title: v.label,
            value: &v.value,
            mixed: false,
        })
        .collect();
    GroupSummary {
        text: summary_text(g, &usable, target, p, no_effect_reason(p, "resize")),
        hover: None,
        changed,
        all_locked: false,
    }
}

/// One unlocked row as a header summary reads it. `mixed` is a bulk row whose textures hold different values: it reads
/// "{title}: mixed" and `value` is not looked at.
struct SumRow<'a> {
    field: &'a str,
    title: &'a str,
    value: &'a Value,
    mixed: bool,
}

/// The summary text of `g` over its unlocked `rows`, shared by the single panel (`summary`) and the bulk panel
/// (`bulk_summary`) so the two read alike. `p` is the texture's profile (the bulk panel's representative one).
fn summary_text(
    g: PanelGroup,
    rows: &[SumRow<'_>],
    target: Target,
    p: &Profile,
    size_no_effect: Option<&str>,
) -> String {
    let row = |field: &str| rows.iter().find(|v| v.field == field);
    // A row's option label, else its value as written; a mixed row says so.
    let label = |v: &SumRow<'_>| {
        if v.mixed {
            return format!("{}: mixed", v.title);
        }
        option_label(v.field, target, p, v.value).unwrap_or_else(|| shown(v.value))
    };
    let named = |field: &str| row(field).map(label);
    let join =
        |parts: Vec<Option<String>>| parts.into_iter().flatten().collect::<Vec<_>>().join(", ");
    match g {
        PanelGroup::Alpha => {
            let policy = row("alpha.policy").map(|v| {
                let cut = !v.mixed && matches!(v.value.as_str(), Some("cutout" | "cutout_blend"));
                match (cut, row("alpha.threshold")) {
                    (true, Some(t)) if !t.mixed => {
                        format!("{} at {}", label(v), shown(t.value))
                    }
                    _ => label(v),
                }
            });
            join(vec![policy])
        }
        PanelGroup::Mipmaps => match row(LEVELS_FIELD.0) {
            // The tick off writes "none": that option's own label, with no filter.
            Some(l) if !l.mixed && !mips_on(l.value) => label(l),
            levels => join(vec![
                named(FILTER_FIELD.0),
                levels.map(|l| {
                    if l.mixed {
                        label(l)
                    } else {
                        label(l).to_lowercase()
                    }
                }),
            ]),
        },
        PanelGroup::NormalMap => join(vec![
            named("mips.normal"),
            named(MSN_SOURCE).map(|b| format!("baked in {b}")),
        ]),
        PanelGroup::Size => match size_no_effect {
            Some(why) => why.to_string(),
            None => match row("resize") {
                Some(r) if !r.mixed && r.value.as_str() == Some("none") => label(r),
                Some(r) => {
                    let of = row("resize_relative_to")
                        .filter(|v| v.mixed || v.value.as_str() != Some("self"))
                        .map(|v| format!(" of {}", label(v)))
                        .unwrap_or_default();
                    let floor = row("resize_floor")
                        .map(|v| format!(", floor {}", label(v)))
                        .unwrap_or_default();
                    format!("{}{of}{floor}", label(r))
                }
                None => String::new(),
            },
        },
        PanelGroup::Compression => join(vec![
            named("color.tag"),
            EFFORT_FIELDS
                .iter()
                .find_map(|f| row(f))
                .map(|v| format!("Effort {}", label(v))),
            row("header")
                .filter(|v| v.mixed || v.value.as_str() != Some("auto"))
                .map(label),
        ]),
        PanelGroup::Cubemap => join(vec![
            named("cubemap.layout"),
            row("cubemap.face_size").map(|v| match v.value.as_u64() {
                Some(n) => format!("{n} px faces"),
                None => "auto-sized faces".to_string(),
            }),
        ]),
        PanelGroup::Channels | PanelGroup::Sources => String::new(),
    }
}

thread_local! {
    static BULK_SUMMARY_BUILDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times `bulk_summary` has run on this thread (tests: a bulk summary is built once per input change, never
/// per frame). The single panel's `summary` is not counted.
pub fn bulk_summary_builds() -> usize {
    BULK_SUMMARY_BUILDS.with(std::cell::Cell::get)
}

/// The header summary of `g` over the bulk panel's rows (`fields`: every row of the selection, any group). A row no
/// selected texture can use (`applies == 0`) is a locked part: left out, and a group of only such rows reads "not
/// used for these textures" (R1-9). A row whose textures hold different values reads "{Label}: mixed". `changed` is
/// the single panel's rule (LQ4): some row of the group shows its ↺ (`edited > 0`, which counts only the textures the
/// row reaches). `p` is the profile the panel's rows read their option labels from, and
/// only that (TPC-T7-1): `size_no_effect` is the no-effect reason of Resize when every applying texture shares it, so
/// one selection reads the same whichever texture is active. `g` is one of `bulk::BULK_GROUPS`.
// shortcut: option labels (the Relative to slot names, the DDS tag list) come from one representative texture's
// profile, as the bulk rows' editors do; upgrade if a selection mixing PBR and other maps must name slots per
// texture.
pub fn bulk_summary(
    g: PanelGroup,
    fields: &[BulkField],
    target: Target,
    p: &Profile,
    size_no_effect: Option<&str>,
) -> GroupSummary {
    BULK_SUMMARY_BUILDS.with(|c| c.set(c.get() + 1));
    debug_assert!(
        super::bulk::BULK_GROUPS.contains(&g),
        "{g:?} is never in bulk"
    );
    let rows: Vec<&BulkField> = fields
        .iter()
        .filter(|f| panel_group(f.field) == Some(g))
        .collect();
    let changed = rows.iter().any(|f| f.edited > 0);
    let usable: Vec<SumRow<'_>> = rows
        .iter()
        .filter(|f| f.applies > 0)
        .map(|f| SumRow {
            field: f.field,
            title: f.label,
            value: f.value.as_ref().unwrap_or(&Value::Null),
            mixed: f.value.is_none(),
        })
        .collect();
    if usable.is_empty() {
        // The shared reason, when every row is locked for one and the same reason.
        let first = rows.first().and_then(|f| match f.skipped.as_slice() {
            [(why, _)] => Some(why.as_str()),
            _ => None,
        });
        let hover = match first {
            Some(why)
                if rows
                    .iter()
                    .all(|f| matches!(f.skipped.as_slice(), [(r, _)] if r == why)) =>
            {
                format!("Every row here: {why}.")
            }
            _ => "Each row below says why it is not used.".to_string(),
        };
        return GroupSummary {
            text: NOT_USED_BULK.into(),
            hover: Some(hover),
            changed,
            all_locked: true,
        };
    }
    GroupSummary {
        text: summary_text(g, &usable, target, p, size_no_effect),
        hover: None,
        changed,
        all_locked: false,
    }
}

pub fn is_open(settings: &GuiSettings, g: PanelGroup) -> bool {
    is_open_in(&settings.texture_groups, g)
}

/// [`is_open`] over the `[texture_groups]` table itself, which a draw's `Ctx` holds.
pub fn is_open_in(groups: &BTreeMap<String, bool>, g: PanelGroup) -> bool {
    groups
        .get(g.id())
        .copied()
        .unwrap_or_else(|| default_open(g))
}

/// A row of the problem list that the texture panel's first screen shows: not a note (`Problem::is_note`: Info, queued
/// or marked as intended) and not a code Plan 3a classes as a Note.
pub fn is_panel_problem(p: &Problem) -> bool {
    !p.is_note() && p.code.is_none_or(|c| row_class(c) == RowClass::Problem)
}

/// The panel field a problem code is about, `(code, field)`: the row a problem's group opens on. A code Plan 3a classes
/// as a Note is not here (it never opens a group), and SET-BAKE-RATIO is never raised (Note HD2).
///
/// Every field here is drawn by a row that follows `settings_panel::focus_if` (`field_row`, the Filter row, the Levels
/// row, the cube rows, the Channel guard row's ticks), and Go to focuses it. Where no control can take the focus the group still opens: a locked or
/// disabled row (a plan pending) and `cubemap.layout` of a six-file cube (a plain label, "Six face files") scroll into
/// view and take the request with nothing focused; a row the panel leaves out (`fields::hidden_reason`, for example
/// `alpha.parallax` outside a complex material) has nothing to scroll to, and the request expires after
/// `REVEAL_FIELD_PASSES`.
pub const FIX_FIELDS: &[(&str, &str)] = &[
    ("LOSS-ALPHA-DATA", "format"),
    ("FMT-BC1-ALPHA", "format"),
    ("FMT-RGB8-ALPHA", "format"),
    ("FMT-BC5-NORMAL", "format"),
    ("FMT-LANDSCAPE-BC1", "format"),
    ("FMT-CS-SKIN-LOADER", "format"),
    ("ALPHA-EXACT-OFF-M", "alpha.exact_opaque"),
    ("ALPHA-PARALLAX-NO-ALPHA", "alpha.parallax"),
    ("ALPHA-BC1-ONE-BIT", "encoder.bc1_alpha"),
    ("VAL-COVERAGE", "mips.coverage"),
    ("VAL-ALPHA-GUARANTEE", "alpha.policy"),
    ("TAG-SRGB-VANILLA", "color.tag"),
    ("TAG-SRGB-NONPBR", "color.tag"),
    ("TAG-UNORM-PBR", "color.tag"),
    ("TAG-SRGB-LOD", "color.tag"),
    ("TAG-CONTENT-MISMATCH", "color.content"),
    ("FMT-PERCEPTUAL-DATA", "encoder.perceptual"),
    ("VAL-MIP-ERROR", "mips.filter"),
    ("DIM-TOO-LARGE", "resize"),
    ("CUBE-NO-MIPS", "mips.levels"),
    ("CUBE-FACES", "cubemap.layout"),
    ("VAL-HEADER", "header"),
];

/// The field `code` points at, from [`FIX_FIELDS`].
pub fn fix_field(code: &str) -> Option<&'static str> {
    FIX_FIELDS.iter().find(|(c, _)| *c == code).map(|(_, f)| *f)
}

/// The groups holding a field a problem row's code points at. `rows` are already filtered by [`is_panel_problem`]. A
/// first-screen field (Format) is in no group. Allocates when a group is found, so the panel builds it once per list
/// change and keeps it (`texture_problems`), never per frame.
pub fn groups_with_problems(rows: &[Problem]) -> BTreeSet<PanelGroup> {
    problem_levels(rows).into_keys().collect()
}

/// [`groups_with_problems`] with each group's worst level (`Level` orders Blocks first). A row counts for its code and
/// for the codes it also names (TPC-T5-3), as `panel_problem_rows` treats them as part of the row.
pub fn problem_levels(rows: &[Problem]) -> BTreeMap<PanelGroup, Level> {
    let mut out: BTreeMap<PanelGroup, Level> = BTreeMap::new();
    for p in rows {
        for c in p.code.iter().chain(p.also.iter()) {
            if let Some(g) = fix_field(c.as_str()).and_then(panel_group) {
                let worst = out.get(&g).map_or(p.level, |l| (*l).min(p.level));
                out.insert(g, worst);
            }
        }
    }
    out
}

pub fn set_open(settings: &mut GuiSettings, g: PanelGroup, open: bool) {
    settings.texture_groups.insert(g.id().to_string(), open);
}
