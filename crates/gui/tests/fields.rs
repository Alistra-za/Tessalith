mod support;
use engine::config::*;
use engine::presets::PresetRegistry;
use sdc_gui::model::fields::*;
use support::*;

#[test]
fn format_labels_state_bc_format_and_colour_space() {
    assert_eq!(format_label(Format::Bc7, ColorTag::Srgb), "BC7 · sRGB");
    assert_eq!(format_label(Format::Bc7, ColorTag::Unorm), "BC7 · linear");
    assert_eq!(
        format_label(Format::Bc5, ColorTag::Unorm),
        "BC5 · linear (two channels)"
    );
    assert_eq!(
        format_label(Format::Bc6hUf16, ColorTag::NotApplicable),
        "BC6H · HDR linear"
    );
}

#[test]
fn locks_say_why() {
    let reg = PresetRegistry::builtin().unwrap();
    let mut p = reg.base_profile(Target::Vanilla, Subtype::Diffuse);
    assert_eq!(
        field_lock(Target::Vanilla, &p, "color.tag"),
        Some("_SRGB is only written under cs")
    );
    p.alpha.policy = AlphaPolicy::Blend;
    assert_eq!(
        field_lock(Target::Vanilla, &p, "alpha.threshold"),
        Some("used only by cutout and cutout + blend")
    );
    // Note FM20: blend stored as BC1 1-bit alpha is alpha-tested, so its threshold and coverage apply.
    let mut b = p.clone();
    (b.format, b.encoder.bc1_alpha) = (Format::Bc1, true);
    assert_eq!(field_lock(Target::Vanilla, &b, "alpha.threshold"), None);
    assert!(!field_hidden(&b, "mips.coverage"));
    b.encoder.bc1_alpha = false;
    assert!(field_hidden(&b, "mips.coverage"));
    p.alpha.policy = AlphaPolicy::Cutout;
    assert_eq!(field_lock(Target::Vanilla, &p, "alpha.threshold"), None);
    let n = reg.base_profile(Target::Cs, Subtype::Normal);
    assert_eq!(field_lock(Target::Cs, &n, "mips.normal"), None);
    assert_eq!(
        field_lock(Target::Cs, &p, "mips.normal"),
        Some("normal maps only")
    );
    // BC6H wins over the target for the tag; perceptual needs colour, no data alpha and BC7.
    let mut h = reg.base_profile(Target::Cs, Subtype::HdrSky);
    h.format = Format::Bc6hUf16;
    assert_eq!(
        field_lock(Target::Cs, &h, "color.tag"),
        Some("BC6H has no _SRGB form")
    );
    assert!(field_lock(Target::Cs, &h, "encoder.perceptual").is_some());
}

#[test]
fn field_views_mark_edited_fields() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.set_field(
        "landscape/leaf.png",
        "alpha.threshold",
        serde_json::json!(90),
    );
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    let job = m.item("landscape/leaf.png").unwrap().job.clone().unwrap();
    let views = field_views(m.target, &job);
    let t = views.iter().find(|v| v.field == "alpha.threshold").unwrap();
    assert!(t.edited);
    assert_eq!(t.value, serde_json::json!(90));
    assert!(!views.iter().find(|v| v.field == "format").unwrap().edited);
    let mut groups: Vec<&str> = views.iter().map(|v| group_of(v.field)).collect();
    groups.dedup();
    assert_eq!(
        groups,
        vec!["Type", "Colour", "Alpha", "Mipmaps", "Compression"]
    );
}

#[test]
fn every_field_is_a_real_engine_path() {
    for (f, _) in FIELDS {
        assert!(engine::overrides::FIELD_PATHS.contains(&f), "{f}");
    }
    let d = tempfile::tempdir().unwrap();
    let m = opened(d.path());
    let job = m.item("landscape/leaf.png").unwrap().job.clone().unwrap();
    // A hidden row (`alpha.parallax` outside complex_material, DECISIONS AH2) may be unset.
    for v in field_views(m.target, &job)
        .into_iter()
        .filter(|v| !field_hidden(&job.profile, v.field))
    {
        assert!(
            !v.value.is_null(),
            "{} has no value in the profile JSON",
            v.field
        );
    }
}

