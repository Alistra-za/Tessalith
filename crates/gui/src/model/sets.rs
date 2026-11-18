//! The user's texture sets (M3b spec §3, §6.1): views for the Sets column and set editor, shared-slot ownership,
//! the engine inputs (`set_edits`, `output_paths`) and the drop rules. No egui types.
use super::history::{sha256_of, CreatedFile};
use super::items::{ItemKind, ItemStatus, SetItem, TextureItem};
use super::txst_map::{fill_from_records, record_paths, record_shared_sets};
use super::AppModel;
use crate::pgp::prefix::{
    active_matches, check_file_name, check_rel_path, derive_match_diffuse, is_path_key, output_rel,
    prefix_of, slot_match_path, slot_suffixes, strip_lead,
};
use crate::pgp::settings::Settings;
use crate::sets_doc::{SetEntry, SetsDoc, TxstMap};
use engine::codes::{Code, Warning};
use engine::config::{Category, Subtype, Target};
use engine::group::{base_of, split_vocab, VocabRole};
use engine::overrides::path_key_matches;
use engine::pack::RecipeKind;
use engine::presets::Provenance;
use engine::sets::{fold_id, same_id, set_id, OutputKey, SetEdit};
use engine::shader::{Shader, ShaderPick};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SlotKind {
    Diffuse,
    Normal,
    Emissive,
    /// The Advanced skin `_sk` (slot 3, shared with the emissive).
    SkinSk,
    Height,
    Rmaos,
    Coat,
    Fuzz,
    Subsurface,
    // Note GI (X12): the landscape sets' XPMF maps; no PGPatcher slot. Last, so the others keep their order.
    ProjectedNoise,
    DetailNormal,
    /// The Advanced skin `_wet` (slot 9) and `_detail_n` (slot 10): numbers past the game's, so the cards, locks and
    /// borrows (keyed by number) work on them while no game-facing writer sees them (`game_slot`).
    SkinWet,
    SkinDetail,
}

/// The highest slot the game (PGPatcher `slotN`, a TXST, a NIF) has; the skin members' 9 and 10 are sdc's own.
pub const LAST_GAME_SLOT: u8 = 8;
/// The highest slot sdc numbers (the skin `_detail_n`).
pub const LAST_SLOT: u8 = 10;

impl SlotKind {
    /// Every slot kind, in the slot commands' order. The set editor orders its cards by `slot_row` (F1 spec §5).
    pub const ALL: [SlotKind; 13] = [
        SlotKind::Diffuse,
        SlotKind::Normal,
        SlotKind::Rmaos,
        SlotKind::Height,
        SlotKind::Emissive,
        SlotKind::SkinSk,
        SlotKind::Subsurface,
        SlotKind::Coat,
        SlotKind::Fuzz,
        SlotKind::ProjectedNoise,
        SlotKind::DetailNormal,
        SlotKind::SkinWet,
        SlotKind::SkinDetail,
    ];
    /// The slot's number for the cards, locks and borrows: PGPatcher's 1-based `slotN` (DECISIONS P6), then 9 and 10
    /// for the skin `_wet` and `_detail_n`; `None` for the XPMF maps, which no slot, lock or borrow holds (Note GI).
    /// Anything the game reads (a `slotN`, a TXST or NIF slot, a 3D preview binding) takes `game_slot` instead.
    pub fn slot_number(self) -> Option<u8> {
        Some(match self {
            SlotKind::Diffuse => 1,
            SlotKind::Normal => 2,
            SlotKind::Emissive | SlotKind::SkinSk => 3,
            SlotKind::Height => 4,
            SlotKind::Rmaos => 6,
            SlotKind::Coat | SlotKind::Fuzz => 7,
            SlotKind::Subsurface => 8,
            SlotKind::SkinWet => 9,
            SlotKind::SkinDetail => 10,
            SlotKind::ProjectedNoise | SlotKind::DetailNormal => return None,
        })
    }
    /// The slot the game has: `slot_number` up to [`LAST_GAME_SLOT`]; `None` for the XPMF maps and the skin members.
    pub fn game_slot(self) -> Option<u8> {
        self.slot_number().filter(|&n| n <= LAST_GAME_SLOT)
    }
    /// One card: the same kind, or two kinds of one PGPatcher slot (3 or 7, Note AO).
    pub fn shares_card(self, o: SlotKind) -> bool {
        self == o
            || self
                .slot_number()
                .is_some_and(|n| o.slot_number() == Some(n))
    }
    /// The canonical file suffix under the prefix (`""` for the diffuse).
    pub fn suffix(self) -> &'static str {
        match self {
            SlotKind::Diffuse => "",
            SlotKind::Normal => "n",
            SlotKind::Emissive => "g",
            SlotKind::SkinSk => "sk",
            SlotKind::Height => "p",
            SlotKind::Rmaos => "rmaos",
            SlotKind::Coat => "cnr",
            SlotKind::Fuzz => "f",
            SlotKind::Subsurface => "s",
            SlotKind::ProjectedNoise => "noise",
            SlotKind::DetailNormal => "detail",
            SlotKind::SkinWet => "wet",
            SlotKind::SkinDetail => "detail_n",
        }
    }
    /// The slot's name without the number: `label`, but a shared slot names both kinds (as `slot_label`'s empty card).
    pub fn short_label(self) -> &'static str {
        match self.slot_number() {
            Some(3) => "Emissive / Skin",
            Some(7) => "Coat normal / Fuzz",
            _ => self.label(),
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            SlotKind::Diffuse => "Diffuse",
            SlotKind::Normal => "Normal _n",
            SlotKind::Emissive => "Emissive _g",
            SlotKind::SkinSk => "Skin _sk",
            SlotKind::Height => "Height _p",
            SlotKind::Rmaos => "RMAOS",
            SlotKind::Coat => "Coat _cnr",
            SlotKind::Fuzz => "Fuzz _f",
            SlotKind::Subsurface => "Subsurface _s",
            SlotKind::ProjectedNoise => "Projected noise",
            SlotKind::DetailNormal => "Detail normal",
            SlotKind::SkinWet => "Skin wet _wet",
            SlotKind::SkinDetail => "Skin detail _detail_n",
        }
    }
}

/// Note GI (X16): the Detail normal card's hover in a landscape set.
pub const DETAIL_NORMAL_HOVER: &str =
    "Community Shaders doesn't draw the projected detail normal in this build; it shows without CS.";

/// The card title of `slot` holding item `key`: a `_coat` source in the Subsurface slot is the coat colour + strength
/// (M6 Track E spec §3.1, DECISIONS Note U2), decided by the file name.
pub fn slot_label(slot: SlotKind, key: &str) -> &'static str {
    let stem = Path::new(key)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    match split_vocab(&stem) {
        Some((_, v)) if slot == SlotKind::Subsurface && v.role == VocabRole::CoatColour => {
            "Coat colour + strength"
        }
        _ if slot.slot_number() == Some(7) => SLOT7_CARD,
        _ if key.is_empty() && slot.slot_number() == Some(3) => SLOT3_CARD,
        _ => slot.label(),
    }
}

/// Note AO: the one card of PGPatcher slot 7, which holds the coat normal `_cnr` or the fuzz `_f`.
pub const SLOT7_CARD: &str = "Coat normal / Fuzz (slot 7)";
/// Note AO: the empty card of PGPatcher slot 3, which holds the emissive `_g` or the Advanced skin `_sk`.
pub const SLOT3_CARD: &str = "Emissive / Skin (slot 3)";

/// Note AO: the kinds a card of `slot`'s PGPatcher slot takes (`Coat _cnr or Fuzz _f` for slot 7).
pub fn slot_kinds_label(slot: SlotKind) -> String {
    let kinds: Vec<&str> = SlotKind::ALL
        .iter()
        .filter(|k| k.shares_card(slot))
        .map(|k| k.label())
        .collect();
    kinds.join(" or ")
}

/// Note FI11 (A15): slot `n` as the set editor's empty card names it, without the "(slot n)" its title adds (the Match
/// menu and Slot commands say the number themselves). Slot 5 has no card: the cubemap. A vanilla set (`!pbr`) takes the
/// Meshes tab's name for the texture-set slot (`edit_slot_label`, read only; keep in step with nif_edit.rs).
pub fn slot_name(pbr: bool, n: u8) -> &'static str {
    if !pbr {
        return super::meshes::nif_edit::edit_slot_label(0, 0, 0, usize::from(n.max(1) - 1));
    }
    match SlotKind::ALL.iter().find(|k| k.slot_number() == Some(n)) {
        Some(k) => k.short_label(),
        None => "Cubemap",
    }
}

/// The card title of `slot` holding `key` in a set drawn by `shader`: a grass file in the Diffuse card is "Complex grass".
pub fn card_title(slot: SlotKind, key: &str, shader: Option<Shader>) -> &'static str {
    if slot == SlotKind::Diffuse && shader == Some(Shader::ComplexGrass) {
        return "Complex grass";
    }
    slot_label(slot, key)
}

/// One row of the Matching header, or one choice of its + menu (F1 spec §3.2): slot number, slot name, path (stored
/// form, no `.dds`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotMatch {
    pub n: u8,
    pub slot: String,
    pub path: String,
}

/// The Matching header of `v`: its active slot matches plus the empty rows (`matchN = ""`, N ≥ 2: added by + and not
/// filled in yet; `match1 = ""` is the diffuse turned off), and the + menu, which offers slots 3–8 without a row. A
/// slot the set fills starts from `slot_match_path` (DECISIONS V1); any other starts empty (owner, 2026-10-12).
pub fn slot_matches(v: &SetView) -> (Vec<SlotMatch>, Vec<SlotMatch>) {
    let derived = v.derived_match.as_deref().unwrap_or("");
    let filled: Vec<SlotKind> = v
        .slots
        .iter()
        .filter(|(_, f)| f.is_filled())
        .map(|(k, _)| *k)
        .collect();
    let name = |n: u8| match filled.iter().find(|k| k.slot_number() == Some(n)) {
        Some(k) => v.slot_text(*k).to_string(),
        None if n == 1 => SlotKind::Diffuse.label().to_string(),
        None if n == 2 => SlotKind::Normal.label().to_string(),
        None => format!("Slot {n}"),
    };
    let empty = (2..=8u8).filter(|n| {
        v.settings
            .get(&format!("match{n}"))
            .and_then(Value::as_str)
            .is_some_and(|s| s.trim().is_empty())
    });
    let mut active: Vec<SlotMatch> = active_matches(&v.settings, derived)
        .into_iter()
        .chain(empty.map(|n| (n, String::new())))
        .map(|(n, path)| SlotMatch {
            n,
            slot: name(n),
            path,
        })
        .collect();
    active.sort_by_key(|a| a.n);
    let add = (3..=8u8)
        .filter(|n| !active.iter().any(|a| a.n == *n))
        .map(|n| SlotMatch {
            n,
            slot: name(n),
            // Prefilled only when PGPatcher's slot-N list strips the suffix, so its prefix is sdc's (controller
            // ruling): `_s` in slot 8 would split it, so that row starts empty.
            path: filled
                .iter()
                .find(|k| {
                    k.slot_number() == Some(n)
                        && slot_suffixes(n).contains(&format!("_{}", k.suffix()).as_str())
                })
                .map_or_else(String::new, |k| slot_match_path(derived, k.suffix())),
        })
        .collect();
    (active, add)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotFill {
    Empty,
    Own {
        item: String,
    },
    /// A finished output of set `from`, used here through a slot command. `from` is the set that owns the file, also
    /// when the share passes through sets that share the slot in turn (Note JN1).
    Borrowed {
        from: String,
        item: String,
    },
}

impl SlotFill {
    /// The item this fill uses (own or borrowed), if any.
    pub fn item(&self) -> Option<&str> {
        match self {
            Self::Own { item } | Self::Borrowed { item, .. } => Some(item),
            Self::Empty => None,
        }
    }

    /// The slot holds a file (own or borrowed).
    pub fn is_filled(&self) -> bool {
        !matches!(self, Self::Empty)
    }
}

/// A line of an `[[sets]]` entry that matches nothing: the card's red "?" chip and a Problems row (Note DU).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Missing {
    /// The entry's files have all gone (its fix is Delete set).
    NoFilesLeft,
    /// An `add` or `remove` path that names no file.
    File(String),
    /// The entry's own `borrow.slot<slot>` line, from set `from`, which fills nothing (Note JN1): set `from` is gone
    /// (`!empty`), or (`empty`) neither it nor a set it shares that slot from in turn has a file there, or the shares
    /// loop back. `itself`: the line names the set itself, which keeps its own file.
    Borrow {
        slot: u8,
        from: String,
        empty: bool,
        itself: bool,
    },
}

impl Missing {
    pub fn text(&self) -> String {
        match self {
            Missing::NoFilesLeft => NO_FILES_LEFT.into(),
            Missing::File(f) => f.clone(),
            Missing::Borrow {
                slot,
                from,
                empty,
                itself,
            } => {
                let none = match (itself, empty) {
                    (true, _) => " (this set itself)",
                    (false, true) => " (it has none)",
                    (false, false) => "",
                };
                // The skin slots past the game's have no `slotN` a user knows: name them.
                let name = SlotKind::ALL
                    .iter()
                    .find(|k| *slot > LAST_GAME_SLOT && k.slot_number() == Some(*slot))
                    .map_or_else(|| format!("slot{slot}"), |k| k.label().to_string());
                format!("{name} from {from}{none}")
            }
        }
    }
}

/// Note HL5: where a set's `derived_match` (the entry's `match1`) comes from, in this order: a mapping (a base, or a
/// record's slot 1), the PBR albedo or diffuse, the set's own base path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchFrom {
    Albedo,
    Mapping,
    BasePath,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SetView {
    /// The engine key (`""` for a created set with no members yet).
    pub key: String,
    pub id: String,
    /// Item keys of every member, packed outputs and raw sources included.
    pub members: Vec<String>,
    pub slots: BTreeMap<SlotKind, SlotFill>,
    /// A TruePBR JSON set: the diffuse resolves to `pbr_albedo`. Not `skin_albedo` since AG90 (owner: Advanced skin
    /// sets get no PGPatcher JSON); the engine's PBR follow-ons still use [`is_pbr_diffuse`].
    pub pbr: bool,
    /// Note HL5: the set is written as a PGPatcher JSON entry: `pbr`, or a PBR-picked set with no `pbr_albedo`
    /// (not a skin or shared set). `pbr` still names the prefix owner.
    pub json: bool,
    /// Note HL5: what `derived_match` came from.
    pub match_from: Option<MatchFrom>,
    /// The diffuse resolves to `skin_albedo`: an Advanced skin set (no JSON, the skin card, AG90).
    pub skin: bool,
    /// A shared skin set (Note HG): marked Advanced skin, holding no diffuse and no Rmaos, only the maps other skin
    /// sets use. Derived in the GUI by [`is_shared`].
    pub shared: bool,
    /// Slots (record numbering, 1-based) an empty slot of the set's texture set record took from the set that writes
    /// that path: slot → that set's id. Each is also a `SlotFill::Borrowed` in `slots` (Note HG).
    pub filled: BTreeMap<u8, String>,
    /// Slots the user turned a record fill off for (`no_fill`): slot → the id of the set that writes the path, empty
    /// when no set does any more.
    pub fill_off: BTreeMap<u8, String>,
    /// Slots the record names that no set in the project writes: slot → the record path. They count as supplied.
    pub record_supplied: BTreeMap<u8, String>,
    /// The Rmaos slot holds (or, empty, would hold) an RFAOS pack: the slot reads RFAOS, not RMAOS (Note HE1).
    pub rfaos: bool,
    pub created: bool,
    /// `add`/`remove`/`share` entries of `sdc-project.toml` that match no file (Review focus 1).
    pub missing: Vec<Missing>,
    /// Note HH7: item keys of the helpers of other sets (or of no set) this set shares (`SetEntry::share`).
    pub shared_helpers: Vec<String>,
    /// Slots whose finished output this set owns while another set borrows it.
    pub owns: Vec<SlotKind>,
    pub derived_match: Option<String>,
    /// The output prefix, or why there is none (shown, and an export block for a PBR set on CS).
    pub prefix: Option<String>,
    pub prefix_error: Option<String>,
    /// The set's own JSON settings.
    pub settings: Settings,
    /// M6: the engine's shader pick (from the plan), the saved choice (from `sdc-project.toml`), and the set's
    /// SHADER-* warnings.
    pub shader: Option<ShaderPick>,
    pub shader_choice: Option<Shader>,
    pub set_warnings: Vec<Warning>,
    /// F3a Task 8: the locked slot numbers (`SetEntry::lock`).
    pub locks: BTreeSet<u8>,
    /// The texture set record the set is mapped to (`SetEntry::txst`), for the card line and the menu.
    pub txst: Option<TxstMap>,
    /// The engine's recipe reports for this set (`SetItem::recipes`), read by the Sets column pill.
    pub recipes: Vec<engine::events::RecipeReport>,
}

/// The joined tails a set's member names carry past the base (Note GW1): `_detail_n` in a Landscape, PBR Landscape
/// or Advanced skin set, `_wet` in an Advanced skin set only, and `_detail` / `_detailnorm` in those and in a plain
/// set that joined one (`Detail`). A lone `rock_detail` is its own set with no tail, as in the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tails {
    Plain,
    Detail,
    Landscape,
    Skin,
}

impl SetView {
    /// The tails this set's member names read (see [`Tails`]).
    pub fn tails(&self) -> Tails {
        if self.skin || self.shared {
            Tails::Skin
        } else if self.landscape_host() {
            Tails::Landscape
        } else if self.joined_detail() {
            Tails::Detail
        } else {
            Tails::Plain
        }
    }

    /// The engine's detail host rule for landscape (`api::detail_host`): the pick or the saved choice is Landscape or
    /// PBR Landscape, or the first member's path says landscape.
    fn landscape_host(&self) -> bool {
        let is = |s: Shader| matches!(s, Shader::Landscape | Shader::PbrLandscape);
        self.shader.as_ref().is_some_and(|p| is(p.shader))
            || self.shader_choice.is_some_and(is)
            || self.members.first().is_some_and(|k| {
                engine::suggest::landscape_path(&k.replace('\\', "/").to_lowercase())
            })
    }

    /// A member is `<another member's base>_detail` or `_detailnorm`: the engine joined it (`group_sets_with`).
    fn joined_detail(&self) -> bool {
        let stems: Vec<String> = self
            .members
            .iter()
            .filter_map(|k| Path::new(k).file_stem())
            .map(|s| s.to_string_lossy().to_lowercase())
            .collect();
        let bases: Vec<String> = stems
            .iter()
            .map(|s| engine::group::split_suffix_with(s, &BTreeMap::new()).base)
            .collect();
        stems.iter().any(|s| {
            engine::group::DETAIL_SPELLINGS.iter().any(|sp| {
                s.strip_suffix(&format!("_{sp}"))
                    .is_some_and(|b| bases.iter().any(|o| o == b))
            })
        })
    }

    pub(crate) fn new(key: String, id: String, members: Vec<String>) -> SetView {
        SetView {
            key,
            id,
            members,
            slots: BTreeMap::new(),
            pbr: false,
            json: false,
            match_from: None,
            skin: false,
            shared: false,
            filled: BTreeMap::new(),
            fill_off: BTreeMap::new(),
            record_supplied: BTreeMap::new(),
            rfaos: false,
            created: false,
            missing: vec![],
            shared_helpers: vec![],
            owns: vec![],
            derived_match: None,
            prefix: None,
            prefix_error: None,
            settings: Settings::new(),
            shader: None,
            shader_choice: None,
            set_warnings: vec![],
            locks: BTreeSet::new(),
            txst: None,
            recipes: vec![],
        }
    }

    /// The slot's name in this set: `SlotKind::label`, but the Rmaos slot of an RFAOS set says RFAOS (Note HE1).
    pub fn slot_text(&self, k: SlotKind) -> &'static str {
        if k == SlotKind::Rmaos && self.rfaos {
            "RFAOS"
        } else {
            k.label()
        }
    }

    /// `slot_label` for this set: the Rmaos slot reads `slot_text`; the coat rule stays `slot_label`'s.
    pub fn slot_label(&self, slot: SlotKind, key: &str) -> &'static str {
        if slot == SlotKind::Rmaos {
            self.slot_text(slot)
        } else {
            slot_label(slot, key)
        }
    }

    /// Whether slot number `n` holds a texture borrowed from another set.
    pub(crate) fn borrows_slot(&self, n: u8) -> bool {
        self.slots
            .iter()
            .any(|(k, f)| k.slot_number() == Some(n) && matches!(f, SlotFill::Borrowed { .. }))
    }
}

/// Whether `v` is written as PGPatcher JSON: a PBR set under Community Shaders (the set card badge), with or without a
/// PBR albedo (Note HL5). An Advanced skin set is not (AG90).
pub fn is_json_set(v: &SetView, target: Target) -> bool {
    v.json && target == Target::Cs
}

/// The set's pick is `pbr_grass` (PBR grass design §4.3): its PGPatcher settings are the grass view.
pub fn is_pbr_grass(v: &SetView) -> bool {
    v.shader
        .as_ref()
        .is_some_and(|p| p.shader == Shader::PbrGrass)
}

pub const BADGE_PBR_LANDSCAPE: &str = "PBR Landscape → CS JSON";
pub const BADGE_PBR_LANDSCAPE_PGP: &str = "PBR Landscape → CS + PGPatcher JSON";
pub const BADGE_PBR_LANDSCAPE_PGP_ONLY: &str = "PBR Landscape → PGPatcher JSON (no CS records yet)";
pub const BADGE_PBR_LANDSCAPE_NONE: &str = "PBR Landscape (no JSON yet)";
/// A PBR pick on Community Shaders whose slots are all empty or kept DDS: the target is not the reason.
pub const BADGE_PBR_NOTHING_TO_WRITE: &str = "PBR set (no texture to write, so no JSON)";

/// The Sets column badge. A PBR Landscape set's says what it writes (Note BM, review I2): CS records when a block
/// has an EDID, the PGPatcher entry when the set is a PGPatcher JSON set its statics rule keeps.
pub fn badge_text(m: &AppModel, v: &SetView) -> &'static str {
    if v.shared {
        "shared skin maps (no JSON)"
    } else if v.skin {
        "Advanced skin"
    } else if is_pbr_landscape(v) {
        match (m.cs_records_written(v), m.pgp_written(v)) {
            (true, true) => BADGE_PBR_LANDSCAPE_PGP,
            (true, false) => BADGE_PBR_LANDSCAPE,
            (false, true) => BADGE_PBR_LANDSCAPE_PGP_ONLY,
            (false, false) => BADGE_PBR_LANDSCAPE_NONE,
        }
    } else if is_json_set(v, m.target) {
        "PBR → JSON"
    } else if super::also::is_pbr_set(v) {
        // Note HL5: the JSON is written on Community Shaders only, and only for a set with a filled slot.
        if m.target == Target::Cs {
            BADGE_PBR_NOTHING_TO_WRITE
        } else {
            "PBR set (no JSON on this target)"
        }
    } else {
        "vanilla set (no JSON)"
    }
}

/// Layout v2 A5: the short kind on the set card's pill (`badge_text` is its hover).
pub fn kind_text(m: &AppModel, v: &SetView) -> &'static str {
    if v.shared {
        "shared"
    } else if v.skin {
        "skin"
    } else if is_pbr_landscape(v) {
        "PBR Landscape"
    } else if is_json_set(v, m.target) {
        "PBR → JSON"
    } else if super::also::is_pbr_set(v) {
        "PBR, no JSON"
    } else {
        "vanilla"
    }
}

