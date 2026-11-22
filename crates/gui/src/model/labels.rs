//! Display names for texture types and schema keys.

use super::also;
use super::sets::{SetView, SlotKind};
use engine::config::Subtype;
use engine::shader::Shader;

pub struct TypeInfo {
    pub label: &'static str,
    pub description: &'static str,
    pub suffix: &'static str,
}

pub fn type_info(st: Subtype) -> TypeInfo {
    use Subtype::*;
    let (label, description, suffix) = match st {
        Diffuse => ("Diffuse", "The colour map of an ordinary (non-PBR) material.", "none, or _d"),
        LandscapeDiffuse => ("Landscape diffuse", "A landscape colour map; its alpha is the snow or height mask.", "none, under a landscape folder"),
        PbrAlbedo => ("PBR albedo", "The base colour of a Community Shaders PBR material.", "none, in a PBR set"),
        PbrEmissive => ("PBR emissive", "The glow of a PBR material; a float source brighter than white becomes HDR.", "_g, in a PBR set"),
        Glow => ("Glow", "The glow map of an ordinary material.", "_g"),
        PbrSubsurface => ("PBR subsurface or coat colour", "The subsurface colour of a PBR material, or its coat colour.", "_s, in a PBR set"),
        PbrFuzz => ("PBR fuzz", "The fuzz (sheen) colour of a PBR material, as on cloth.", "_f, _fuzz, _sheen in PBR; in Advanced Skin only _f beside _fuzz or _sheen stays PBR fuzz"),
        SubsurfaceTint => ("Subsurface tint", "The subsurface colour map the vanilla shaders read. ENB users often make it black to turn the effect off. Community Shaders ignores it unless Advanced skin is on, and Advanced skin takes Skin subsurface mask instead.", "_s"),
        BacklightTint => ("Backlight tint", "The colour that shows when light comes from behind, as through leaves.", "_b, _bl"),
        UiSky => ("Interface or sky", "A menu or sky image, written without mipmaps.", "none, under interface or sky"),
        SkinAlbedo => ("Skin albedo", "The skin colour of an Advanced skin set.", "none, in an Advanced skin set"),
        ComplexGrass => ("Complex grass", "Diffuse and normal combined in one texture, because grass has no separate normal texture in the base game. For CS and ENB complex grass.", "none (picked by hand or from the GRAS record)"),
        Cubemap => ("Cubemap", "A six-sided reflection map.", "_e, or under textures\\cubemaps"),
        Normal => ("Normal map", "The surface bumps of an ordinary material (tangent space).", "_n"),
        LandscapeNormal => ("Landscape normal", "The normal map of a landscape texture.", "_n, under a landscape folder"),
        PbrNormal => ("PBR normal", "The normal map of a PBR material. The default drops specular alpha. Pick Data on the Alpha row to keep it.", "_n, in a PBR set"),
        DetailNormal => ("Detail normal", "A landscape set's fine detail normal.", "_detail, _detailnorm, _detail_n in a Landscape or PBR Landscape set"),
        Msn => ("Model-space normal", "A normal map baked in the model's own space, as for vanilla skin and eyes.", "_msn"),
        CoatNormal => ("PBR coat normal", "The normal of a PBR coat layer, with the coat roughness in alpha.", "_cnr, _crn"),
        SkinWet => ("Skin wet", "The wetness normal of an Advanced skin set.", "_wet, in an Advanced skin set"),
        SkinDetail => ("Skin detail", "A skin detail normal with a strength mask in alpha.", "_detail, _detailnorm, _detail_n in an Advanced skin set"),
        Envmask => ("Environment mask", "A grey mask that sets where the cubemap shines.", "_m, _em, _envmask"),
        Height => ("Height (parallax)", "A grey height map for parallax. White is raised.", "_p"),
        ProjectedNoise => ("Projected noise", "A grey noise map for a landscape profile's projected detail.", "_noise"),
        MsnSpecular => ("Model-space specular", "The grey glossiness beside a model-space normal.", "_s, beside an _msn"),
        SkinSk => ("Skin subsurface mask", "A greyscale thickness (density) map for the subsurface of an Advanced skin set.", "_sk, in an Advanced skin set"),
        Gray => ("Grey map", "A plain grey map you pick by hand.", "none (picked by hand)"),
        ComplexMaterial => ("Complex material", "An _m that packs four maps for complex material: env mask in red, glossiness in green, metallic in blue and parallax height in alpha.", "_m with a varying alpha"),
        Rmaos => ("PBR RMAOS", "A PBR packed map: roughness, metallic, AO and specular in R, G, B and A.", "_rmaos"),
        SkinRfaos => ("Skin RFAOS", "An Advanced skin packed map: roughness, fuzz, AO and specular.", "_rfaos"),
        Flow => ("Hair flow", "The direction hair strands run, for the CS hair shader.", "_flow"),
        GenericTwoChannel => ("Two-channel data", "A data map that only uses red and green.", "none (picked by hand)"),
        GenericData => ("Other data", "A data map Tessalith has no type for. Stored as it is.", "none (picked by hand)"),
        HdrSky => ("HDR sky", "A sky image brighter than white, written as BC6H.", "none, a float source under sky"),
    };
    TypeInfo {
        label,
        description,
        suffix,
    }
}

