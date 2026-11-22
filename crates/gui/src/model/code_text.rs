//! Note FB8 (amendment D): the UI carries no code text. A row or banner shows a sentence that names the action; the
//! code stays on the hover and in the Messages log.
use engine::codes::Code;
use engine::config::{AlphaPolicy, Target};

pub(super) fn borrowed_line(data: &serde_json::Value, packed: bool) -> String {
    let output = std::path::Path::new(data["output"].as_str().unwrap_or_default());
    let name = output.file_name().unwrap_or_default().to_string_lossy();
    let n = data["slot"].as_u64().unwrap_or_default();
    let lender = data["lender"].as_str().unwrap_or_default();
    let count = data["more"].as_array().map_or(0, Vec::len);
    let more = if count == 0 {
        String::new()
    } else {
        format!(" and {count} more")
    };
    let sources = data["causes"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|c| {
            let source = c["source"].as_str().unwrap_or_default();
            match (c["shared"] == true, c["set"].as_str()) {
                (true, Some(owner)) => format!("{source}, which this set shares from {owner}"),
                (true, None) => format!("{source}, which this set shares"),
                (false, _) => source.to_owned(),
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    let how = if packed { "packed from" } else { "from" };
    format!("Slot {n} uses {lender}'s file, but this set still writes its own {name}{more}, {how} {sources}.")
}

pub(crate) fn borrowed_detail(w: &engine::codes::Warning) -> String {
    let mut detail = String::from("Remove the share or the own file, or drop the borrow. If a mesh or another mod uses the file, mark this row as intended.");
    for path in w.data["more"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| p.as_str())
    {
        detail.push('\n');
        detail.push_str(path);
    }
    detail
}

/// A mode's `sdc.toml` name (`target = "cs"`), as the header, the Settings tab and the unmatched-entries row (Note JI6)
/// show it.
pub fn target_name(t: Target) -> &'static str {
    match t {
        Target::Cs => "cs",
        Target::Vanilla => "vanilla",
    }
}

/// A mode as the Settings Mode combo reads it: its plain name, then its `sdc.toml` name.
pub fn target_label(t: Target) -> &'static str {
    match t {
        Target::Cs => "Community Shaders (cs)",
        Target::Vanilla => "Base game (vanilla)",
    }
}

/// The action sentence of `code`, one table for every code Problems or a banner can show (warnings, blockers and
/// errors). Empty for the codes with none (most Info results; CFG-FILE-UNMATCHED's row words its own text, Note JI6): the
/// caller then shows the row's or the engine's message.
pub fn code_sentence(code: Code) -> &'static str {
    use Code::*;
    match code {
        LossAlphaData => {
            "This alpha channel carries data the format would drop — choose what it means"
        }
        LossClampHdr => "Some values are brighter than this format can hold — pick an HDR format or accept the clipping",
        LossReencode => "This DDS would be decoded and encoded again, losing quality — keep its format or use the original image",
        ChanNotGray => "This map should be grey but has colour — replace the file or convert it to grey",
        ChanNormalLength => "This normal map's blue (Z) channel doesn't match its red and green — use Rebuild Z or replace the file",
        FmtBc5Normal => "BC5 can't store this normal map the way it is set up — choose another format",
        FmtCsSkinLoader => "Community Shaders can't load this skin map in this format — choose BC7, BC3, BC1 or RGBA8 (BC5 for a detail map)",
        SkinWetGrey => "Community Shaders no longer reads a grey wet map, so this file is not written — use a normal map with a mask in alpha",
        FmtLandscapeBc1 => "Landscape textures can't use BC1 — choose another format",
        FmtBc1Alpha => "BC1 can't keep this texture's alpha — choose a format with alpha, remove the alpha, or for see-through alpha turn on 1-bit alpha",
        FmtRgb8Alpha => "RGB8 can't keep this texture's alpha — choose a format with alpha or remove the alpha",
        FmtPerceptualData => "Perceptual weighting is wrong for a data map — turn it off or change the format",
        AlphaExactOffM => "This map needs exact alpha but it is switched off — turn it on",
        AlphaBc1OneBit => "BC1 1-bit alpha turns this texture's soft alpha edges hard — choose BC7 or BC3 to keep them, or accept",
        AlphaParallaxNoAlpha => "Parallax is on but the source has no alpha — add an alpha channel or turn parallax off",
        TagSrgbVanilla => "This vanilla texture is tagged sRGB but the game reads it as linear — change the colour tag",
        TagSrgbNonpbr => "sRGB is not valid for this texture type — change the colour tag",
        TagUnormPbr => "Linear (UNORM) is not valid for this PBR texture — change the colour tag",
        TagSrgbLod => "LOD textures can't be tagged sRGB — change the colour tag",
        TagContentMismatch => "The colour tag doesn't match what the image holds — check the colour setting",
        CfgUnavailable => "A setting needs something that isn't available here — change the setting",
        CfgInvalid => "A setting has a value that isn't allowed — fix it in the settings",
        CfgPresetSkipped => "A preset file could not be loaded and was skipped — fix or remove the file",
        CfgGreenConflict => "The green-channel setting contradicts the normal-map convention — pick one of them",
        DimNpot => "Keeps a size that isn't a power of two. That's fine: it has no mips.",
        DimSmall => "This texture is too small for its format — use a bigger image",
        DimFlowSmall => "This flow map is small — use a bigger image, or mark it as intended",
        DimTooLarge => "This texture is larger than the format allows — resize it",
        SetCompanionDds => "A companion file of this set is a DDS and can't be combined — use its original image",
        SetBakeRatio => "The maps of this set differ a lot in size — use matching sizes",
        SetSlotBorrowedUnused => "This set borrows a slot and still writes its own file for it.",
        SugLowConfidence => "Tessalith isn't sure what this alpha means — choose what it means",
        SugContradiction => "The file name and the contents point to different types. Set the right one.",
        ValAlphaGuarantee => "The written alpha doesn't keep its guarantee — check the alpha settings",
        ValCoverage => "Cutout coverage drifted across the mips — adjust the threshold or the coverage setting",
        ValMComplexMargin => "The smallest mip's alpha is too low for complex material — change the alpha settings",
        ValMGrayscale => "This metal map isn't grey — use a grey source",
        ValMHeightClip => "Height values in this map are clipped — lower its contrast",
        ValPgpatcher50 => "PGPatcher may read this file's alpha wrongly — check the alpha values",
        ValBc6hPeak => "Bright peaks lose accuracy in BC6H — check the highlights",
        ValHeader => "The written DDS header isn't valid — export again",
        ValMipError => "A mip level strays from its source — check the filter or the source",
        EncWeightsIgnored => "The BC7 weighting has no effect with this format, so it is ignored. The file is fine.",
        InDuplicate => "Two files claim the same slot — remove or rename one",
        InPackingInput => "A packing input is missing or unusable — check the files of this set",
        InDecode => "This image could not be read — replace the file",
        InNonfinite => "This image had broken pixel values that were replaced — re-export the source",
        InUnreadable => "A file in the folder could not be read and was skipped — check it",
        InAliasInvalid => "A name alias was refused — fix it in the preset",
        OutUnknownExisting => "An existing output file isn't ours — move or remove it",
        PackDefaulted => "A packed channel had no input and got a default — add the missing map",
        PackSizeMismatch => "The packed maps have different aspect ratios — use maps of the same proportions",
        PackConflict => "A packed file collides with another output — remove one of them",
        PackUnusedInput => "A packing input isn't used by this set — remove it, or change the set's shader or the file's type",
        PackRecipeConflict => "The two packed entries name different recipes — clear the recipe",
        PackRecipeUnbuildable => "The written recipe can't be built from this set's maps — clear the recipe",
        ShaderUnavailable => "The chosen shader isn't available — pick another shader",
        ShaderExtraMap => "This set has a map its shader doesn't read — remove it or change the shader",
        ShaderMissingMap => "This set lacks a map its shader needs — add it or change the shader",
        GrassShape => "This grass texture has the wrong layout — use a grass atlas shape",
        GrassTopOpaque => "The tips of this grass texture are fully opaque — check its alpha",
        GrassNormals => "This grass texture's normals look wrong — check or regenerate them",
        GrassDetectSource => "This grass source's corner normal isn't unit length — the written file is renormalised, so Community Shaders still reads it as grass",
        ValGrassDetect => "The grass check failed on the output — check the grass settings",
        PbrGrassDetect => "Community Shaders would read this grass albedo as complex grass — change its alpha or name",
        ChkZSign => "This normal map's blue channel points the wrong way — use Rebuild Z",
        ChkParallax => "The parallax height looks wrong for this map — check the height map",
        ChkParallaxScale => "The parallax scale looks too strong or too weak for this set — adjust it",
        ChkLowDetail => "This texture has little detail for its size — consider Resize: Half",
        DeriveDdsSource => "A DDS source can't be converted to vanilla — use the original image",
        DeriveNoAlbedo => "This set has no albedo so it can't be converted — add the albedo",
        DerivePathConflict => "A converted file lands on another file's output — rename one of them",
        AlsoPhaseFailed => "An extra export step stopped with an error — see Messages and export again",
        CubeFaces => "A cubemap's faces are missing or differ in size — fix the six faces",
        CubeNoMips => "A cubemap without mips sparkles on curved surfaces — turn mips on",
        // Note FC3: the Info results that are rows, since the user acts on them.
        DeriveNifSuggest => "This converted set needs NIF values set by hand — hover the row for them",
        ChkGreenGuess => "This normal map's green channel looks flipped — treat it as OpenGL if its lighting looks inverted",
        // The other Info results: no row, no sentence.
        _ => "",
    }
}

