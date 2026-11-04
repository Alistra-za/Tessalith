//! The configuration model (spec §3.2): one set of types for CLI, GUI, presets and the published schema.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

macro_rules! cfg_enum {
    ($(#[$m:meta])* $name:ident, $case:literal { $($(#[$vm:meta])* $v:ident $(= $ren:literal)?),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema)]
        #[serde(rename_all = $case)]
        $(#[$m])*
        pub enum $name { $($(#[$vm])* $(#[serde(rename = $ren)])? $v),+ }
    };
}

cfg_enum!(Target, "lowercase" { Cs, Vanilla });
cfg_enum!(Category, "snake_case" { Color, Normal, MixedPacked, Grayscale, Hdr });
/// A texture sub-type.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Subtype {
    Diffuse,
    LandscapeDiffuse,
    PbrAlbedo,
    PbrEmissive,
    Glow,
    PbrSubsurface,
    PbrFuzz,
    SubsurfaceTint,
    BacklightTint,
    UiSky,
    Normal,
    LandscapeNormal,
    PbrNormal,
    Msn,
    CoatNormal,
    Envmask,
    ComplexMaterial,
    Rmaos,
    SkinRfaos,
    Flow,
    Height,
    MsnSpecular,
    GenericTwoChannel,
    GenericData,
    HdrSky,
    SkinAlbedo,
    SkinSk,
    SkinWet,
    // Complex grass (design 2026-10-12): one 1:2 image, top = diffuse + alpha, bottom = normal + specular in alpha.
    // A `//` comment on purpose: a `///` doc on one variant makes schemars emit `oneOf` instead of the flat `enum`,
    // which rewrites every schema.
    ComplexGrass,
    // A single cubemap (`_e`, cubemap design 2026-10-26): six square faces, BC1/BC3/BC7/RGBA8 UNORM, `cubemap.*` keys.
    Cubemap,
    // XPMF landscape maps (design 2026-10-31): `_noise` is a BC4 projected-noise map, `_detail` (landscape sets only) a detail normal.
    ProjectedNoise,
    DetailNormal,
    // A plain grey map (Note FM37): BC4 or R8, read from the red channel unless `channels.grayscale_source` says
    // otherwise. Picked by hand only: no suffix, suggestion or shader mapping.
    Gray,
}

cfg_enum!(ColorContent, "lowercase" { Srgb, Linear });
cfg_enum!(ColorTag, "lowercase" { Srgb, Unorm, NotApplicable = "n/a" });
cfg_enum!(AlphaPolicy, "snake_case" { Remove, Cutout, Blend, CutoutBlend, Data });
cfg_enum!(AlphaCompare, "lowercase" { Gt, Ge });
// `//` on purpose, as on `Subtype`: a `///` doc on a variant turns the schema into `oneOf`.
cfg_enum!(GraySource, "lowercase" { R, G, B, A, Luma });
impl GraySource {
    /// The value a grayscale read takes from one texel: the picked channel, or Rec. 709 luma (Note FM36).
    pub fn read(self, q: &[f32; 4]) -> f32 {
        match self {
            GraySource::R => q[0],
            GraySource::G => q[1],
            GraySource::B => q[2],
            GraySource::A => q[3],
            GraySource::Luma => crate::image::luma(q),
        }
    }
    /// The channel as provenance and warnings name it: `R`, `G`, `B`, `A` or `luma`.
    pub fn label(self) -> &'static str {
        match self {
            GraySource::R => "R",
            GraySource::G => "G",
            GraySource::B => "B",
            GraySource::A => "A",
            GraySource::Luma => "luma",
        }
    }
}
cfg_enum!(MipKeyword, "lowercase" { Full, None });
cfg_enum!(MipFilter, "lowercase" { Box, Tent, Mitchell, Kaiser });
cfg_enum!(#[derive(Default)] MipEdge, "lowercase" { #[default] Wrap, Clamp });
cfg_enum!(#[derive(Default)] ResizeMode, "kebab-case" { #[default] None, Half, Quarter, Eighth });
impl MipEdge {
    pub fn is_wrap(&self) -> bool {
        *self == MipEdge::Wrap
    }
}
impl ResizeMode {
    pub fn is_none(&self) -> bool {
        *self == ResizeMode::None
    }
}
// What `resize` is measured from (Note FM44): the map's own size, or the source size of the set's map in a texture-set
// slot. `//` on purpose: a `///` doc on a variant turns the schema into `oneOf`.
cfg_enum!(#[derive(Default)] ResizeRelativeTo, "lowercase" {
    #[default] Own = "self", Slot1, Slot2, Slot3, Slot4, Slot5, Slot6, Slot7, Slot8
});
impl ResizeRelativeTo {
    pub fn is_own(&self) -> bool {
        *self == ResizeRelativeTo::Own
    }
    /// The 1-based slot, `None` for the map's own size.
    pub fn slot(self) -> Option<u8> {
        use ResizeRelativeTo::*;
        Some(match self {
            Own => return None,
            Slot1 => 1,
            Slot2 => 2,
            Slot3 => 3,
            Slot4 => 4,
            Slot5 => 5,
            Slot6 => 6,
            Slot7 => 7,
            Slot8 => 8,
        })
    }
}
/// `resize_floor` when nothing sets it.
pub const DEFAULT_RESIZE_FLOOR: u32 = 512;
fn default_resize_floor() -> u32 {
    DEFAULT_RESIZE_FLOOR
}
fn is_default_resize_floor(v: &u32) -> bool {
    *v == DEFAULT_RESIZE_FLOOR
}
cfg_enum!(#[derive(Default)] ValidationMips, "lowercase" { #[default] All, Smallest });
impl ValidationMips {
    pub fn is_all(&self) -> bool {
        *self == ValidationMips::All
    }
}
cfg_enum!(NormalMips, "snake_case" { Renormalize, RenormalizeBake });
cfg_enum!(NormalBase, "lowercase" { Renormalize, Keep });
cfg_enum!(CoverageMode, "lowercase" { Auto, On, Off });
cfg_enum!(Format, "lowercase" { Bc1, Bc3, Bc4, Bc5, Bc6hUf16 = "bc6h_uf16", Bc7, Rgba8, Rgb8, R8, Rg8, Rgba16f });
cfg_enum!(HeaderForm, "lowercase" { Auto, Dx10, Legacy });
// bc7e quality level. `max` is sdc patch 0002 (veryslow's search plus a fully refined, wider partition ranking,
// uber 4, two least-squares passes and a quantised endpoint search); the vendored objects always carry it
// (docs/research/bc7e-tuning.md).
cfg_enum!(Effort, "lowercase" { Ultrafast, Veryfast, Fast, Basic, Veryslow, Slowest, Max });
cfg_enum!(SearchRadius, "lowercase" { Default, Max, Fast });
// BC7 linear error weights for bc7e (DECISIONS Note CP): alpha priority. `linear` is bc7e's own 1,1,1,1 (the plain
// call): it overrides an inherited weighting.
cfg_enum!(Bc7Weights, "lowercase" { Linear, Alpha2, Alpha8, Alpha16 });
impl Bc7Weights {
    /// Every value, in order; `all_lists_every_variant` ties it to the enum.
    pub const ALL: [Bc7Weights; 4] = [
        Bc7Weights::Linear,
        Bc7Weights::Alpha2,
        Bc7Weights::Alpha8,
        Bc7Weights::Alpha16,
    ];

    /// The R, G, B, A weights bc7e is given.
    pub fn weights(self) -> [u32; 4] {
        match self {
            Bc7Weights::Linear => [1, 1, 1, 1],
            Bc7Weights::Alpha2 => [1, 1, 1, 2],
            Bc7Weights::Alpha8 => [1, 1, 1, 8],
            Bc7Weights::Alpha16 => [1, 1, 1, 16],
        }
    }
}
cfg_enum!(GreenConvention, "lowercase" { Directx, Opengl });
cfg_enum!(
    /// One RNM normal layer (M6 Track F2 spec §2.1).
    LayerId, "lowercase" { Bump, Bump2, Detail, Detail2, Bent }
);
cfg_enum!(#[derive(Default)] BentMode, "snake_case" { #[default] AoBlend, Detail });
cfg_enum!(
    /// Which base normal a layer file feeds (F2 spec §2.3): `n` (`_n`), `cnr` (`_cnr`) or both.
    RnmTarget, "lowercase" { N, Cnr, Both }
);

impl LayerId {
    /// The default blend order (F2 spec §3.1 step 5).
    pub const ORDER: [LayerId; 5] = [
        LayerId::Bent,
        LayerId::Bump,
        LayerId::Bump2,
        LayerId::Detail,
        LayerId::Detail2,
    ];
    pub fn id(self) -> &'static str {
        match self {
            LayerId::Bump => "bump",
            LayerId::Bump2 => "bump2",
            LayerId::Detail => "detail",
            LayerId::Detail2 => "detail2",
            LayerId::Bent => "bent",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            LayerId::Bump => "bump",
            LayerId::Bump2 => "bump 2",
            LayerId::Detail => "detail",
            LayerId::Detail2 => "detail 2",
            LayerId::Bent => "bent",
        }
    }
    /// F2 spec §3.4 (rnm.py's defaults).
    pub fn default_strength(self) -> f64 {
        match self {
            LayerId::Bump | LayerId::Bump2 => 0.4,
            LayerId::Detail => 0.6,
            LayerId::Detail2 => 0.3,
            LayerId::Bent => 0.35,
        }
    }
}

impl BentMode {
    pub fn is_ao_blend(&self) -> bool {
        *self == BentMode::AoBlend
    }
}

fn yes() -> bool {
    true
}
fn is_true(b: &bool) -> bool {
    *b
}

/// One layer's settings (F2 spec §4). Every field is optional at its default: `strength` = the layer's
/// `default_strength`, `flip_green` = the base's own `channels.flip_green`, `mask` = on when the mask file exists.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayerSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strength: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flip_green: Option<bool>,
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub mask: bool,
}

impl Default for LayerSpec {
    fn default() -> Self {
        LayerSpec {
            strength: None,
            flip_green: None,
            mask: true,
        }
    }
}

impl LayerSpec {
    pub fn is_default(&self) -> bool {
        *self == LayerSpec::default()
    }
}

/// The `bent` layer: a layer plus its AO-blend settings (F2 spec §3.1, §3.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BentSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strength: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flip_green: Option<bool>,
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub mask: bool,
    #[serde(default, skip_serializing_if = "BentMode::is_ao_blend")]
    pub mode: BentMode,
    /// The power on the occlusion `1 − AO`; `None` = 1.5.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ao_power: Option<f64>,
    /// Read the AO source; `false` = a uniform weight.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub ao: bool,
}

impl Default for BentSpec {
    fn default() -> Self {
        BentSpec {
            strength: None,
            flip_green: None,
            mask: true,
            mode: BentMode::AoBlend,
            ao_power: None,
            ao: true,
        }
    }
}

impl BentSpec {
    pub fn is_default(&self) -> bool {
        *self == BentSpec::default()
    }
}

/// A layer's resolved settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerView {
    pub strength: f64,
    /// `None` = follow the base's `channels.flip_green`.
    pub flip_green: Option<bool>,
    pub mask: bool,
}

/// RNM normal layers (M6 Track F2 spec §4). A field of every profile, read only by base normals (`Subtype::rnm_base`);
/// not serialised at its default, so profile hashes written before F2 hold (DECISIONS Note W6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RnmSpec {
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub enabled: bool,
    /// Blend order; see [`RnmSpec::effective_order`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<Vec<LayerId>>,
    #[serde(default, skip_serializing_if = "LayerSpec::is_default")]
    pub bump: LayerSpec,
    #[serde(default, skip_serializing_if = "LayerSpec::is_default")]
    pub bump2: LayerSpec,
    #[serde(default, skip_serializing_if = "LayerSpec::is_default")]
    pub detail: LayerSpec,
    #[serde(default, skip_serializing_if = "LayerSpec::is_default")]
    pub detail2: LayerSpec,
    #[serde(default, skip_serializing_if = "BentSpec::is_default")]
    pub bent: BentSpec,
}

impl Default for RnmSpec {
    fn default() -> Self {
        RnmSpec {
            enabled: true,
            order: None,
            bump: LayerSpec::default(),
            bump2: LayerSpec::default(),
            detail: LayerSpec::default(),
            detail2: LayerSpec::default(),
            bent: BentSpec::default(),
        }
    }
}

impl RnmSpec {
    pub fn is_default(&self) -> bool {
        *self == RnmSpec::default()
    }
    /// `order` without repeats (the first wins), then the missing ids in [`LayerId::ORDER`] (DECISIONS Note W6).
    pub fn effective_order(&self) -> Vec<LayerId> {
        let mut v: Vec<LayerId> = vec![];
        for id in self.order.iter().flatten().chain(LayerId::ORDER.iter()) {
            if !v.contains(id) {
                v.push(*id);
            }
        }
        v
    }
    pub fn layer(&self, id: LayerId) -> LayerView {
        let (strength, flip_green, mask) = match id {
            LayerId::Bump => (self.bump.strength, self.bump.flip_green, self.bump.mask),
            LayerId::Bump2 => (self.bump2.strength, self.bump2.flip_green, self.bump2.mask),
            LayerId::Detail => (
                self.detail.strength,
                self.detail.flip_green,
                self.detail.mask,
            ),
            LayerId::Detail2 => (
                self.detail2.strength,
                self.detail2.flip_green,
                self.detail2.mask,
            ),
            LayerId::Bent => (self.bent.strength, self.bent.flip_green, self.bent.mask),
        };
        LayerView {
            strength: strength.unwrap_or(id.default_strength()),
            flip_green,
            mask,
        }
    }
    pub fn ao_power(&self) -> f64 {
        self.bent.ao_power.unwrap_or(1.5)
    }
}
// `FuzzWeight` and `SubsurfaceWeight` (Note DY) come only from a hand-set `helper.role`: no name carries them.
cfg_enum!(RawMap, "lowercase" { Roughness, Metallic, Ao, Specular, Glossiness, Height, Opacity, Fuzz, Orm, Curves, Cavity, Rmaos, FuzzWeight, SubsurfaceWeight });

impl RawMap {
    /// The single-channel roles, each with a `packing.fill.<role>`. `Orm` is not one (DECISIONS Note AL), nor are
    /// `Curves` (a 0.5-centred curvature map), `Cavity` and the multi-channel `Rmaos`, read only by Also export
    /// (Note CN).
    pub const ALL: [RawMap; 8] = {
        use RawMap::*;
        [
            Roughness, Metallic, Ao, Specular, Glossiness, Height, Opacity, Fuzz,
        ]
    };

    /// Every raw-map role a file name can carry: [`RawMap::ALL`], the multi-channel `Orm` (Note AL), `Curves` and
    /// `Cavity` (Note CN). Not `Rmaos`: `"rmaos"` is also the §6.1 suffix of a pre-packed file, and callers match
    /// `set.suffixes` on `id()`.
    pub const INPUTS: [RawMap; 11] = {
        use RawMap::*;
        [
            Roughness, Metallic, Ao, Specular, Glossiness, Height, Opacity, Fuzz, Orm, Curves,
            Cavity,
        ]
    };

    /// The role id a raw map counts as after Discover (spec §4.4): `roughness`, `ao`, …
    pub fn id(self) -> String {
        serde_id(self)
    }

    /// Spec §4.3 item 1, §6.4: an input that starts the cs `_rmaos`/`_rfaos` recipe, so its set is PBR.
    pub fn starts_rmaos(self) -> bool {
        matches!(
            self,
            RawMap::Roughness | RawMap::Glossiness | RawMap::Metallic | RawMap::Ao | RawMap::Orm
        )
    }
}

cfg_enum!(PackPrefer, "lowercase" { Prepacked, Raw });

/// Which cs recipe a set packs (DECISIONS Note R-fills-1): `auto` = `_rfaos` when a fuzz source exists, else
/// `_rmaos`; `rfaos`/`rmaos` force one.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum PackRecipe {
    #[default]
    Auto,
    Rmaos,
    Rfaos,
}