pub const OWN_SET_HOVER: &str =
    "A file alone under its base name becomes a set of its own once it has a type.";

pub fn type_hover(st: Subtype) -> String {
    let info = type_info(st);
    let name = match info.suffix.strip_prefix("none ") {
        Some(reason) => format!("No file ending gives this type {reason}"),
        None => format!("File name: {}", info.suffix),
    };
    format!("{}\n{name}\n{OWN_SET_HOVER}", info.description)
}

pub fn subtype_name(st: Subtype) -> String {
    type_info(st).label.to_string()
}

/// What a set card's chip for `kind` is, in the words of the type table: the plain label of an ordinary set, `pbr`'s
/// PBR names, `skin`'s Advanced skin names (the three differ for the diffuse, emissive and subsurface only, and the
/// normal reads PBR in both). `rfaos` makes the Rmaos slot the skin recipe's pack.
pub fn slot_type_label(kind: SlotKind, pbr: bool, skin: bool, rfaos: bool) -> &'static str {
    match kind {
        SlotKind::Diffuse if skin => "Skin albedo",
        SlotKind::Diffuse if pbr => "PBR albedo",
        SlotKind::Diffuse => "Diffuse",
        SlotKind::Normal if pbr || skin => "PBR normal",
        SlotKind::Normal => "Normal map",
        SlotKind::Emissive if pbr && !skin => "PBR emissive",
        SlotKind::Emissive => "Glow",
        SlotKind::SkinSk => "Skin subsurface mask",
        SlotKind::Height => "Height (parallax)",
        SlotKind::Rmaos if rfaos => "Skin RFAOS (roughness, fuzz, AO, specular)",
        SlotKind::Rmaos => "PBR RMAOS (roughness, metallic, AO, specular)",
        SlotKind::Coat => "PBR coat normal",
        SlotKind::Fuzz => "PBR fuzz",
        SlotKind::Subsurface if pbr && !skin => "PBR subsurface or coat colour",
        SlotKind::Subsurface => "Subsurface tint",
        SlotKind::ProjectedNoise => "Projected noise",
        SlotKind::DetailNormal => "Detail normal",
        SlotKind::SkinWet => "Skin wet",
        SlotKind::SkinDetail => "Skin detail",
    }
}

/// `slot_type_label` for `kind` on set `v`. The resolved shader decides (an Advanced skin set takes its own column);
/// a card with no files yet reads its saved shader; only a view with neither falls back to the member flags.
pub fn set_slot_label(v: &SetView, kind: SlotKind) -> &'static str {
    let document_only = v.shader.is_none() && v.key.is_empty();
    let pbr = if v.shader.is_some() {
        also::is_pbr_set(v)
    } else if document_only {
        also::document_pbr_set(v)
    } else {
        v.pbr
    };
    let saved = if document_only { v.shader_choice } else { None };
    let skin = match (v.shader.as_ref(), saved) {
        (Some(p), _) => p.shader == Shader::AdvancedSkin,
        (None, Some(saved)) => saved == Shader::AdvancedSkin,
        (None, None) => v.skin,
    };
    slot_type_label(kind, pbr, skin, v.rfaos)
}