/// The set's pick is `pbr_landscape` (Note BM): it gets the PBR Landscape card and badge.
pub fn is_pbr_landscape(v: &SetView) -> bool {
    v.shader
        .as_ref()
        .is_some_and(|p| p.shader == Shader::PbrLandscape)
}

/// Whether the set editor draws the TruePBR panel and the JSON path errors: a JSON set, or any set but a skin set
/// that already has JSON settings (so existing edits are never hidden). `pbr_export::export_blocks` checks `v.pbr`.
pub fn shows_json_panel(v: &SetView, target: Target) -> bool {
    is_json_set(v, target) || (!v.skin && !v.settings.is_empty())
}

/// A diffuse sub-type whose members follow as PBR (DECISIONS P5, Note R): `pbr_albedo`, or the Advanced skin
/// `skin_albedo` (final review C1). It no longer puts a skin set in the PGPatcher JSON: `SetView::pbr` does (AG90).
/// Not the engine's `is_pbr_color`, which decides sRGB and leaves `skin_albedo` (UNORM) out.
pub fn is_pbr_diffuse(st: Subtype) -> bool {
    matches!(st, Subtype::PbrAlbedo | Subtype::SkinAlbedo)
}

/// The slot card a resolved sub-type fills; `None` for sub-types without a card (env mask, complex `_m`,
/// `msn_specular`, backlight, …). Complex grass is one file in the Diffuse card.
pub fn slot_kind(st: Subtype) -> Option<SlotKind> {
    Some(match st {
        Subtype::PbrAlbedo
        | Subtype::Diffuse
        | Subtype::LandscapeDiffuse
        | Subtype::SkinAlbedo
        | Subtype::ComplexGrass => SlotKind::Diffuse,
        Subtype::PbrNormal | Subtype::Normal | Subtype::LandscapeNormal | Subtype::Msn => {
            SlotKind::Normal
        }
        Subtype::PbrEmissive | Subtype::Glow => SlotKind::Emissive,
        Subtype::SkinSk => SlotKind::SkinSk,
        Subtype::Height => SlotKind::Height,
        Subtype::Rmaos | Subtype::SkinRfaos => SlotKind::Rmaos,
        Subtype::CoatNormal => SlotKind::Coat,
        Subtype::PbrFuzz => SlotKind::Fuzz,
        Subtype::PbrSubsurface => SlotKind::Subsurface,
        Subtype::ProjectedNoise => SlotKind::ProjectedNoise,
        Subtype::DetailNormal => SlotKind::DetailNormal,
        Subtype::SkinWet => SlotKind::SkinWet,
        Subtype::SkinDetail => SlotKind::SkinDetail,
        _ => return None,
    })
}

/// The slot a texture fills, by its resolved sub-type. Raw packing sources and items without a job have none.
pub fn slot_of(item: &TextureItem) -> Option<SlotKind> {
    if matches!(item.status, ItemStatus::PackedInto(_)) || item.kind == ItemKind::Helper {
        return None;
    }
    slot_kind(item.job.as_ref()?.profile.subtype)
}

/// Review E10: the slot card a shader map role (`MapRole::label`, a SHADER-MISSING-MAP's `data.map`) goes in, for Go
/// to set's outline; `None` for a role with no card (`_i`, `complex_grass`) or a label nothing knows.
pub fn slot_for_shader_map(label: &str) -> Option<SlotKind> {
    // Not in `map_slot`: that table answers which slot a missing map a record or a borrow could fill, and nothing
    // fills the diffuse slot there.
    if matches!(label, "albedo" | "diffuse" | "skin_albedo") {
        return Some(SlotKind::Diffuse);
    }
    // `map_slot`'s numbers name these kinds; 7 is `_i`, which has no card (Coat and Fuzz share the number).
    match map_slot(label)? {
        2 => Some(SlotKind::Normal),
        3 => Some(SlotKind::Emissive),
        4 => Some(SlotKind::Height),
        6 => Some(SlotKind::Rmaos),
        _ => None,
    }
}

/// The slot cards shader `s` can use (F1 spec §5, DECISIONS V8): each role's sub-types mapped through `slot_kind`.
/// Roles matched by suffix only (`_e`, `_i`, `_sk` on Skin/Eye) map to none.
pub fn usable_slots(s: Shader) -> BTreeSet<SlotKind> {
    let spec = s.spec();
    spec.required
        .iter()
        .chain(spec.optional)
        .flat_map(|r| r.subtypes.iter().copied())
        .filter_map(slot_kind)
        .collect()
}

/// One slot card of the set editor (F1 spec §5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotCell {
    pub kind: SlotKind,
    pub fill: SlotFill,
    /// False for a filled slot the set's shader does not use: it stays, outlined, and says why.
    pub usable: bool,
    /// Note AO: another filled kind in the same PGPatcher slot (`_f` beside `_cnr`, `_sk` beside `_g`): a conflict.
    pub clash: Option<SlotKind>,
}

/// The set's slot cards in PGPatcher slot order 1–8, one per slot (Note AO: slots 3 and 7 hold two kinds each; the
/// card holds the filled one, else the first in `SlotKind` order, and a second filled one is its `clash`). These
/// are every slot the picked shader can use, filled or empty, plus every filled slot it cannot use. With no pick yet,
/// only the filled slots show (F1 spec §5).
pub fn slot_row(pick: Option<&ShaderPick>, slots: &BTreeMap<SlotKind, SlotFill>) -> Vec<SlotCell> {
    let usable = pick.map(|p| usable_slots(p.shader));
    let filled = |k: &SlotKind| slots.get(k).is_some_and(SlotFill::is_filled);
    let mut kinds: Vec<SlotKind> = SlotKind::ALL
        .into_iter()
        .filter(|k| filled(k) || usable.as_ref().is_some_and(|u| u.contains(k)))
        .collect();
    // The XPMF maps (no PGPatcher slot) come last.
    kinds.sort_by_key(|k| (k.slot_number().unwrap_or(u8::MAX), *k));
    let mut cells: Vec<SlotCell> = vec![];
    for kind in kinds {
        let cell = SlotCell {
            kind,
            fill: slots.get(&kind).cloned().unwrap_or(SlotFill::Empty),
            usable: usable.as_ref().is_none_or(|u| u.contains(&kind)),
            clash: None,
        };
        match cells.last_mut() {
            Some(c) if c.kind.shares_card(kind) => {
                if filled(&c.kind) && filled(&kind) {
                    c.clash = Some(kind);
                } else if filled(&kind) {
                    *c = cell;
                }
            }
            _ => cells.push(cell),
        }
    }
    cells
}

/// Note HG4: a shared set shows only the maps other skin sets use; its diffuse and `_rfaos` cards are not drawn.
pub fn shared_cells(v: &SetView, cells: Vec<SlotCell>) -> Vec<SlotCell> {
    if !v.shared {
        return cells;
    }
    cells
        .into_iter()
        .filter(|c| {
            matches!(
                c.kind,
                SlotKind::Normal | SlotKind::SkinSk | SlotKind::SkinWet | SlotKind::SkinDetail
            )
        })
        .collect()
}

/// The slot rows the set editor draws for `v`, in order: its `slot_row` cards (a shared set's own kinds only), less the
/// skin `_wet` and `_detail_n` cards outside an Advanced skin set. Review E4's status line sends a missing map to its
/// row's **Assign…** only when the row is here.
pub fn editor_cells(v: &SetView) -> Vec<SlotCell> {
    shared_cells(v, slot_row(v.shader.as_ref(), &v.slots))
        .into_iter()
        .filter(|c| {
            v.skin || v.shared || !matches!(c.kind, SlotKind::SkinWet | SlotKind::SkinDetail)
        })
        .collect()
}

/// A shared skin set: marked Advanced skin, with members, and every one of them a map a shared set may hold. That
/// is the engine's rule (`group::mark_shared_skin`: `_n`, `_sk`, `_wet`, `_detail_n`), so a set the engine
/// unmarked after a hand edit (a `_p`, a diffuse, an `_rfaos`) is not shared here either. A lone `_rfaos` is not shared.
fn is_shared(v: &SetView, aliases: &BTreeMap<String, String>) -> bool {
    let allowed = |k: &SlotKind| {
        matches!(
            k,
            SlotKind::Normal | SlotKind::SkinSk | SlotKind::SkinWet | SlotKind::SkinDetail
        )
    };
    v.shader
        .as_ref()
        .is_some_and(|p| p.shader == Shader::AdvancedSkin)
        && !v.members.is_empty()
        && v.slots
            .iter()
            .all(|(k, f)| allowed(k) || !f.is_filled())
        // Every member is one of those maps. One in a slot is decided by the slot; one in no slot (a kept DDS has no
        // job, so no slot) by its name, as the engine reads it (`member_split`, so a joined `x_wet` counts).
        && v.members.iter().all(|m| {
            let slot = v
                .slots
                .iter()
                .find(|(_, f)| f.item() == Some(m.as_str()))
                .map(|(k, _)| k);
            match slot {
                Some(k) => allowed(k),
                None => kept_member_is_shared_map(m, aliases),
            }
        })
}

/// A member with no slot reads as `_n`, `_sk`, `_wet` or `_detail_n` by its file name (the engine's member rule).
fn kept_member_is_shared_map(key: &str, aliases: &BTreeMap<String, String>) -> bool {
    use engine::group::{member_split, TextureSet, SKIN_DETAIL, SKIN_WET};
    let set = TextureSet {
        key: String::new(),
        members: vec![],
        suffixes: [SKIN_WET, SKIN_DETAIL].map(String::from).into(),
        shared_skin: false,
        shares: vec![],
    };
    let stem = Path::new(key)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let suffix = member_split(&stem, &set, aliases).suffix;
    matches!(suffix.as_deref(), Some("n" | "sk" | SKIN_WET | SKIN_DETAIL))
}

/// Note AO: why a card's slot is in conflict (both kinds of slot 3 or 7 filled), if it is.
pub fn slot_clash_text(cell: &SlotCell) -> Option<String> {
    slot_conflict(&[cell.kind], cell.clash?)
}

/// Why `incoming` cannot join a set holding `existing`, if it cannot (spec §4.1: `_cnr` and `_f` share slot 7;
/// the emissive `_g` and the Advanced skin `_sk` share slot 3).
pub fn slot_conflict(existing: &[SlotKind], incoming: SlotKind) -> Option<String> {
    let clash = |other: SlotKind| existing.contains(&other);
    match incoming {
        SlotKind::Coat if clash(SlotKind::Fuzz) => Some(SLOT7_CLASH.into()),
        SlotKind::Fuzz if clash(SlotKind::Coat) => Some(SLOT7_CLASH.into()),
        SlotKind::Emissive if clash(SlotKind::SkinSk) => Some(SLOT3_CLASH.into()),
        SlotKind::SkinSk if clash(SlotKind::Emissive) => Some(SLOT3_CLASH.into()),
        _ => None,
    }
}

const SLOT7_CLASH: &str =
    "a set cannot hold both a coat normal (_cnr) and a fuzz map (_f): both use slot 7";
const SLOT3_CLASH: &str =
    "a set cannot hold both an emissive map (_g) and a skin _sk map: both use slot 3";

pub(super) fn entry_of<'a>(entries: &'a [SetEntry], id: &str) -> Option<&'a SetEntry> {
    entries.iter().find(|e| same_id(&e.id, id))
}

fn dir_of(key: &str) -> &str {
    key.rsplit_once('/').map_or("", |(d, _)| d)
}

/// The automatic set id file `item_key` takes from its name: its folder and base name (`a/rock01_n.png` → `a/rock01`).
fn auto_id(item_key: &str) -> String {
    let stem = Path::new(item_key)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    let (dir, base) = (dir_of(item_key), base_of(&stem));
    if dir.is_empty() {
        base
    } else {
        format!("{dir}/{base}")
    }
}

/// Fuzz-9: the PBR type an explicit PBR-family pick gives an untouched member holding `st`: `diffuse` on the
/// Diffuse-slot member (`diffuse_slot`) becomes `pbr_albedo` (an `_i` elsewhere stays `diffuse`), `normal` becomes
/// `pbr_normal`, `glow` becomes `pbr_emissive`; anything else stays (`None`).
fn pbr_member_type(st: Subtype, diffuse_slot: bool) -> Option<Subtype> {
    match st {
        Subtype::Diffuse if diffuse_slot => Some(Subtype::PbrAlbedo),
        Subtype::Normal => Some(Subtype::PbrNormal),
        Subtype::Glow => Some(Subtype::PbrEmissive),
        _ => None,
    }
}

/// Final review B-I2: the inverse of `pbr_member_type`, the vanilla type an explicit pick outside the PBR family gives
/// an untouched recorded member holding `st`: `pbr_albedo` on the Diffuse-slot member becomes `diffuse`, `pbr_normal`
/// `normal`, `pbr_emissive` `glow`; anything else stays (`None`).
fn vanilla_member_type(st: Subtype, diffuse_slot: bool) -> Option<Subtype> {
    match st {
        Subtype::PbrAlbedo if diffuse_slot => Some(Subtype::Diffuse),
        Subtype::PbrNormal => Some(Subtype::Normal),
        Subtype::PbrEmissive => Some(Subtype::Glow),
        _ => None,
    }
}

/// Fuzz-9: member `key` is a fuzz map by its file name (`_fuzz`, `_sheen`).
fn is_fuzz_member(key: &str) -> bool {
    engine::group::is_fuzz_stem(
        &Path::new(key)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy(),
    )
}

/// Note JH1: the path texture `item` is read as: its `[inputs]` alias when the plan applied it (the job's game path
/// ends with it, as `input_name_plan` checks), else its key. A refused alias (IN-ALIAS-INVALID) leaves the key.
fn read_name(m: &AppModel, item: &TextureItem) -> String {
    let alias = m.read_as_of(&item.key).map(|a| a.replace('\\', "/"));
    let applied = |a: &String| {
        let (game, a) = (
            super::rename::lower(super::input_name::no_ext(
                item.job.as_ref().map_or("", |j| j.game_path.as_str()),
            )),
            super::rename::lower(super::input_name::no_ext(a)),
        );
        game == a || game.ends_with(&format!("/{a}"))
    };
    alias.filter(applied).unwrap_or_else(|| item.key.clone())
}

/// The stem the diffuse's output has when it mirrors its source (DECISIONS N2): the stem of the name it is read as
/// (`read_name`), without a colour term (`rock01_albedo` → `rock01`, as Discover names the output). Taken from the
/// source, never from the plan's output, which an output path has already moved under the prefix.
fn mirrored_stem(read: &str) -> String {
    let stem = Path::new(read)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    match split_vocab(&stem) {
        Some((base, v)) if v.role == VocabRole::Colour => base,
        _ => stem,
    }
}

/// Every set, in display order (audit 7, DECISIONS Note Y7): built once per change of its inputs.
pub fn views(m: &AppModel) -> Arc<Vec<SetView>> {
    super::cached(
        m,
        |c| c.views.clone(),
        |c, v| c.views = Some(v),
        || Arc::new(build_views(m)),
    )
}