#[test]
fn provenance_reads_as_prose() {
    assert_eq!(
        provenance_text("project_file"),
        "from this texture's settings"
    );
    assert_eq!(
        provenance_text("preset:cs-quality"),
        "from preset cs-quality"
    );
    assert_eq!(channel_text("default:0.5"), "default value 0.5");
    assert_eq!(channel_text("fill:255"), "fill value 255");
    assert_eq!(
        provenance_text("mesh:alpha"),
        "from the meshes that use this texture"
    );
}

#[test]
fn coverage_belongs_to_alpha_and_levels_to_the_drawer() {
    use sdc_gui::model::fields::{mips_count, mips_on, ADVANCED_FIELDS};
    assert_eq!(group_of("mips.coverage"), "Alpha");
    assert_eq!(group_of("mips.normal"), "Mipmaps");
    assert!(!FIELDS.iter().any(|(f, _)| *f == "mips.levels"));
    assert!(mips_on(&serde_json::json!("full")) && mips_on(&serde_json::json!(4)));
    assert!(!mips_on(&serde_json::json!("none")));
    assert_eq!(mips_count(&serde_json::json!(4)), Some(4));
    assert_eq!(mips_count(&serde_json::json!("full")), None);
    assert!(!ADVANCED_FIELDS.iter().any(|(f, _)| *f == "mips.levels"));
}

#[test]
fn normal_base_level_is_a_normal_map_field() {
    let reg = PresetRegistry::builtin().unwrap();
    let n = reg.base_profile(Target::Cs, Subtype::PbrNormal);
    let d = reg.base_profile(Target::Cs, Subtype::Diffuse);
    assert!(FIELDS.iter().any(|(f, _)| *f == "mips.normal_base"));
    assert_eq!(field_lock(Target::Cs, &n, "mips.normal_base"), None);
    assert_eq!(
        field_lock(Target::Cs, &d, "mips.normal_base"),
        Some("normal maps only")
    );
    assert_eq!(
        provenance_text("set"),
        "decided by the texture set (variance bake)"
    );
}

#[test]
fn uncompressed_format_labels_say_so() {
    assert_eq!(
        format_label(Format::Rgba8, ColorTag::Srgb),
        "RGBA8 · sRGB (uncompressed)"
    );
    assert_eq!(
        format_label(Format::R8, ColorTag::Unorm),
        "R8 · linear (one channel, uncompressed)"
    );
    assert_eq!(
        format_label(Format::Rg8, ColorTag::Unorm),
        "RG8 · linear (two channels, uncompressed)"
    );
    assert_eq!(
        format_label(Format::Rgba16f, ColorTag::NotApplicable),
        "RGBA16F · HDR linear (uncompressed)"
    );
}

#[test]
fn uncompressed_formats_are_offered_and_rgba16f_only_under_cs() {
    let reg = PresetRegistry::builtin().unwrap();
    let formats = |t: Target, st: Subtype| -> Vec<Format> {
        encoding_options(t, &reg.base_profile(t, st))
            .into_iter()
            .map(|((f, _), _)| f)
            .collect()
    };
    for f in [Format::Rgba8, Format::R8, Format::Rg8] {
        assert!(
            formats(Target::Vanilla, Subtype::Diffuse).contains(&f),
            "{f:?}"
        );
    }
    assert!(formats(Target::Cs, Subtype::HdrSky).contains(&Format::Rgba16f));
    assert!(!formats(Target::Vanilla, Subtype::Diffuse).contains(&Format::Rgba16f));
}

/// Note FM37: a Gray map's Format dropdown lists BC4 and R8 only, whatever the target.
#[test]
fn the_gray_format_dropdown_has_two_entries() {
    let reg = PresetRegistry::builtin().unwrap();
    for t in [Target::Cs, Target::Vanilla] {
        let o = encoding_options(t, &reg.base_profile(t, Subtype::Gray));
        let formats: Vec<_> = o.iter().map(|((f, tag), _)| (*f, *tag)).collect();
        assert_eq!(
            formats,
            [
                (Format::Bc4, ColorTag::Unorm),
                (Format::R8, ColorTag::Unorm)
            ],
            "{t:?}: {o:?}"
        );
    }
}

