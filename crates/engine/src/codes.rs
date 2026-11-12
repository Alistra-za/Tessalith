//! Warning and error codes (spec §10). Codes added by this plan beyond the spec's original catalogue: VAL-MIP-ERROR
//! (§5.8 per-mip error warning), CFG-INVALID (a field value outside its allowed range or an encoder/format
//! mismatch), IN-NONFINITE (NaN or infinite source samples replaced in Preprocess), IN-UNREADABLE (a folder entry
//! discovery could not read; skipped, audit 6) and the four `PACK-*` codes of channel packing (spec §6.4, plan M2.5), and CFG-FILE-UNMATCHED (a
//! `[files]` key of `sdc.toml` that matches no file, M3a) and SET-EDIT-STALE (a GUI set edit that names a file or
//! set that no longer exists, M3b), PACK-RECIPE-CONFLICT and PACK-RECIPE-UNBUILDABLE (a `packing.recipe` the set
//! could not follow, packing-fills Task 1) and
//! FMT-HDR-SOURCE (why `pbr_emissive` is or is not BC6H, O2) and
//! DIM-RESIZED (the power of two or `resize` changed the base size, S-mips-7) and CFG-FILTER-IGNORED (an unscoped non-box
//! `mips.filter` a texture cannot use, which then uses box, S-mips-13) and the four `SHADER-*` codes (M6 Track A) and
//! CFG-PRESET-SKIPPED (a user preset folder file that failed to load, skipped) and CFG-GREEN-CONFLICT (an explicit
//! `flip_green` that contradicts `green_convention`) (M6 Track D).
//! and the five `GRASS-*` codes and VAL-GRASS-DETECT (complex grass design 2026-10-12 §3.6, §3.7, §4)
//! and IN-ALIAS-INVALID / IN-ALIAS-STALE (an `[inputs]` alias that was refused or matched no file, M6 Track F3b)
//! and ALPHA-PARALLAX-NO-ALPHA (`alpha.parallax` on for a complex `_m` whose source has no alpha, DECISIONS AH2).
//! and PBR-GRASS-DETECT (a PBR grass albedo CS would read as complex grass) (PBR grass design 2026-10-14 §4.4).
//! and the four `CHK-*` codes of the GUI's artist checks, and CHK-LOW-DETAIL of check 3 at convert time (v2 artist checks, DECISIONS Note CL).
//! and ENC-WEIGHTS-IGNORED (an `encoder.bc7_weights` that perceptual or the format overrides, DECISIONS Note CP).
//! and the two `GEN-*` codes of generated maps: GEN-REPLACED and GEN-AO (`_generated` files, DECISIONS Notes CO and GV).
//! and CUBE-FACES / CUBE-NO-MIPS of cubemaps (cubemap support v1, DECISIONS Note DS).
//! and ALPHA-BC1-ONE-BIT (BC1 1-bit alpha on a source with soft alpha, DECISIONS Note FM)
//! and PACK-SOURCE-RESIZED (a smaller pack, derive or fill source enlarged to the largest, DECISIONS Note FM30).
//! and FMT-RGB8-ALPHA (a 24-bit RGB output for a texture that keeps its alpha, DECISIONS Note FM38).
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Blocking,
    Error,
}

macro_rules! codes {
    ($($v:ident = $s:literal => $sev:ident),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema)]
        pub enum Code { $(#[serde(rename = $s)] $v),+ }
        impl Code {
            pub const ALL: &'static [Code] = &[$(Code::$v),+];
            pub fn as_str(self) -> &'static str { match self { $(Code::$v => $s),+ } }
            pub fn severity(self) -> Severity { match self { $(Code::$v => Severity::$sev),+ } }
            pub fn parse(s: &str) -> Option<Code> { match s { $($s => Some(Code::$v),)+ _ => None } }
        }
    };
}

impl Code {
    /// A `--allow` value: the code, or an error that names the fix for a code 2.0 removed (same text as a config's
    /// `allow` list, `migrate::REMOVED`).
    pub fn parse_allow(s: &str) -> Result<Code, String> {
        if let Some(removed) = crate::migrate::removed_set("allow", s) {
            return Err(format!("--{removed}"));
        }
        Code::parse(s).ok_or_else(|| format!("--allow {s}: unknown code"))
    }
}