/// The serde string a unit enum variant serialises as (its id).
fn serde_id<T: serde::Serialize>(v: T) -> String {
    serde_json::to_value(v)
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned()
}

impl PackRecipe {
    pub fn id(self) -> &'static str {
        match self {
            PackRecipe::Auto => "auto",
            PackRecipe::Rmaos => "rmaos",
            PackRecipe::Rfaos => "rfaos",
        }
    }

    pub fn is_auto(&self) -> bool {
        *self == PackRecipe::Auto
    }
}

/// Global fill values for packed channels without a source map (spec §6.4, DECISIONS Notes H and M5): an 8-bit
/// code per §4.4 raw-map role, `None` = the neutral default. Unset fills are not serialised.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FillSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roughness: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metallic: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ao: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub specular: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glossiness: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fuzz: Option<u8>,
}

impl FillSpec {
    pub fn get(&self, role: RawMap) -> Option<u8> {
        match role {
            RawMap::Roughness => self.roughness,
            RawMap::Metallic => self.metallic,
            RawMap::Ao => self.ao,
            RawMap::Specular => self.specular,
            RawMap::Glossiness => self.glossiness,
            RawMap::Height => self.height,
            RawMap::Opacity => self.opacity,
            RawMap::Fuzz => self.fuzz,
            RawMap::Orm
            | RawMap::Curves
            | RawMap::Cavity
            | RawMap::Rmaos
            | RawMap::FuzzWeight
            | RawMap::SubsurfaceWeight => None,
        }
    }
}