#[test]
fn uncompressed_locks_explain_themselves() {
    let reg = engine::presets::PresetRegistry::builtin().unwrap();
    let mut p = reg.base_profile(Target::Cs, Subtype::Height);
    p.format = Format::R8;
    assert_eq!(
        field_lock(Target::Cs, &p, "color.tag"),
        Some("R8 and RG8 have no _SRGB form")
    );
    assert_eq!(
        field_lock(Target::Cs, &p, "encoder.effort"),
        Some("no encoder runs for uncompressed formats")
    );
    p.format = Format::Rgba16f;
    assert_eq!(
        field_lock(Target::Cs, &p, "color.tag"),
        Some("float formats have no _SRGB form")
    );
}

#[test]
fn filter_and_edge_stay_in_the_basic_mipmaps_group() {
    assert_eq!(group_of(FILTER_FIELD.0), "Mipmaps");
    assert_eq!(group_of(EDGE_FIELD.0), "Mipmaps");
    assert_eq!(group_of("mips.filter"), "Mipmaps");
    assert_eq!(group_of("mips.edge"), "Mipmaps");
    assert!(!ADVANCED_FIELDS.iter().any(|f| f.0 == "mips.filter"));
    assert!(ADVANCED_FIELDS.iter().any(|f| f.0 == "resize"));
    for f in ["mips.filter", "mips.edge", "resize"] {
        assert!(engine::overrides::FIELD_PATHS.contains(&f), "{f}");
    }
}

#[test]
fn filter_options_lock_per_s_mips_2() {
    use MipFilter::*;
    let reg = PresetRegistry::builtin().unwrap();
    let mut p = reg.base_profile(Target::Cs, Subtype::Diffuse);
    p.alpha.policy = AlphaPolicy::Remove;
    for f in [Box, Tent, Mitchell, Kaiser] {
        assert_eq!(filter_option_lock(&p, f), None, "diffuse {f:?}");
    }
    let n = reg.base_profile(Target::Cs, Subtype::Normal);
    assert_eq!(filter_option_lock(&n, Box), None);
    for f in [Tent, Mitchell, Kaiser] {
        assert!(filter_option_lock(&n, f).is_some(), "normal {f:?}");
    }
    let mut c = p.clone();
    c.alpha.policy = AlphaPolicy::Cutout;
    assert_eq!(filter_option_lock(&c, Box), None);
    for f in [Mitchell, Kaiser] {
        assert!(
            filter_option_lock(&c, f).is_some_and(|r| r.contains("box or tent")),
            "cutout {f:?}"
        );
    }
    let mut e = reg.base_profile(Target::Cs, Subtype::PbrEmissive);
    e.format = Format::Bc6hUf16;
    e.alpha.policy = AlphaPolicy::Remove;
    assert_eq!(filter_option_lock(&e, Tent), None);
    assert!(filter_option_lock(&e, Mitchell).is_some());
    assert!(filter_option_lock(&e, Kaiser).is_some());
    // F2: rgba16f is HDR too; tent only without alpha weighting.
    let mut r = p.clone();
    r.format = Format::Rgba16f;
    assert_eq!(filter_option_lock(&r, Tent), None);
    assert!(filter_option_lock(&r, Kaiser).is_some());
    r.alpha.policy = AlphaPolicy::Cutout;
    assert!(filter_option_lock(&r, Tent).is_some_and(|m| m.contains("alpha weighting")));
}

/// Note FM28: alpha-weighted colour (cutout, blend, cutout_blend) can pick tent; Mitchell and Kaiser stay barred
/// with the reason as the option's hover text.
#[test]
fn the_filter_menu_offers_tent_for_a_cutout_texture() {
    use MipFilter::*;
    let reg = PresetRegistry::builtin().unwrap();
    for policy in [
        AlphaPolicy::Cutout,
        AlphaPolicy::Blend,
        AlphaPolicy::CutoutBlend,
    ] {
        let mut p = reg.base_profile(Target::Cs, Subtype::Diffuse);
        p.alpha.policy = policy;
        assert_eq!(filter_option_lock(&p, Box), None, "{policy:?}");
        assert_eq!(filter_option_lock(&p, Tent), None, "{policy:?}");
        for f in [Mitchell, Kaiser] {
            assert!(filter_option_lock(&p, f).is_some(), "{policy:?} {f:?}");
        }
    }
}