#[cfg(test)]
thread_local! {
    /// Test probe: how many deep clones `set_views` made on this thread.
    pub static SET_VIEWS_CLONES: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

pub fn set_views(m: &AppModel) -> Vec<SetView> {
    #[cfg(test)]
    SET_VIEWS_CLONES.with(|c| c.set(c.get() + 1));
    // shortcut: a clone per call for the ~20 callers that take ownership; switch them to `views` if it shows in a profile.
    (*views(m)).clone()
}

/// Tests: how many whole `build_views` builds ran (a splice is not one).
#[cfg(feature = "test-hooks")]
pub fn view_builds(m: &AppModel) -> u64 {
    m.view_builds.get()
}

/// What the views read of an item besides its key and set: an item whose value of this is unchanged leaves its set's
/// view as it was (final fix I2). Every item field `build_views` reads (round 2 B): `base_view` `slot_of` (status
/// `PackedInto`, kind `Helper`, `job.profile.subtype`); `entry_view` the keys (`add`/`remove` matching);
/// `finish_view` the diffuse's `job.profile.subtype`, its key and the name it is read as (`read_name`: its alias,
/// checked against `job.game_path`, so an alias that keeps the set id still rebuilds the view, Note JH1). The keys are
/// compared apart (`full_plan_changes`), the rest of a view comes from its `SetItem` (compared whole) and the sets
/// document (in the views key), and `waiting` decides whether a lone member's set has a card (`card_hidden`).
/// `output_suffix` and `recipe_suffix` feed `engine_inputs` and the export blocks, not the views.
pub(crate) fn view_reads(
    i: &TextureItem,
) -> (Option<SlotKind>, Option<Subtype>, bool, Option<&str>) {
    (
        slot_of(i),
        i.job.as_ref().map(|j| j.profile.subtype),
        i.waiting,
        i.job.as_ref().map(|j| j.game_path.as_str()),
    )
}

/// Order: the saved `order` first (ids compared folded, the first listing of an id wins), then the rest by id. The
/// order is folded into a map once, so each view costs one lookup, not a scan of `order` per comparison.
fn sort_by_saved_order(views: &mut [SetView], order: &[String]) {
    let mut at: HashMap<String, usize> = HashMap::with_capacity(order.len());
    for (i, o) in order.iter().enumerate() {
        at.entry(fold_id(o)).or_insert(i);
    }
    views.sort_by_cached_key(|v| {
        (
            at.get(&fold_id(&v.id)).copied().unwrap_or(usize::MAX),
            v.id.clone(),
        )
    });
}

/// Every set, in display order, with its slots, ownership and prefix.
fn build_views(m: &AppModel) -> Vec<SetView> {
    #[cfg(feature = "test-hooks")]
    m.view_builds.set(m.view_builds.get() + 1);
    let root = m.sets_root();
    let entries = m
        .sets_doc
        .as_ref()
        .map(|d| d.entries().to_vec())
        .unwrap_or_default();
    let order = m.sets_doc.as_ref().map(|d| d.order()).unwrap_or_default();

    let (shown, hidden): (Vec<&SetItem>, Vec<&SetItem>) = m
        .sets
        .iter()
        .partition(|s| !card_hidden(m, &entries, &root, s));
    let hidden: Vec<String> = hidden.iter().map(|s| set_id(&root, &s.key)).collect();
    let mut views: Vec<SetView> = shown.iter().map(|s| base_view(m, &root, s)).collect();
    // Created sets that have no members yet, and entries whose files have all gone (spec §7.1: shown as missing,
    // never silently dropped). Note GY5: an entry that says nothing about a hidden set is neither.
    for e in &entries {
        if !views.iter().any(|v| same_id(&v.id, &e.id)) && !hidden.iter().any(|h| same_id(h, &e.id))
        {
            views.push(SetView::new(String::new(), e.id.clone(), vec![]));
        }
    }
    sort_by_saved_order(&mut views, &order);
    // Note HF4: a group's members come straight after its first member, so `index`, Move up, Move down and Move to
    // see what is on screen.
    if let Some(d) = m.sets_doc.as_ref() {
        super::set_groups::pull_groups(&mut views, &d.groups());
    }
    // Entry data: created flag, settings, missing members, borrowed slots.
    let own_slots: Vec<(String, BTreeMap<SlotKind, SlotFill>)> = views
        .iter()
        .map(|v| (v.id.clone(), v.slots.clone()))
        .collect();
    let own = |from: &str| {
        own_slots
            .iter()
            .find(|(id, _)| same_id(id, from))
            .map(|(_, s)| s.clone())
    };
    let lender_borrow =
        |id: &str, n: u8| entry_of(&entries, id).and_then(|e| e.borrow.get(&n).cloned());
    for v in &mut views {
        if let Some(e) = entry_of(&entries, &v.id) {
            entry_view(m, v, e, own, lender_borrow);
        }
    }
    fill_from_records(m, &mut views, &entries, &own_slots);
    // PBR, derived match, prefix.
    for v in &mut views {
        finish_view(m, v);
    }
    // Ownership marks.
    let owners = owners(&views);
    for v in &mut views {
        v.owns = owns_of(v, &owners);
    }
    views
}

/// Note HF1: the set has a member and every one that is not a packed output is in no set (a packed output follows
/// its raw maps). Such a set has no card, whatever its entry holds.
fn all_no_set(m: &AppModel, s: &SetItem) -> bool {
    let mut any = false;
    for k in &s.members {
        if m.item(k).is_some_and(|i| i.kind == ItemKind::Packed) {
            continue;
        }
        if !m.is_no_set(k) {
            return false;
        }
        any = true;
    }
    any
}

/// Note GY5: a set whose only member waits for a type has no card, unless the user created it or set something on it
/// (its sets-doc entry holds more than its id: a shader back to Auto or the last lock removed leaves only the id).
/// Note HF1: nor has a set whose files are all in no set.
fn card_hidden(m: &AppModel, entries: &[SetEntry], root: &Path, s: &SetItem) -> bool {
    all_no_set(m, s)
        || matches!(s.members.as_slice(), [k] if m.item(k).is_some_and(|i| i.waiting))
            && entry_of(entries, &set_id(root, &s.key)).is_none_or(|e| {
                *e == SetEntry {
                    id: e.id.clone(),
                    ..Default::default()
                }
            })
}

/// Note GY5: whether the plan's set `key` has a card, without building the views (a plan's selection pruning).
pub(crate) fn has_card(m: &AppModel, key: &str) -> bool {
    let entries = m.sets_doc.as_ref().map(|d| d.entries()).unwrap_or_default();
    m.sets
        .iter()
        .find(|s| s.key == key)
        .is_some_and(|s| !card_hidden(m, &entries, &m.sets_root(), s))
}

/// Note GY5: how many of the plan's sets have a card (the status line's count; run per plan and per undo, not per
/// frame).
pub(crate) fn carded_sets(m: &AppModel) -> usize {
    let entries = m.sets_doc.as_ref().map(|d| d.entries()).unwrap_or_default();
    let root = m.sets_root();
    m.sets
        .iter()
        .filter(|s| !card_hidden(m, &entries, &root, s))
        .count()
}

/// Whether set-line key `key` (project-relative, possibly without an extension) names something on disk under `root`.
/// A key whose folder cannot be read (renamed, unplugged, permission) counts as present: a line is cleared only when
/// its folder is there and the file is not (Note HE3).
fn key_on_disk(root: &Path, key: &str) -> bool {
    if root.join(key).exists() {
        return true;
    }
    let (dir, prefix) = match key.rsplit_once('/') {
        Some((d, _)) => (root.join(d), format!("{d}/")),
        None => (root.to_path_buf(), String::new()),
    };
    match std::fs::read_dir(dir) {
        Ok(rd) => rd.flatten().any(|e| {
            path_key_matches(key, &format!("{prefix}{}", e.file_name().to_string_lossy()))
        }),
        Err(_) => true,
    }
}

/// A set's view from its plan alone: members, shader pick, warnings, recipes and its own (unborrowed) slots.
fn base_view(m: &AppModel, root: &Path, s: &SetItem) -> SetView {
    let mut v = SetView::new(s.key.clone(), set_id(root, &s.key), s.members.clone());
    v.shader = s.shader.clone();
    v.set_warnings = s.warnings.clone();
    v.recipes = s.recipes.clone();
    for mk in &s.members {
        if let Some(slot) = m.item(mk).and_then(slot_of) {
            v.slots
                .entry(slot)
                .or_insert(SlotFill::Own { item: mk.clone() });
        }
    }
    drop_supplied_missing_maps(m, &mut v);
    v
}

/// The lighting-shader slot (1-based, as `SlotKind::slot_number`) a required map of `Shader::spec` goes in, from
/// `MapRole::label`; `None` for the diffuse-like roles, which are never the missing one beside a diffuse.
pub fn map_slot(label: &str) -> Option<u8> {
    match label {
        "_n" | "_n/_msn" | "_n (model-space)" => Some(2),
        "_g" => Some(3),
        "_p" => Some(4),
        "_rmaos" | "skin_rfaos" | "_m complex" => Some(6),
        // Multilayer's inner layer: texture slot t6, no `SlotKind` (it has no subtype).
        "_i" => Some(7),
        _ => None,
    }
}

/// The slot number of the map a `SHADER-MISSING-MAP` names (`map_slot`); `None` for any other warning. A number, not
/// a `SlotKind`: `_i` has slot 7 and no kind.
pub(crate) fn missing_map_slot(w: &Warning) -> Option<u8> {
    (w.code == Code::ShaderMissingMap)
        .then(|| w.data["map"].as_str().and_then(map_slot))
        .flatten()
}

/// A tangent-space normal: the twin of the engine's private `policy::takes_green` (a normal-category sub-type that is
/// not model-space).
fn tangent_space_normal(st: Subtype) -> bool {
    st.category() == Category::Normal && st != Subtype::Msn
}

/// Note HH8: whether a resolved borrow fills the slot of the missing map `w` names with a file whose sub-type answers
/// the role the set's shader needs for it. B1-5: the engine's `MapRole::matches` decides (sub-type only, so a role
/// matched by suffix, such as `_i`, is never answered by a borrow). B1-6: a role a tangent-space normal answers takes
/// any tangent-space normal, since the borrower only names the lender's exported file.
fn borrow_answers(m: &AppModel, v: &SetView, w: &Warning) -> bool {
    let Some(n) = missing_map_slot(w) else {
        return false;
    };
    let Some(spec) = v.shader.as_ref().map(|p| p.shader.spec_for(m.target)) else {
        return false;
    };
    let Some(role) = spec
        .required
        .iter()
        .find(|r| w.data["map"].as_str() == Some(r.label))
    else {
        return false;
    };
    let takes_normal = [Subtype::Normal, Subtype::PbrNormal]
        .into_iter()
        .any(|st| role.matches(None, Some(st)));
    let answers =
        |st: Subtype| role.matches(None, Some(st)) || takes_normal && tangent_space_normal(st);
    v.slots.iter().any(|(k, f)| match f {
        SlotFill::Borrowed { item, .. } if k.slot_number() == Some(n) => m
            .item(item)
            .and_then(|i| i.job.as_ref())
            .is_some_and(|j| answers(j.profile.subtype)),
        _ => false,
    })
}

/// Note HE4: a missing-map warning goes when a live mesh's texture set or a winning TXST that names this set's
/// diffuse (under the name the set is read as) has that map's slot filled. The scan is not asked whether the game
/// uses it (owner's word).
fn drop_supplied_missing_maps(m: &AppModel, v: &mut SetView) {
    if m.known.supplies.is_empty() {
        return;
    }
    let Some(SlotFill::Own { item }) = v.slots.get(&SlotKind::Diffuse) else {
        return;
    };
    let Some(job) = m.item(item).and_then(|i| i.job.as_ref()) else {
        return;
    };
    let Some(stem) = job.output.file_stem().map(|s| s.to_string_lossy()) else {
        return;
    };
    let diffuse = format!("{}.dds", engine::shader::mesh_key(&job.game_path, &stem));
    let Some(&bits) = m.known.supplies.get(&diffuse) else {
        return;
    };
    v.set_warnings
        .retain(|w| !missing_map_slot(w).is_some_and(|n| bits & (1 << (n - 1)) != 0));
}

/// Entry data: created flag, settings, missing members, borrowed slots. `own` gives a set's own slots by id (`None`
/// for no such set); `lender_borrow` gives set `id`'s own `borrow.slot<n>` line (Note JN1: a chain is followed).
fn entry_view(
    m: &AppModel,
    v: &mut SetView,
    e: &SetEntry,
    own: impl Fn(&str) -> Option<BTreeMap<SlotKind, SlotFill>>,
    lender_borrow: impl Fn(&str, u8) -> Option<String>,
) {
    v.created = e.created;
    v.settings = e.settings.clone();
    v.shader_choice = e.shader;
    v.locks = e.lock.clone();
    v.txst = e.txst.clone();
    if v.key.is_empty() && !e.created {
        v.missing.push(Missing::NoFilesLeft);
    }
    for f in e.add.iter().chain(&e.remove).chain(&e.share) {
        if !m.items.iter().any(|i| path_key_matches(f, &i.key)) {
            v.missing.push(Missing::File(f.clone()));
        }
    }
    v.shared_helpers = e
        .share
        .iter()
        .filter_map(|f| m.items.iter().find(|i| path_key_matches(f, &i.key)))
        .map(|i| i.key.clone())
        // The engine ignores a share of a member (Note HH1).
        .filter(|k| !v.members.contains(k))
        .collect();
    for (slot, from) in &e.borrow {
        // Note JN1 (B2-2): follow a lender's own borrow line to the set that owns the file; own-first, so a lender with
        // its own file and a borrow line lends its own, as one hop did.
        let mut seen = vec![fold_id(&v.id)];
        let mut cur = from.clone();
        let mut hops = 0;
        let hit = loop {
            let folded = fold_id(&cur);
            if seen.contains(&folded) {
                break Err(true);
            }
            seen.push(folded);
            // A gone set on the first hop is the line's own `from` (not `empty`); further on, the chain ends empty.
            let Some(src) = own(&cur) else {
                break Err(hops > 0);
            };
            let found = src.into_iter().find_map(|(k, f)| match f {
                SlotFill::Own { item } if k.slot_number() == Some(*slot) => Some((k, item)),
                _ => None,
            });
            if let Some(found) = found {
                break Ok(found);
            }
            match lender_borrow(&cur, *slot) {
                Some(next) => {
                    cur = next;
                    hops += 1;
                }
                None => break Err(true),
            }
        };
        match hit {
            Ok((kind, item)) => {
                v.slots.insert(kind, SlotFill::Borrowed { from: cur, item });
            }
            Err(empty) => v.missing.push(Missing::Borrow {
                slot: *slot,
                from: from.clone(),
                empty,
                itself: same_id(from, &v.id),
            }),
        }
    }
    // Note HH8: a slot a resolved borrow fills answers the shader's missing-map warning when the borrowed file's type
    // fits the role. An unresolved borrow (a `Missing::Borrow`) fills nothing and keeps it.
    let mut warnings = std::mem::take(&mut v.set_warnings);
    warnings.retain(|w| !borrow_answers(m, v, w));
    v.set_warnings = warnings;
}

/// `derive_match_diffuse` of a texture path (`textures/a/b.dds`, any slashes, `.dds` optional).
fn match_of_path(p: &str) -> String {
    let p = p.replace('\\', "/");
    let p = p.strip_suffix(".dds").unwrap_or(&p);
    let (dir, stem) = p.rsplit_once('/').unwrap_or(("", p));
    derive_match_diffuse(dir, stem)
}

/// Note HL5: `derived_match` and where it came from. A JSON set takes the first that exists: its mapping's base, a
/// record's slot 1 (no albedo owner), the albedo (the diffuse, as before, under the name it is read as, `read_name`),
/// the set's own base path.
fn set_match(v: &mut SetView, diffuse: Option<&str>) {
    let albedo = diffuse.map(|r| derive_match_diffuse(dir_of(r), &mirrored_stem(r)));
    let mapped = v.txst.as_ref().and_then(|t| match &t.base {
        Some(b) => Some(match_of_path(b)),
        None => t.slots.get(&1).filter(|_| !v.pbr).map(|p| match_of_path(p)),
    });
    let (m, from) = match (v.json, mapped, albedo) {
        (true, Some(m), _) => (Some(m), Some(MatchFrom::Mapping)),
        (_, _, Some(a)) => (Some(a), Some(MatchFrom::Albedo)),
        (true, None, None) => (Some(match_of_path(&v.id)), Some(MatchFrom::BasePath)),
        _ => (None, None),
    };
    v.derived_match = m;
    v.match_from = from;
}

/// PBR, derived match, prefix.
fn finish_view(m: &AppModel, v: &mut SetView) {
    let diffuse = match v.slots.get(&SlotKind::Diffuse) {
        Some(SlotFill::Own { item: k } | SlotFill::Borrowed { item: k, .. }) => m.item(k),
        _ => None,
    };
    let st = diffuse
        .and_then(|i| i.job.as_ref())
        .map(|j| j.profile.subtype);
    // AG90: everything that reads `pbr` is PGPatcher JSON (entries, blocks, pitfalls, prefixes, ownership, rename).
    v.pbr = st == Some(Subtype::PbrAlbedo);
    v.skin = st == Some(Subtype::SkinAlbedo);
    v.shared = is_shared(v, &m.suffix_aliases);
    // Note HL5: a PBR pick with no `pbr_albedo` is a JSON set too (the mesh keeps its own diffuse).
    // A set with no texture filled (all kept DDS, no job) writes none.
    v.json = v.pbr
        || (!v.skin
            && !v.shared
            && v.slots.values().any(SlotFill::is_filled)
            && v.shader.as_ref().is_some_and(|p| p.shader == Shader::Pbr));
    // Note HE1: the Rmaos slot reads RFAOS when its item is an RFAOS pack; an empty slot follows the shader.
    v.rfaos = match v.slots.get(&SlotKind::Rmaos).and_then(SlotFill::item) {
        Some(k) => m.item(k).and_then(|i| i.job.as_ref()).is_some_and(|j| {
            j.pack.as_ref().is_some_and(|p| p.kind == RecipeKind::Rfaos)
                || j.profile.subtype == Subtype::SkinRfaos
        }),
        None => v
            .shader
            .as_ref()
            .is_some_and(|p| p.shader == Shader::AdvancedSkin),
    };
    if is_pbr_grass(v) {
        crate::pgp::settings::grass_view(&mut v.settings);
    }
    set_match(v, diffuse.map(|i| read_name(m, i)).as_deref());
    match prefix_of(&v.settings, v.derived_match.as_deref().unwrap_or("")) {
        Ok(p) => v.prefix = Some(p),
        Err(e) => v.prefix_error = Some(e.0),
    }
}

/// The slots of `v` whose output it owns while another set also uses it.
fn owns_of(v: &SetView, owners: &Ownership) -> Vec<SlotKind> {
    owners
        .iter()
        .filter(|((_, users), owner)| owner.as_deref() == Some(v.id.as_str()) && users.len() > 1)
        .filter_map(|((item, _), _)| {
            v.slots
                .iter()
                .find(|(_, f)| f.item() == Some(item))
                .map(|(k, _)| *k)
        })
        .collect()
}

/// Note FL: `views` after a partial plan replaced the `scope` sets (the engine refuses any membership change, and the
/// sets document is unchanged): the scope sets' views are built again, with every view that borrows from one of them
/// and the ownership marks of every view sharing an output with them. Equal to `build_views` on the same model
/// (pinned by `fast_replan::views_after_a_partial_equal_a_fresh_build`).
pub(crate) fn splice_views(m: &AppModel, views: &mut Vec<SetView>, scope: &BTreeSet<String>) {
    // shortcut: the lookups below are linear in the sets per scope set; a partial over many sets (a bulk edit)
    // builds every view instead. Index the sets by key if a big bulk edit's frame shows in a profile.
    let entries = m.sets_doc.as_ref().map(|d| d.entries()).unwrap_or_default();
    let root = m.sets_root();
    let set_of = |key: &str| m.sets.iter().find(|s| s.key == key);
    // Note GY5: a scope set whose card comes or goes changes the list itself.
    let card_moves = scope.iter().filter_map(|k| set_of(k)).any(|s| {
        let shown = views.iter().any(|v| v.key == s.key);
        shown == card_hidden(m, &entries, &root, s)
    });
    // shortcut: a mapped project rebuilds every view (a fill reads other sets' paths); index writers by path if a
    // project with many mapped sets shows it in a profile.
    if card_moves || scope.len() > 32 || entries.iter().any(|e| e.txst.is_some()) {
        *views = build_views(m);
        return;
    }
    let scope_ids: Vec<String> = views
        .iter()
        .filter(|v| scope.contains(&v.key))
        .map(|v| v.id.clone())
        .collect();
    // A set's own slots by id, from the plan (what `build_views` reads before any borrow).
    let own = |from: &str| {
        let v = views.iter().find(|v| same_id(&v.id, from))?;
        Some(set_of(&v.key).map_or_else(BTreeMap::new, |s| base_view(m, &root, s).slots))
    };
    let lender_borrow =
        |id: &str, n: u8| entry_of(&entries, id).and_then(|e| e.borrow.get(&n).cloned());
    // Note JN1: a chain's far end reads the scope set's files too, so every hop is rebuilt (pinned by
    // fast_replan::views_after_a_partial_equal_a_fresh_build).
    let mut borrowers: Vec<&str> = scope_ids.iter().map(String::as_str).collect();
    loop {
        let more: Vec<&str> = entries
            .iter()
            .filter(|e| !borrowers.iter().any(|b| same_id(b, &e.id)))
            .filter(|e| {
                e.borrow
                    .values()
                    .any(|f| borrowers.iter().any(|b| same_id(b, f)))
            })
            .map(|e| e.id.as_str())
            .collect();
        if more.is_empty() {
            break;
        }
        borrowers.extend(more);
    }
    let affected: Vec<usize> = (0..views.len())
        .filter(|&i| {
            let v = &views[i];
            scope.contains(&v.key) || borrowers.iter().any(|b| same_id(b, &v.id))
        })
        .collect();
    let mut rebuilt = vec![];
    for &i in &affected {
        let old = &views[i];
        let mut v = match set_of(&old.key) {
            Some(s) => base_view(m, &root, s),
            None => SetView::new(old.key.clone(), old.id.clone(), vec![]),
        };
        if let Some(e) = entry_of(&entries, &v.id) {
            entry_view(m, &mut v, e, own, lender_borrow);
        }
        finish_view(m, &mut v);
        rebuilt.push(v);
    }
    // The outputs whose users may have changed: every item the rebuilt views used before or use now.
    let mut touched: HashSet<String> = HashSet::new();
    for (&i, v) in affected.iter().zip(&rebuilt) {
        for f in views[i].slots.values().chain(v.slots.values()) {
            touched.extend(f.item().map(str::to_string));
        }
    }
    for (&i, v) in affected.iter().zip(rebuilt) {
        views[i] = v;
    }
    // Every view using a touched output gets its marks again; their marks read all their own outputs' users.
    let marked: Vec<usize> = (0..views.len())
        .filter(|&i| {
            views[i]
                .slots
                .values()
                .any(|f| f.item().is_some_and(|it| touched.contains(it)))
        })
        .collect();
    let items: HashSet<&str> = marked
        .iter()
        .flat_map(|&i| views[i].slots.values().filter_map(SlotFill::item))
        .collect();
    let owners: Ownership = owners_of(views, |it| items.contains(it));
    let owns: Vec<Vec<SlotKind>> = marked
        .iter()
        .map(|&i| owns_of(&views[i], &owners))
        .collect();
    for (i, o) in marked.into_iter().zip(owns) {
        views[i].owns = o;
    }
}

/// For each used output item: (item, the ids of the sets using it in display order) → the owner id, the first PBR
/// set among them (spec §3.3); `None` when no user is a PBR set.
type Ownership = Vec<((String, Vec<String>), Option<String>)>;

fn owners(views: &[SetView]) -> Ownership {
    owners_of(views, |_| true)
}

/// `owners`, for the items `keep` takes only.
fn owners_of(views: &[SetView], keep: impl Fn(&str) -> bool) -> Ownership {
    let mut users: BTreeMap<String, Vec<(String, bool)>> = BTreeMap::new();
    for v in views {
        for f in v.slots.values() {
            if let Some(item) = f.item() {
                if !keep(item) {
                    continue;
                }
                users
                    .entry(item.to_string())
                    .or_default()
                    .push((v.id.clone(), v.pbr));
            }
        }
    }
    users
        .into_iter()
        .map(|(item, us)| {
            let owner = us.iter().find(|(_, pbr)| *pbr).map(|(id, _)| id.clone());
            ((item, us.into_iter().map(|(id, _)| id).collect()), owner)
        })
        .collect()
}

/// The suffix of `item`'s output under a set prefix (M3b spec: `<prefix><canonical suffix>.dds`): its slot's,
/// except that an `_rfaos` keeps `rfaos` (the packed file suffix stays `_rfaos`, DECISIONS Note R-skin-1), where
/// the RMAOS slot's suffix would rename it `_rmaos` (final review C1).
fn output_suffix(item: &TextureItem, slot: SlotKind) -> &'static str {
    match item.job.as_ref().map(|j| j.profile.subtype) {
        Some(Subtype::SkinRfaos) => "rfaos",
        _ => slot.suffix(),
    }
}

/// The recipe suffix of a packed item (`rmaos`, `rfaos`, `p`, `m`).
pub(super) fn recipe_suffix(i: &TextureItem) -> Option<String> {
    let pk = i.job.as_ref()?.pack.as_ref()?;
    Some(pk.kind.suffix().to_string())
}

/// The engine inputs of the current documents and plan (spec §2.1, DECISIONS P1–P2).
pub fn engine_inputs(m: &AppModel) -> (Vec<SetEdit>, BTreeMap<OutputKey, PathBuf>) {
    let entries = m.sets_doc.as_ref().map(|d| d.entries()).unwrap_or_default();
    let mut edits = vec![];
    for e in entries.iter().filter(|e| e.created) {
        edits.push(SetEdit::Create { set: e.id.clone() });
    }
    for e in entries.iter() {
        if let Some(from) = &e.renamed_from {
            edits.push(SetEdit::Rename {
                from: from.clone(),
                to: e.id.clone(),
            });
        }
    }
    for e in entries.iter() {
        for f in &e.add {
            edits.push(SetEdit::Move {
                file: f.clone(),
                to: e.id.clone(),
            });
        }
    }
    for e in entries.iter() {
        for f in &e.remove {
            edits.push(SetEdit::Remove { file: f.clone() });
        }
    }
    // Note HF1: the no-set files, last, so each leaves the set the other edits put it in.
    if let Some(d) = &m.sets_doc {
        edits.extend(d.no_set().into_iter().map(|file| SetEdit::NoSet { file }));
    }
    // Note HH: shares after that, so each names its set in its final shape.
    for e in entries.iter() {
        for f in &e.share {
            edits.push(SetEdit::Share {
                file: f.clone(),
                to: e.id.clone(),
            });
        }
    }
    let mut paths = BTreeMap::new();
    let views = views(m);
    // F3b D2: a set's `output_name` on any target; on CS a PBR set's prefix names its outputs instead.
    let mut named = output_name_paths(m, &views);
    let record = record_paths(m, &views);
    if m.target != Target::Cs && record.is_empty() {
        paths.extend(named);
        return (edits, paths);
    }
    // Note HG (agent ruling, overrides the brief's insertion order): a record path beats every other path for its
    // key *before* the duplicate checks, so a name a record replaced reserves nothing. The checks below then run on
    // final paths, on every target, and a record path that loses one leaves its file on its default path.
    let cs = m.target == Target::Cs;
    let rec_keys: BTreeSet<OutputKey> = record.iter().map(|(k, _)| k.clone()).collect();
    named.retain(|(k, _)| !rec_keys.contains(k));
    let mut candidates = if cs {
        output_candidates(m, &views)
    } else {
        vec![]
    };
    candidates.retain(|(_, k, _)| !rec_keys.contains(k));
    // Mirrored outputs: textures with no explicit path, now or in the plan on screen, write their job key. An owned
    // path on one is held back too (the engine would skip both as IN-DUPLICATE and the plan would not settle);
    // `path_collisions` reports it.
    let mut explicit: BTreeSet<&OutputKey> = candidates.iter().map(|(_, c, _)| c).collect();
    explicit.extend(named.iter().map(|(c, _)| c));
    explicit.extend(rec_keys.iter());
    let last = m.last_inputs.as_ref().map(|l| &l.output_paths);
    let owners = packed_owners(&views);
    let mirrored: BTreeSet<String> = m
        .items
        .iter()
        // Only a texture that will be written (skipped and failed files have no job, as in `path_collisions`).
        .filter(|it| it.job.is_some() && !it.job_key.is_empty())
        .filter(|it| {
            let k = item_output_key_in(&owners, it).unwrap_or(OutputKey::File(it.key.clone()));
            !explicit.contains(&k) && !last.is_some_and(|l| l.contains_key(&k))
        })
        // Note HQ6: explicit paths are under the export folder, job keys under the plan's output root.
        .map(|it| m.export_key(&it.job_key))
        .collect();
    let mut seen = BTreeSet::new();
    let named = named.into_iter().map(|(k, rel)| (String::new(), k, rel));
    for (_, key, rel) in named.chain(candidates) {
        if !cs {
            // Off cs an output name takes no checks, as before records existed; it only reserves its path.
            seen.insert(rel.to_string_lossy().replace('\\', "/").to_lowercase());
            paths.insert(key, rel);
            continue;
        }
        if mirrored.contains(&rel.to_string_lossy().replace('\\', "/").to_lowercase()) {
            continue;
        }
        // Two outputs on one path: keep the first in display order. Export is blocked by `path_collisions` (Task 11),
        // and the engine never sees the collision, so the plan is stable. (Sending both would make the engine skip
        // both as IN-DUPLICATE, the sets would stop being PBR sets, the paths would vanish, and the two-pass planning
        // would flip between two plans.)
        if seen.insert(rel.to_string_lossy().replace('\\', "/").to_lowercase()) {
            paths.insert(key, rel);
        }
    }
    // Record paths last: they take the same checks on every target.
    for (key, rel) in record {
        let low = rel.to_string_lossy().replace('\\', "/").to_lowercase();
        if !mirrored.contains(&low) && seen.insert(low) {
            paths.insert(key, rel);
        }
    }
    (edits, paths)
}

/// F3b D2: the explicit path of every output of each set with an `output_name`: the output's own folder and Skyrim
/// suffix, the new base name. A PBR set on CS is left out (its prefix and PGPatcher `rename` name its outputs), and
/// so is a name `check_output_name` refuses (a hand edit).
pub fn output_name_paths(m: &AppModel, views: &[SetView]) -> Vec<(OutputKey, PathBuf)> {
    let entries = m.sets_doc.as_ref().map(|d| d.entries()).unwrap_or_default();
    let mut out = vec![];
    // Once per pass: the identity the engine accepts for every file.
    let defaults = entries
        .iter()
        .any(|e| e.output_name.is_some())
        .then(|| DefaultOutputs::new(m))
        .flatten();
    for e in entries.iter() {
        let Some(name) = e.output_name.as_deref() else {
            continue;
        };
        if check_output_name(name).is_err() {
            continue;
        }
        let Some(v) = views.iter().find(|v| same_id(&v.id, &e.id)) else {
            continue;
        };
        if m.renames_outputs(v) {
            continue;
        }
        for key in &v.members {
            let Some(item) = m.item(key) else { continue };
            if item.job.is_none() || item.job_key.is_empty() {
                continue;
            }
            // Note HG: the default output of the file as it is now, never the last plan's `job_key`, which a record
            // mapping or an earlier name may have moved.
            let Some(from) = defaults.as_ref().and_then(|d| d.key(item, &v.members)) else {
                continue;
            };
            let Some(rel) = renamed_output(&from, name, v.tails()) else {
                continue;
            };
            let k = if item.kind == ItemKind::Packed {
                let Some(suffix) = recipe_suffix(item) else {
                    continue;
                };
                OutputKey::Packed {
                    set: v.id.clone(),
                    suffix,
                }
            } else {
                OutputKey::File(item.key.clone())
            };
            out.push((k, m.export_rel(&rel)));
        }
    }
    out
}