/// Channel-packing settings (spec §3.2, §6.4). Fields of the packed output's profile, resolved like any other.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PackingSpec {
    /// Pack this output's recipe; `false` leaves its raw maps as packing inputs (`IN-PACKING-INPUT`).
    pub enabled: bool,
    /// Neutral roughness (0..=1) for a channel without a roughness or glossiness source.
    pub roughness_default: f64,
    /// Which side wins when the set already holds the packed output as a file (`PACK-CONFLICT`).
    pub prefer: PackPrefer,
    #[serde(default)]
    pub fill: FillSpec,
    /// Which cs recipe this set packs (`auto`, `rmaos`, `rfaos`; DECISIONS Note R-fills-1). Set per set at either packed key,
    /// `[files."<folder/base>_rfaos"]` or `..._rmaos"]`.
    #[serde(default, skip_serializing_if = "PackRecipe::is_auto")]
    pub recipe: PackRecipe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum MipLevels {
    Keyword(MipKeyword),
    Count(u32),
}

impl Subtype {
    /// Which base an RNM layer stack can blend into (F2 spec §2.3, DECISIONS Note W5); `None` for every other
    /// sub-type, `msn` included.
    pub fn rnm_base(self) -> Option<RnmTarget> {
        match self {
            Subtype::Normal | Subtype::PbrNormal | Subtype::LandscapeNormal => Some(RnmTarget::N),
            Subtype::CoatNormal => Some(RnmTarget::Cnr),
            _ => None,
        }
    }