#[test]
fn edge_is_locked_while_the_filter_is_box() {
    let reg = PresetRegistry::builtin().unwrap();
    let mut p = reg.base_profile(Target::Cs, Subtype::Diffuse);
    p.mips.filter = MipFilter::Box;
    assert_eq!(
        field_lock(Target::Cs, &p, "mips.edge"),
        Some("only tent, Mitchell and Kaiser read the edge")
    );
    p.mips.filter = MipFilter::Kaiser;
    assert_eq!(field_lock(Target::Cs, &p, "mips.edge"), None);
    assert_eq!(field_lock(Target::Cs, &p, "mips.filter"), None);
}

/// F5: default `mips.edge` and `resize` are not serialised; the views fall back to their defaults.
#[test]
fn filter_edge_and_resize_views_show_their_defaults() {
    let d = tempfile::tempdir().unwrap();
    let m = opened(d.path());
    let job = m.item("landscape/leaf.png").unwrap().job.clone().unwrap();
    assert_eq!(filter_view(m.target, &job).value, serde_json::json!("box"));
    assert_eq!(edge_view(m.target, &job).value, serde_json::json!("wrap"));
    let resize = advanced_field_views(m.target, &job)
        .into_iter()
        .find(|v| v.field == "resize")
        .unwrap();
    assert_eq!(resize.value, serde_json::json!("none"));
    // Note FM29: the floor shows its default 512 and stays editable while nothing halves, so Copy settings can
    // carry it together with `resize`.
    let floor = advanced_field_views(m.target, &job)
        .into_iter()
        .find(|v| v.field == "resize_floor")
        .unwrap();
    assert_eq!((floor.value, floor.lock), (serde_json::json!(512), None));
}

/// Note FM29: the Resize row offers None, Half, Quarter and Eighth (the power of two is automatic); the Size floor
/// row offers powers of two.
#[test]
fn the_resize_row_offers_the_halving_steps() {
    let p = PresetRegistry::builtin()
        .unwrap()
        .base_profile(Target::Cs, Subtype::Diffuse);
    let values = |f: &str| -> Vec<serde_json::Value> {
        sdc_gui::ui::presets::field_options(f, Target::Cs, &p)
            .into_iter()
            .map(|(v, _)| v)
            .collect()
    };
    assert_eq!(
        values("resize"),
        serde_json::json!(["none", "half", "quarter", "eighth"])
            .as_array()
            .unwrap()
            .clone()
    );
    assert!(values("resize_floor").contains(&serde_json::json!(256)));
    assert_eq!(field_lock(Target::Cs, &p, "resize_floor"), None);
    // A hand-written 300 is listed in order and reads "300 px".
    let mut q = p.clone();
    q.resize_floor = 300;
    let opts = sdc_gui::ui::presets::field_options("resize_floor", Target::Cs, &q);
    let at = opts.iter().position(|(v, _)| *v == serde_json::json!(300));
    assert_eq!(at, Some(3), "{opts:?}");
    assert_eq!(opts[3].1, "300 px");
}

/// Owner, 2026-10-13: the format pickers list only what the engine allows (`policy::allowed_encodings`).
#[test]
fn format_pickers_offer_only_compatible_encodings() {
    let reg = PresetRegistry::builtin().unwrap();
    let pairs = |st: Subtype| -> Vec<(Format, ColorTag)> {
        let p = reg.base_profile(Target::Cs, st);
        encoding_options(Target::Cs, &p)
            .into_iter()
            .map(|(e, _)| e)
            .collect()
    };
    let grass = reg.base_profile(Target::Cs, Subtype::ComplexGrass);
    assert_eq!(
        encoding_options(Target::Cs, &grass),
        vec![((Format::Bc7, ColorTag::Unorm), "BC7 · linear".to_string())]
    );
    assert!(pairs(Subtype::PbrAlbedo)
        .iter()
        .all(|(_, t)| *t == ColorTag::Srgb));
    let n = pairs(Subtype::Normal);
    assert!(n.contains(&(Format::Bc7, ColorTag::Unorm)));
    assert!(n
        .iter()
        .all(|(f, _)| !matches!(f, Format::Bc4 | Format::Bc6hUf16)));
    let mut emissive = reg.base_profile(Target::Cs, Subtype::PbrEmissive);
    assert!(encoding_options(Target::Cs, &emissive)
        .iter()
        .any(|(e, _)| *e == (Format::Bc6hUf16, ColorTag::NotApplicable)));
    // Vanilla has no HDR.
    let d = reg.base_profile(Target::Vanilla, Subtype::Diffuse);
    assert!(encoding_options(Target::Vanilla, &d)
        .iter()
        .all(|((f, t), _)| !f.is_hdr() && *t == ColorTag::Unorm));
    emissive.color.tag = ColorTag::Srgb;
    emissive.format = Format::Bc7;
    assert!(!encoding_options(Target::Cs, &emissive)
        .iter()
        .any(|(_, l)| l.ends_with(NOT_VALID)));
}