codes! {
    LossAlphaData = "LOSS-ALPHA-DATA" => Blocking,
    LossClampHdr = "LOSS-CLAMP-HDR" => Warning,
    LossReencode = "LOSS-REENCODE" => Warning,
    ChanNotGray = "CHAN-NOT-GRAY" => Warning,
    ChanNormalLength = "CHAN-NORMAL-LENGTH" => Warning,
    FmtBc5Normal = "FMT-BC5-NORMAL" => Blocking,
    FmtLandscapeBc1 = "FMT-LANDSCAPE-BC1" => Blocking,
    // Community Shaders' texture loader reads only RGBA8, BGRA8, BC1, BC2, BC3 and BC7 on a skin map.
    FmtCsSkinLoader = "FMT-CS-SKIN-LOADER" => Blocking,
    // CS removed the grey/height wet form (CS 176a0083); a grey `skin_wet` is not written.
    SkinWetGrey = "SKIN-WET-GREY" => Warning,
    FmtBc1Alpha = "FMT-BC1-ALPHA" => Blocking,
    // Note FM38: rgb8 stores no alpha, so a texture whose alpha is kept cannot be written as one.
    FmtRgb8Alpha = "FMT-RGB8-ALPHA" => Blocking,
    FmtPerceptualData = "FMT-PERCEPTUAL-DATA" => Error,
    FmtHdrSource = "FMT-HDR-SOURCE" => Info,
    AlphaExactOffM = "ALPHA-EXACT-OFF-M" => Blocking,
    AlphaParallaxNoAlpha = "ALPHA-PARALLAX-NO-ALPHA" => Warning,
    // Note FM: BC1 1-bit alpha on a source with more than 1 % soft alpha (1..=254).
    AlphaBc1OneBit = "ALPHA-BC1-ONE-BIT" => Warning,
    TagSrgbVanilla = "TAG-SRGB-VANILLA" => Error,
    TagSrgbNonpbr = "TAG-SRGB-NONPBR" => Blocking,
    TagUnormPbr = "TAG-UNORM-PBR" => Blocking,
    TagSrgbLod = "TAG-SRGB-LOD" => Blocking,
    TagContentMismatch = "TAG-CONTENT-MISMATCH" => Warning,
    CfgUnavailable = "CFG-UNAVAILABLE" => Blocking,
    CfgInvalid = "CFG-INVALID" => Error,
    CfgFileUnmatched = "CFG-FILE-UNMATCHED" => Info,
    CfgFilterIgnored = "CFG-FILTER-IGNORED" => Info,
    CfgPresetSkipped = "CFG-PRESET-SKIPPED" => Warning,
    CfgGreenConflict = "CFG-GREEN-CONFLICT" => Warning,
    // Note FM29: only a `mips.levels = "none"` texture keeps an odd size, and it may.
    DimNpot = "DIM-NPOT" => Warning,
    DimSmall = "DIM-SMALL" => Blocking,
    DimFlowSmall = "DIM-FLOW-SMALL" => Warning,
    DimResized = "DIM-RESIZED" => Info,
    DimTooLarge = "DIM-TOO-LARGE" => Error,
    // Note FM44: `resize_relative_to` names a slot the set has no map in, or more than one; the map keeps its size.
    ResizeNoReference = "RESIZE-NO-REFERENCE" => Info,
    SetMissingCompanion = "SET-MISSING-COMPANION" => Info,
    SetCompanionDds = "SET-COMPANION-DDS" => Warning,
    // Never raised since Note HD2; kept so an `allow` list naming it still loads.
    SetBakeRatio = "SET-BAKE-RATIO" => Warning,
    SetEditStale = "SET-EDIT-STALE" => Info,
    SugLowConfidence = "SUG-LOW-CONFIDENCE" => Blocking,
    SugContradiction = "SUG-CONTRADICTION" => Warning,
    ValAlphaGuarantee = "VAL-ALPHA-GUARANTEE" => Error,
    ValCoverage = "VAL-COVERAGE" => Warning,
    ValMComplexMargin = "VAL-M-COMPLEX-MARGIN" => Warning,
    ValMGrayscale = "VAL-M-GRAYSCALE" => Warning,
    ValMHeightClip = "VAL-M-HEIGHT-CLIP" => Warning,
    ValPgpatcher50 = "VAL-PGPATCHER-50" => Warning,
    ValBc6hPeak = "VAL-BC6H-PEAK" => Warning,
    ValHeader = "VAL-HEADER" => Error,
    ValMipError = "VAL-MIP-ERROR" => Warning,
    EncWeightsIgnored = "ENC-WEIGHTS-IGNORED" => Warning,
    InDuplicate = "IN-DUPLICATE" => Blocking,
    InDdsSkipped = "IN-DDS-SKIPPED" => Info,
    InPackingInput = "IN-PACKING-INPUT" => Warning,
    InGreenUnstated = "IN-GREEN-UNSTATED" => Info,
    InDecode = "IN-DECODE" => Error,
    InNonfinite = "IN-NONFINITE" => Warning,
    InUnreadable = "IN-UNREADABLE" => Warning,
    InAliasInvalid = "IN-ALIAS-INVALID" => Warning,
    InAliasStale = "IN-ALIAS-STALE" => Info,
    OutUnknownExisting = "OUT-UNKNOWN-EXISTING" => Warning,
    PackDefaulted = "PACK-DEFAULTED" => Warning,
    // Note FM30: sources of one aspect ratio but different sizes; the smaller ones are enlarged to the largest.
    PackSourceResized = "PACK-SOURCE-RESIZED" => Info,
    // Sources of different aspect ratios (Note FM30).
    PackSizeMismatch = "PACK-SIZE-MISMATCH" => Error,
    PackConflict = "PACK-CONFLICT" => Warning,
    PackUnusedInput = "PACK-UNUSED-INPUT" => Warning,
    PackRecipeConflict = "PACK-RECIPE-CONFLICT" => Warning,
    PackRecipeUnbuildable = "PACK-RECIPE-UNBUILDABLE" => Warning,
    ShaderAuto = "SHADER-AUTO" => Info,
    ShaderUnavailable = "SHADER-UNAVAILABLE" => Warning,
    ShaderExtraMap = "SHADER-EXTRA-MAP" => Warning,
    ShaderMissingMap = "SHADER-MISSING-MAP" => Warning,
    GrassShape = "GRASS-SHAPE" => Error,
    GrassTopOpaque = "GRASS-TOP-OPAQUE" => Warning,
    GrassNormals = "GRASS-NORMALS" => Warning,
    GrassDetectSource = "GRASS-DETECT-SOURCE" => Warning,
    ValGrassDetect = "VAL-GRASS-DETECT" => Error,
    PbrGrassDetect = "PBR-GRASS-DETECT" => Warning,
    // v2 artist checks (DECISIONS Note CL): GUI-only check results, never emitted by the engine's plan or run.
    ChkGreenGuess = "CHK-GREEN-GUESS" => Info,
    ChkZSign = "CHK-Z-SIGN" => Warning,
    ChkParallax = "CHK-PARALLAX" => Warning,
    ChkParallaxScale = "CHK-PARALLAX-SCALE" => Warning,
    // Check 3 (fake resolution): the one CHK code the engine emits, during a run whose request sets `low_detail`.
    ChkLowDetail = "CHK-LOW-DETAIL" => Warning,
    // Track A derive step (DECISIONS Note CN): PBR sets converted to vanilla and complex-material maps.
    // Set skipped: a source it needs is DDS (never re-encoded, M9).
    DeriveDdsSource = "DERIVE-DDS-SOURCE" => Warning,
    // No albedo: no diffuse (and no CM diffuse).
    DeriveNoAlbedo = "DERIVE-NO-ALBEDO" => Warning,
    // A derived output lands on another job's output (TA12); the other job is kept.
    DerivePathConflict = "DERIVE-PATH-CONFLICT" => Error,
    // A member with no recipe role (TA11).
    DeriveNotConverted = "DERIVE-NOT-CONVERTED" => Info,
    // Data {glossiness, specular_strength, env_map}: suggested NIF values for the set.
    DeriveNifSuggest = "DERIVE-NIF-SUGGEST" => Info,
    // Track A Also export (Note CN): the Vanilla or CM phase stopped with an engine error; its exit code is kept.
    AlsoPhaseFailed = "ALSO-PHASE-FAILED" => Error,
    // A `_generated` file (Note CO, GV) skipped because a real map fills its slot.
    GenReplaced = "GEN-REPLACED" => Info,
    // A packed _rmaos whose B channel is an `_ao_generated` file (Note CO, GV).
    GenAo = "GEN-AO" => Info,
    // Cubemap (DECISIONS Note DS): a cube's faces are missing, not square or not the same size.
    CubeFaces = "CUBE-FACES" => Error,
    // Cubemap: `mips.levels = none` on a cube; curved surfaces sparkle and CS reads the last mip.
    CubeNoMips = "CUBE-NO-MIPS" => Warning,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Warning {
    pub code: Code,
    pub severity: Severity,
    pub message: String,
    #[serde(default)]
    pub data: serde_json::Value,
}

impl Warning {
    pub fn new(code: Code, message: impl Into<String>) -> Self {
        Warning {
            code,
            severity: code.severity(),
            message: message.into(),
            data: serde_json::Value::Null,
        }
    }
    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = data;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn severities_follow_spec_section_10() {
        assert_eq!(Code::FmtBc5Normal.severity(), Severity::Blocking);
        assert_eq!(Code::TagSrgbVanilla.severity(), Severity::Error);
        assert_eq!(Code::ValCoverage.severity(), Severity::Warning);
        assert_eq!(Code::ValMipError.severity(), Severity::Warning);
        assert_eq!(Code::CfgInvalid.severity(), Severity::Error);
        assert_eq!(Code::DimResized.severity(), Severity::Info);
        assert_eq!(Code::CfgFilterIgnored.severity(), Severity::Info);
        assert_eq!(Code::InPackingInput.severity(), Severity::Warning);
        assert_eq!(Code::InGreenUnstated.severity(), Severity::Info);
        assert_eq!(
            Code::parse("IN-GREEN-UNSTATED"),
            Some(Code::InGreenUnstated)
        );
        assert_eq!(Code::parse("IN-PACKING-INPUT"), Some(Code::InPackingInput));
    }

    #[test]
    fn codes_serialise_as_their_strings_and_are_unique() {
        assert_eq!(
            serde_json::to_string(&Code::LossAlphaData).unwrap(),
            "\"LOSS-ALPHA-DATA\""
        );
        assert_eq!(Code::parse("VAL-PGPATCHER-50"), Some(Code::ValPgpatcher50));
        let names: BTreeSet<_> = Code::ALL.iter().map(|c| c.as_str()).collect();
        assert_eq!(names.len(), Code::ALL.len());
        assert_eq!(Code::EncWeightsIgnored.severity(), Severity::Warning);
        assert_eq!(Code::GrassShape.severity(), Severity::Error);
        assert_eq!(Code::ValGrassDetect.severity(), Severity::Error);
        assert_eq!(
            Code::parse("GRASS-DETECT-SOURCE"),
            Some(Code::GrassDetectSource)
        );
        assert_eq!(Code::PbrGrassDetect.severity(), Severity::Warning);
        assert_eq!(Code::ChkGreenGuess.severity(), Severity::Info);
        assert_eq!(Code::parse("CHK-Z-SIGN"), Some(Code::ChkZSign));
        assert_eq!(Code::CfgPresetSkipped.severity(), Severity::Warning);
        assert_eq!(
            Code::parse("CFG-GREEN-CONFLICT"),
            Some(Code::CfgGreenConflict)
        );
        assert_eq!(Code::ShaderAuto.severity(), Severity::Info);
        assert_eq!(Code::ShaderMissingMap.severity(), Severity::Warning);
        assert_eq!(Code::parse("SHADER-EXTRA-MAP"), Some(Code::ShaderExtraMap));
    }

    #[test]
    fn codes_count() {
        assert_eq!(Code::ALL.len(), 91);
        assert_eq!(Code::ResizeNoReference.severity(), Severity::Info);
        assert_eq!(Code::parse("FMT-RGB8-ALPHA"), Some(Code::FmtRgb8Alpha));
        assert_eq!(Code::FmtRgb8Alpha.severity(), Severity::Blocking);
        assert_eq!(Code::ALL.last(), Some(&Code::CubeNoMips));
        assert_eq!(Code::CubeFaces.severity(), Severity::Error);
        assert_eq!(Code::parse("CUBE-NO-MIPS"), Some(Code::CubeNoMips));
        assert_eq!(Code::CubeNoMips.severity(), Severity::Warning);
        assert_eq!(Code::AlphaBc1OneBit.severity(), Severity::Warning);
    }
}