    pub const ALL: [Subtype; 33] = {
        use Subtype::*;
        [
            Diffuse,
            LandscapeDiffuse,
            PbrAlbedo,
            PbrEmissive,
            Glow,
            PbrSubsurface,
            PbrFuzz,
            SubsurfaceTint,
            BacklightTint,
            UiSky,
            Normal,
            LandscapeNormal,
            PbrNormal,
            Msn,
            CoatNormal,
            Envmask,
            ComplexMaterial,
            Rmaos,
            SkinRfaos,
            Flow,
            Height,
            MsnSpecular,
            GenericTwoChannel,
            GenericData,
            HdrSky,
            SkinAlbedo,
            SkinSk,
            SkinWet,
            ComplexGrass,
            Cubemap,
            ProjectedNoise,
            DetailNormal,
            Gray,
        ]
    };

    pub fn category(self) -> Category {
        use Subtype::*;
        match self {
            Diffuse | LandscapeDiffuse | PbrAlbedo | PbrEmissive | Glow | PbrSubsurface
            | PbrFuzz | SubsurfaceTint | BacklightTint | UiSky | SkinAlbedo | Cubemap => {
                Category::Color
            }
            Normal | LandscapeNormal | PbrNormal | Msn | CoatNormal | SkinWet | DetailNormal => {
                Category::Normal
            }
            Envmask | Height | MsnSpecular | SkinSk | ProjectedNoise | Gray => Category::Grayscale,
            ComplexMaterial | Rmaos | SkinRfaos | Flow | GenericTwoChannel | GenericData
            | ComplexGrass => Category::MixedPacked,
            HdrSky => Category::Hdr,
        }
    }

    /// PBR colour data: the only sub-types that may carry `_SRGB` (cs only) [D#4, D#5].
    /// The 1-based texture-set slot this sub-type fills (Note FM44): the set editor's slot card where it has one
    /// (`gui::model::sets::slot_kind`, held equal by a gui test), else the vanilla TXST slot. `None` for maps that
    /// fill no slot.
    pub fn txst_slot(self) -> Option<u8> {
        use Subtype::*;
        match self {
            Diffuse | LandscapeDiffuse | PbrAlbedo | SkinAlbedo | ComplexGrass => Some(1),
            Normal | LandscapeNormal | PbrNormal | Msn => Some(2),
            PbrEmissive | Glow | SkinSk | SubsurfaceTint => Some(3),
            Height => Some(4),
            Cubemap => Some(5),
            Envmask | ComplexMaterial | Rmaos | SkinRfaos => Some(6),
            CoatNormal | PbrFuzz => Some(7),
            PbrSubsurface | BacklightTint | MsnSpecular => Some(8),
            UiSky | Flow | GenericTwoChannel | GenericData | HdrSky | SkinWet | ProjectedNoise
            | DetailNormal | Gray => None,
        }
    }

    pub fn is_pbr_color(self) -> bool {
        matches!(
            self,
            Subtype::PbrAlbedo | Subtype::PbrEmissive | Subtype::PbrSubsurface | Subtype::PbrFuzz
        )
    }

    /// Alpha meaning depends on the mesh (NiAlphaProperty), so it is never a high-confidence guess [SC §1].
    pub fn alpha_from_mesh(self) -> bool {
        matches!(
            self,
            Subtype::Diffuse
                | Subtype::PbrAlbedo
                | Subtype::UiSky
                | Subtype::SkinAlbedo
                | Subtype::ComplexGrass
        )
    }

    pub fn is_roughness_member(self) -> bool {
        matches!(self, Subtype::Rmaos | Subtype::SkinRfaos)
    }

    pub fn id(self) -> String {
        serde_id(self)
    }
}