/// Review E2: what a row of a code is. A Note means the engine applied a documented default or made a documented
/// choice and the output is valid; a Problem means the output may be wrong or a file is not written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowClass {
    Problem,
    Note,
}

/// Review E2: the class of `code`. No `_` arm, so a new engine code fails to build until it is classed. The
/// Blocking and Error codes keep their own rows and are `Problem` here.
pub fn row_class(code: Code) -> RowClass {
    use Code::*;
    use RowClass::{Note, Problem};
    match code {
        // The Info codes, but the two the user acts on (CHK-GREEN-GUESS, DERIVE-NIF-SUGGEST).
        FmtHdrSource | CfgFileUnmatched | CfgFilterIgnored | DimResized | ResizeNoReference
        | SetMissingCompanion | SetEditStale | InDdsSkipped | InGreenUnstated | InAliasStale
        | OutKeptOlderEncode | OutKeptLowerTarget | PackSourceResized | ShaderAuto
        | DeriveNotConverted | GenReplaced | GenAo => Note,
        // The Warning codes whose output is valid as it is.
        DimNpot | EncWeightsIgnored | PackDefaulted | PackUnusedInput | ShaderExtraMap => Note,
        ChkGreenGuess | DeriveNifSuggest => Problem,
        LossClampHdr
        | LossReencode
        | ChanNotGray
        | ChanNormalLength
        | SkinWetGrey
        | AlphaParallaxNoAlpha
        | AlphaBc1OneBit
        | TagContentMismatch
        | CfgPresetSkipped
        | CfgGreenConflict
        | DimFlowSmall
        | SetCompanionDds
        | SetBakeRatio
        | SetSlotBorrowedUnused
        | SugContradiction
        | ValCoverage
        | ValMComplexMargin
        | ValMGrayscale
        | ValMHeightClip
        | ValPgpatcher50
        | ValBc6hPeak
        | ValMipError
        | InPackingInput
        | InNonfinite
        | InUnreadable
        | InAliasInvalid
        | OutUnknownExisting
        | PackConflict
        | PackRecipeConflict
        | PackRecipeUnbuildable
        | ShaderUnavailable
        | ShaderMissingMap
        | GrassTopOpaque
        | GrassNormals
        // its worst case: `bottom::warning_class` makes it a Note while the job renormalises (Note JP6)
        | GrassDetectSource
        | PbrGrassDetect
        | ChkZSign
        | ChkParallax
        | ChkParallaxScale
        | ChkLowDetail
        | DeriveDdsSource
        | DeriveNoAlbedo
        | CubeNoMips => Problem,
        // Blocking and Error: they stay Blocks, Decide or "Failed: …" rows.
        LossAlphaData | FmtBc5Normal | FmtLandscapeBc1 | FmtCsSkinLoader | FmtBc1Alpha
        | FmtRgb8Alpha | FmtPerceptualData | AlphaExactOffM | TagSrgbVanilla | TagSrgbNonpbr
        | TagUnormPbr | TagSrgbLod | CfgUnavailable | CfgInvalid | DimSmall | DimTooLarge
        | SugLowConfidence | ValAlphaGuarantee | ValHeader | InDuplicate | InDecode
        | PackSizeMismatch | GrassShape | ValGrassDetect | DerivePathConflict | AlsoPhaseFailed
        | CubeFaces => Problem,
    }
}