/// Note HG: the default output key (`/`, lower case, relative to the export root) of every file when no output path
/// is sent for it, from what is true now and as the engine accepts it: one `apply_read_as` over all the project's
/// files with the project's `[inputs]` (so a refused alias leaves its file as it is, as in the plan), then
/// `canonicalise_outputs`. A packed output is named after its first source by `pack::plan::virtual_input` (a shared
/// or joined helper's after the set member it stands in for). No output
/// name or record mapping can reach it. Built once per `output_name_paths` pass.
pub struct DefaultOutputs<'a> {
    out_root: &'a Path,
    root: &'a Path,
    files: HashMap<String, engine::discover::InputFile>,
}

impl<'a> DefaultOutputs<'a> {
    /// `None` without a root.
    pub fn new(m: &'a AppModel) -> Option<Self> {
        use engine::discover::{apply_read_as, canonicalise_outputs, InputFile, InputKind};
        let (root, out_root) = (m.root.as_deref()?, m.sets_root.as_deref()?);
        let (keys, mut list): (Vec<&str>, Vec<InputFile>) = m
            .items
            .iter()
            .filter(|i| i.kind != ItemKind::Packed)
            .map(|i| {
                let rel = PathBuf::from(&i.key);
                let f = InputFile {
                    path: root.join(&rel),
                    output: out_root.join(&rel).with_extension("dds"),
                    game_path: String::new(),
                    rel,
                    kind: if i.kind == ItemKind::Dds {
                        InputKind::Dds
                    } else {
                        InputKind::Image
                    },
                    real_rel: None,
                };
                (i.key.as_str(), f)
            })
            .unzip();
        let aliases = m
            .project
            .as_ref()
            .map(|p| p.read_as_all())
            .unwrap_or_default();
        apply_read_as(&mut list, root, out_root, &aliases);
        canonicalise_outputs(&mut list, &m.suffix_aliases);
        Some(DefaultOutputs {
            out_root,
            root,
            files: keys.into_iter().map(String::from).zip(list).collect(),
        })
    }

    /// `item`'s default output key, `members` being its set's; `None` for a packed output without a job or a known
    /// first source.
    pub fn key(&self, item: &TextureItem, members: &[String]) -> Option<String> {
        use engine::pack::plan::{files_stem, virtual_input, virtual_stem};
        let output = if item.kind == ItemKind::Packed {
            let job = item.job.as_ref()?;
            let pk = job.pack.as_ref()?;
            let first = pk.sources.first()?.path.strip_prefix(self.root).ok()?;
            let first = self
                .files
                .get(first.to_string_lossy().replace('\\', "/").as_str())?;
            // Note HS2: a shared or joined helper stands in for the set's member its output is named after (Note
            // HH2), so the output takes that member's folder and name, not the helper's.
            let stem = files_stem(job, pk.kind).to_lowercase();
            let named = |f: &&engine::discover::InputFile| {
                virtual_stem(&f.rel, pk.kind).to_lowercase() == stem
            };
            let anchor = Some(first)
                .filter(named)
                .or_else(|| members.iter().filter_map(|k| self.files.get(k)).find(named))
                .unwrap_or(first);
            virtual_input(anchor, pk.kind).output
        } else {
            self.files.get(&item.key)?.output.clone()
        };
        Some(engine::manifest::output_key(self.out_root, &output))
    }
}
/// What follows the base name of a member `stem` of a set: its Skyrim suffix or exporter term (`_n`, `_roughness`),
/// and the joined `_wet`, `_detail_n`, `_detail` and `_detailnorm` of the sets that take them ([`Tails`]), as the engine reads them (`group::member_split`, with the preset's suffix `aliases`). `""` for the colour member.
pub fn member_tail<'a>(stem: &'a str, tails: Tails, aliases: &BTreeMap<String, String>) -> &'a str {
    use engine::group::{member_split, TextureSet, DETAIL_SPELLINGS, SKIN_DETAIL, SKIN_WET};
    // `member_split` reads only the set's suffixes: a skin set may hold every joined member.
    let mut suffixes: BTreeSet<String> = BTreeSet::new();
    if tails != Tails::Plain {
        suffixes.extend(DETAIL_SPELLINGS.map(String::from));
    }
    if matches!(tails, Tails::Landscape | Tails::Skin) {
        suffixes.insert(SKIN_DETAIL.into());
    }
    if tails == Tails::Skin {
        suffixes.insert(SKIN_WET.into());
    }
    let set = TextureSet {
        key: String::new(),
        members: vec![],
        suffixes,
        shared_skin: false,
        shares: vec![],
    };
    let base = member_split(stem, &set, aliases).base;
    // The base is a prefix of `stem`, so the cut is on a char boundary.
    &stem[base.len().min(stem.len())..]
}

/// `job_key` (an output path relative to the export root) with its base name replaced by `name`: the folder and the
/// suffix stay (`armor/wall_n.dds`, `steel` → `armor/steel_n.dds`; in a skin set `x_wet.dds` → `steel_wet.dds`).
/// Applying it twice gives the same path, because `check_output_name` refuses a name that ends in a suffix.
pub fn renamed_output(job_key: &str, name: &str, tails: Tails) -> Option<PathBuf> {
    let p = Path::new(job_key);
    let stem = p.file_stem()?.to_string_lossy();
    // Everything after the base stays: a Skyrim suffix, or an exporter term an output keeps (`_roughness` on a
    // target that packs nothing), so two members never land on one name.
    let file = format!("{name}{}.dds", member_tail(&stem, tails, &BTreeMap::new()));
    Some(match p.parent() {
        Some(d) if !d.as_os_str().is_empty() => d.join(file),
        _ => PathBuf::from(file),
    })
}

/// The prefix PBR set `v` gets with `rename` in place of its own (Note HJ5): a file name or a whole path.
pub fn renamed_prefix(v: &SetView, rename: &str) -> Result<String, String> {
    let mut s = v.settings.clone();
    s.insert("rename".into(), Value::String(rename.to_string()));
    prefix_of(&s, v.derived_match.as_deref().unwrap_or("")).map_err(|e| e.0)
}

/// The `rename` PBR set `v` stores for `typed` (Note HJ5, HJ6): its last component alone when that gives the same
/// prefix, so a set whose folder stays keeps the `Key` form (HJ4); else the whole path, with `\`. The folder is
/// compared ignoring ASCII case, as Windows and PGPatcher do (a Case change over a whole path keeps it).
pub fn rename_value(v: &SetView, typed: &str) -> Result<String, String> {
    let full = renamed_prefix(v, typed)?;
    let last = typed.rsplit(['\\', '/']).next().unwrap_or(typed);
    // `last` ends both prefixes, so ignoring case here ignores it in the folder only.
    Ok(
        if renamed_prefix(v, last).is_ok_and(|p| p.eq_ignore_ascii_case(&full)) {
            last.to_string()
        } else {
            typed.replace('/', "\\")
        },
    )
}

/// F3b: whether `name` can be an output base name. A name ending in a map suffix (`wall_n`) is refused: the
/// suffix belongs to each output, and the name would be read back as a base plus that suffix.
pub fn check_output_name(name: &str) -> Result<(), String> {
    check_named(name, "output name")
}

/// `check_output_name` for a named field (a PGPatcher `rename`'s last component shares it).
pub fn check_named(name: &str, field: &str) -> Result<(), String> {
    check_file_name(name, field).map_err(|e| e.0)?;
    let base = base_of(name);
    if base.len() < name.len() {
        return Err(format!(
            "{field}: ends in {}, a map suffix; each output keeps its own ({name:?})",
            &name[base.len()..]
        ));
    }
    // Note GW1: `_detail` and `_detailnorm` are tails in the sets that joined them.
    let lower = name.to_lowercase();
    if let Some(sp) = engine::group::DETAIL_SPELLINGS
        .iter()
        .find(|sp| lower.ends_with(&format!("_{sp}")))
    {
        return Err(format!(
            "{field}: ends in _{sp}, a map suffix; each output keeps its own ({name:?})"
        ));
    }
    Ok(())
}

/// Ids of every set whose Rmaos slot (`_rmaos`/`_rfaos`) holds an output another set's Rmaos slot holds too, owner
/// and borrowers alike (M3b spec §3.3, v1 instruction; DECISIONS R7). The engine bakes no normal variance into a
/// shared roughness and reports no missing companion for these sets. Empty off `cs`.
pub fn shared_roughness(m: &AppModel) -> BTreeSet<String> {
    if m.target != Target::Cs {
        return BTreeSet::new();
    }
    let mut users: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for v in views(m).iter() {
        if let Some(item) = v.slots.get(&SlotKind::Rmaos).and_then(SlotFill::item) {
            users
                .entry(item.to_string())
                .or_default()
                .push(v.id.clone());
        }
    }
    users
        .into_values()
        .filter(|ids| ids.len() > 1)
        .flatten()
        .collect()
}

/// Borrower id → owner id for every set whose Normal slot borrows another set's `_n` (DECISIONS Note R
/// M4-final-1). The engine bakes the borrower's own roughness member from that `_n`. Empty off `cs`.
pub fn borrowed_normal(m: &AppModel) -> BTreeMap<String, String> {
    if m.target != Target::Cs {
        return BTreeMap::new();
    }
    let views = views(m);
    views
        .iter()
        .filter_map(|v| match v.slots.get(&SlotKind::Normal) {
            Some(SlotFill::Borrowed { from, .. }) => {
                // The owner's id as the engine knows it (`from` is matched case-insensitively in `set_views`).
                let owner = views.iter().find(|o| same_id(&o.id, from))?;
                Some((v.id.clone(), owner.id.clone()))
            }
            _ => None,
        })
        .collect()
}

/// Entry id → the saved shader choice (M6, `BatchRequest::set_shaders`).
pub fn set_shaders(m: &AppModel) -> BTreeMap<String, Shader> {
    let entries = m.sets_doc.as_ref().map(|d| d.entries()).unwrap_or_default();
    let mut out: BTreeMap<String, Shader> = entries
        .iter()
        .filter_map(|e| Some((e.id.clone(), e.shader?)))
        .collect();
    // Note HG (R7): a set with no colour map, mapped to a record whose colour a skin set writes, is skin too. A
    // `shader` the user chose is in `out` already and wins.
    if entries.iter().any(|e| e.txst.is_some()) {
        for id in record_shared_sets(m, &views(m)) {
            out.entry(id).or_insert(Shader::AdvancedSkin);
        }
    }
    out
}

/// Note BM: the ids of the sets with a `[sets.cs_txst]` block (sent as `BatchRequest::cs_txst_sets`).
pub fn cs_txst_sets(m: &AppModel) -> BTreeSet<String> {
    let entries = m.sets_doc.as_ref().map(|d| d.entries()).unwrap_or_default();
    entries
        .iter()
        .filter(|e| e.cs_txst.is_some())
        .map(|e| e.id.clone())
        .collect()
}

/// The Shader dropdown's choices for `target` (after "Auto").
pub fn shader_options(target: Target) -> Vec<Shader> {
    Shader::available(target).to_vec()
}

/// Every owned output's explicit path, sorted by its owner's display position: (owner id, key, path relative to
/// the export root). Duplicate paths are kept here; `engine_inputs` drops them and `path_collisions` reports them.
/// A path that fails `check_rel_path` is never a candidate: the engine joins these under the export folder as given.
pub fn output_candidates(m: &AppModel, views: &[SetView]) -> Vec<(String, OutputKey, PathBuf)> {
    // Every view of an id with its position, in display order: the first one is what a linear `find` would give.
    let mut by_id: HashMap<&str, Vec<(usize, &SetView)>> = HashMap::new();
    for (n, v) in views.iter().enumerate() {
        by_id.entry(v.id.as_str()).or_default().push((n, v));
    }
    let pos = |id: &str| by_id.get(id).map_or(usize::MAX, |vs| vs[0].0);
    let mut out = vec![];
    for ((item_key, users), owner) in owners(views) {
        let Some(owner) = owner else { continue };
        let Some(&(_, ov)) = by_id.get(owner.as_str()).and_then(|vs| vs.first()) else {
            continue;
        };
        let Some(prefix) = &ov.prefix else { continue };
        let Some(item) = m.item(&item_key) else {
            continue;
        };
        let Some(slot) = slot_of(item) else { continue };
        let rel = output_rel(prefix, output_suffix(item, slot));
        if check_rel_path(&rel.to_string_lossy(), "output path").is_err() {
            continue;
        }
        let key = if item.kind == ItemKind::Packed {
            // The set holding the item (its source set) names a packed output.
            let source = users
                .iter()
                .find(|id| {
                    by_id.get(id.as_str()).is_some_and(|vs| {
                        vs.iter().any(|(_, v)| {
                            matches!(v.slots.get(&slot), Some(SlotFill::Own { item }) if *item == item_key)
                        })
                    })
                })
                .cloned()
                .unwrap_or(owner.clone());
            let Some(suffix) = recipe_suffix(item) else {
                continue;
            };
            OutputKey::Packed {
                set: source,
                suffix,
            }
        } else {
            OutputKey::File(item.key.clone())
        };
        out.push((owner, key, rel));
    }
    out.sort_by_key(|(o, _, _)| pos(o));
    out
}

/// The linear `engine_inputs` and `output_candidates` the indexed ones replaced (fast re-plan final fix I1), kept to
/// pin that both give equal results (`model::tests::engine_inputs_equal_the_linear_scan`).
#[cfg(test)]
pub(crate) mod linear {
    use super::*;

    pub(crate) fn engine_inputs(m: &AppModel) -> (Vec<SetEdit>, BTreeMap<OutputKey, PathBuf>) {
        let (edits, _) = super::engine_inputs(m);
        let mut paths = BTreeMap::new();
        let views = views(m);
        let mut named = output_name_paths(m, &views);
        let record = record_paths(m, &views);
        let cs = m.target == Target::Cs;
        if !cs && record.is_empty() {
            paths.extend(named);
            return (edits, paths);
        }
        named.retain(|(k, _)| !record.iter().any(|(r, _)| r == k));
        let mut candidates = if cs {
            output_candidates(m, &views)
        } else {
            vec![]
        };
        candidates.retain(|(_, k, _)| !record.iter().any(|(r, _)| r == k));
        let explicit = |k: &OutputKey| {
            candidates.iter().any(|(_, c, _)| c == k)
                || named.iter().any(|(c, _)| c == k)
                || record.iter().any(|(c, _)| c == k)
                || m.last_inputs
                    .as_ref()
                    .is_some_and(|l| l.output_paths.contains_key(k))
        };
        let mirrored: BTreeSet<String> = m
            .items
            .iter()
            .filter(|it| it.job.is_some() && !it.job_key.is_empty())
            .filter(|it| {
                let k = item_output_key(&views, it).unwrap_or(OutputKey::File(it.key.clone()));
                !explicit(&k)
            })
            .map(|it| m.export_key(&it.job_key))
            .collect();
        let mut seen = BTreeSet::new();
        let named = named.into_iter().map(|(k, rel)| (String::new(), k, rel));
        for (_, key, rel) in named.chain(candidates) {
            let low = rel.to_string_lossy().replace('\\', "/").to_lowercase();
            if !cs {
                seen.insert(low);
                paths.insert(key, rel);
            } else if !mirrored.contains(&low) && seen.insert(low) {
                paths.insert(key, rel);
            }
        }
        for (key, rel) in record {
            let low = rel.to_string_lossy().replace('\\', "/").to_lowercase();
            if !mirrored.contains(&low) && seen.insert(low) {
                paths.insert(key, rel);
            }
        }
        (edits, paths)
    }

    pub(crate) fn output_candidates(
        m: &AppModel,
        views: &[SetView],
    ) -> Vec<(String, OutputKey, PathBuf)> {
        let pos = |id: &str| views.iter().position(|v| v.id == id).unwrap_or(usize::MAX);
        let mut out = vec![];
        for ((item_key, users), owner) in owners(views) {
            let Some(owner) = owner else { continue };
            let Some(ov) = views.iter().find(|v| v.id == owner) else {
                continue;
            };
            let Some(prefix) = &ov.prefix else { continue };
            let Some(item) = m.item(&item_key) else {
                continue;
            };
            let Some(slot) = slot_of(item) else { continue };
            let rel = output_rel(prefix, output_suffix(item, slot));
            if check_rel_path(&rel.to_string_lossy(), "output path").is_err() {
                continue;
            }
            let key = if item.kind == ItemKind::Packed {
                let source = users
                    .iter()
                    .find(|id| {
                        views.iter().any(|v| {
                            &v.id == *id
                                && matches!(v.slots.get(&slot), Some(SlotFill::Own { item }) if *item == item_key)
                        })
                    })
                    .cloned()
                    .unwrap_or(owner.clone());
                let Some(suffix) = recipe_suffix(item) else {
                    continue;
                };
                OutputKey::Packed {
                    set: source,
                    suffix,
                }
            } else {
                OutputKey::File(item.key.clone())
            };
            out.push((owner, key, rel));
        }
        out.sort_by_key(|(o, _, _)| pos(o));
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropChoice {
    Move,
    Share,
    Duplicate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropOutcome {
    Joined,
    AskReplace { existing: String },
    AskChoice { from: String },
    Refused(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotCommand {
    FromSet,
    Path(String),
    Clear,
}

pub(super) const READ_ONLY: &str = "sdc-project.toml is read-only";
const PACKED_MOVE: &str =
    "a packed map follows its raw maps; move those, or share the finished map";
/// The `missing` line of an `[[sets]]` entry whose files have all gone.
const NO_FILES_LEFT: &str = "no texture of this set is left in the folder";

/// Match-related keys change output paths, so editing them re-plans (DECISIONS P7).
fn replans(key: &str) -> bool {
    key.starts_with("match") || key == "path_contains" || key == "rename"
}

/// F3a Task 8: why a locked slot refuses a slot command or a bulk edit.
pub const SLOT_LOCKED: &str = "slot locked";

/// Why `slot` of `v` takes no change: it is locked (F3a Task 8).
fn lock_refusal(v: &SetView, slot: Option<SlotKind>) -> Result<(), String> {
    match slot.and_then(SlotKind::slot_number) {
        Some(n) if v.locks.contains(&n) => {
            Err(format!("slot {n} of {} is locked; unlock it first", v.id))
        }
        _ => Ok(()),
    }
}

/// The view whose members list `item` (`AppModel::set_of_item`'s lookup, borrowed).
fn view_with_member<'a>(views: &'a [SetView], item: &str) -> Option<&'a SetView> {
    views.iter().find(|v| v.members.iter().any(|k| k == item))
}

/// Note JN4 (B2-3): the sets a Delete set or a Remove from set leaves with an empty shared slot, listed before the
/// edit (B2-9).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct LostShares {
    /// The sets' ids, in display order.
    pub sets: Vec<String>,
    /// The file names (`TextureItem::name`) they shared, sorted.
    pub files: Vec<String>,
    /// The same files' item keys, sorted.
    pub keys: Vec<String>,
    /// How many (set, slot) pairs go empty.
    pub slots: usize,
}

/// Note JN5 (B2-6, B2-7): a shared file that leaves its set for the first set sharing it that can take it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Handoff {
    /// The file's item key.
    pub file: String,
    /// Its file name, for the toast.
    pub name: String,
    /// The set it leaves, and its slot number there.
    pub lender: String,
    pub slot: u8,
    /// The set that takes it.
    pub heir: String,
    /// How many other sets share it from the heir afterwards: every other entry in `sdc-project.toml` whose line
    /// named the lender for slot `n`, the skipped ones included. Counted from the entries, NOT the views: a set
    /// whose files are all in no set has no view (`all_no_set`, `card_hidden`) yet its line is rewritten too.
    pub others: usize,
}

/// What `delete_counted` did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Deleted {
    /// How many textures went to no set.
    pub moved: usize,
    pub lost: LostShares,
    /// The files handed to a sharing set, in slot order (Note JN5).
    pub handoffs: Vec<Handoff>,
}

/// Note JN5: the toast sentence for one hand-over.
fn handoff_text(h: &Handoff) -> String {
    let (name, heir) = (&h.name, &h.heir);
    match h.others {
        0 => format!("{name} went to {heir}, which shared it."),
        1 => {
            format!("{name} went to {heir}, which shared it; 1 other set now shares it from there.")
        }
        k => format!(
            "{name} went to {heir}, which shared it; {k} other sets now share it from there."
        ),
    }
}

/// Note JN4: "a", "a and b", "a, b and c", or "a, b, c and 2 more" (at most three names).
fn list_ids(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [a] => a.clone(),
        [a, b] => format!("{a} and {b}"),
        [a, b, c] => format!("{a}, {b} and {c}"),
        [a, b, c, rest @ ..] => format!("{a}, {b}, {c} and {} more", rest.len()),
    }
}

/// Note JN4 (B2-3): the sentence that names the sets left with an empty shared slot; `None` when `lost.sets` is
/// empty. `what` is "it" for Remove from set, and `list_ids(&lost.files)` for Delete set.
fn lost_share_text(lost: &LostShares, what: &str) -> Option<String> {
    let (who, tail) = match lost.sets.as_slice() {
        [] => return None,
        [one] => (one.clone(), ".".to_string()),
        all => (
            format!("{} sets", all.len()),
            format!(": {}.", list_ids(all)),
        ),
    };
    let place = if lost.slots == 1 {
        "that slot is empty now"
    } else {
        "those slots are empty now"
    };
    Some(format!("{who} shared {what}; {place}{tail}"))
}

/// Note DW3: the items in a locked slot of their own set (`BatchRequest::pinned_files`); a borrowing set's lock is
/// left out, so it never changes the owner's reading.
pub fn pinned_files(m: &AppModel) -> BTreeSet<String> {
    locked_items(m)
        .into_iter()
        .filter(|(_, why)| why == SLOT_LOCKED)
        .map(|(k, _)| k)
        .collect()
}

/// The items filling a locked slot of any set (F3a Task 8), with the reason bulk edits skip them: "slot locked" in
/// the set that owns the texture, else "slot locked in <set>" for a set borrowing it. The own lock wins.
pub fn locked_items(m: &AppModel) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for v in views(m).iter() {
        for (k, f) in &v.slots {
            if !k.slot_number().is_some_and(|n| v.locks.contains(&n)) {
                continue;
            }
            match f {
                SlotFill::Own { item } => {
                    out.insert(item.clone(), SLOT_LOCKED.to_string());
                }
                SlotFill::Borrowed { item, .. } => {
                    out.entry(item.clone())
                        .or_insert_with(|| format!("{SLOT_LOCKED} in {}", v.id));
                }
                SlotFill::Empty => {}
            }
        }
    }
    out
}

/// What `slot_gives` says for a slot the set has no file for.
pub const NOTHING: &str = "nothing";