impl Format {
    pub fn id(self) -> String {
        serde_id(self)
    }
    /// The DDS pixel format this format and tag are written as (DECISIONS S4: `rgba8` is B8G8R8A8; Note FM38: `rgb8` is
    /// the legacy 24-bit B8G8R8).
    pub fn pixel_format(self, srgb: bool) -> crate::dds::DdsPixelFormat {
        use crate::dds::DdsPixelFormat as P;
        match self {
            Format::Bc1 => P::Bc1 { srgb },
            Format::Bc3 => P::Bc3 { srgb },
            Format::Bc4 => P::Bc4,
            Format::Bc5 => P::Bc5,
            Format::Bc6hUf16 => P::Bc6hUf16,
            Format::Bc7 => P::Bc7 { srgb },
            Format::Rgba8 => P::Bgra8 { srgb },
            Format::Rgb8 => P::Rgb8,
            Format::R8 => P::R8,
            Format::Rg8 => P::Rg8,
            Format::Rgba16f => P::Rgba16f,
        }
    }
    /// Bytes per 4x4 block, or `None` for an uncompressed format.
    pub fn block_bytes(self) -> Option<usize> {
        self.pixel_format(false).block_bytes()
    }
    /// Bytes per texel of an uncompressed format (0 for a block format).
    pub fn bytes_per_pixel(self) -> usize {
        self.pixel_format(false).bytes_per_pixel()
    }
    pub fn is_block(self) -> bool {
        self.block_bytes().is_some()
    }
    /// Float formats: no `_SRGB`/UNORM tag (`n/a`), the `Hdr` domain, `cs` only (DECISIONS S4).
    pub fn is_hdr(self) -> bool {
        matches!(self, Format::Bc6hUf16 | Format::Rgba16f)
    }
    pub fn has_alpha(self) -> bool {
        matches!(
            self,
            Format::Bc1 | Format::Bc3 | Format::Bc7 | Format::Rgba8 | Format::Rgba16f
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ColorSpec {
    pub content: ColorContent,
    pub tag: ColorTag,
}

/// The allowed `alpha.threshold` values (owner O2): a NIF threshold is clamped into it, a preset value outside it is
/// CFG-INVALID.
pub const ALPHA_THRESHOLD: std::ops::RangeInclusive<u8> = 1..=254;

/// ALPHA-BC1-ONE-BIT fires when more than this share of source texels has alpha 1..=254 (Note FM).
pub const BC1_SOFT_ALPHA_SHARE: f64 = 0.01;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AlphaSpec {
    pub policy: AlphaPolicy,
    /// 8-bit alpha-test threshold, per texture; allowed 1..=254 (checked in policy).
    pub threshold: u8,
    pub compare: AlphaCompare,
    pub exact_opaque: bool,
    /// `complex_material` only (DECISIONS AH2): `false` writes alpha 0 at every mip (complex material on, parallax
    /// off); `true` keeps the source alpha as height. Resolve sets it from the source alpha; `None` elsewhere.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parallax: Option<bool>,
    /// An OpenEXR source with alpha is straight alpha, read as stored: no un-premultiply on load (trace-debug #30,
    /// Note BQ15). Off: RGB / A, as OpenEXR stores premultiplied colour. Read only for an `.exr` source with alpha.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub exr_straight: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChannelSpec {
    /// Four characters from r, g, b, a, 0, 1 (e.g. "bgra", "rrr1"); None = identity.
    #[serde(default)]
    pub swizzle: Option<String>,
    pub flip_green: bool,
    pub reconstruct_z: bool,
    pub grayscale_source: GraySource,
    /// Green (Y) convention of a normal source (spec §3.2, §4.4, DECISIONS Note H); None = unstated.
    /// `opengl` makes Resolve set `flip_green`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub green_convention: Option<GreenConvention>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MipSpec {
    pub levels: MipLevels,
    pub filter: MipFilter,
    /// Edge handling of the mip filter taps: read only by tent, Mitchell and Kaiser (spec §3.2).
    #[serde(default, skip_serializing_if = "MipEdge::is_wrap")]
    pub edge: MipEdge,
    pub normal: NormalMips,
    /// Mip 0 of a normal map (spec §5.4, DECISIONS R1): `renormalize` (default) stores every base vector at unit
    /// length; `keep` stores the source as authored. Read only for the Normal category.
    #[serde(default = "default_normal_base")]
    pub normal_base: NormalBase,
    /// Clamp `t` of the variance bake (spec §6.2, DECISIONS Note E): the added variance is at most `t²`. 0.25;
    /// allowed (0, 1]. Read from the profile the bake writes into: a roughness member, or a `_cnr`.
    #[serde(default = "default_bake_clamp")]
    pub bake_clamp: f64,
    /// Coverage preservation (spec §5.5). It only ever applies to the alpha-tested policies `cutout` and
    /// `cutout_blend`: `auto` and `on` both enable it there, `off` disables it, and on any other policy (`blend`,
    /// `data`, `remove`) every mode is a no-op, because blended alpha must keep box-averaged opacity.
    pub coverage: CoverageMode,
}

fn default_normal_base() -> NormalBase {
    NormalBase::Renormalize
}

fn default_bake_clamp() -> f64 {
    0.25
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EncoderSpec {
    pub effort: Effort,
    // Perceptual colour error: bc7e's own weights for BC7, Rec.709 luma weights in rgbcx for BC1 and BC3 colour (FM42).
    pub perceptual: bool,
    /// BC7 linear error weights (bc7e only; ignored when `perceptual` is on). Not serialised when unset, so existing
    /// profile hashes hold (DECISIONS Note CP).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bc7_weights: Option<Bc7Weights>,
    /// BC1 with 1-bit alpha (DECISIONS Note FM): texels below the alpha-test cutoff become transparent black. Off by
    /// default and never set automatically; not serialised when off.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bc1_alpha: bool,
    pub bc45_search_radius: SearchRadius,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ValidationSpec {
    /// M6 Track D (DECISIONS Notes T5, Y6): `smallest` runs the per-mip PSNR check on the smallest mip only and the
    /// coverage check on every level (`fast-preview`). Not serialised at its default.
    #[serde(default, skip_serializing_if = "ValidationMips::is_all")]
    pub mips: ValidationMips,
}

// Cubemap styles (DECISIONS DS4): each fills `face_size`, `format`, `blur` (and `vanilla_metal` `saturation`) where
// no explicit layer set them.
cfg_enum!(CubeStyle, "snake_case" { VanillaMetal, Room, Eye, Hd });
// How a cubemap source is laid out; unset = from the aspect ratio. `equirect` is a 2:1 360-degree panorama.
cfg_enum!(CubeLayout, "snake_case" { Faces, Hcross, Vcross, Hstrip, Vstrip, Equirect, MirrorBall, Angular, Photo });
cfg_enum!(#[derive(Default)] FaceFlip, "lowercase" { #[default] None, H, V });
// The ground fill look: the horizon colour darkening towards the nadir.
cfg_enum!(GroundMode, "lowercase" { Gradient });
/// What fills the cube below `cubemap.horizon`: `"gradient"` or `[r, g, b]` code values (game gamma, like `tint`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum GroundFill {
    Mode(GroundMode),
    Color([u8; 3]),
}

// Hand-written so a bad value names the accepted forms (an untagged derive says only "did not match any variant").
impl<'de> Deserialize<'de> for GroundFill {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::{Error, SeqAccess, Unexpected, Visitor};
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = GroundFill;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("\"gradient\" or [r, g, b] with each 0..=255")
            }
            fn visit_str<E: Error>(self, s: &str) -> Result<GroundFill, E> {
                match s {
                    "gradient" => Ok(GroundFill::Mode(GroundMode::Gradient)),
                    _ => Err(E::invalid_value(Unexpected::Str(s), &self)),
                }
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<GroundFill, A::Error> {
                let mut rgb = [0u8; 3];
                for (i, c) in rgb.iter_mut().enumerate() {
                    // Read as f64 so a fractional element also gets the accepted forms (DS25).
                    let v: f64 = a
                        .next_element()?
                        .ok_or_else(|| A::Error::invalid_length(i, &self))?;
                    if v.fract() != 0.0 || !(0.0..=255.0).contains(&v) {
                        return Err(A::Error::invalid_value(Unexpected::Float(v), &self));
                    }
                    *c = v as u8;
                }
                match a.next_element::<serde::de::IgnoredAny>()? {
                    None => Ok(GroundFill::Color(rgb)),
                    Some(_) => Err(A::Error::invalid_length(4, &self)),
                }
            }
        }
        d.deserialize_any(V)
    }
}
fn is_zero(v: &f64) -> bool {
    *v == 0.0
}
fn one() -> f64 {
    1.0
}
fn is_one(v: &f64) -> bool {
    *v == 1.0
}
fn is_zeros(v: &[u16; 6]) -> bool {
    *v == [0; 6]
}
fn no_flips(v: &[FaceFlip; 6]) -> bool {
    *v == [FaceFlip::None; 6]
}

/// The `cubemap` profile section (cubemap design 2026-10-26, "Settings"). Read only by `Subtype::Cubemap`; every key
/// is skipped at its default, so 2D profile hashes hold (DECISIONS DS4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CubemapSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<CubeStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<CubeLayout>,
    /// Face edge in pixels: a power of two in 4..=4096; unset = from the source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub face_size: Option<u32>,
    /// Turn about world up in degrees, -360..=360; positive is clockwise seen from above (Skyrim's heading).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub yaw: f64,
    /// Swap world east and west.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub mirror: bool,
    /// Per-face rotation in degrees (0, 90, 180, 270), in +X, -X, +Y, -Y, +Z, -Z order.
    #[serde(default, skip_serializing_if = "is_zeros")]
    pub face_rotate: [u16; 6],
    #[serde(default, skip_serializing_if = "no_flips")]
    pub face_flip: [FaceFlip; 6],
    /// A vertical cross stores -Z upside down; rotate it back.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub vcross_negz_rotate: bool,
    /// Cone blur amount, 0..=1; unset = none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blur: Option<f64>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub exposure: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub brightness: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub contrast: f64,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub midtones: f64,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub saturation: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tint: Option<[u8; 3]>,
    /// Highlights above this (linear) are clamped; at least 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlight_clamp: Option<f64>,
    /// Fill below the horizon line; unset = no fill (the horizon alone does nothing).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ground_fill: Option<GroundFill>,
    /// Horizon elevation in degrees, -45..=45; read only with ground_fill.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub horizon: f64,
}

impl Default for CubemapSpec {
    fn default() -> Self {
        CubemapSpec {
            style: None,
            layout: None,
            face_size: None,
            yaw: 0.0,
            mirror: false,
            face_rotate: [0; 6],
            face_flip: [FaceFlip::None; 6],
            vcross_negz_rotate: true,
            blur: None,
            exposure: 0.0,
            brightness: 0.0,
            contrast: 0.0,
            midtones: 1.0,
            saturation: 1.0,
            tint: None,
            highlight_clamp: None,
            ground_fill: None,
            horizon: 0.0,
        }
    }
}

impl CubemapSpec {
    pub fn is_default(&self) -> bool {
        *self == CubemapSpec::default()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub target: Target,
    pub category: Category,
    pub subtype: Subtype,
    pub color: ColorSpec,
    pub alpha: AlphaSpec,
    pub channels: ChannelSpec,
    pub mips: MipSpec,
    pub format: Format,
    pub header: HeaderForm,
    /// Preprocess step 6 (spec §5.4): `half`, `quarter` and `eighth` halve both sides up to 1, 2 or 3 times, each time
    /// only while the longer side stays at or above `resize_floor`. The power-of-two step before it is automatic.
    #[serde(default, skip_serializing_if = "ResizeMode::is_none")]
    pub resize: ResizeMode,
    /// The longer side `resize` never halves below, in pixels (Note FM29).
    #[serde(
        default = "default_resize_floor",
        skip_serializing_if = "is_default_resize_floor"
    )]
    pub resize_floor: u32,
    /// What `resize` is measured from (Note FM44): `self`, the map's own size, or `slot1` … `slot8`, the source size
    /// of the set's map in that texture-set slot. The map is halved while its longer side is above that size halved
    /// by `resize`; it is never enlarged. Read only when `resize` is not `none`.
    #[serde(default, skip_serializing_if = "ResizeRelativeTo::is_own")]
    pub resize_relative_to: ResizeRelativeTo,
    /// Written by the planner, not a setting: a config value is refused or replaced. The source size of the map
    /// `resize_relative_to` names, or `[0, 0]` when the set has no single map in that slot (the map then keeps its
    /// own size).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resize_reference: Option<(u32, u32)>,
    pub encoder: EncoderSpec,
    #[serde(default)]
    pub validation: ValidationSpec,
    pub packing: PackingSpec,
    /// RNM normal layers (M6 Track F2); read only by base normals (`Subtype::rnm_base`).
    #[serde(default, skip_serializing_if = "RnmSpec::is_default")]
    pub rnm: RnmSpec,
    /// Cubemap settings; read only by `Subtype::Cubemap`, reset for every other sub-type.
    #[serde(default, skip_serializing_if = "CubemapSpec::is_default")]
    pub cubemap: CubemapSpec,
}

impl Profile {
    /// Coverage preservation: on for `cutout` and `cutout_blend` unless `mips.coverage = off` (DECISIONS Note E).
    /// Never on for other policies, even with `mips.coverage = on` (spec §5.5: blended alpha keeps box-averaged
    /// opacity), except `blend` on a BC1 with `encoder.bc1_alpha`: its 1-bit alpha is a cutout (Note FM).
    pub fn coverage_enabled(&self) -> bool {
        self.mips.coverage != CoverageMode::Off
            && match self.alpha.policy {
                AlphaPolicy::Cutout | AlphaPolicy::CutoutBlend => true,
                AlphaPolicy::Blend => self.bc1_one_bit_alpha(),
                AlphaPolicy::Data | AlphaPolicy::Remove => false,
            }
    }
    /// BC1 output with `encoder.bc1_alpha` on: the alpha is cut to 0 or 255 before encoding (Note FM).
    pub fn bc1_one_bit_alpha(&self) -> bool {
        self.format == Format::Bc1 && self.encoder.bc1_alpha
    }
    pub fn alpha_weighted_mips(&self) -> bool {
        matches!(
            self.alpha.policy,
            AlphaPolicy::Cutout | AlphaPolicy::Blend | AlphaPolicy::CutoutBlend
        )
    }
    pub fn srgb_tag(&self) -> bool {
        self.color.tag == ColorTag::Srgb
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Note CP: `Bc7Weights::ALL` lists every variant once, in schema order (a new variant fails here until added).
    #[test]
    fn all_lists_every_variant() {
        let schema = serde_json::to_value(schemars::schema_for!(Bc7Weights)).unwrap();
        let names: Vec<_> = Bc7Weights::ALL
            .iter()
            .map(|w| serde_json::to_value(w).unwrap())
            .collect();
        assert_eq!(schema["enum"], serde_json::Value::Array(names));
    }

    #[test]
    fn serde_names_match_the_spec() {
        assert_eq!(
            serde_json::to_string(&Format::Bc6hUf16).unwrap(),
            "\"bc6h_uf16\""
        );
        assert_eq!(
            serde_json::to_string(&ColorTag::NotApplicable).unwrap(),
            "\"n/a\""
        );
        assert_eq!(
            serde_json::to_string(&AlphaPolicy::CutoutBlend).unwrap(),
            "\"cutout_blend\""
        );
        assert_eq!(
            serde_json::to_string(&Subtype::MsnSpecular).unwrap(),
            "\"msn_specular\""
        );
        assert_eq!(
            serde_json::from_str::<MipLevels>("\"full\"").unwrap(),
            MipLevels::Keyword(MipKeyword::Full)
        );
        assert_eq!(
            serde_json::from_str::<MipLevels>("3").unwrap(),
            MipLevels::Count(3)
        );
    }

    #[test]
    fn subtypes_map_to_the_categories_of_section_4_1() {
        assert_eq!(Subtype::ComplexGrass.id(), "complex_grass");
        assert_eq!(Subtype::ComplexGrass.category(), Category::MixedPacked);
        assert_eq!(Subtype::PbrEmissive.category(), Category::Color);
        assert_eq!(Subtype::Msn.category(), Category::Normal);
        assert_eq!(Subtype::Envmask.category(), Category::Grayscale);
        assert_eq!(Subtype::Flow.category(), Category::MixedPacked);
        assert_eq!(Subtype::HdrSky.category(), Category::Hdr);
        assert!(Subtype::SkinRfaos.is_roughness_member());
        assert!(Subtype::PbrFuzz.is_pbr_color());
        assert!(!Subtype::Glow.is_pbr_color());
        assert_eq!(Subtype::ComplexMaterial.id(), "complex_material");
    }

    #[test]
    fn ground_fill_parses_both_forms() {
        let p = |s: &str| toml::from_str::<CubemapSpec>(s).unwrap();
        assert_eq!(
            p("ground_fill = \"gradient\"").ground_fill,
            Some(GroundFill::Mode(GroundMode::Gradient))
        );
        let s = p("ground_fill = [10, 20, 30]\nhorizon = -5.0");
        assert_eq!(s.ground_fill, Some(GroundFill::Color([10, 20, 30])));
        assert_eq!(s.horizon, -5.0);
    }

    #[test]
    fn bad_ground_fill_names_the_accepted_forms() {
        for bad in [
            "\"gradiant\"",
            "[1, 2]",
            "[300, 0, 0]",
            "[1, 2, 3, 4]",
            "[-1, 0, 0]",
            "[1.5, 0, 0]",
            "5",
        ] {
            let e = toml::from_str::<CubemapSpec>(&format!("ground_fill = {bad}"))
                .unwrap_err()
                .to_string();
            assert!(
                e.contains("\"gradient\" or [r, g, b] with each 0..=255"),
                "{bad}: {e}"
            );
        }
    }
    #[test]
    fn subtype_all_order() {
        assert_eq!(Subtype::ALL.len(), 33);
        assert_eq!(
            Subtype::ALL[Subtype::ALL.len() - 3..],
            [
                Subtype::ProjectedNoise,
                Subtype::DetailNormal,
                Subtype::Gray
            ]
        );
        assert_eq!(Subtype::ALL[Subtype::ALL.len() - 4], Subtype::Cubemap);
        assert_eq!(Subtype::Cubemap.id(), "cubemap");
        assert_eq!(Subtype::Cubemap.category(), Category::Color);
    }

    #[test]
    fn gray_is_a_grayscale_bc4_read_from_red() {
        use crate::presets::PresetRegistry;
        assert_eq!(Subtype::Gray.id(), "gray");
        assert_eq!(Subtype::Gray.category(), Category::Grayscale);
        let p = PresetRegistry::builtin()
            .unwrap()
            .profile("sdc-base", Target::Cs, Subtype::Gray)
            .unwrap();
        assert_eq!(p.format, Format::Bc4);
        assert_eq!(p.color.tag, ColorTag::Unorm);
        assert_eq!(p.color.content, ColorContent::Linear);
        assert_eq!(p.alpha.policy, AlphaPolicy::Remove);
        assert_eq!(p.channels.grayscale_source, GraySource::R);
    }

    #[test]
    fn projected_noise_is_grayscale_bc4() {
        use crate::presets::PresetRegistry;
        assert_eq!(Subtype::ProjectedNoise.id(), "projected_noise");
        assert_eq!(Subtype::ProjectedNoise.category(), Category::Grayscale);
        let p = PresetRegistry::builtin()
            .unwrap()
            .profile("sdc-base", Target::Cs, Subtype::ProjectedNoise)
            .unwrap();
        assert_eq!(p.format, Format::Bc4);
        assert_eq!(p.color.tag, ColorTag::Unorm);
        assert_eq!(p.color.content, ColorContent::Linear);
        assert_eq!(crate::dds::dxgi_format(Format::Bc4, false), 80);
    }

    #[test]
    fn detail_normal_is_a_non_base_normal() {
        use crate::presets::PresetRegistry;
        assert_eq!(Subtype::DetailNormal.id(), "detail_normal");
        assert_eq!(Subtype::DetailNormal.category(), Category::Normal);
        assert_eq!(Subtype::DetailNormal.rnm_base(), None);
        let r = PresetRegistry::builtin().unwrap();
        for target in [Target::Cs, Target::Vanilla] {
            let d = r
                .profile("sdc-base", target, Subtype::DetailNormal)
                .unwrap();
            let mut l = r
                .profile("sdc-base", target, Subtype::LandscapeNormal)
                .unwrap();
            l.subtype = Subtype::DetailNormal;
            assert_eq!(d, l, "{target:?}");
        }
    }

    #[test]
    fn mip_spec_defaults_the_m4_fields() {
        let m: MipSpec = serde_json::from_str(
            r#"{"levels":"full","filter":"box","normal":"renormalize","coverage":"auto"}"#,
        )
        .unwrap();
        assert_eq!(
            (m.normal_base, m.bake_clamp),
            (NormalBase::Renormalize, 0.25)
        );
        assert_eq!(
            serde_json::to_string(&NormalBase::Keep).unwrap(),
            "\"keep\""
        );
    }

    #[test]
    fn schema_lists_enum_values() {
        let s = serde_json::to_string(&schemars::schema_for!(Profile)).unwrap();
        for needle in [
            "bc6h_uf16",
            "n/a",
            "cutout_blend",
            "renormalize_bake",
            "normal_base",
            "keep",
            "rgba16f",
        ] {
            assert!(s.contains(needle), "{needle} missing from schema");
        }
    }

    #[test]
    fn subtype_ids_are_unique_and_round_trip() {
        let mut ids: Vec<String> = Subtype::ALL.iter().map(|s| s.id()).collect();
        for s in Subtype::ALL {
            let back: Subtype = serde_json::from_str(&format!("\"{}\"", s.id())).unwrap();
            assert_eq!(back, s);
        }
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), Subtype::ALL.len());
    }