/// Review E2: the sentence of a Note-class warning's row, which says why the output is fine; the engine message
/// for any other code, and for a Note whose message or `data` does not give what the sentence needs.
pub fn note_sentence(w: &engine::codes::Warning) -> String {
    let built = match w.code {
        Code::PackDefaulted => pack_defaulted_note(&w.data),
        // `"{name}: {reason}; not converted"` (`engine/src/pack/plan.rs`).
        Code::PackUnusedInput => w.message.split_once(": ").and_then(|(name, rest)| {
            let reason = rest.strip_suffix("; not converted")?;
            Some(format!(
                "{name} isn't used: {reason}. Nothing else changes."
            ))
        }),
        Code::ShaderExtraMap => member_and_shader(w).map(|(member, shader)| {
            format!("{member} isn't read by the {shader} shader. It is still converted, so nothing breaks.")
        }),
        Code::DimNpot | Code::EncWeightsIgnored => Some(code_sentence(w.code).to_string()),
        _ => None,
    };
    built.unwrap_or_else(|| w.message.clone())
}

/// A shader warning's `data.member` and `data.shader`.
fn member_and_shader(w: &engine::codes::Warning) -> Option<(&str, &str)> {
    let s = |k: &str| w.data.get(k).and_then(|v| v.as_str());
    s("member").zip(s("shader"))
}