/// What set `set` itself gives at slot `n` (the Slot commands window, the slot menu): the file names of its own
/// slot cards there, or "nothing".
pub fn slot_gives(m: &AppModel, set: &str, n: u8) -> String {
    let views = views(m);
    let Some(v) = views.iter().find(|v| same_id(&v.id, set)) else {
        return String::new();
    };
    let names: Vec<String> = SlotKind::ALL
        .iter()
        .filter(|s| s.slot_number() == Some(n))
        .filter_map(|s| match v.slots.get(s)? {
            SlotFill::Own { item } => m.item(item).map(|i| i.name.clone()),
            _ => None,
        })
        .collect();
    if names.is_empty() {
        NOTHING.into()
    } else {
        names.join(", ")
    }
}

/// Why a member outside a `textures` folder cannot be linked (F3a Task 8).
pub const NO_GAME_PATH: &str = "no game path (not under a textures folder)";

/// F3a Task 8: the path the game loads `item`'s output from, in the form Path… stores (relative to Data\,
/// backslashes, `.dds`), from the current documents: its planned output path when it has one (a PBR output under
/// `textures\pbr\`, as the automatic slot paths, DECISIONS P11), else its game path (`textures\…`). A member with no
/// `textures` folder above it has none.
pub fn member_game_path(m: &AppModel, item: &str) -> Result<String, String> {
    let paths = engine_inputs(m).1;
    game_path_in(m, &views(m), &paths, item)
}

/// `member_game_path` over views and output paths already built (the slot menu builds them once).
pub(crate) fn game_path_in(
    m: &AppModel,
    views: &[SetView],
    paths: &BTreeMap<OutputKey, PathBuf>,
    item: &str,
) -> Result<String, String> {
    let it = m.item(item).ok_or_else(|| format!("no texture {item}"))?;
    let job = it
        .job
        .as_ref()
        .ok_or_else(|| format!("{} writes no output", it.name))?;
    if let Some(p) = item_output_key(views, it).and_then(|k| paths.get(&k)) {
        return Ok(p.to_string_lossy().replace('/', "\\"));
    }
    // `game_path` starts at the last `textures` folder, else it is the path under the opened folder.
    if !job.game_path.starts_with("textures/") {
        return Err(NO_GAME_PATH.into());
    }
    Ok(written_game_path(job).replace('/', "\\"))
}

/// The game path a member's export writes (wave 2 #4, Note R9): the folder of `game_path` and the file name of
/// `output`, as the engine's `colour_mesh_key` and `gras_key` take them (`leaf_normal.png` writes `leaf_n.dds`).
/// `/`-separated.
pub(crate) fn written_game_path(job: &engine::policy::ResolvedJob) -> String {
    let name = job.output.file_name().unwrap_or_default().to_string_lossy();
    match job.game_path.rsplit_once('/') {
        Some((dir, _)) => format!("{dir}/{name}"),
        None => name.into_owned(),
    }
}

/// Empty `slot` of set `id` in the document: its own occupant leaves the set, a borrow is dropped. Part of a replace.
fn clear_slot(s: &mut SetsDoc, id: &str, slot: Option<SlotKind>, existing: &Option<SlotFill>) {
    match existing {
        Some(SlotFill::Own { item }) => s.remove_member(id, item),
        Some(SlotFill::Borrowed { .. }) => {
            if let Some(n) = slot.and_then(SlotKind::slot_number) {
                s.set_borrow(id, n, None);
            }
        }
        _ => {}
    }
}

/// A set that may take files (a drop, a hand-over: Note JN5): it still has files, or the user created it.
fn takes_files(v: &SetView) -> bool {
    !v.key.is_empty() || v.created
}

/// Note JN5: hand-over `h` in the document. The heir takes the file; the lender's own line for that slot, when it
/// names the lender itself, goes (the lender would share back the file it gave away: t5 Minor 5); the sets sharing
/// the slot from the lender share it from the heir.
fn hand_over(s: &mut SetsDoc, h: &Handoff) {
    s.add_member(&h.heir, &h.file);
    // Fresh: a `SetsDoc` mutator that changes the entries bumps the revision (`delete_set` does). On Delete set the
    // lender's entry is gone already, so nothing is written for it (`set_borrow` would make the entry again).
    let self_line = s.entries().iter().any(|e| {
        same_id(&e.id, &h.lender) && e.borrow.get(&h.slot).is_some_and(|f| same_id(f, &h.lender))
    });
    if self_line {
        s.set_borrow(&h.lender, h.slot, None);
    }
    s.retarget_slot_borrows(&h.lender, h.slot, &h.heir);
}

impl AppModel {
    /// Note HF1: the texture `item_key` is in no set (`[project] no_set`): not on a card, written nowhere, its
    /// warnings and questions counted by nothing.
    pub fn is_no_set(&self, item_key: &str) -> bool {
        self.sets_doc.as_ref().is_some_and(|d| {
            d.no_set_keys()
                .iter()
                .any(|k| engine::sets::no_set_names(k, item_key))
        })
    }

    /// The plan's set keys whose files are all in no set, per view key.
    pub(super) fn no_set_sets(&self) -> Arc<HashSet<String>> {
        super::cached(
            self,
            |c| c.no_set_sets.clone(),
            |c, v| c.no_set_sets = Some(v),
            || {
                Arc::new(
                    self.sets
                        .iter()
                        .filter(|s| all_no_set(self, s))
                        .map(|s| s.key.clone())
                        .collect(),
                )
            },
        )
    }

    /// Notes HF1, HF9: whether set `set_key` takes part in the run: false for a no-set texture's set; with an export
    /// scope, true only for its sets; otherwise false for an excluded group's sets. Every gate and count that reads a
    /// texture's questions or files asks this.
    pub fn in_run(&self, set_key: &str) -> bool {
        if self.no_set_sets().contains(set_key) {
            return false;
        }
        match self.export_scope() {
            Some(s) => s.keys.contains(set_key),
            None => !self.excluded_sets().contains(set_key),
        }
    }

    /// Note HF1: whether `item`'s questions, warnings and file count: it is not in no set and its set is in the run.
    pub fn item_in_run(&self, item: &TextureItem) -> bool {
        !self.is_no_set(&item.key) && item.set_key.as_deref().is_none_or(|k| self.in_run(k))
    }

    pub(super) fn view_of(&self, id: &str) -> Option<SetView> {
        views(self).iter().find(|v| same_id(&v.id, id)).cloned()
    }

    /// Tests: `set_of_item` (the lookup stays private).
    #[cfg(feature = "test-hooks")]
    pub fn set_of_item_probe(&self, item: &str) -> Option<SetView> {
        self.set_of_item(item)
    }

    /// The one check from a texture to its set: a no-set texture has none, because its set has no view.
    pub(crate) fn set_of_item(&self, item: &str) -> Option<SetView> {
        view_with_member(&views(self), item).cloned()
    }

    /// `set_of_item`'s engine key, for the callers that only open or reveal the set.
    pub(crate) fn set_key_of_item(&self, item: &str) -> Option<String> {
        self.set_of_item(item).map(|v| v.key)
    }

    pub(crate) fn sets_editable(&self) -> bool {
        self.sets_doc.as_ref().is_some_and(|d| !d.read_only())
    }

    /// Whether slot `n` of set `set` is locked (F3a Task 8).
    pub fn slot_locked(&self, set: &str, n: u8) -> bool {
        views(self)
            .iter()
            .any(|v| same_id(&v.id, set) && v.locks.contains(&n))
    }

    /// Lock (or unlock) slot `n` of set `set`, as one step (F3a Task 8), re-planned: the engine reads the pinned
    /// files (Note DW).
    pub fn set_slot_lock(&mut self, set: &str, n: u8, on: bool) -> Result<(), String> {
        if !self.sets_editable() {
            return Err(READ_ONLY.into());
        }
        // sdc's pin covers the skin `_wet` and `_detail_n` (9, 10) too.
        if !(1..=LAST_SLOT).contains(&n) {
            return Err(format!("slot{n}: there are slots 1 to {LAST_SLOT}"));
        }
        let id = self.view_of(set).ok_or_else(|| format!("no set {set}"))?.id;
        let label = if on { "Lock" } else { "Unlock" };
        self.edit_docs(
            &format!("{label} slot {n} of {id}"),
            None,
            true,
            move |_, d| d.set_lock(&id, n, on),
        );
        Ok(())
    }

    /// Drop the borrow of slot `n` (9 or 10: the skin `_wet` and `_detail_n`, which no slot command clears) of set `set`.
    /// Refused on a pinned slot and a read-only `sdc-project.toml`; writes no PGPatcher setting.
    pub fn clear_slot_borrow(&mut self, set: &str, n: u8) -> Result<(), String> {
        if !self.sets_editable() {
            return Err(READ_ONLY.into());
        }
        let v = self.view_of(set).ok_or_else(|| format!("no set {set}"))?;
        if n <= LAST_GAME_SLOT || n > LAST_SLOT {
            return Err(format!("slot{n} is cleared by its slot command"));
        }
        if v.locks.contains(&n) {
            return Err(SLOT_LOCKED.into());
        }
        if !v.borrows_slot(n) {
            return Err(format!("slot {n} of {} borrows nothing", v.id));
        }
        // Note HG: a record fill is turned off; there is no borrow line to drop.
        let fill = v.filled.contains_key(&n);
        let to = if fill { None } else { self.passed_on(&v, n) };
        let id = v.id;
        self.edit_docs(
            &format!("Clear slot {n} of {id}"),
            None,
            true,
            move |_, s| {
                if fill {
                    s.set_no_fill(&id, n, true);
                } else {
                    s.set_borrow(&id, n, None);
                    // Note JN4 (B2-12): a line that passed a chain on passes its sharing sets on, in this step.
                    if let Some(x) = &to {
                        s.retarget_slot_borrows(&id, n, x);
                    }
                }
            },
        );
        Ok(())
    }

    /// Note HG: turn slot `n`'s record fill of set `set` back on (the `no_fill` key removed), re-planned. Refused on a
    /// pinned slot, a read-only `sdc-project.toml`, and a slot whose fill is not off.
    pub fn restore_fill(&mut self, set: &str, n: u8) -> Result<(), String> {
        if !self.sets_editable() {
            return Err(READ_ONLY.into());
        }
        let v = self.view_of(set).ok_or_else(|| format!("no set {set}"))?;
        if v.locks.contains(&n) {
            return Err(SLOT_LOCKED.into());
        }
        if !v.fill_off.contains_key(&n) {
            return Err(format!("slot {n} of {} is not turned off", v.id));
        }
        let id = v.id;
        self.edit_docs(
            &format!("Use slot {n} from the record"),
            None,
            true,
            move |_, d| d.set_no_fill(&id, n, false),
        );
        Ok(())
    }

    /// Note AN: turn slot `n`'s PGPatcher lock key of set `set` on (`true`) or off (the key removed), as one step.
    pub fn set_pgp_lock(&mut self, set: &str, n: u8, on: bool) -> Result<(), String> {
        if !self.sets_editable() {
            return Err(READ_ONLY.into());
        }
        let key = crate::pgp::prefix::lock_key(n)
            .ok_or_else(|| format!("slot{n} has no PGPatcher lock that keeps its texture"))?;
        let id = self.view_of(set).ok_or_else(|| format!("no set {set}"))?.id;
        self.set_json_value(&id, key, on.then_some(Value::Bool(true)));
        Ok(())
    }

    /// Why `item` cannot leave its set: it fills a locked slot there (F3a Task 8).
    pub(crate) fn leaves_locked(&self, item: &str) -> Result<(), String> {
        // Borrowed from the cached views, not cloned: this runs per drawn row per frame (Note JP3).
        let views = views(self);
        let Some(v) = view_with_member(&views, item) else {
            return Ok(());
        };
        let slot = v
            .slots
            .iter()
            .find(|(_, f)| matches!(f, SlotFill::Own { item: i } if i == item))
            .map(|(k, _)| *k);
        lock_refusal(v, slot)
    }

    /// Note JN2 (B2-1): the slot `item` lends, as (its set's id, slot number `n`): a borrow of slot `n` from that set
    /// resolves to `item`. That is, among the set's own fills (the map `own` gives `entry_view`: `base_view`'s slots,
    /// before any borrow), `item` is the first `SlotFill::Own`, in `SlotKind` order, whose kind has slot number `n`:
    /// the same rule as `entry_view`'s hit. `None` for a no-set texture, a second file of a kind, the second kind of
    /// slot 3 or 7, a helper or packing source with no slot, or a slot with no number (Projected noise, Detail normal).
    // shortcut: a share follows its file only through the GUI edits that call this (Move, New set, Fill from names,
    // Share, Delete set, Remove from set: Notes JN2-JN5). These leave it stale with no toast (JN6):
    // - a type change that moves the file to another slot number, or makes it a helper;
    // - a packed output whose raw maps move to another set (the pack follows them; its sharing sets stay put);
    // - files renamed on disk outside the GUI;
    // - a lent file deleted on disk.
    // Upgrade when a user loses a share that way: re-point the sharing sets when the plan sees the file move.
    pub(crate) fn lent_slot(&self, item: &str) -> Option<(String, u8)> {
        let v = self.set_of_item(item)?;
        let own = self.own_fills(&v.key);
        let n = own
            .iter()
            .find(|(_, f)| matches!(f, SlotFill::Own { item: i } if i == item))
            .and_then(|(k, _)| k.slot_number())?;
        let first = own.into_iter().find_map(|(k, f)| match f {
            SlotFill::Own { item } if k.slot_number() == Some(n) => Some(item),
            _ => None,
        })?;
        (first == item).then_some((v.id, n))
    }

    /// Note JN4 (B2-3, B2-9): call it BEFORE the edit that takes the files `leaving` out of set `except`. The sets
    /// other than `except`, in display order, with an explicit share that resolves to one of `items` (a
    /// `SlotFill::Borrowed` on a slot that is not in `SetView::filled`, so not a record fill; through a chain too), and
    /// the slot would be empty after the edit. A share is not counted when `except` keeps a member (one not in
    /// `leaving`) whose `slot_of` has that slot number (B2-8: the slot stays filled from the lender), nor when the
    /// sharing set has a file of its own in that slot number (it fills the slot again). A set with no card is never
    /// counted (it is out of the run).
    pub(crate) fn sets_sharing(
        &self,
        items: &[String],
        leaving: &[String],
        except: &str,
    ) -> LostShares {
        let views = views(self);
        let kept: BTreeSet<u8> = views
            .iter()
            .find(|v| same_id(&v.id, except))
            .map(|v| {
                v.members
                    .iter()
                    .filter(|k| !leaving.contains(k))
                    .filter_map(|k| self.item(k))
                    .filter_map(slot_of)
                    .filter_map(SlotKind::slot_number)
                    .collect()
            })
            .unwrap_or_default();
        let mut lost = LostShares::default();
        let mut files = BTreeSet::new();
        for v in views.iter().filter(|v| !same_id(&v.id, except)) {
            let mut slots = BTreeSet::new();
            let mut own: Option<BTreeSet<u8>> = None;
            for (k, f) in &v.slots {
                let (SlotFill::Borrowed { item, .. }, Some(n)) = (f, k.slot_number()) else {
                    continue;
                };
                if v.filled.contains_key(&n) || kept.contains(&n) || !items.contains(item) {
                    continue;
                }
                // Final review Important 2: the set's own file fills the slot once the shared one is gone.
                if own
                    .get_or_insert_with(|| self.own_slot_numbers(v))
                    .contains(&n)
                {
                    continue;
                }
                slots.insert(n);
                files.insert(item.clone());
            }
            if !slots.is_empty() {
                lost.sets.push(v.id.clone());
                lost.slots += slots.len();
            }
        }
        // By key, so two files of one name in different folders both print.
        lost.files = files
            .iter()
            .filter_map(|k| self.item(k).map(|i| i.name.clone()))
            .collect();
        lost.files.sort();
        lost.keys = files.into_iter().collect();
        lost
    }

    /// Note JN4: `pack` is a packed output that `item` is a raw map of, and every other raw map of it is in no set
    /// already, so taking `item` out takes the pack out of its set's slots. Conservative: a raw map with no item, or
    /// one in another set, counts as staying, so the pack is not counted then (t4-r1).
    fn last_raw_map_of(&self, item: &str, pack: &str) -> bool {
        let Some(input) = self.item(item).map(|i| &i.input) else {
            return false;
        };
        let Some(pk) = self
            .item(pack)
            .filter(|p| p.kind == ItemKind::Packed)
            .and_then(|p| p.job.as_ref()?.pack.as_ref())
        else {
            return false;
        };
        pk.sources.iter().any(|s| s.path == *input)
            && pk.sources.iter().all(|s| {
                s.path == *input
                    || self
                        .items
                        .iter()
                        .any(|i| i.input == s.path && self.is_no_set(&i.key))
            })
    }

    /// Note JN5 (B2-6, B2-7, B2-8): where `item` goes instead of no set when it leaves its set together with the
    /// files `leaving` (`[item]` for Remove from set, every member for Delete set); `None` when it stays a no-set
    /// file. Call it before the edit (B2-9).
    pub(crate) fn handoff(&self, item: &str, leaving: &[String]) -> Option<Handoff> {
        if self.is_packed(item) {
            return None;
        }
        let (lender, n) = self.lent_slot(item)?;
        let t = self.item(item)?;
        let kind = slot_of(t)?;
        let views = views(self);
        let kinds_of = |v: &SetView, skip: &[String]| -> Vec<SlotKind> {
            v.members
                .iter()
                .filter(|k| !skip.contains(k))
                .filter_map(|k| self.item(k))
                .filter_map(slot_of)
                .collect()
        };
        // B2-8: the lender keeps a file in slot number `n`, so the slot stays filled from it.
        let lv = views.iter().find(|v| same_id(&v.id, &lender))?;
        if kinds_of(lv, leaving)
            .iter()
            .any(|k| k.slot_number() == Some(n))
        {
            return None;
        }
        let entries = self.sets_doc.as_ref()?.entries();
        let sharing: Vec<&str> = entries
            .iter()
            .filter(|e| !same_id(&e.id, &lender))
            .filter(|e| e.borrow.get(&n).is_some_and(|f| same_id(f, &lender)))
            .map(|e| e.id.as_str())
            .collect();
        // B2-7: the first in display order that can take it.
        let heir = views.iter().find(|v| {
            if !sharing.iter().any(|s| same_id(s, &v.id)) {
                return false;
            }
            if v.locks.contains(&n) || !takes_files(v) {
                return false;
            }
            let own = kinds_of(v, &[]);
            if own.iter().any(|k| k.slot_number() == Some(n)) {
                return false;
            }
            // B2-7; the slot-number check above already covers it for one handed file.
            slot_conflict(&own, kind).is_none()
        })?;
        Some(Handoff {
            file: item.to_string(),
            name: t.name.clone(),
            lender,
            slot: n,
            heir: heir.id.clone(),
            others: sharing.len() - 1,
        })
    }

    /// Note JN2: the set with engine key `key`'s own fills, before any share: the map `own` gives `entry_view`
    /// (`base_view`'s slots). Empty for no such set.
    fn own_fills(&self, key: &str) -> BTreeMap<SlotKind, SlotFill> {
        self.sets
            .iter()
            .find(|s| s.key == key)
            .map(|s| base_view(self, &self.sets_root(), s).slots)
            .unwrap_or_default()
    }

    /// Note JN5: the slot numbers set `v` fills with a file of its own (`own_fills`, before any share).
    fn own_slot_numbers(&self, v: &SetView) -> BTreeSet<u8> {
        self.own_fills(&v.key)
            .into_iter()
            .filter(|(_, f)| matches!(f, SlotFill::Own { .. }))
            .filter_map(|(k, _)| k.slot_number())
            .collect()
    }

    /// Note JN4 (B2-12): the set that set `v`'s share line for slot `n` names, when `v` has no file of its own in
    /// slot number `n`: the sets sharing that slot from `v` read the file through that line, so dropping it passes
    /// them on there. `None` when `v` owns a file in that slot number or has no such line.
    fn passed_on(&self, v: &SetView, n: u8) -> Option<String> {
        if self.own_slot_numbers(v).contains(&n) {
            return None;
        }
        self.sets_doc
            .as_ref()?
            .entries()
            .iter()
            .find(|e| same_id(&e.id, &v.id))?
            .borrow
            .get(&n)
            .cloned()
    }

    fn is_packed(&self, item: &str) -> bool {
        self.item(item).is_some_and(|i| i.kind == ItemKind::Packed)
    }

    /// The set ids the engine knows or will know (lower-case): the sets on screen, and the original names of renamed
    /// sets (the rename applies first in the engine, but another set taking that name would make it fail).
    fn taken_ids(&self) -> BTreeSet<String> {
        let mut ids: BTreeSet<String> = set_views(self)
            .into_iter()
            .map(|v| fold_id(&v.id))
            .collect();
        if let Some(d) = &self.sets_doc {
            ids.extend(
                d.entries()
                    .iter()
                    .filter_map(|e| e.renamed_from.as_deref().map(fold_id)),
            );
        }
        ids
    }

    /// The set a texture may be dropped on: it exists and still has files (or is a set the user created).
    fn drop_target(&self, set: &str) -> Result<SetView, String> {
        let target = self.view_of(set).ok_or_else(|| format!("no set {set}"))?;
        if !takes_files(&target) {
            return Err(format!("{}: {NO_FILES_LEFT}", target.id));
        }
        Ok(target)
    }

    /// What fills `slot` of `target` now: `None` when the drop needs no replace. A packed occupant cannot leave (it
    /// follows its raw maps); a borrow of the same set's output is already there (`None`).
    fn occupant(
        &self,
        target: &SetView,
        slot: Option<SlotKind>,
        share_from: Option<&str>,
    ) -> Result<Option<SlotFill>, String> {
        lock_refusal(target, slot)?;
        match slot.and_then(|s| target.slots.get(&s)).cloned() {
            Some(SlotFill::Own { item }) if self.is_packed(&item) => Err(format!(
                "that slot holds a packed map ({item}); move its raw maps out of the set first"
            )),
            Some(SlotFill::Borrowed { from, .. })
                if share_from.is_some_and(|f| same_id(f, &from)) =>
            {
                Ok(None)
            }
            Some(f @ (SlotFill::Own { .. } | SlotFill::Borrowed { .. })) => Ok(Some(f)),
            _ => Ok(None),
        }
    }

    /// A texture dropped on set `set` (spec §3.1, §3.3).
    pub fn drop_on_set(&mut self, item: &str, set: &str) -> DropOutcome {
        if !self.sets_editable() {
            return DropOutcome::Refused(READ_ONLY.into());
        }
        let target = match self.drop_target(set) {
            Ok(t) => t,
            Err(e) => return DropOutcome::Refused(e),
        };
        let from = self.set_of_item(item);
        if from.as_ref().is_some_and(|f| f.id == target.id) {
            return DropOutcome::Joined;
        }
        let incoming = self.item(item).and_then(slot_of);
        let existing: Vec<SlotKind> = target.slots.keys().copied().collect();
        if let Some(why) = incoming.and_then(|s| slot_conflict(&existing, s)) {
            return DropOutcome::Refused(why);
        }
        if let Err(why) = lock_refusal(&target, incoming) {
            return DropOutcome::Refused(why);
        }
        // Note HH5: a no-set helper can be shared, so it asks too; every other no-set texture moves.
        if from.is_none() && self.is_no_set(item) && self.is_shareable_helper(item) {
            return DropOutcome::AskChoice {
                from: "no set".into(),
            };
        }
        // A texture that belongs to a real set asks what to do.
        if let Some(f) = from.as_ref().filter(|f| self.is_real_set(f)) {
            return DropOutcome::AskChoice { from: f.id.clone() };
        }
        self.drop_choice(item, set, DropChoice::Move)
    }