/// The "?" legend beside the Sets heading, one line per entry, each "{name}: {meaning}".
pub const LEGEND: [&str; 18] = [
    "D: Diffuse, PBR albedo or Skin albedo",
    "N: Normal map or PBR normal",
    "G: Glow or PBR emissive",
    "SK: Skin subsurface mask",
    "P: Height (parallax)",
    "RMAOS: PBR RMAOS (roughness, metallic, AO, specular)",
    "RFAOS: Skin RFAOS (roughness, fuzz, AO, specular)",
    "CNR: PBR coat normal",
    "F: PBR fuzz",
    "S: Subsurface tint, or PBR subsurface or coat colour",
    "NOISE: Projected noise",
    "DETAIL: Detail normal",
    "WET: Skin wet",
    "DETAIL_N: Skin detail",
    "PBR → JSON: a PBR set that writes a PGPatcher JSON entry.",
    "unused: a helper that fills no slot of this set.",
    "(auto): Tessalith picked this shader from the set's files.",
    "Empty slots: the letters shown while you drag have no file yet.",
];

/// Words kept upper case (compared lower case).
const INITIALISMS: [&str; 5] = ["ao", "uv", "rgb", "sss", "f0"];

/// A schema key as a label. CamelCase and snake_case split into words, first letter upper, the rest lower, known
/// initialisms kept upper ("AO", "UV", "RGB", "SSS", "F0").
pub fn readable(key: &str) -> String {
    let mut words: Vec<String> = vec![];
    for part in key.split('_').filter(|p| !p.is_empty()) {
        let cs: Vec<char> = part.chars().collect();
        let mut cur = String::new();
        for (i, &c) in cs.iter().enumerate() {
            let prev = i.checked_sub(1).map(|j| cs[j]);
            let next = cs.get(i + 1).copied();
            let split = c.is_uppercase()
                && prev.is_some_and(|p| {
                    p.is_lowercase() || (p.is_uppercase() && next.is_some_and(char::is_lowercase))
                });
            if split {
                words.push(std::mem::take(&mut cur));
            }
            cur.push(c);
        }
        words.push(cur);
    }
    let mut out = String::new();
    for (i, w) in words.iter().enumerate() {
        let lw = w.to_lowercase();
        let word = if INITIALISMS.contains(&lw.as_str()) {
            lw.to_uppercase()
        } else if i == 0 {
            let mut c = lw.chars();
            c.next()
                .map(|f| f.to_uppercase().chain(c).collect())
                .unwrap_or_default()
        } else {
            lw
        };
        if i > 0 {
            out.push(' ');
        }
        out.push_str(&word);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::shader::{ShaderOrigin, ShaderPick};

    fn pick(shader: Shader) -> ShaderPick {
        ShaderPick {
            shader,
            provenance: ShaderOrigin::Set,
            wanted: None,
        }
    }

    /// The label follows the resolved shader (the answer `also::is_pbr_set` gives), the saved one on a card with no
    /// files yet, and the member flags only when neither exists: each case below disagrees with the flags.
    #[test]
    fn a_chip_reads_the_resolved_shader_before_the_member_flags() {
        let mut v = SetView::new("k".into(), "s".into(), vec![]);
        v.shader = Some(pick(Shader::Pbr));
        assert_eq!(set_slot_label(&v, SlotKind::Diffuse), "PBR albedo");
        v.pbr = true;
        v.shader = Some(pick(Shader::Default));
        assert_eq!(set_slot_label(&v, SlotKind::Diffuse), "Diffuse");
        v.shader = Some(pick(Shader::AdvancedSkin));
        assert_eq!(set_slot_label(&v, SlotKind::Diffuse), "Skin albedo");
        assert_eq!(set_slot_label(&v, SlotKind::Emissive), "Glow");
        // a card with no files: the saved shader, then the flags
        let mut doc = SetView::new(String::new(), "s".into(), vec![]);
        doc.shader_choice = Some(Shader::PbrLandscape);
        assert_eq!(set_slot_label(&doc, SlotKind::Normal), "PBR normal");
        doc.pbr = true;
        doc.shader_choice = Some(Shader::Default);
        assert_eq!(set_slot_label(&doc, SlotKind::Diffuse), "Diffuse");
        doc.shader_choice = None;
        assert_eq!(set_slot_label(&doc, SlotKind::Diffuse), "PBR albedo");
        // a keyed card the plan has not picked for yet: the flags
        let mut keyed = SetView::new("k".into(), "s".into(), vec![]);
        keyed.skin = true;
        assert_eq!(set_slot_label(&keyed, SlotKind::Diffuse), "Skin albedo");
    }
}