/// PACK-DEFAULTED's note from its `data` (`engine/src/pack/plan.rs`): the output's file name, the channel, the
/// slot's default and why it is fine. `None` for missing data or a slot with no sentence.
fn pack_defaulted_note(data: &serde_json::Value) -> Option<String> {
    let output = data.get("output")?.as_str()?;
    let output = output.rsplit('/').next().unwrap_or(output);
    let channel = data.get("channel")?.as_str()?;
    // The engine's value is an f32: print it as one, so 1.0 reads "1" and 0.04 reads "0.04", as the message does.
    let value = data.get("value")?.as_f64()? as f32;
    let head = format!("{output} {channel}");
    Some(match data.get("slot")?.as_str()? {
        "metallic" => format!("{head}: no metallic map, so the default {value} (not metal) is used. Fine unless part of it is metal."),
        "specular" => format!("{head}: no specular map, so the default {value} is used. Fine for most surfaces that aren't metal."),
        "ao" => format!("{head}: no AO map, so the default {value} (no extra shading) is used. Fine; creases just get no extra shadow."),
        "roughness" => format!("{head}: no roughness map, so the default {value} is used everywhere. Fine if the surface is evenly rough."),
        "glossiness" => format!("{head}: no glossiness map, so the default {value} is used everywhere. Fine if the surface is evenly glossy."),
        "env_mask" => format!("{head}: no environment mask, so the default {value} (no reflection) is used. Fine unless part of it should reflect."),
        "height" => format!("{head}: no height map, so the default {value} (flat) is used. Fine unless you want parallax depth."),
        // A fuzz map shared in that its own set converts (the engine's `data.why`, `pack::plan::Candidate::fuzz_why`).
        "fuzz" => match data.get("why").and_then(|w| w.as_str()) {
            Some(why) => format!("{head}: {why}, so the default {value} (no fuzz) is used."),
            None => format!("{head}: no fuzz map, so the default {value} (no fuzz) is used. Fine unless it is cloth or fur."),
        },
        _ => return None,
    })
}

/// Note GU2: the engine's reason a cube-typed image is no cube source (so the two cannot drift), else a plain line.
pub fn not_a_cube(job: Option<&engine::policy::ResolvedJob>) -> String {
    let s = match job.map(|j| &j.status) {
        Some(engine::policy::JobStatus::Unresolved { reasons, .. }) if !reasons.is_empty() => {
            reasons.join("; ")
        }
        _ => format!(
            "not a cube source: it must be {}",
            engine::cube::plan::CUBE_SOURCES
        ),
    };
    let mut c = s.chars();
    c.next()
        .map_or(String::new(), |f| f.to_uppercase().chain(c).collect())
}

/// [`code_sentence`], or a plain line for a code with none (a place that has no engine message to fall back on).
pub fn code_line(code: Code) -> &'static str {
    match code_sentence(code) {
        "" => "Tessalith reported a problem — see Messages",
        s => s,
    }
}