    /// A set the user would lose by moving a texture out of it: more than one member, or one the user touched. An
    /// entry holding only its id (left by a rename back) is no edit.
    pub(super) fn is_real_set(&self, f: &SetView) -> bool {
        let edited = self.sets_doc.as_ref().is_some_and(|d| {
            entry_of(&d.entries(), &f.id).is_some_and(|e| {
                *e != SetEntry {
                    id: e.id.clone(),
                    ..SetEntry::default()
                }
            })
        });
        f.members.len() > 1 || edited || f.created
    }

    /// Note JD1: slot `kind` (an empty `_wet` or `_detail_n`) of set `set` takes `item` from its folder, as a drop
    /// would move it; an undecided `item` is typed for the slot in the same step.
    pub fn move_into_slot(&mut self, item: &str, set: &str, kind: SlotKind) -> Result<(), String> {
        if !self.sets_editable() {
            return Err(READ_ONLY.into());
        }
        let sub = match kind {
            SlotKind::SkinWet => Subtype::SkinWet,
            SlotKind::SkinDetail => Subtype::SkinDetail,
            _ => return Err("only the skin wet and detail slots take a file this way".into()),
        };
        let target = self.drop_target(set)?;
        // The menu lists files for an empty slot only; one filled since then is not replaced from here.
        if self.occupant(&target, Some(kind), None)?.is_some() {
            return Err("the slot is filled; clear it first".into());
        }
        self.choice_allowed(item, &target, Some(kind), DropChoice::Move)?;
        let undecided = self.item(item).is_some_and(|i| i.waiting);
        if undecided {
            if let Some(why) = self.toml_read_only_reason() {
                return Err(why);
            }
        }
        let mut out = Ok(());
        self.batch_edit(&format!("Move into {}", target.id), |m| {
            if undecided {
                m.edit_subtype("Assign", item, Some(sub));
            }
            out = m.apply_choice(item, &target, Some(kind), DropChoice::Move, None);
        });
        out
    }

    /// Note JD2: slot `kind` (an empty `_wet` or `_detail_n`) of set `set` borrows `item` from its set, as Share does.
    pub fn borrow_into_slot(
        &mut self,
        item: &str,
        set: &str,
        kind: SlotKind,
    ) -> Result<(), String> {
        // A stale menu entry: the item was re-typed since, so it would fill another slot.
        if self.item(item).and_then(slot_of) != Some(kind) {
            return Err(format!("{item} is no longer a {}", kind.label()));
        }
        match self.drop_choice(item, set, DropChoice::Share) {
            DropOutcome::Joined => Ok(()),
            DropOutcome::Refused(e) => Err(e),
            _ => Err("the slot is filled; clear it first".into()),
        }
    }

    /// The answer to `AskChoice` (or a plain move). Any choice that would put a finished texture into a slot the set
    /// already fills asks to replace first (spec §3.1); answer that with `drop_replace` and the same choice.
    pub fn drop_choice(&mut self, item: &str, set: &str, choice: DropChoice) -> DropOutcome {
        if !self.sets_editable() {
            return DropOutcome::Refused(READ_ONLY.into());
        }
        let target = match self.drop_target(set) {
            Ok(t) => t,
            Err(e) => return DropOutcome::Refused(e),
        };
        if choice == DropChoice::Share && self.is_shareable_helper(item) {
            return match self.share_helper(item, &target) {
                Ok(()) => DropOutcome::Joined,
                Err(e) => DropOutcome::Refused(e),
            };
        }
        let slot = self.item(item).and_then(slot_of);
        let from = self.set_of_item(item).map(|f| f.id);
        let share_from = (choice == DropChoice::Share)
            .then_some(from.as_deref())
            .flatten();
        match self.occupant(&target, slot, share_from) {
            Err(e) => return DropOutcome::Refused(e),
            Ok(Some(
                SlotFill::Own { item: existing } | SlotFill::Borrowed { item: existing, .. },
            )) => {
                // Checked first so a refusal (a packed map, a raw map to share) is not asked about.
                if let Err(e) = self.choice_allowed(item, &target, slot, choice) {
                    return DropOutcome::Refused(e);
                }
                return DropOutcome::AskReplace { existing };
            }
            Ok(_) => {}
        }
        match self.apply_choice(item, &target, slot, choice, None) {
            Ok(()) => DropOutcome::Joined,
            Err(e) => DropOutcome::Refused(e),
        }
    }

    /// The answer to `AskReplace` for `choice`: the occupant leaves (it becomes a set of its own, or the borrow is
    /// dropped) and `item` comes in by `choice`, as one step. The reason comes back when it is refused.
    pub fn drop_replace(
        &mut self,
        item: &str,
        set: &str,
        choice: DropChoice,
    ) -> Result<(), String> {
        if !self.sets_editable() {
            return Err(READ_ONLY.into());
        }
        let target = self.drop_target(set)?;
        let slot = self.item(item).and_then(slot_of);
        let from = self.set_of_item(item).map(|f| f.id);
        let share_from = (choice == DropChoice::Share)
            .then_some(from.as_deref())
            .flatten();
        let existing = self.occupant(&target, slot, share_from)?;
        self.apply_choice(item, &target, slot, choice, existing)
    }

    /// Note HH5: a helper with a raw-map role, which a set can read without owning it (a `share` line).
    pub(crate) fn is_shareable_helper(&self, item: &str) -> bool {
        self.item(item).is_some_and(|i| {
            i.kind == ItemKind::Helper
                && i.helper
                    .as_ref()
                    .is_some_and(|h| matches!(h.role, engine::helper::HelperRole::Raw(_)))
        })
    }

    /// Note HH5: `target` reads helper `item` as its own, as one step; nothing is copied or moved.
    fn share_helper(&mut self, item: &str, target: &SetView) -> Result<(), String> {
        // Its own set, or for a no-set file the set its name puts it in, which it rejoins on the way back.
        // shortcut: the automatic set is found by name only (`dir/base`), so a renamed one still gets the line, which
        // the views and the engine ignore once it is a member again; upgrade if that line confuses a user.
        let automatic = auto_id(item);
        let own = match self.set_of_item(item) {
            Some(f) => f.id == target.id,
            None => self.is_no_set(item) && same_id(&automatic, &target.id),
        };
        if own {
            return Ok(());
        }
        // The engine has nothing to read it into (a share into a memberless set is stale).
        if target.members.is_empty() {
            return Err(format!(
                "{} has no texture of its own; move one in first",
                target.id
            ));
        }
        let name = self.item(item).map_or(item.to_string(), |i| i.name.clone());
        let (id, file) = (target.id.clone(), item.to_string());
        self.edit_docs(
            &format!("Share {name} with {id}"),
            None,
            true,
            move |_, d| d.add_share(&id, &file),
        );
        Ok(())
    }

    /// Note HH: set `set` stops sharing helper `file`, as one step.
    pub fn stop_sharing(&mut self, set: &str, file: &str) -> Result<(), String> {
        if !self.sets_editable() {
            return Err(READ_ONLY.into());
        }
        let v = self.view_of(set).ok_or_else(|| format!("no set {set}"))?;
        let name = self.item(file).map_or(file.to_string(), |i| i.name.clone());
        if !v.shared_helpers.iter().any(|k| k == file) {
            return Err(format!("{} does not share {name}", v.id));
        }
        let (id, file) = (v.id, file.to_string());
        self.edit_docs(&format!("Stop sharing {name}"), None, true, move |_, d| {
            d.remove_share(&id, &file)
        });
        Ok(())
    }

    /// Why `choice` cannot bring `item` into `target`, if it cannot (whatever the slot holds).
    fn choice_allowed(
        &self,
        item: &str,
        target: &SetView,
        slot: Option<SlotKind>,
        choice: DropChoice,
    ) -> Result<(), String> {
        if choice == DropChoice::Move {
            self.leaves_locked(item)?;
        }
        match choice {
            DropChoice::Move if self.is_packed(item) => Err(PACKED_MOVE.into()),
            // A borrow is a PGPatcher slot (`borrow.slotN`): the XPMF maps have none.
            DropChoice::Share if slot.is_some_and(|s| s.slot_number().is_none()) => Err(
                "Projected noise and Detail normal can't be shared between sets; duplicate the texture instead"
                    .into(),
            ),
            // Final review M4: only a skin set shows (and can clear) cards 9 and 10, and only its JSON reads them.
            DropChoice::Share
                if matches!(slot, Some(SlotKind::SkinWet | SlotKind::SkinDetail))
                    && !(target.skin || target.shared) =>
            {
                Err(format!(
                    "{} is not an Advanced skin set; only those use a wet or detail map",
                    target.id
                ))
            }
            DropChoice::Share if slot.is_none() => Err(
                "only a finished texture can be shared; duplicate the source to use it in two sets"
                    .into(),
            ),
            DropChoice::Share if self.set_of_item(item).is_none() => {
                Err("the texture is in no set".into())
            }
            DropChoice::Duplicate if self.is_packed(item) => {
                Err("a packed map cannot be duplicated; duplicate its raw maps".into())
            }
            _ => Ok(()),
        }
    }

    /// Bring `item` into `target` by `choice`, first emptying the slot of `existing` (a replace). One step.
    fn apply_choice(
        &mut self,
        item: &str,
        target: &SetView,
        slot: Option<SlotKind>,
        choice: DropChoice,
        existing: Option<SlotFill>,
    ) -> Result<(), String> {
        self.choice_allowed(item, target, slot, choice)?;
        let id = target.id.clone();
        match choice {
            DropChoice::Move => {
                let it = item.to_string();
                let label = if existing.is_some() {
                    "Replace in"
                } else {
                    "Move into"
                };
                let lent = self.lent_slot(item);
                self.edit_docs(&format!("{label} {id}"), None, true, move |_, s| {
                    clear_slot(s, &id, slot, &existing);
                    s.add_member(&id, &it);
                    // Note JN2 (B2-1): the sets sharing the moved file's slot follow it, in this step (as HF4/HS13 do
                    // for a rename).
                    if let Some((l, n)) = &lent {
                        s.retarget_slot_borrows(l, *n, &id);
                    }
                });
                Ok(())
            }
            DropChoice::Share => {
                // Sharing adds no member: a set whose only texture leaves would vanish from the engine's sets.
                if matches!(&existing, Some(SlotFill::Own { .. }))
                    && target.members.len() == 1
                    && !target.created
                {
                    return Err(format!(
                        "{id} would have no texture of its own left; move another texture in first"
                    ));
                }
                let fid = self.set_of_item(item).map(|f| f.id).unwrap_or_default();
                let n = slot.and_then(SlotKind::slot_number).unwrap_or_default();
                self.edit_docs(&format!("Share into {id}"), None, true, move |_, s| {
                    clear_slot(s, &id, slot, &existing);
                    s.set_borrow(&id, n, Some(&fid));
                    // Note JN3 (B2-1b): the slot's sharing sets follow the slot to the new source, so no chain is
                    // written; a set that is the new source owns the file and shares nothing.
                    s.retarget_slot_borrows(&id, n, &fid);
                });
                Ok(())
            }
            DropChoice::Duplicate => self.duplicate_into(item, &id, slot, existing),
        }
    }

    /// A texture dropped on the empty area: a new set named after its base name (spec §3.1). Returns the new id;
    /// the reason when nothing was done (read-only `sdc-project.toml`, a packed map, which follows its raw maps, or a
    /// texture filling a locked slot).
    pub fn drop_new_set(&mut self, item: &str) -> Result<String, String> {
        if !self.sets_editable() {
            return Err(READ_ONLY.into());
        }
        if self.is_packed(item) {
            return Err(PACKED_MOVE.into());
        }
        self.leaves_locked(item)?;
        let first = auto_id(item);
        let taken = self.taken_ids();
        let mut id = first.clone();
        let mut n = 2;
        while taken.contains(&fold_id(&id)) {
            id = format!("{first}-{n}");
            n += 1;
        }
        let (i, it) = (id.clone(), item.to_string());
        let lent = self.lent_slot(item);
        self.edit_docs(&format!("New set {id}"), None, true, move |_, s| {
            s.create_set(&i);
            s.add_member(&i, &it);
            // Note JN2 (B2-1): the sets sharing the moved file's slot follow it, in this step (as HF4/HS13 do for a
            // rename).
            if let Some((l, n)) = &lent {
                s.retarget_slot_borrows(l, *n, &i);
            }
        });
        self.note_new_set(&id);
        Ok(id)
    }

    /// Copy `item`'s source next to it under set `set`'s base name and move the copy in (spec §3.4); `existing`
    /// first leaves `slot` (a replace).
    fn duplicate_into(
        &mut self,
        item: &str,
        set: &str,
        slot: Option<SlotKind>,
        existing: Option<SlotFill>,
    ) -> Result<(), String> {
        if !self.sets_editable() {
            return Err(READ_ONLY.into());
        }
        if self.is_packed(item) {
            return Err("a packed map cannot be duplicated; duplicate its raw maps".into());
        }
        let src = self
            .item(item)
            .map(|i| i.input.clone())
            .ok_or("no such texture")?;
        let root = self.root.clone().ok_or("no folder open")?;
        let stem = src
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let ext = src
            .extension()
            .map(|e| e.to_string_lossy().into_owned())
            .unwrap_or_default();
        // `base_of` returns a prefix of `stem`, so the cut is on a char boundary.
        let tail = stem[base_of(&stem).len()..].to_string(); // e.g. "_roughness"
        let set_base = set.rsplit('/').next().unwrap_or(set).to_string();
        let dir = src
            .parent()
            .ok_or("the source has no folder")?
            .to_path_buf();
        let name = |b: &str| dir.join(format!("{b}{tail}.{ext}"));
        // #39: a name another file is read as (its alias) is taken too, as in the rename dialog.
        let aliases: BTreeSet<String> = self
            .project
            .as_ref()
            .map(|p| {
                self.items
                    .iter()
                    .filter_map(|i| p.read_as(&i.key))
                    .map(|a| a.replace('\\', "/").to_lowercase())
                    .collect()
            })
            .unwrap_or_default();
        let aliased = |p: &Path| {
            p.strip_prefix(&root).is_ok_and(|r| {
                aliases.contains(&r.to_string_lossy().replace('\\', "/").to_lowercase())
            })
        };
        let mut dst = name(&set_base);
        let mut n = 2;
        while dst.exists() || aliased(&dst) {
            dst = name(&format!("{set_base}-{n}"));
            n += 1;
        }
        let key = dst
            .strip_prefix(&root)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .map_err(|_| "the copy would be outside the folder".to_string())?;
        std::fs::copy(&src, &dst).map_err(|e| format!("could not copy {}: {e}", src.display()))?;
        let Some(sha256) = sha256_of(&dst) else {
            let _ = std::fs::remove_file(&dst);
            return Err("could not read the copy".into());
        };
        let id = set.to_string();
        let before = self.sets_doc.as_ref().map(|d| d.text());
        self.edit_docs(&format!("Duplicate into {id}"), None, true, move |_, s| {
            clear_slot(s, &id, slot, &existing);
            s.add_member(&id, &key);
        });
        // A changed document means `edit_docs` pushed a new step (no merge key): that step, and only it, carries
        // the file.
        let recorded = self.sets_doc.as_ref().map(|d| d.text()) != before;
        match self.history.undo.last_mut().filter(|_| recorded) {
            Some(last) => {
                last.created_file = Some(CreatedFile {
                    path: dst,
                    source: src,
                    sha256,
                });
                Ok(())
            }
            None => {
                let _ = std::fs::remove_file(&dst);
                Err("the set did not change".into())
            }
        }
    }

    /// Move set `id` to display position `to` (the order decides ownership, so output paths: plan again, P7).
    /// Move set `id` to position `to`; false when nothing moved (trace-debug #55: no step, no dirty document).
    pub fn reorder_set(&mut self, id: &str, to: usize) -> bool {
        if !self.sets_editable() {
            return false;
        }
        let before: Vec<String> = set_views(self).into_iter().map(|v| v.id).collect();
        // Note HF4: the column's order after the groups pull their members together; no visible move, no step.
        let Some(ids) = super::set_groups::moved_order(&before, &self.groups(), id, to) else {
            return false;
        };
        self.edit_docs("Reorder sets", None, true, move |_, s| s.set_order(&ids));
        true
    }

    /// Rename set `id` to `new_id`; refused (returns the reason) when the name is unusable or taken.
    pub fn rename_set(&mut self, id: &str, new_id: &str) -> Result<(), String> {
        if !self.sets_editable() {
            return Err(READ_ONLY.into());
        }
        let new_id = new_id.trim().replace('\\', "/");
        let new_id = new_id.trim_matches('/').to_string();
        check_rel_path(&new_id, "set name").map_err(|e| e.0)?;
        let Some(v) = self.view_of(id) else {
            return Err(format!("no set {id}"));
        };
        if same_id(&v.id, &new_id) {
            return Err(format!("the set is already named {}", v.id));
        }
        // The set's own original name is free: renaming back undoes the rename.
        let own_original = self.sets_doc.as_ref().is_some_and(|d| {
            entry_of(&d.entries(), &v.id)
                .and_then(|e| e.renamed_from.as_deref())
                .is_some_and(|f| same_id(f, &new_id))
        });
        if !own_original && self.taken_ids().contains(&fold_id(&new_id)) {
            return Err(format!("a set {new_id} already exists"));
        }
        // Note HS11: the engine still makes a file's automatic set, card or not (deleted, or emptied by a move), when
        // it applies the rename, so it refuses the rename and the set's entry is left naming no files. A created set's
        // rename is not sent to the engine as a Rename. A file's id follows its input name (`[inputs] read_as`).
        // shortcut: the id is predicted with `base_of` (no preset aliases, no `_detail` join); upgrade to
        // `engine::group::group_sets_with` over the file list if it mis-predicts.
        if !own_original
            && !v.created
            && self.items.iter().any(|i| {
                let read = self
                    .read_as_of(&i.key)
                    .map_or_else(|| i.key.clone(), |a| a.replace('\\', "/"));
                i.kind != ItemKind::Packed && same_id(&auto_id(&read), &new_id)
            })
        {
            return Err(format!(
                "{new_id} is still the automatic set of a file; pick another name"
            ));
        }
        let (a, b) = (v.id.clone(), new_id);
        // TA31: the Also export opt-out (`[export] skip`, set ids) follows, in the same step.
        self.edit_docs(&format!("Rename {a}"), None, true, move |p, s| {
            s.rename_set(&a, &b);
            p.rename_skip_also(&a, &b);
        });
        Ok(())
    }

    /// Note HF1: the no-set textures among `keys` go back to the sets their names make, as one undo step.
    pub fn back_to_automatic_set(&mut self, keys: &[String]) -> Result<(), String> {
        if let Some(why) = self.sets_read_only_reason() {
            return Err(why);
        }
        let keys: Vec<String> = keys.iter().filter(|k| self.is_no_set(k)).cloned().collect();
        if keys.is_empty() {
            return Ok(());
        }
        self.edit_docs("Back to its automatic set", None, true, move |_, s| {
            for k in &keys {
                s.set_no_set(k, false);
            }
        });
        Ok(())
    }

    /// Note HF1: delete any set. Its textures (not the packed outputs, which follow their raw maps) go to no set and
    /// the automatic set does not come back; an entry with no files just goes. A file other sets share goes to the
    /// first of them that can take it instead (Note JN5). Returns what it did (how many textures went to no set, the
    /// files handed to a sharing set, and the sets left with an empty shared slot), or `None` (nothing done) when
    /// there is no such set or the project file takes no edit.
    fn delete_counted(&mut self, id: &str) -> Option<Deleted> {
        if !self.sets_editable() {
            return None;
        }
        let v = self.view_of(id)?;
        let files: Vec<String> = v
            .members
            .iter()
            .filter(|k| !self.is_packed(k))
            .cloned()
            .collect();
        // Note JN5 (B2-6, owner: "Option 1, hand it over"; B2-7, B2-8): a file other sets share goes to the first of
        // them that can take it, in this step; amends HF1.
        let mut handoffs: Vec<Handoff> = files
            .iter()
            .filter_map(|f| self.handoff(f, &v.members))
            .collect();
        handoffs.sort_by_key(|h| h.slot);
        let rest: Vec<String> = v
            .members
            .iter()
            .filter(|k| !handoffs.iter().any(|h| h.file == **k))
            .cloned()
            .collect();
        // Note JN4 (B2-3, B2-9): the sets whose share this edit empties, listed before it; the shares stay (HE3).
        let lost = self.sets_sharing(&rest, &v.members, &v.id);
        let own = self.own_slot_numbers(&v);
        let i = id.to_string();
        let moved = files.len() - handoffs.len();
        let handed = handoffs.clone();
        self.edit_docs(&format!("Delete {id}"), None, true, move |_, s| {
            // Note JN4 (B2-11): a share that passed through this set is passed on to the set it shared from, before
            // the entry goes. Only for a slot number this set has no file of its own in: where it has one, the sharing
            // sets used that file, and the hand-over below (or the lost sentence) covers them. A hand-over is always
            // a slot number with an own file (`lent_slot`), so the two rewrites never touch the same slot number.
            let through: Vec<(u8, String)> = s
                .entries()
                .iter()
                .find(|e| same_id(&e.id, &i))
                .map(|e| {
                    e.borrow
                        .iter()
                        .filter(|(n, _)| !own.contains(n))
                        .map(|(n, x)| (*n, x.clone()))
                        .collect()
                })
                .unwrap_or_default();
            for (n, x) in &through {
                s.retarget_slot_borrows(&i, *n, x);
            }
            s.delete_set(&i, &files);
            for h in &handed {
                hand_over(s, h);
            }
        });
        self.prune_collapsed();
        Some(Deleted {
            moved,
            lost,
            handoffs,
        })
    }

    /// `delete_counted`, as done or not.
    pub fn delete_set(&mut self, id: &str) -> bool {
        self.delete_counted(id).is_some()
    }

    /// Note FC5 (supersedes AG13's confirm): the set menu's Delete set: no question, one undo step and a toast
    /// `Deleted set {id} ({n} textures now in no set)` with Undo (Note HF1), followed by where each shared file went
    /// (Note JN5) and the sets left with an empty shared slot (Note JN4).
    pub fn delete_set_with_toast(&mut self, id: &str) {
        let Some(d) = self.delete_counted(id) else {
            return;
        };
        let step = (self.history.undo.len(), self.history.pushes);
        let mut text = match d.moved {
            0 => format!("Deleted set {id}"),
            1 => format!("Deleted set {id} (1 texture now in no set)"),
            n => format!("Deleted set {id} ({n} textures now in no set)"),
        };
        let mut sentences: Vec<String> = d.handoffs.iter().map(handoff_text).collect();
        sentences.extend(lost_share_text(&d.lost, &list_ids(&d.lost.files)));
        if !sentences.is_empty() {
            text = format!("{text}. {}", sentences.join(" "));
        }
        self.messages.notify_with(
            super::messages::Severity::Info,
            text,
            Some(super::messages::ToastAction::UndoDeleteSet(step.0, step.1)),
        );
    }