/// An incompatible current value (an old project, a hand edit) stays listed, marked, never rewritten.
#[test]
fn an_incompatible_current_encoding_is_kept_and_marked() {
    let reg = PresetRegistry::builtin().unwrap();
    let mut n = reg.base_profile(Target::Cs, Subtype::Normal);
    n.format = Format::Bc4;
    let o = encoding_options(Target::Cs, &n);
    assert!(o.contains(&(
        (Format::Bc4, ColorTag::Unorm),
        format!("BC4 · linear (one channel){NOT_VALID}")
    )));
    assert_eq!(o.iter().filter(|(_, l)| l.ends_with(NOT_VALID)).count(), 1);
    let mut a = reg.base_profile(Target::Cs, Subtype::PbrAlbedo);
    a.color.tag = ColorTag::Unorm;
    assert!(encoding_options(Target::Cs, &a).contains(&(
        (Format::Bc7, ColorTag::Unorm),
        format!("BC7 · linear{NOT_VALID}")
    )));
}

#[test]
fn every_format_label_starts_with_its_one_name() {
    for f in [
        Format::Bc1,
        Format::Bc3,
        Format::Bc7,
        Format::Bc4,
        Format::Bc5,
        Format::Bc6hUf16,
        Format::Rgba8,
        Format::Rgb8,
        Format::R8,
        Format::Rg8,
        Format::Rgba16f,
    ] {
        let label = format_label(f, ColorTag::Srgb);
        assert!(
            label.starts_with(&format!("{} · ", format_name(f))),
            "{label}"
        );
    }
    assert_eq!(format_name(Format::Bc6hUf16), "BC6H");
}

/// A `skin_wet` picker never offers R8 as valid: CS's loader reads no one-channel skin map, grey source or not.
#[test]
fn a_skin_wet_picker_never_offers_r8() {
    let reg = PresetRegistry::builtin().unwrap();
    let mut p = reg.base_profile(Target::Cs, Subtype::SkinWet);
    let r8 = |p: &Profile| {
        encoding_options(Target::Cs, p)
            .into_iter()
            .any(|((f, _), l)| f == Format::R8 && !l.ends_with(NOT_VALID))
    };
    assert!(!r8(&p), "a colour skin_wet has no R8");
    p.category = Category::Grayscale;
    assert!(!r8(&p));
}