/// The Acknowledge button's text for `code` (Note FI14): acknowledging `LossAlphaData` exports the texture without its
/// alpha (the profile stays `remove`, or the format cannot store it), so the button says that.
pub fn ack_label(code: Code) -> &'static str {
    match code {
        Code::LossAlphaData => "Drop the alpha anyway",
        _ => "Acknowledge",
    }
}

/// The user-facing name of an alpha type: the dropdown's entries and the confirm line.
pub fn alpha_label(policy: AlphaPolicy) -> &'static str {
    match policy {
        AlphaPolicy::Remove => "Remove",
        AlphaPolicy::Cutout => "Cutout",
        AlphaPolicy::Blend => "Blend",
        AlphaPolicy::CutoutBlend => "Cutout + Blend",
        AlphaPolicy::Data => "Data",
    }
}

/// Note FM35: the one line of the alpha question, on the Problems row and in the texture panel: the type the dropdown
/// opens on, the engine's proposal (a mesh-decided texture is never asked).
pub fn alpha_confirm_text(policy: AlphaPolicy) -> String {
    format!("Alpha: {} (proposed) — confirm", alpha_label(policy))
}

/// Note JF3: the sentence of a block Acknowledge cannot clear (`Decision::Change`). A type the mode does not have
/// names its two ways out; any other code keeps its own sentence, which names the fix already.
pub fn change_line(code: Code) -> &'static str {
    match code {
        Code::CfgUnavailable => {
            "This type isn't available in this mode — set another type, or change the mode in Settings…"
        }
        _ => code_line(code),
    }
}

/// [`ack_label`] with " for this texture", for the panels that act on the one texture.
pub fn ack_label_for_texture(code: Code) -> String {
    format!("{} for this texture", ack_label(code))
}

/// Every code `text` names, as `(start, end, code)`: a whole token (`LOSS-ALPHA-DATA`), never a part of a longer one.
fn code_spans(text: &str) -> Vec<(usize, usize, Code)> {
    let token = |c: char| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-';
    let mut out = vec![];
    let mut i = 0;
    while i < text.len() {
        let rest = &text[i..];
        let at_start = text[..i]
            .chars()
            .next_back()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '-' || c == '_'));
        let n = rest.find(|c| !token(c)).unwrap_or(rest.len());
        if at_start && n > 0 {
            if let Some(c) = Code::parse(&rest[..n]) {
                out.push((i, i + n, c));
                i += n;
                continue;
            }
        }
        i += rest.chars().next().map_or(1, char::len_utf8);
    }
    out
}

/// The first code `text` names.
pub fn first_code(text: &str) -> Option<Code> {
    code_spans(text).first().map(|s| s.2)
}

/// `text` without the codes it names: a leading `CODE: ` prefix (the engine's habit) goes with its colon, a code in
/// the middle of a sentence goes alone (its backticks and the doubled space it leaves with it). The code stays in
/// the Messages log, which gets the text as it was.
pub fn without_codes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    for (s, e, _) in code_spans(text) {
        out.push_str(&text[i..s]);
        i = e;
        if text[..s].ends_with('`') && text[e..].starts_with('`') {
            out.pop();
            i += 1;
        }
        // "CODE: sentence" keeps the sentence.
        if text[i..].starts_with(':') {
            i += 1;
        }
        if out.is_empty() || out.ends_with(' ') {
            i += text[i..].len() - text[i..].trim_start_matches(' ').len();
        }
    }
    out.push_str(&text[i..]);
    out
}

/// Note HC2 (amends FB8): the codes whose engine text names the map, channel or file at fault, which their sentence
/// cannot.
fn names_its_map(code: Code) -> bool {
    matches!(
        code,
        Code::PackDefaulted
            | Code::PackConflict
            | Code::PackUnusedInput
            | Code::ShaderExtraMap
            | Code::SetSlotBorrowedUnused
            | Code::ShaderMissingMap
    )
}

/// GRASS-DETECT-SOURCE's sentence when the job keeps the normal base as authored (`mips.normal_base = keep`, the
/// setting's label "Normal base level", its value "Keep as authored"): the written file is not renormalised, so
/// `code_sentence`'s "still reads it as grass" is false there. `bottom::warning_text` gives it as the warning's text.
pub const GRASS_SOURCE_KEPT: &str = "This grass source's corner normal isn't unit length, and its normal base level is kept as authored — Community Shaders won't read the written file as grass";