    /// The Delete set toast's Undo: undoes the step only while it is still the top one (nothing done since).
    pub fn undo_delete_set(&mut self, depth: usize, pushes: u64) {
        if self.history.undo.len() == depth && self.history.pushes == pushes {
            self.undo();
        } else {
            self.messages.notify(
                super::messages::Severity::Info,
                "Not undone: other changes came after the delete; use Undo in the header",
            );
        }
    }

    /// Note HF1 (supersedes DU5): take `item` out of set `id` into no set, whichever way it was in (automatic, moved
    /// in, the only member). Refused for a packed map (it follows its raw maps). A file other sets share goes to the
    /// first of them that can take it, and a toast says where (Note JN5); else, when sets shared the file, a toast
    /// names them (Note JN4).
    pub fn remove_member(&mut self, id: &str, item: &str) -> Result<(), String> {
        let v = self.remove_member_check(id, item)?;
        let (i, it) = (v.id.clone(), item.to_string());
        // A packed output of the set leaves it with the last of its raw maps (Note HF1).
        let mut gone = vec![item.to_string()];
        gone.extend(
            v.members
                .iter()
                .filter(|p| self.last_raw_map_of(item, p))
                .cloned(),
        );
        // Note JN5 (B2-6, owner: "Option 1, hand it over"; B2-7, B2-8): a file other sets share goes to the first of
        // them that can take it, in this step; amends HF1.
        let h = self.handoff(item, &[item.to_string()]);
        // Note JN4 (B2-3, B2-9): the sets whose share this edit empties, listed before it; the shares stay (HE3).
        // A handed file is never a pack's last raw map: `handoff` needs a `slot_of`, and a pack's raw map is a
        // `PackedInto` item (`items.rs`), whose own job the engine drops (`pack::plan`), so `gone` is `[item]` then.
        // The assert catches an engine change that breaks this (t5 Minor 2).
        debug_assert!(
            h.is_none() || gone.len() == 1,
            "a handed file is a pack's last raw map"
        );
        let lost = if h.is_some() {
            LostShares::default()
        } else {
            self.sets_sharing(&gone, &gone, &v.id)
        };
        let handed = h.clone();
        // Final review B-M7: the `pbr_fuzz` an explicit PBR pick wrote on the fuzz map, and its `auto_pbr` key, go
        // with it in this step: the pick was this set's (a value changed since is the user's and stays).
        let fuzz_step = is_fuzz_member(item);
        self.edit_docs(&format!("Remove from {i}"), None, true, move |p, s| {
            let mut auto = s.auto_pbr();
            if fuzz_step && auto.contains(&it) {
                if p.file_value(&it, "subtype") == Some(serde_json::json!(Subtype::PbrFuzz)) {
                    p.clear_file_value(&it, "subtype");
                }
                auto.retain(|a| a != &it);
                s.set_auto_pbr(&auto);
            }
            s.remove_member(&i, &it);
            if let Some(h) = &handed {
                hand_over(s, h);
            }
        });
        if let Some(h) = h {
            self.messages
                .notify(super::messages::Severity::Info, handoff_text(&h));
            return Ok(());
        }
        let name = self.item(item).map_or(item, |t| t.name.as_str());
        // By key (t4-r1): another file of the same name is not "it".
        let what = if lost.keys == [item] {
            "it".to_string()
        } else {
            list_ids(&lost.files)
        };
        if let Some(sentence) = lost_share_text(&lost, &what) {
            let text = format!("{name} is in no set now. {sentence}");
            self.messages.notify(super::messages::Severity::Info, text);
        }
        Ok(())
    }

    /// Review E10: why `remove_member(id, item)` would refuse, or `None` (the Problems row's disabled hover).
    pub fn remove_member_refusal(&self, id: &str, item: &str) -> Option<String> {
        self.remove_member_check(id, item).err()
    }

    /// `remove_member`'s refusals, in its order; the set's view when it may go ahead.
    fn remove_member_check(&self, id: &str, item: &str) -> Result<SetView, String> {
        if !self.sets_editable() {
            return Err(READ_ONLY.into());
        }
        if self.is_packed(item) {
            return Err(PACKED_MOVE.into());
        }
        let Some(v) = self.view_of(id) else {
            return Err(format!("no set {id}"));
        };
        if !v.members.iter().any(|k| k == item) {
            return Err(format!("{item} is not in {}", v.id));
        }
        self.leaves_locked(item)?;
        Ok(v)
    }

    /// Note HE2: a set line (`add` or `remove`) naming a file that is gone from disk clears itself, with an Info line
    /// in Messages and no undo step of its own (undoing a clear would clear again at once). Run at the end of a full
    /// plan. Guards, so a drive or folder that is briefly away never wipes lines: the file must be editable, the plan
    /// must list items, the root must exist, and the file must really be absent. Borrows and `NoFilesLeft` keep their
    /// chips (Note HE3). One re-plan follows when a line went; it clears nothing more, so it cannot loop.
    pub(super) fn clear_deleted_lines(&mut self) {
        let Some(root) = self.root.clone() else {
            return;
        };
        if !self.sets_editable() || self.items.is_empty() || !root.exists() {
            return;
        }
        let gone: Vec<(String, String)> = views(self)
            .iter()
            .flat_map(|v| {
                v.missing.iter().filter_map(|m| match m {
                    Missing::File(f) if !key_on_disk(&root, f) => Some((v.id.clone(), f.clone())),
                    _ => None,
                })
            })
            .collect();
        let mut cleared = false;
        for (id, f) in gone {
            let Some(doc) = self.sets_doc.as_mut() else {
                return;
            };
            let before = doc.text();
            doc.drop_entry(&id, &f);
            if doc.text() == before {
                continue;
            }
            cleared = true;
            self.messages.log(
                super::messages::Severity::Info,
                format!("{id}: {f} no longer exists, so it was cleared from the set"),
            );
        }
        // Note HF1: a no-set file gone from disk takes its entry with it.
        let gone_no_set: Vec<String> = self
            .sets_doc
            .as_ref()
            .map(|d| d.no_set())
            .unwrap_or_default()
            .into_iter()
            .filter(|k| !key_on_disk(&root, k))
            .collect();
        for k in gone_no_set {
            let Some(doc) = self.sets_doc.as_mut() else {
                return;
            };
            doc.set_no_set(&k, false);
            cleared = true;
            self.messages.log(
                super::messages::Severity::Info,
                format!("{k} no longer exists, so it was cleared from the textures in no set"),
            );
        }
        if cleared {
            // Not `replan_after_edit`: that claims a user edit (it clears the encoding note and the pending type
            // note). The document is marked dirty by `drop_entry`, so the usual save covers it.
            self.replan();
        }
    }

    /// Note DU1-DU2: drop entry `m` of set `id` that matches no file, as one step: a path leaves `add`/`remove`, a
    /// borrow is dropped, and an entry with no files left is deleted (`delete_set`).
    pub fn remove_missing(&mut self, id: &str, m: &Missing) -> Result<(), String> {
        if let Some(why) = self.sets_read_only_reason() {
            return Err(why);
        }
        let id = self.view_of(id).ok_or_else(|| format!("no set {id}"))?.id;
        match m.clone() {
            Missing::NoFilesLeft => {
                self.delete_set(&id);
            }
            Missing::File(f) => self.edit_docs("Remove from set", None, true, move |_, s| {
                s.drop_entry(&id, &f)
            }),
            // F3a Task 8: a locked slot keeps its borrow, dead or not.
            Missing::Borrow { slot, .. } if self.slot_locked(&id, slot) => {
                return Err(SLOT_LOCKED.into());
            }
            // Note JN6: unlike clearing a share that fills its slot (B2-12, `passed_on`), this passes nothing on: the
            // line filled nothing, so the sets sharing through it showed nothing before and still show nothing.
            Missing::Borrow { slot, .. } => {
                self.edit_docs("Remove from set", None, true, move |_, s| {
                    s.set_borrow(&id, slot, None)
                })
            }
        }
        Ok(())
    }

    /// Why `sdc-project.toml` takes no edit, when it does not (the set edits' disabled reason): `None` exactly when
    /// `sets_editable()`. Every greyed set control shows this, never the banner (Note FN3).
    pub fn sets_read_only_reason(&self) -> Option<String> {
        if self.sets_editable() {
            return None;
        }
        Some(match self.sets_doc.as_ref().and_then(|d| d.error()) {
            Some(e) => format!("{READ_ONLY}: {e}"),
            None => READ_ONLY.into(),
        })
    }

    /// Choose set `set`'s shader, or with `None` go back to Auto (M6 spec §7). Plans again.
    pub fn set_shader(&mut self, set: &str, shader: Option<Shader>) {
        if !self.sets_editable() {
            return;
        }
        let id = set.to_string();
        let retype = self.untouched_for_shader(set, shader);
        // Note FC1 R5: a sub-type the plan recorded on its own and nobody changed takes the chosen shader's type in
        // this step (the CLI never reads the set's shader, so `sdc.toml` must name it). Back to Auto it goes, and the
        // next settled plan records what the plan picks then.
        for (k, _, _) in &retype {
            self.history.auto_written.remove(&(k.clone(), "subtype"));
        }
        // Fuzz-9: a value the PBR step clears in this pick was its own, never the user's.
        let step_cleared: HashSet<String> = retype
            .iter()
            .filter(|(_, new, step)| *step && new.is_none())
            .map(|(k, _, _)| k.clone())
            .collect();
        let pick = move |p: &mut crate::project::ProjectDoc, d: &mut crate::sets_doc::SetsDoc| {
            d.set_shader(&id, shader);
            let before = d.auto_pbr();
            let mut auto = before.clone();
            for (k, new, pbr_step) in &retype {
                match new {
                    Some(v) => p.set_file_value(k, "subtype", v),
                    None => p.clear_file_value(k, "subtype"),
                }
                // Fuzz-9: the PBR step's own writes are listed in `auto_pbr`, so leaving PBR (or the diffuse's
                // Revert, `pbr_follow`) finds them; a value it clears leaves the list.
                if !*pbr_step {
                    continue;
                }
                if new.is_none() {
                    auto.retain(|a| a != k);
                } else if !auto.contains(k) {
                    auto.push(k.clone());
                }
            }
            if auto != before {
                d.set_auto_pbr(&auto);
            }
        };
        let Some((item, was, on)) = self.skin_follow(set, shader) else {
            self.edit_docs("Shader", None, true, pick);
            return;
        };
        // Leaving Advanced Skin later (DU7) writes back `was`: not the PBR step's `pbr_albedo` this pick clears.
        let was = if on && step_cleared.contains(&item) {
            String::new()
        } else {
            was
        };
        // The sub-type written before (DQ3 takes the packed output back to `_rmaos`); none was written: the explicit
        // `diffuse` moves the recipe back, then goes, so the sub-type is inferred again. Fuzz-9 (review t6 I1): under
        // an explicit PBR-family pick in a set holding a fuzz map, a `was` Tessalith recorded (`recorded` below) is
        // mapped by `pbr_member_type` (`diffuse` to `pbr_albedo`) as the step's own, listed in `auto_pbr` so leaving
        // PBR reverts it; a hand-written `was` is restored as it is.
        let back: Subtype =
            serde_json::from_value(serde_json::json!(was)).unwrap_or(Subtype::Diffuse);
        let pbr_step = !on
            && shader.is_some_and(Shader::is_pbr_family)
            && self
                .view_of(set)
                .is_some_and(|v| v.members.iter().any(|k| is_fuzz_member(k)));
        // Only a value Tessalith recorded is mapped (re-review t6-r1 N1). DU7's `was` counts as recorded when it is
        // non-empty, the recorder is on (`auto_pick` gives a value), and `was` is what the recorder writes on a colour
        // member: the current pick itself, or `diffuse`/`pbr_albedo` while the current plan suggests `skin_albedo`
        // (the suggestion an explicit Advanced Skin puts on the diffuse). A hand-written value equal to the
        // recorder's is the recorder's (DU8). Under accept off DU7 restores the user's value or clears it, and the
        // engine infers `pbr_albedo` from the written `pbr_fuzz`.
        // The `skin_albedo` ↔ `diffuse`/`pbr_albedo` pair is the colour-member arm of the engine's `suggest.rs`
        // (`Some("d") | None`: skin set, PBR set, else); change it here if that arm changes.
        // shortcut: under High, a hand-written `diffuse` on a diffuse the recorder would have typed `pbr_albedo`
        // before the skin pick is treated as recorded and mapped; upgrade when DU6 stores whether the value it
        // replaced was untouched.
        let picked = self.auto_pick(&item, None);
        let recorded = !was.is_empty()
            && (picked == Some(back)
                || (picked == Some(Subtype::SkinAlbedo)
                    && matches!(back, Subtype::Diffuse | Subtype::PbrAlbedo)));
        let pbr_back = pbr_member_type(back, true).filter(|_| pbr_step && recorded);
        // Note DU6-DU7: the shader and the diffuse's sub-type (with Note DQ's recipe follow) are one step.
        self.batch_edit("Shader", |m| {
            m.edit_docs("Shader", None, true, pick);
            if on {
                m.edit_subtype("Shader", &item, Some(Subtype::SkinAlbedo));
            } else {
                m.edit_subtype("Shader", &item, Some(pbr_back.unwrap_or(back)));
                if was.is_empty() && pbr_back.is_none() {
                    m.edit_docs("Shader", None, true, |p, _| {
                        p.clear_file_value(&item, "subtype")
                    });
                }
            }
            m.edit_docs("Shader", None, true, |_, s| {
                let mut skin = s.auto_skin();
                if on {
                    skin.insert(item.clone(), was.clone());
                } else {
                    skin.remove(&item);
                }
                s.set_auto_skin(&skin);
                // After `edit_subtype_alone` took the key off the list (it reads every edit as the user's).
                let mut auto = s.auto_pbr();
                if pbr_back.is_some() && !auto.contains(&item) {
                    auto.push(item.clone());
                    s.set_auto_pbr(&auto);
                }
            });
        });
    }

    /// Note FC1 R5: set `set`'s members whose recorded sub-type is untouched under the shader being left and whose
    /// pick under `shader` differs (`None`, Auto: the pick without a shader; the plan re-applies an automatic one);
    /// a locked slot's member too, as the engine's shader re-type ignored locks before FC1 (the lock guards its
    /// encoding). Each with the value to write (`None` under Auto, or an untouched `pbr_fuzz` under Advanced Skin,
    /// Note JE2: cleared), and whether it is the PBR step's own (Fuzz-9: it joins `auto_pbr`, or leaves it when
    /// cleared). In a set holding a fuzz map, an explicit PBR-family pick also types the fuzz map `pbr_fuzz` and the
    /// untouched members their PBR counterpart (`pbr_member_type`), and leaving that pick clears what it wrote. A
    /// `skin_albedo` recorded only because of a fuzz map (Fuzz-5) is cleared under any pick but Advanced Skin.
    fn untouched_for_shader(
        &self,
        set: &str,
        shader: Option<Shader>,
    ) -> Vec<(String, Option<serde_json::Value>, bool)> {
        let views = views(self);
        let Some(v) = views.iter().find(|v| same_id(&v.id, set)) else {
            return vec![];
        };
        let now = v.shader.as_ref().map(|p| p.shader);
        let chosen = v
            .shader
            .as_ref()
            .is_some_and(|p| matches!(p.provenance, engine::shader::ShaderOrigin::Set));
        // Auto while already on Auto changes nothing.
        if shader.is_none() && !chosen {
            return vec![];
        }
        let held = |k: &String| {
            self.project
                .as_ref()
                .and_then(|p| p.file_value(k, "subtype"))
        };
        // Note DW1: a pinned diffuse keeps the set from turning skin, so its `_f` stays `pbr_fuzz`.
        let pinned = match v.slots.get(&SlotKind::Diffuse) {
            Some(SlotFill::Own { item }) => locked_items(self).contains_key(item),
            _ => false,
        };
        let skin = shader == Some(Shader::AdvancedSkin) && self.target == Target::Cs && !pinned;
        let stem = |k: &String| {
            Path::new(k)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        };
        let fuzz_map = |k: &String| is_fuzz_member(k);
        // Fuzz-9: only a set holding a fuzz map takes the PBR step; one with its own `_f` keeps the fuzz map
        // untyped (Fuzz-4 lists it unused).
        let has_fuzz = v.members.iter().any(fuzz_map);
        let has_f = v.members.iter().any(|k| {
            engine::group::split_suffix_with(&stem(k), &self.suffix_aliases)
                .suffix
                .as_deref()
                == Some("f")
        });
        let pbr_pick = has_fuzz && shader.is_some_and(Shader::is_pbr_family);
        // Final review B-I2: an explicit pick outside the PBR family, from a set read PBR (by Auto or a pick).
        // Advanced Skin keeps its own diffuse step (`skin_follow`, DU6-DU7).
        let vanilla = |s: Shader| !s.is_pbr_family() && s != Shader::AdvancedSkin;
        let vanilla_pick =
            has_fuzz && now.is_some_and(Shader::is_pbr_family) && shader.is_some_and(vanilla);
        // Its way back: Auto after a chosen vanilla shader clears the untouched vanilla types `vanilla_pick` (or the
        // recorder under that shader) wrote, so the next settled plan records what Auto reads (FC1 R5's Auto rule);
        // the generic arm below keeps a value equal to the vanilla plan's pick.
        let vanilla_back = has_fuzz && chosen && shader.is_none() && now.is_some_and(vanilla);
        let auto_pbr: HashSet<String> = self
            .sets_doc
            .as_ref()
            .map(|s| s.auto_pbr().into_iter().collect())
            .unwrap_or_default();
        let diffuse_slot = |k: &String| matches!(v.slots.get(&SlotKind::Diffuse), Some(SlotFill::Own { item }) if item == k);
        let held_type =
            |k: &String| held(k).and_then(|v| serde_json::from_value::<Subtype>(v).ok());
        let pbr_fuzz = serde_json::json!(Subtype::PbrFuzz);
        // Leaving the pick reverts in a set holding a fuzz map, or one this step wrote: a listed member holding a value
        // only it lists (the fuzz map's `pbr_fuzz`, the Diffuse-slot member's `pbr_albedo`; `pbr_follow` lists
        // neither), so it still runs after the fuzz map left the set. A set with neither, where only `pbr_follow`
        // switched members, is left to `pbr_follow`.
        let step_wrote = v.members.iter().any(|k| {
            auto_pbr.contains(k)
                && held_type(k).is_some_and(|st| {
                    (st == Subtype::PbrFuzz && fuzz_map(k))
                        || (st == Subtype::PbrAlbedo && diffuse_slot(k))
                })
        });
        // `has_fuzz` too (re-review t6-r1 N2): a pick that wrote only `pbr_normal`/`pbr_emissive` (diffuse pinned or
        // borrowed, fuzz map kept or hand-roled) lists no such value, while the set still holds its fuzz map.
        let leaves_pbr = (has_fuzz || step_wrote)
            && chosen
            && now.is_some_and(Shader::is_pbr_family)
            && !shader.is_some_and(Shader::is_pbr_family);
        // Ruling R1: a member in a locked slot keeps its type (the pin guards its encoding), as in `pbr_follow` and
        // DW1.
        let locked = locked_items(self);
        v.members
            .iter()
            .filter_map(|k| {
                // Fuzz-9: the fuzz map the user has typed nothing on, with no hand role and no `_f` beside it, is
                // the PBR fuzz texture. Fuzz-28: not a kept DDS, whose bytes a type cannot change.
                if pbr_pick
                    && !has_f
                    && !locked.contains_key(k)
                    && fuzz_map(k)
                    && held(k).is_none()
                    && self
                        .project
                        .as_ref()
                        .is_some_and(|p| p.file_value(k, engine::helper::ROLE_FIELD).is_none())
                    && self.item(k).is_some_and(|i| !i.kept_dds())
                {
                    return Some((k.clone(), Some(pbr_fuzz.clone()), true));
                }
                // Leaving the pick: what the step wrote and nobody changed since (an edit drops the key from
                // `auto_pbr`, DU8's rule) goes, under Auto or a shader outside the PBR family. A member in a locked
                // slot keeps it (ruling R1; final review B-M1), and its key stays listed.
                if leaves_pbr && auto_pbr.contains(k) && !locked.contains_key(k) {
                    let wrote = held_type(k).is_some_and(|st| {
                        (st == Subtype::PbrFuzz && fuzz_map(k))
                            || [Subtype::Diffuse, Subtype::Normal, Subtype::Glow]
                                .into_iter()
                                .any(|from| pbr_member_type(from, diffuse_slot(k)) == Some(st))
                    });
                    if wrote {
                        return Some((k.clone(), None, true));
                    }
                }
                if !self.untouched_subtype(k, now) {
                    return None;
                }
                // Fuzz-5: a `skin_albedo` an older Tessalith recorded because of the fuzz map goes under any pick but
                // Advanced Skin; the engine then infers the new pick's type, and `sdc convert` plans the same from
                // the cleared text. It runs before the pick stores the shader (origin not yet `Set`).
                if shader != Some(Shader::AdvancedSkin) && self.fuzz_recorded_skin(k) {
                    return Some((k.clone(), None, false));
                }
                // Note JE2: under Advanced Skin an untouched `pbr_fuzz` is the skin fuzz mask, which packs into the
                // `_rfaos` G channel only while no sub-type is written (Note DR2): clear it. shortcut: a set with a
                // `_fuzz`/`_sheen` map keeps its `_f` as `pbr_fuzz` (DR7), so the next plan records it again in a
                // step of its own; check the set's raw maps here if that shows up.
                if skin && held(k) == Some(pbr_fuzz.clone()) {
                    return Some((k.clone(), None, false));
                }
                // Fuzz-9, Fuzz-27: `member_subtype` maps no `diffuse`, `normal` or `glow` under a PBR-family shader,
                // so the untouched members take their PBR counterpart here (an untouched `normal` bypasses B3's
                // Use PBR normal: the pick is the user's act, and leaving it reverts the type).
                if pbr_pick && !locked.contains_key(k) {
                    if let Some(st) =
                        held_type(k).and_then(|st| pbr_member_type(st, diffuse_slot(k)))
                    {
                        return Some((k.clone(), Some(serde_json::json!(st)), true));
                    }
                }
                // Final review B-I2: the mirror for an explicit pick outside the PBR family. `member_subtype` keeps a
                // PBR type under a vanilla shader (JQ16), so an untouched PBR type FC1 recorded while the set was
                // read PBR takes its vanilla counterpart here. Not listed in `auto_pbr`: the value is what the new
                // shader's recorder writes (DU8), and a later PBR pick maps it back through the arm above.
                if vanilla_pick && !locked.contains_key(k) {
                    if let Some(st) =
                        held_type(k).and_then(|st| vanilla_member_type(st, diffuse_slot(k)))
                    {
                        return Some((k.clone(), Some(serde_json::json!(st)), false));
                    }
                }
                if vanilla_back
                    && !locked.contains_key(k)
                    && held_type(k).is_some_and(|st| pbr_member_type(st, diffuse_slot(k)).is_some())
                {
                    return Some((k.clone(), None, false));
                }
                let new = self.auto_pick(k, shader).map(|st| serde_json::json!(st));
                (new.is_some() && new != held(k))
                    .then(|| (k.clone(), new.filter(|_| shader.is_some()), false))
            })
            .collect()
    }