/// Note FM: the "1-bit alpha" row sits in the Alpha group and is drawn only for BC1. Hidden, it leaves no gap: the
/// row after it moves up into its place.
#[test]
fn the_one_bit_alpha_box_shows_only_for_bc1() {
    use egui_kittest::kittest::Queryable;
    const F: &str = "encoder.bc1_alpha";
    assert!(FIELDS.contains(&(F, "1-bit alpha")));
    assert_eq!(group_of(F), "Alpha");
    let reg = PresetRegistry::builtin().unwrap();
    let mut p = reg.base_profile(Target::Cs, Subtype::Diffuse);
    p.format = Format::Bc7;
    assert_eq!(hidden_reason(&p, F), Some("BC1 only"));
    p.format = Format::Bc1;
    assert_eq!(hidden_reason(&p, F), None);

    const LEAF: &str = "landscape/leaf.png";
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.decide(LEAF, sdc_gui::model::Answer::Alpha(AlphaPolicy::Cutout));
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    m.select_item(LEAF);
    let mut h = egui_kittest::Harness::builder()
        .with_size(eframe::egui::vec2(1280.0, 800.0))
        .build_ui_state(
            |ui, model: &mut sdc_gui::model::AppModel| sdc_gui::ui::draw(ui, model),
            m,
        );
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h.run();
    let top = |h: &egui_kittest::Harness<'_, sdc_gui::model::AppModel>, label: &str| {
        h.query_all_by_label(label).next().map(|n| n.rect().top())
    };
    assert_ne!(
        h.state()
            .item(LEAF)
            .unwrap()
            .job
            .as_ref()
            .unwrap()
            .profile
            .format,
        Format::Bc1
    );
    assert_eq!(top(&h, "1-bit alpha"), None, "no row outside BC1");
    // Offsets from the Coverage row: what is drawn above the Alpha group may change with the format.
    let next_row =
        top(&h, "Exact opaque alpha").expect("the row after it") - top(&h, "Coverage").unwrap();

    h.state_mut()
        .set_field(LEAF, "format", serde_json::json!("bc1"));
    h.state_mut().wait_idle(std::time::Duration::from_secs(60));
    h.run();
    let coverage = top(&h, "Coverage").unwrap();
    let row = top(&h, "1-bit alpha").expect("the row under BC1") - coverage;
    assert!(row > 0.0, "inside the Alpha group, after Coverage");
    assert!(
        (row - next_row).abs() < 1.0,
        "hidden, the row left a gap: {row} vs {next_row}"
    );
    assert!(top(&h, "Exact opaque alpha").unwrap() - coverage > row);
}

/// Note FM38: RGB8 is listed beside RGBA8 UNORM, linear only; a cubemap and a Gray map keep their own lists.
#[test]
fn the_dropdown_offers_rgb8_where_rgba8_unorm_is_offered() {
    let reg = PresetRegistry::builtin().unwrap();
    assert_eq!(
        format_label(Format::Rgb8, ColorTag::Unorm),
        "RGB8 · linear (24-bit, uncompressed)"
    );
    for t in [Target::Cs, Target::Vanilla] {
        for st in [
            Subtype::Diffuse,
            Subtype::Normal,
            Subtype::Envmask,
            Subtype::Height,
        ] {
            let o: Vec<_> = encoding_options(t, &reg.base_profile(t, st))
                .into_iter()
                .map(|(pair, _)| pair)
                .collect();
            assert_eq!(
                o.contains(&(Format::Rgb8, ColorTag::Unorm)),
                o.contains(&(Format::Rgba8, ColorTag::Unorm)),
                "{st:?} {t:?}: {o:?}"
            );
            assert!(!o.contains(&(Format::Rgb8, ColorTag::Srgb)), "{st:?} {t:?}");
        }
    }
    let mut p = reg.base_profile(Target::Cs, Subtype::Diffuse);
    p.format = Format::Rgb8;
    assert_eq!(
        field_lock(Target::Cs, &p, "color.tag"),
        Some("RGB8 has no _SRGB form")
    );
    let o = encoding_options(Target::Cs, &reg.base_profile(Target::Cs, Subtype::Diffuse));
    assert!(o
        .iter()
        .any(|((f, _), l)| *f == Format::Rgb8 && !l.ends_with(NOT_VALID)));
}

/// L62: the preset editor's `value_of` reads every key as the field views do, the keys the profile does not serialise
/// (`unset_default`) included.
#[test]
fn value_of_agrees_with_the_field_views() {
    let d = tempfile::tempdir().unwrap();
    let m = opened(d.path());
    let job = m.item("landscape/leaf.png").unwrap().job.clone().unwrap();
    let doc = serde_json::to_value(&job.profile).unwrap();
    let mut unserialised = 0;
    for (key, _) in all_fields().into_iter().filter(|(f, _)| *f != WEIGHTING) {
        let Some(view) = field_view(m.target, &job, key) else {
            continue;
        };
        if view.field == "format" {
            continue;
        }
        if doc
            .pointer(&format!("/{}", key.replace('.', "/")))
            .is_none()
        {
            unserialised += 1;
        }
        assert_eq!(
            sdc_gui::ui::presets::value_of(&job.profile, key),
            view.value,
            "{key}"
        );
    }
    assert!(unserialised > 0, "no key exercised unset_default");
}