/// What a row or line shows for `code` and its engine `text` (Notes FB8, HC2): the code's sentence, else the engine's
/// text without its code. A text that is already the code's job-dependent sentence (`GRASS_SOURCE_KEPT`) shows as it is.
pub fn shown_for(code: Option<Code>, text: &str) -> std::borrow::Cow<'_, str> {
    if code == Some(Code::GrassDetectSource) && text == GRASS_SOURCE_KEPT {
        return text.into();
    }
    match code.filter(|c| !names_its_map(*c)).map(code_sentence) {
        Some(s) if !s.is_empty() => s.into(),
        _ => without_codes(text).into(),
    }
}

/// A banner line (Note FB8): the first `CODE:` in it (`CFG-INVALID: preset 'x': …`, often behind a file path) says
/// the code's sentence and keeps the engine's detail after it; any other code goes. The log keeps the line whole.
pub fn banner_line(line: &str) -> String {
    let lead = code_spans(line)
        .into_iter()
        .find(|&(_, e, c)| !code_sentence(c).is_empty() && line[e..].starts_with(':'));
    match lead {
        Some((s, e, c)) => {
            without_codes(&format!("{}{}{}", &line[..s], code_sentence(c), &line[e..]))
        }
        None => without_codes(line),
    }
}

/// `n` and its noun, singular for one: "1 kind", "2 kinds" (review E18).
pub fn counted(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::codes::Severity;

    #[test]
    fn counted_says_one_and_many() {
        assert_eq!(counted(0, "kind", "kinds"), "0 kinds");
        assert_eq!(counted(1, "kind", "kinds"), "1 kind");
        assert_eq!(counted(2, "row", "rows"), "2 rows");
    }

    #[test]
    fn every_code_a_row_can_show_has_a_sentence_without_a_code_in_it() {
        for &c in Code::ALL {
            let s = code_sentence(c);
            if c.severity() != Severity::Info {
                assert!(!s.is_empty(), "{} has no sentence", c.as_str());
            }
            assert!(
                Code::ALL.iter().all(|k| !s.contains(k.as_str())),
                "{s} names a code"
            );
        }
    }

    /// Note HC2: "add the missing map" never said which map.
    #[test]
    fn a_row_about_a_missing_map_names_the_map() {
        for (code, text) in [
            (
                Code::PackDefaulted,
                "eye_rmaos.dds A: no specular source; neutral default 0.04",
            ),
            (
                Code::ShaderMissingMap,
                "the pbr shader needs _rmaos; none is in the set (nothing is generated)",
            ),
        ] {
            assert_eq!(shown_for(Some(code), text), text);
        }
        // Every other code keeps its action sentence.
        assert_eq!(
            shown_for(Some(Code::ChanNotGray), "engine text"),
            code_sentence(Code::ChanNotGray)
        );
    }

    #[test]
    fn without_codes_drops_the_code_and_its_colon_only() {
        assert_eq!(
            without_codes("CFG-INVALID: preset 'x': alias is bad"),
            "preset 'x': alias is bad"
        );
        assert_eq!(
            without_codes("acknowledge CHK-PARALLAX to use it"),
            "acknowledge to use it"
        );
        // No empty backticks, and no doubled space, where a quoted code goes.
        assert_eq!(
            without_codes("raises `CHK-PARALLAX` and `DIM-NPOT`."),
            "raises and ."
        );
        assert_eq!(
            first_code("see CHK-PARALLAX and DIM-NPOT"),
            Some(Code::ChkParallax)
        );
        // A banner line that leads with a code says its sentence, then the detail.
        assert_eq!(
            banner_line("CFG-INVALID: preset 'x': bad — sdc.toml is ignored"),
            format!(
                "{}: preset 'x': bad — sdc.toml is ignored",
                code_sentence(Code::CfgInvalid)
            )
        );
        assert_eq!(banner_line("plain line"), "plain line");
        // Not a code: kept; a longer token that merely starts with a code: kept.
        assert_eq!(without_codes("NO-SUCH-CODE stays"), "NO-SUCH-CODE stays");
        assert_eq!(
            without_codes("CHK-PARALLAX-SCALE-X stays"),
            "CHK-PARALLAX-SCALE-X stays"
        );
        assert_eq!(
            without_codes("plain text — ünïcode"),
            "plain text — ünïcode"
        );
    }
}