    /// M6: coverage preservation only ever applies to alpha-tested policies; `on` cannot force it onto blended
    /// or data alpha (spec §5.5: blended alpha keeps box-averaged opacity).
    #[test]
    fn coverage_applies_only_to_cutout_policies() {
        let reg = crate::presets::PresetRegistry::builtin().unwrap();
        let mut p = reg.base_profile(Target::Cs, Subtype::Diffuse);
        for (mode, policy, want) in [
            (CoverageMode::On, AlphaPolicy::Blend, false),
            (CoverageMode::On, AlphaPolicy::Data, false),
            (CoverageMode::On, AlphaPolicy::Remove, false),
            (CoverageMode::On, AlphaPolicy::Cutout, true),
            (CoverageMode::On, AlphaPolicy::CutoutBlend, true),
            (CoverageMode::Auto, AlphaPolicy::Cutout, true),
            (CoverageMode::Auto, AlphaPolicy::CutoutBlend, true),
            (CoverageMode::Auto, AlphaPolicy::Blend, false),
            (CoverageMode::Off, AlphaPolicy::Cutout, false),
            (CoverageMode::Off, AlphaPolicy::CutoutBlend, false),
        ] {
            p.mips.coverage = mode;
            p.alpha.policy = policy;
            assert_eq!(p.coverage_enabled(), want, "{mode:?} {policy:?}");
        }
        // Note FM: BC1 1-bit alpha is a cutout whatever the policy, so `blend` preserves coverage there too.
        p.format = Format::Bc1;
        p.encoder.bc1_alpha = true;
        for (mode, policy, want) in [
            (CoverageMode::Auto, AlphaPolicy::Blend, true),
            (CoverageMode::Off, AlphaPolicy::Blend, false),
            (CoverageMode::Auto, AlphaPolicy::Data, false),
        ] {
            p.mips.coverage = mode;
            p.alpha.policy = policy;
            assert_eq!(p.coverage_enabled(), want, "bc1_alpha {mode:?} {policy:?}");
        }
        p.format = Format::Bc7;
        p.alpha.policy = AlphaPolicy::Blend;
        assert!(!p.coverage_enabled(), "bc1_alpha is inert on BC7");
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let err = serde_json::from_str::<ColorSpec>(r#"{"content":"srgb","tag":"srgb","extra":1}"#);
        assert!(err.is_err());
        assert_eq!(Format::Bc7.block_bytes(), Some(16));
        assert_eq!(Format::Bc4.block_bytes(), Some(8));
        assert_eq!(Format::Rgba8.block_bytes(), None);
        assert!(!Format::Bc5.has_alpha());
    }

    #[test]
    fn uncompressed_formats_serialise_and_classify() {
        for (f, id) in [
            (Format::Rgba8, "rgba8"),
            (Format::Rgb8, "rgb8"),
            (Format::R8, "r8"),
            (Format::Rg8, "rg8"),
            (Format::Rgba16f, "rgba16f"),
        ] {
            assert_eq!(f.id(), id);
            assert!(!f.is_block());
        }
        assert!(Format::Rgba16f.is_hdr() && Format::Bc6hUf16.is_hdr() && !Format::Rgba8.is_hdr());
        assert!(
            Format::Rgba8.has_alpha() && Format::Rgba16f.has_alpha() && !Format::R8.has_alpha()
        );
        assert_eq!(
            (
                Format::Rgba8.bytes_per_pixel(),
                Format::Rgba16f.bytes_per_pixel()
            ),
            (4, 8)
        );
        // Note FM38: 24-bit RGB stores no alpha and packs three bytes a texel.
        assert!(!Format::Rgb8.has_alpha() && !Format::Rgb8.is_hdr());
        assert_eq!(Format::Rgb8.bytes_per_pixel(), 3);
    }

    #[test]
    fn m6_mip_and_resize_ids_match_the_spec() {
        for (f, id) in [
            (MipFilter::Box, "box"),
            (MipFilter::Tent, "tent"),
            (MipFilter::Mitchell, "mitchell"),
            (MipFilter::Kaiser, "kaiser"),
        ] {
            assert_eq!(serde_json::to_value(f).unwrap(), id);
        }
        assert_eq!(serde_json::to_value(MipEdge::Clamp).unwrap(), "clamp");
        assert_eq!(
            serde_json::to_value(ResizeMode::Quarter).unwrap(),
            "quarter"
        );
        assert_eq!(serde_json::to_value(ResizeMode::None).unwrap(), "none");
    }

    #[test]
    fn default_edge_and_resize_are_not_serialised() {
        // S-mips-9: profile_sha256 must not change for unchanged settings.
        let p = crate::presets::PresetRegistry::builtin()
            .unwrap()
            .profile("cs-quality", Target::Cs, Subtype::Diffuse)
            .unwrap();
        assert_eq!((p.mips.edge, p.resize), (MipEdge::Wrap, ResizeMode::None));
        let v = serde_json::to_value(&p).unwrap();
        assert!(
            v.get("resize").is_none()
                && v.get("resize_floor").is_none()
                && v["mips"].get("edge").is_none(),
            "{v}"
        );
        let mut q = p.clone();
        q.resize = ResizeMode::Eighth;
        q.resize_floor = 256;
        q.mips.edge = MipEdge::Clamp;
        let v = serde_json::to_value(&q).unwrap();
        assert_eq!(
            (
                v["resize"].as_str(),
                v["resize_floor"].as_u64(),
                v["mips"]["edge"].as_str()
            ),
            (Some("eighth"), Some(256), Some("clamp"))
        );
        assert_eq!(serde_json::from_value::<Profile>(v).unwrap(), q);
    }
}