    /// Note DU6-DU9: what choosing `shader` for set `set` does to its own diffuse: `(item, its own [files] sub-type
    /// before, true)` to make it `skin_albedo` (Advanced Skin, on cs, a free diffuse whose sub-type is inferred, a
    /// plain `diffuse` or an untouched recorded one), or `(item, the recorded sub-type, false)` to revert one Tessalith made `skin_albedo` and the
    /// user has not changed since. `None`: the shader alone.
    fn skin_follow(&self, set: &str, shader: Option<Shader>) -> Option<(String, String, bool)> {
        let views = views(self);
        let v = views.iter().find(|v| same_id(&v.id, set))?;
        let SlotFill::Own { item } = v.slots.get(&SlotKind::Diffuse)? else {
            return None;
        };
        let p = self.project.as_ref().filter(|p| !p.read_only())?;
        let s = self.sets_doc.as_ref()?;
        // F3a Task 8: a pinned diffuse keeps its sub-type both ways (review issue 2).
        if locked_items(self).contains_key(item) {
            return None;
        }
        let own = p.file_value(item, "subtype");
        if shader == Some(Shader::AdvancedSkin) {
            if self.target != Target::Cs {
                return None;
            }
            let job = self.item(item)?.job.as_ref()?;
            let inferred = job.provenance.get("subtype") == Some(&Provenance::Suggestion);
            let plain = job.profile.subtype == Subtype::Diffuse;
            // Note FC1 R5: a sub-type the plan recorded on its own (`pbr_albedo`) and nobody changed counts as
            // inferred; `was` keeps it, so leaving Advanced Skin writes it back (DU7).
            let untouched = self.untouched_subtype(item, v.shader.as_ref().map(|p| p.shader));
            if job.profile.subtype == Subtype::SkinAlbedo || !(inferred || plain || untouched) {
                return None;
            }
            let was = own.as_ref().and_then(|v| v.as_str()).unwrap_or("");
            return Some((item.clone(), was.to_string(), true));
        }
        let was = s.auto_skin().get(item)?.clone();
        (own == Some(serde_json::json!(Subtype::SkinAlbedo))).then(|| (item.clone(), was, false))
    }

    /// F3b D2/D3: rename set `set`'s outputs to `name` (`None` restores the names that mirror the sources). Only a
    /// PBR set on CS takes a name (Note HJ1): it writes its PGPatcher `rename` key, a file name or a whole path
    /// (HJ5; `rename_value`); the outputs and the JSON follow the prefix. Any other set refuses a name; `None` still
    /// clears an old `output_name` (HJ2). A re-plan follows; inside `batch_edit` it joins the batch.
    pub fn set_output_name(&mut self, set: &str, name: Option<&str>) -> Result<(), String> {
        if !self.sets_editable() {
            return Err(READ_ONLY.into());
        }
        let Some(v) = self.view_of(set) else {
            return Err(format!("no set {set}"));
        };
        let name = name.map(strip_lead).filter(|n| !n.is_empty());
        let id = v.id.clone();
        if self.renames_outputs(&v) {
            let value = name
                .map(|n| rename_value(&v, n).map(Value::String))
                .transpose()?;
            self.edit_docs("Rename outputs", None, true, move |_, d| {
                d.set_value(&id, "rename", value);
                d.set_output_name(&id, None);
            });
        } else if name.is_none() {
            // An old `output_name` (HJ2) can still be cleared.
            self.edit_docs("Rename outputs", None, true, move |_, d| {
                d.set_output_name(&id, None)
            });
        } else {
            return Err("output names are for PGPatcher PBR sets".into());
        }
        Ok(())
    }

    /// F3b: set `set`'s `output_name`, if it has one (on a CS PBR set it is ignored; the set editor says so).
    pub fn output_name_of(&self, set: &str) -> Option<String> {
        let entries = self.sets_doc.as_ref()?.entries();
        entry_of(&entries, set)?.output_name.clone()
    }

    /// F3b: remove set `set`'s `output_name` only (its PGPatcher `rename` stays).
    pub fn clear_output_name(&mut self, set: &str) {
        if !self.sets_editable() {
            return;
        }
        let id = set.to_string();
        self.edit_docs("Clear output name", None, true, move |_, d| {
            d.set_output_name(&id, None)
        });
    }

    /// Several keys of one set as one undo step and one plan (the Subsurface selector writes two). One key keeps
    /// `set_json_value`'s merge with the edit before it.
    pub fn set_json_values(&mut self, set: &str, edits: Vec<(String, Option<Value>)>) {
        if let [(k, v)] = edits.as_slice() {
            return self.set_json_value(set, k, v.clone());
        }
        if edits.is_empty() || !self.sets_editable() {
            return;
        }
        let label = format!(
            "Change {}",
            edits
                .iter()
                .map(|(k, _)| k.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
        self.batch_edit(&label, |m| {
            for (k, v) in edits {
                m.set_json_value(set, &k, v);
            }
        });
    }

    /// Note FN5: turn feature `f` of set `set` off as a step of its own, so the click never joins a hand edit of the
    /// same key just before it (`set_json_value` merges per set and key) and one undo brings the feature back.
    pub fn turn_feature_off(&mut self, set: &str, f: crate::pgp::settings::Feature) {
        let key = crate::pgp::settings::feature_key(f);
        let label = format!("Turn off {}", key.trim_end_matches("_on"));
        self.batch_edit(&label, |m| {
            m.set_json_value(set, key, Some(Value::Bool(false)))
        });
    }

    /// A JSON value of set `set`. Material values do not plan; match fields do (P7).
    pub fn set_json_value(&mut self, set: &str, key: &str, v: Option<Value>) {
        if !self.sets_editable() {
            return;
        }
        // DECISIONS AF6: a locked slot's command is changed by nothing, the JSON editor's x on its pill included.
        let slot = key.strip_prefix("slot").and_then(|n| n.parse::<u8>().ok());
        if slot.is_some_and(|n| self.slot_locked(set, n)) {
            return;
        }
        let replan = replans(key);
        let v = match v {
            Some(Value::String(t)) if is_path_key(key) => Some(Value::from(strip_lead(&t))),
            v => v,
        };
        let (s_id, k) = (set.to_string(), key.to_string());
        let merge = Some(format!("{set}:{key}"));
        self.edit_docs(&format!("Change {key}"), merge, replan, move |_, d| {
            d.set_value(&s_id, &k, v)
        });
    }

    /// A slot command (spec §4.2). A path is checked first; the reason comes back when it is refused. One step each
    /// (no merge: F3a Task 8), refused on a locked slot.
    pub fn set_slot_command(
        &mut self,
        set: &str,
        slot: u8,
        cmd: SlotCommand,
    ) -> Result<(), String> {
        if !self.sets_editable() {
            return Err(READ_ONLY.into());
        }
        if !(1..=8).contains(&slot) {
            return Err(format!("slot{slot}: there are slots 1 to 8"));
        }
        if self.slot_locked(set, slot) {
            return Err(SLOT_LOCKED.into());
        }
        let key = format!("slot{slot}");
        // Note HC7: `slotN` alone leaves a borrow in the slot. The plan reads borrows (the bake), so this one re-plans.
        let borrower = self.view_of(set).filter(|v| v.borrows_slot(slot));
        if let (Some(v), SlotCommand::FromSet | SlotCommand::Clear) = (borrower, &cmd) {
            // Note HG: a record fill is turned off, not written as a setting (`slotN = ""` is a PGPatcher command).
            if v.filled.contains_key(&slot) {
                let id = v.id;
                self.edit_docs(
                    &format!("Clear slot {slot} of {id}"),
                    None,
                    true,
                    move |_, d| d.set_no_fill(&id, slot, true),
                );
                return Ok(());
            }
            let to = self.passed_on(&v, slot);
            let id = v.id;
            self.edit_docs(
                &format!("Clear slot {slot} of {id}"),
                None,
                true,
                move |_, d| {
                    d.set_borrow(&id, slot, None);
                    d.set_value(&id, &key, None);
                    // Note JN4 (B2-12): a line that passed a chain on passes its sharing sets on, in this step.
                    if let Some(x) = &to {
                        d.retarget_slot_borrows(&id, slot, x);
                    }
                },
            );
            return Ok(());
        }
        let v = match cmd {
            SlotCommand::FromSet => None,
            SlotCommand::Clear => Some(Value::from("")),
            SlotCommand::Path(p) => {
                let p = strip_lead(&p);
                check_rel_path(p, &key).map_err(|e| e.0)?;
                if !p.to_ascii_lowercase().ends_with(".dds") {
                    return Err(format!("{key}: the path must end in .dds"));
                }
                Some(Value::from(p.replace('/', "\\")))
            }
        };
        let id = set.to_string();
        self.edit_docs(&format!("Change {key}"), None, false, move |_, d| {
            d.set_value(&id, &key, v)
        });
        Ok(())
    }

    /// Item 17 (DECISIONS AG98): files dropped from Explorer on slot card `slot` of set `set`. The first file, when it
    /// is a texture of the open folder that fills that slot, moves in as the Sets column's move does (`drop_choice`
    /// with Move: Replace / Cancel on a filled slot, refused on a locked one). Nothing is ever copied: a file outside
    /// the folder is refused, and Share or Duplicate are not offered.
    pub fn drop_file_on_slot(
        &mut self,
        set: &str,
        slot: SlotKind,
        paths: &[PathBuf],
    ) -> SlotFileDrop {
        let note = (paths.len() > 1).then_some(ONLY_FIRST_FILE);
        let refused = |why: String| SlotFileDrop {
            item: None,
            outcome: DropOutcome::Refused(why),
            note,
        };
        let Some(first) = paths.first() else {
            return refused("nothing was dropped".into());
        };
        let name = first.file_name().map_or_else(
            || first.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        // Explorer's case can differ from the folder walk's: compare without case, `/` for `\`.
        let norm = |p: &Path| p.to_string_lossy().replace('\\', "/").to_lowercase();
        let rel = self.root.as_deref().and_then(|r| {
            let (r, f) = (norm(r), norm(first));
            f.strip_prefix(&format!("{}/", r.trim_end_matches('/')))
                .map(str::to_string)
        });
        let Some(rel) = rel else {
            return refused(format!(
                "{name} is outside the open folder; files are never copied"
            ));
        };
        let Some(it) = self.items.iter().find(|i| i.key.to_lowercase() == rel) else {
            return refused(format!("{name} is not a texture of the open folder"));
        };
        let key = it.key.clone();
        match slot_of(it) {
            // Note AO: a card is a PGPatcher slot; slot 7's takes `_cnr` or `_f`.
            Some(k) if k.shares_card(slot) => {}
            Some(k) => {
                return refused(format!(
                    "{name} goes in the {} slot, not {}",
                    k.label(),
                    slot.label()
                ))
            }
            None => {
                return refused(format!(
                    "{name} fills no slot row; drag it onto the set in the Sets column"
                ))
            }
        }
        let outcome = if self.set_of_item(&key).is_some_and(|f| same_id(&f.id, set)) {
            DropOutcome::Joined
        } else if let Some(why) = self.view_of(set).and_then(|t| {
            // The slot 3/7 clash guard `drop_on_set` applies (AG99).
            slot_conflict(&t.slots.keys().copied().collect::<Vec<_>>(), slot_of(it)?)
        }) {
            DropOutcome::Refused(why)
        } else {
            self.drop_choice(&key, set, DropChoice::Move)
        };
        SlotFileDrop {
            item: Some(key),
            outcome,
            note,
        }
    }

    /// Item 18 (DECISIONS AG99): move the ticked proposals of `fill_proposals` into set `set` as one step and one
    /// plan, each emptying its slot first. A blocked row (a locked slot, …) is never applied. Returns how many moved.
    pub fn fill_from_names(&mut self, set: &str, ticked: &[String]) -> Result<usize, String> {
        if !self.sets_editable() {
            return Err(READ_ONLY.into());
        }
        let v = self.view_of(set).ok_or_else(|| format!("no set {set}"))?;
        // (item, slot, what fills that slot now, the slot the item lends).
        let moves: Vec<_> = fill_proposals(self, set)
            .into_iter()
            .filter(|r| r.blocked.is_none() && ticked.contains(&r.item))
            .map(|r| {
                let lent = self.lent_slot(&r.item);
                (r.item, r.slot, v.slots.get(&r.slot).cloned(), lent)
            })
            .collect();
        if moves.is_empty() {
            return Err("nothing to fill".into());
        }
        let (id, n) = (v.id.clone(), moves.len());
        self.edit_docs(&format!("Fill {id} from names"), None, true, move |_, s| {
            for (item, slot, existing, lent) in &moves {
                clear_slot(s, &id, Some(*slot), existing);
                s.add_member(&id, item);
                // Note JN2 (B2-1): the sets sharing the moved file's slot follow it, in this step (as HF4/HS13 do for
                // a rename).
                if let Some((l, n)) = lent {
                    s.retarget_slot_borrows(l, *n, &id);
                }
            }
        });
        Ok(n)
    }
}

/// Item 17: what a file drop on a slot card did. `item` is the texture it named, when it is one of the folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotFileDrop {
    pub item: Option<String>,
    pub outcome: DropOutcome,
    pub note: Option<&'static str>,
}

/// Item 17: several files dropped on one slot.
pub const ONLY_FIRST_FILE: &str = "only the first file was used";

/// Item 18: one row of the Fill from names dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FillProposal {
    pub item: String,
    /// The file name.
    pub name: String,
    pub slot: SlotKind,
    /// The file name of what fills that slot now.
    pub replaces: Option<String>,
    /// Why the row cannot be ticked: "locked" for a locked slot, else the move's own refusal.
    pub blocked: Option<String>,
}

/// Item 18 (DECISIONS AG99): the textures of set `set`'s folder that are not its members and whose name is the set's
/// base name plus a tail the engine recognises (its §6.1 suffixes and §4.4 vocabulary, through `base_of`), each with
/// the slot its sub-type fills. By item key; a second file for one slot is blocked.
pub fn fill_proposals(m: &AppModel, set: &str) -> Vec<FillProposal> {
    let views = views(m);
    let Some(v) = views.iter().find(|v| same_id(&v.id, set)) else {
        return vec![];
    };
    // The base name of the diffuse's file (a renamed set keeps its files' names), else of the set's id.
    let src = match v.slots.get(&SlotKind::Diffuse) {
        Some(SlotFill::Own { item }) => {
            let stem = Path::new(item)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy();
            format!("{}/{}", dir_of(item), base_of(&stem))
        }
        _ => v.id.clone(),
    };
    let (dir, base) = (dir_of(&src), src.rsplit('/').next().unwrap_or(&src));
    let mut cands: Vec<&TextureItem> = m
        .items
        .iter()
        .filter(|i| i.kind != ItemKind::Packed && !v.members.contains(&i.key))
        .filter(|i| dir_of(&i.key).eq_ignore_ascii_case(dir))
        .filter(|i| {
            let stem = Path::new(&i.key)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy();
            stem.len() > base.len() && base_of(&stem).eq_ignore_ascii_case(base)
        })
        .collect();
    cands.sort_by(|a, b| a.key.cmp(&b.key));
    let name_of = |k: &str| m.item(k).map_or(k.to_string(), |i| i.name.clone());
    // Grows with each accepted row, so a batch cannot add both halves of a slot 3/7 clash (AG99).
    let mut existing: Vec<SlotKind> = v.slots.keys().copied().collect();
    let mut taken = BTreeSet::new();
    cands
        .into_iter()
        .filter_map(|i| {
            let slot = slot_of(i)?;
            let replaces = v.slots.get(&slot).and_then(SlotFill::item).map(name_of);
            let blocked = if slot.slot_number().is_some_and(|n| v.locks.contains(&n)) {
                Some("locked".to_string())
            } else if !taken.insert(slot) {
                Some("another file above takes this slot".to_string())
            } else {
                // The move's own checks, as `drop_choice` makes them (a packed occupant, leaving a locked slot, …).
                slot_conflict(&existing, slot).or_else(|| {
                    m.occupant(v, Some(slot), None)
                        .and_then(|_| m.choice_allowed(&i.key, v, Some(slot), DropChoice::Move))
                        .err()
                })
            };
            if blocked.is_none() {
                existing.push(slot);
            }
            Some(FillProposal {
                item: i.key.clone(),
                name: i.name.clone(),
                slot,
                replaces,
                blocked,
            })
        })
        .collect()
}

/// Whether `item` writes an output of its own: it has a job and is neither a raw packing source nor a helper. A
/// record-only slot (`_s` backlight, env mask, cubemap) has no `slot_of` but writes a file like any other, so the
/// output key must not depend on the slot.
fn writes_own_output(item: &TextureItem) -> bool {
    item.job.is_some()
        && !matches!(item.status, ItemStatus::PackedInto(_))
        && item.kind != ItemKind::Helper
}

/// The engine key of the output `item` writes: its file, or its set and recipe for a packed output. `None` for an
/// item that writes no output of its own.
pub fn item_output_key(views: &[SetView], item: &TextureItem) -> Option<OutputKey> {
    writes_own_output(item).then_some(())?;
    if item.kind != ItemKind::Packed {
        return Some(OutputKey::File(item.key.clone()));
    }
    let suffix = recipe_suffix(item)?;
    let set = views
        .iter()
        .find(|v| {
            v.slots
                .values()
                .any(|f| matches!(f, SlotFill::Own { item: i } if *i == item.key))
        })?
        .id
        .clone();
    Some(OutputKey::Packed { set, suffix })
}

/// Item key → the id of the first view, in display order, holding it as its own: the set `item_output_key` names for
/// a packed output, built once for a loop over every item.
pub(crate) fn packed_owners(views: &[SetView]) -> HashMap<&str, &str> {
    let mut out = HashMap::new();
    for v in views {
        for f in v.slots.values() {
            if let SlotFill::Own { item } = f {
                out.entry(item.as_str()).or_insert(v.id.as_str());
            }
        }
    }
    out
}

/// `item_output_key` with the owners `packed_owners` built.
pub(crate) fn item_output_key_in(
    owners: &HashMap<&str, &str>,
    item: &TextureItem,
) -> Option<OutputKey> {
    writes_own_output(item).then_some(())?;
    if item.kind != ItemKind::Packed {
        return Some(OutputKey::File(item.key.clone()));
    }
    let suffix = recipe_suffix(item)?;
    let set = owners.get(item.key.as_str())?.to_string();
    Some(OutputKey::Packed { set, suffix })
}

/// `build_views`'s sort as it was before `sort_by_saved_order`: a linear `same_id` lookup per comparison.
#[cfg(test)]
mod order_tests {
    use super::*;

    fn linear_sort(views: &mut [SetView], order: &[String]) {
        let pos = |v: &SetView| {
            order
                .iter()
                .position(|o| same_id(o, &v.id))
                .unwrap_or(usize::MAX)
        };
        views.sort_by(|a, b| pos(a).cmp(&pos(b)).then_with(|| a.id.cmp(&b.id)));
    }

    /// Audit §1 item 2 (Note FM7): the folded-map sort gives the order the linear one gave, on a dragged order with
    /// Unicode-folded, back-slashed and slash-wrapped ids, an id listed twice, ids that are not listed, and views
    /// whose ids differ only by case.
    #[test]
    fn the_saved_order_sort_equals_the_linear_one() {
        let names = [
            "Äpfel/Stein",
            "ÄPFEL/stein",
            "äpfel\\Holz",
            "Öl/Fass",
            "landscape/rock01",
            "Landscape/Rock02",
            "armor/Ωmega",
            "armor/ωmega",
            "/trim/lead",
            "zeta",
            "Alpha",
            "alpha",
        ];
        let mut ids: Vec<String> = names.iter().map(|s| (*s).to_string()).collect();
        ids.extend((0..60).map(|i| format!("mod{}/Set{}", i % 7, i * 31 % 17)));
        let mut state = 0x2545_f491_4f6c_dd1du64;
        let mut next = move |n: usize| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state % n as u64) as usize
        };
        for round in 0..20 {
            // A dragged order: some ids (shuffled, some folded differently, one twice), then ids no view has.
            let mut order: Vec<String> = ids.iter().filter(|_| next(3) != 0).cloned().collect();
            for i in (1..order.len()).rev() {
                order.swap(i, next(i + 1));
            }
            if let Some(o) = order.first().cloned() {
                order.push(o.to_uppercase());
            }
            order.extend(["gone/one".to_string(), "Gone\\Two".to_string()]);
            let mut views: Vec<SetView> = ids
                .iter()
                .map(|id| SetView::new(String::new(), id.clone(), vec![]))
                .collect();
            for i in (1..views.len()).rev() {
                views.swap(i, next(i + 1));
            }
            let mut old = views.clone();
            linear_sort(&mut old, &order);
            sort_by_saved_order(&mut views, &order);
            let ids_of = |v: &[SetView]| v.iter().map(|s| s.id.clone()).collect::<Vec<_>>();
            assert_eq!(ids_of(&views), ids_of(&old), "round {round}: {order:?}");
        }
    }
}

#[cfg(test)]
mod badge_tests {
    use super::*;
    use engine::shader::ShaderOrigin;

    fn pbr_set(json: bool) -> SetView {
        let mut v = SetView::new("k".into(), "a/b".into(), vec![]);
        v.json = json;
        v.shader = Some(ShaderPick {
            shader: Shader::Pbr,
            provenance: ShaderOrigin::Set,
            wanted: None,
        });
        v
    }

    #[test]
    fn a_pbr_pick_with_nothing_filled_does_not_blame_the_target() {
        let mut m = super::super::tests::bare_model();
        let v = pbr_set(false);
        m.target = Target::Cs;
        assert_eq!(badge_text(&m, &v), BADGE_PBR_NOTHING_TO_WRITE);
        assert_eq!(kind_text(&m, &v), "PBR, no JSON");
        m.target = Target::Vanilla;
        assert_eq!(badge_text(&m, &v), "PBR set (no JSON on this target)");
        assert_eq!(kind_text(&m, &v), "PBR, no JSON");
    }

    /// A no-albedo PBR set (a JSON set, Note HL5) reads "PBR, no JSON" off Community Shaders and "PBR → JSON" on it.
    #[test]
    fn a_no_albedo_pbr_set_reads_no_json_off_cs() {
        let mut m = super::super::tests::bare_model();
        let v = pbr_set(true);
        m.target = Target::Vanilla;
        assert_eq!(kind_text(&m, &v), "PBR, no JSON");
        assert_eq!(badge_text(&m, &v), "PBR set (no JSON on this target)");
        m.target = Target::Cs;
        assert_eq!(kind_text(&m, &v), "PBR → JSON");
    }
}

#[cfg(test)]
mod lost_share_tests {
    use super::*;

    fn lost(sets: &[&str], files: &[&str], slots: usize) -> LostShares {
        let own = |v: &[&str]| v.iter().map(|s| (*s).to_string()).collect();
        LostShares {
            sets: own(sets),
            files: own(files),
            keys: vec![],
            slots,
        }
    }

    /// Note JN4: one set that lost several slots says "those slots" with its id; more than three sets are cut to
    /// three names and "N more".
    #[test]
    fn the_lost_share_sentence_follows_the_counts() {
        let one = lost(&["a/one"], &["x.png", "y"], 2);
        assert_eq!(
            lost_share_text(&one, &list_ids(&one.files)).as_deref(),
            Some("a/one shared x.png and y; those slots are empty now.")
        );
        let five = lost(&["s/1", "s/2", "s/3", "s/4", "s/5"], &["x.png"], 5);
        assert_eq!(
            lost_share_text(&five, "it").as_deref(),
            Some("5 sets shared it; those slots are empty now: s/1, s/2, s/3 and 2 more.")
        );
        assert_eq!(lost_share_text(&lost(&[], &[], 0), "it"), None);
    }
}
