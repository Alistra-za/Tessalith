//! Resize (spec §5.4 step 6, Note FM29): every texture with mips is written at a power-of-two size ([`pot_dims`]),
//! then `resize = half | quarter | eighth` halves it down to `resize_floor` ([`shrink`]). A texture with
//! `mips.levels = "none"` keeps its own size and still shrinks. A growing axis is first enlarged by linear
//! interpolation ([`enlarge`]); a shrinking one uses the down path, in the content's domain and with the mip filter.
//! Alpha is always the area-exact box; RGB is the box too unless [`mips::effective_filter`] gives tent, Mitchell or
//! Kaiser. Roughness is resized on r², alpha-weighted colour on area × alpha, exactly as the box chain does. Normals
//! are not renormalised here (that is `base_level`'s job).
use crate::config::{MipFilter, MipKeyword, MipLevels, Profile, ResizeMode, Subtype};
use crate::image::Rgba32F;
use crate::mips;
use crate::ops::Work;
use rayon::prelude::*;

/// The largest power of two ≤ `v` (`v` ≥ 1).
fn floor_pow2(v: u64) -> u64 {
    1 << v.ilog2()
}

/// The power-of-two size of a `w`×`h` image (spec B4). The longer side L rounds up to the next power of two when it
/// is within 5 % of it, else down. The shorter side is first scaled with it (Ss = S × L' / L), then rounds up when
/// within 15 % of the next power of two, else down (owner, 2026-11-02: 1024×768 becomes 1024×512, 2000×900 becomes
/// 2048×1024); never below 1. Axes keep their orientation.
pub fn pot_dims((w, h): (u32, u32)) -> (u32, u32) {
    let (l, s) = (u64::from(w.max(h).max(1)), u64::from(w.min(h).max(1)));
    let lp = if l.is_power_of_two() {
        l
    } else {
        let down = floor_pow2(l);
        if 100 * l >= 95 * 2 * down {
            2 * down
        } else {
            down
        }
    };
    // Ss = num / den, compared in integers.
    let (num, den) = (s * lp, l);
    let sp = if num < den {
        1
    } else {
        let down = floor_pow2(num / den);
        if down * den == num || 100 * num < 85 * 2 * down * den {
            down
        } else {
            2 * down
        }
    };
    let (lp, sp) = (lp as u32, sp as u32);
    if w >= h {
        (lp, sp)
    } else {
        (sp, lp)
    }
}

/// `half`, `quarter` and `eighth` halve both sides (integer division, never below 1) up to 1, 2 or 3 times, each
/// time only while the longer side / 2 stays at or above `floor`, so the shape never changes.
pub fn shrink((mut w, mut h): (u32, u32), mode: ResizeMode, floor: u32) -> (u32, u32) {
    let times = match mode {
        ResizeMode::None => 0,
        ResizeMode::Half => 1,
        ResizeMode::Quarter => 2,
        ResizeMode::Eighth => 3,
    };
    for _ in 0..times {
        if w.max(h) / 2 < floor {
            break;
        }
        (w, h) = ((w / 2).max(1), (h / 2).max(1));
    }
    (w, h)
}

/// A texture written at its own size: `mips.levels = "none"` (menu and sky art) skips [`pot_dims`].
pub fn pot_exempt(p: &Profile) -> bool {
    p.mips.levels == MipLevels::Keyword(MipKeyword::None)
}

/// The size Preprocess writes: [`pot_dims`] unless `mips.levels = "none"`, then [`shrink`]. A cube is sized by
/// `cubemap.face_size` (`cube::layout::face_size`), so its source keeps its size here. With a planned
/// `resize_reference` (Note FM44) the map is instead halved, as `shrink` halves, while its longer side is above the
/// reference's longer side after the same power-of-two step and `shrink`; `[0, 0]` (no single map in the slot)
/// falls back to the map's own size, as `self` (ruling T28B-f).
pub fn target_dims(dims: (u32, u32), p: &Profile) -> (u32, u32) {
    if p.subtype == Subtype::Cubemap {
        return dims;
    }
    let pot = |d| if pot_exempt(p) { d } else { pot_dims(d) };
    let (mut w, mut h) = pot(dims);
    match p.resize_reference {
        None | Some((0, _) | (_, 0)) => shrink((w, h), p.resize, p.resize_floor),
        Some(r) => {
            let (rw, rh) = shrink(pot(r), p.resize, p.resize_floor);
            while w.max(h) > rw.max(rh) && w.max(h) / 2 >= p.resize_floor {
                (w, h) = ((w / 2).max(1), (h / 2).max(1));
            }
            (w, h)
        }
    }
}

/// Note FM44: sets every job's `resize_reference` from its `resize_relative_to`, once the plan's jobs are final.
/// `source` is a job's source size (a packed or derived job's sources' common size). A slot is read only with a
/// `resize` mode and when it is not the map's own slot (relative to itself is the self rule); with no single map in
/// the slot the reference is `[0, 0]`, the map is resized from its own size and gets RESIZE-NO-REFERENCE. A relative
/// job's DIM-RESIZED is recomputed; a `self` job's warnings are left as `check_profile` made them.
pub(crate) fn plan_references(
    jobs: &mut [crate::policy::ResolvedJob],
    source: impl Fn(&crate::policy::ResolvedJob) -> Option<(u32, u32)>,
) {
    use crate::codes::{Code, Warning};
    let refs: Vec<Option<(u32, u32, usize)>> = jobs
        .iter()
        .map(|j| {
            let p = &j.profile;
            let n = p.resize_relative_to.slot()?;
            // A cube is sized by `cubemap.face_size`, as `target_dims` says (ruling FR-c).
            if p.resize.is_none()
                || p.subtype == Subtype::Cubemap
                || p.subtype.txst_slot() == Some(n)
            {
                return None;
            }
            let maps: Vec<_> = jobs
                .iter()
                .filter(|o| o.set_key == j.set_key && o.profile.subtype.txst_slot() == Some(n))
                .collect();
            Some(match maps[..] {
                [one] => source(one).map_or((0, 0, 1), |(w, h)| (w, h, 1)),
                _ => (0, 0, maps.len()),
            })
        })
        .collect();
    for (j, r) in jobs.iter_mut().zip(refs) {
        j.profile.resize_reference = r.map(|(w, h, _)| (w, h));
        let Some((w, h, maps)) = r else {
            continue;
        };
        let n = j.profile.resize_relative_to.slot().unwrap_or(0);
        if (w, h) == (0, 0) {
            let what = match maps {
                0 => "no map".to_string(),
                1 => "a map whose size could not be read".to_string(),
                _ => format!("{maps} maps"),
            };
            j.warnings.push(
                Warning::new(
                    Code::ResizeNoReference,
                    format!(
                        "resize_relative_to = \"slot{n}\": the set has {what} in slot {n}, so this map was resized from its own size"
                    ),
                )
                .with_data(serde_json::json!({ "slot": n, "maps": maps })),
            );
        }
        if let Some(src) = source(j) {
            j.warnings.retain(|w| w.code != Code::DimResized);
            j.warnings.extend(crate::policy::resized_warning(
                src,
                target_dims(src, &j.profile),
                &j.profile,
            ));
        }
    }
}

/// One axis resampled to `n` texels by linear interpolation, texel-centre aligned, edges clamped.
fn lerp_axis(img: &Rgba32F, n: u32, along_x: bool) -> Rgba32F {
    let (w, h) = (img.width, img.height);
    let len = if along_x { w } else { h };
    if n == len {
        return img.clone();
    }
    let last = f64::from(len - 1);
    let taps: Vec<(u32, u32, f32)> = (0..n)
        .map(|i| {
            let s = ((f64::from(i) + 0.5) * f64::from(len) / f64::from(n) - 0.5).clamp(0.0, last);
            let i0 = s.floor() as u32;
            (i0, (i0 + 1).min(len - 1), (s - f64::from(i0)) as f32)
        })
        .collect();
    let (ow, oh) = if along_x { (n, h) } else { (w, n) };
    let mut out = Rgba32F::new(ow, oh, [0.0; 4]);
    out.px
        .par_chunks_mut(ow as usize)
        .enumerate()
        .for_each(|(y, row)| {
            let y = y as u32;
            for (x, o) in (0..ow).zip(row) {
                let (a, b, t) = if along_x {
                    let (i0, i1, t) = taps[x as usize];
                    (img.at(i0, y), img.at(i1, y), t)
                } else {
                    let (i0, i1, t) = taps[y as usize];
                    (img.at(x, i0), img.at(x, i1), t)
                };
                *o = std::array::from_fn(|c| a[c] + (b[c] - a[c]) * t);
            }
        });
    out
}

/// `img` enlarged to `to` (each side ≥ the source's) by linear interpolation on all four channels; with `weighted`,
/// RGB is interpolated premultiplied by alpha, and where the interpolated alpha is 0 the plain RGB is kept.
pub(crate) fn enlarge(img: &Rgba32F, to: (u32, u32), weighted: bool) -> Rgba32F {
    let lerp = |src: &Rgba32F| lerp_axis(&lerp_axis(src, to.0, true), to.1, false);
    if !weighted {
        return lerp(img);
    }
    let mut pm = img.clone();
    for px in &mut pm.px {
        for c in 0..3 {
            px[c] *= px[3];
        }
    }
    let mut out = lerp(&pm);
    let plain = lerp(img);
    for (o, q) in out.px.iter_mut().zip(&plain.px) {
        if o[3] > 0.0 {
            for c in 0..3 {
                o[c] /= o[3];
            }
        } else {
            o[..3].copy_from_slice(&q[..3]);
        }
    }
    out
}

/// Resize `work.img` to [`target_dims`] (a no-op when the size already fits): growing axes are enlarged first,
/// shrinking ones then take the down path.
pub fn resize_work(work: &mut Work, p: &Profile) {
    let (w, h) = (work.img.width, work.img.height);
    let to = target_dims((w, h), p);
    if to == (w, h) {
        return;
    }
    let weighted = mips::weighted_rgb(work, p);
    let filt = mips::effective_filter(work, p);
    if work.roughness_r_squared {
        for px in &mut work.img.px {
            px[0] *= px[0];
        }
    }
    let grown = (to.0.max(w), to.1.max(h));
    if grown != (w, h) {
        work.img = enlarge(&work.img, grown, weighted);
    }
    let down = grown != to;
    if down {
        // Alpha (and RGB for box) is the area-exact box; `filt` is tent or box while `weighted`.
        let mut out = mips::area_resample(&work.img, to, weighted);
        if filt != MipFilter::Box {
            let direct = mips::resample_direct(&work.img, to, filt, weighted, p.mips.edge);
            for (px, d) in out.px.iter_mut().zip(&direct.px) {
                px[..3].copy_from_slice(&d[..3]);
            }
        }
        work.img = out;
    }
    for px in &mut work.img.px {
        if work.roughness_r_squared {
            px[0] = px[0].max(0.0).sqrt();
        }
        // Only the polyphase pass overshoots (M8): a box resize and linear interpolation stay in range, so a float
        // LDR source keeps any value > 1 until Quantise, as without resize.
        if down && filt != MipFilter::Box {
            mips::clamp_rgb(px, work.domain);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::*;
    use crate::image::Rgba32F;
    use crate::mips::{area_resample, build_chain};
    use crate::ops::{preprocess, Domain};
    use crate::presets::PresetRegistry;

    fn profile(st: Subtype) -> Profile {
        PresetRegistry::builtin()
            .unwrap()
            .profile("cs-quality", Target::Cs, st)
            .unwrap()
    }
    fn work(img: Rgba32F, domain: Domain, alpha: bool) -> Work {
        Work {
            img,
            alpha_present: alpha,
            domain,
            roughness_r_squared: false,
            nonfinite_samples: 0,
        }
    }
    fn close(a: f32, b: f64) -> bool {
        (f64::from(a) - b).abs() < 1e-6
    }
    fn rnd_image(w: u32, h: u32, seed: u32) -> Rgba32F {
        let mut seed = seed;
        let mut rnd = || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            (seed >> 8) as f32 / 16_777_216.0
        };
        let mut img = Rgba32F::new(w, h, [0.0; 4]);
        for p in &mut img.px {
            *p = [rnd(), rnd(), rnd(), rnd()];
        }
        img
    }

    /// Note FM44: `plan_references` leaves a `self` job's warnings alone, even when its source size (a packed job's
    /// common size, a cube's cell) differs from what `check_profile` measured; only a relative job's DIM-RESIZED is
    /// recomputed.
    #[test]
    fn a_self_jobs_dim_resized_is_left_alone() {
        use crate::codes::{Code, Warning};
        let job = |st: Subtype, rel: ResizeRelativeTo| {
            let mut profile = profile(st);
            profile.resize = ResizeMode::Half;
            profile.resize_floor = 16;
            profile.resize_relative_to = rel;
            crate::policy::ResolvedJob {
                input: "a.png".into(),
                output: "a.dds".into(),
                game_path: "a".into(),
                set_key: "x/a".into(),
                profile,
                provenance: Default::default(),
                suggestions: vec![],
                warnings: vec![
                    Warning::new(Code::DimResized, "from check_profile"),
                    Warning::new(Code::DimNpot, "after it"),
                ],
                status: crate::policy::JobStatus::Ready,
                suffix_alias: None,
                source: None,
                pack: None,
                bake: None,
                derive: None,
                shader: None,
                cube: None,
                fill: None,
                unknown_suffix: false,
            }
        };
        let mut jobs = vec![
            job(Subtype::Rmaos, ResizeRelativeTo::Own),
            job(Subtype::Normal, ResizeRelativeTo::Slot6),
        ];
        let before = jobs[0].warnings.clone();
        plan_references(&mut jobs, |_| Some((2048, 2048)));
        assert_eq!(jobs[0].warnings, before);
        assert_eq!(jobs[0].profile.resize_reference, None);
        // The relative job in the same pass is recomputed: its DIM-RESIZED now names the source size.
        assert_eq!(jobs[1].profile.resize_reference, Some((2048, 2048)));
        let dim: Vec<_> = jobs[1]
            .warnings
            .iter()
            .filter(|w| w.code == Code::DimResized)
            .collect();
        assert_eq!(dim.len(), 1);
        assert_ne!(dim[0].message, "from check_profile");
    }

    /// Note FM29 (spec B4): the long side rounds up within 5 %, the short side at its scale within 15 %.
    #[test]
    fn pot_rule_examples() {
        for ((w, h), out) in [
            ((1000, 1000), (1024, 1024)),
            ((2000, 1000), (2048, 1024)),
            ((2000, 900), (2048, 1024)),
            ((4096, 1000), (4096, 1024)),
            ((900, 900), (512, 512)),
            ((973, 973), (1024, 1024)),
            ((972, 972), (512, 512)),
            ((1024, 768), (1024, 512)),
            ((768, 1024), (512, 1024)),
            ((1024, 880), (1024, 1024)),
            ((1024, 860), (1024, 512)),
            ((3, 5), (2, 4)),
            ((4096, 1), (4096, 1)),
            ((1, 1), (1, 1)),
            ((512, 256), (512, 256)),
        ] {
            assert_eq!(pot_dims((w, h)), out, "{w}x{h}");
        }
        // The short side scales below 1: it stays 1.
        assert_eq!(pot_dims((4000, 1)), (4096, 1));
        assert_eq!(pot_dims((900, 1)), (512, 1));
    }

    #[test]
    fn shrink_respects_the_floor() {
        for (mode, out) in [
            (ResizeMode::Half, 2048),
            (ResizeMode::Quarter, 1024),
            (ResizeMode::Eighth, 512),
        ] {
            assert_eq!(shrink((4096, 4096), mode, 512).0, out);
            assert_eq!(shrink((1024, 1024), mode, 512).0, 512);
        }
        assert_eq!(shrink((512, 512), ResizeMode::Half, 256).0, 256);
        assert_eq!(shrink((512, 256), ResizeMode::Eighth, 512), (512, 256));
        assert_eq!(shrink((2048, 1), ResizeMode::Quarter, 512), (512, 1));
        assert_eq!(shrink((4096, 4096), ResizeMode::None, 1), (4096, 4096));
    }

    /// Menu and sky art (`mips.levels = "none"`) keeps its own size, and still shrinks; a cube source is never resized
    /// here (its face size decides).
    #[test]
    fn no_mip_textures_keep_their_size() {
        let mut p = profile(Subtype::Diffuse);
        p.mips.levels = MipLevels::Keyword(MipKeyword::None);
        assert_eq!(target_dims((1000, 600), &p), (1000, 600));
        p.resize = ResizeMode::Half;
        p.resize_floor = 256;
        assert_eq!(target_dims((1000, 600), &p), (500, 300));
        assert_eq!(
            target_dims((1000, 600), &profile(Subtype::Diffuse)),
            (1024, 512)
        );
        assert_eq!(
            target_dims((384, 64), &profile(Subtype::Cubemap)),
            (384, 64)
        );
    }

    #[test]
    fn enlarge_keeps_a_constant_image_constant() {
        let c = [0.25, 0.5, 0.75, 0.4];
        let img = Rgba32F::new(5, 3, c);
        assert!(enlarge(&img, (8, 4), false).px.iter().all(|p| *p == c));
        for p in &enlarge(&img, (8, 4), true).px {
            assert!(p.iter().zip(c).all(|(a, b)| (a - b).abs() < 1e-6), "{p:?}");
        }
        // One axis grows, the other shrinks: 31x20 is written 32x16, still constant.
        let mut w = work(Rgba32F::new(31, 20, c), Domain::LinearLight, true);
        let mut p = profile(Subtype::Diffuse);
        p.alpha.policy = AlphaPolicy::Blend;
        resize_work(&mut w, &p);
        assert_eq!((w.img.width, w.img.height), (32, 16));
        for px in &w.img.px {
            assert!(
                px.iter().zip(c).all(|(a, b)| (a - b).abs() < 1e-5),
                "{px:?}"
            );
        }
    }

    /// Texel-centre aligned with clamped edges: 0, 1 becomes 0, 0.25, 0.75, 1. Weighted, a transparent texel's colour
    /// does not bleed, and where every tap is transparent the plain colour is kept.
    #[test]
    fn enlarge_is_linear_and_weighted_by_alpha() {
        let mut img = Rgba32F::new(2, 1, [0.0, 0.0, 0.0, 1.0]);
        img.px[1] = [1.0, 1.0, 1.0, 1.0];
        let out = enlarge(&img, (4, 1), false);
        let r: Vec<f32> = out.px.iter().map(|p| p[0]).collect();
        assert_eq!(r, [0.0, 0.25, 0.75, 1.0]);
        img.px[1] = [1.0, 1.0, 1.0, 0.0];
        let out = enlarge(&img, (4, 1), true);
        assert!(out.px[..3].iter().all(|p| p[0] == 0.0), "{:?}", out.px);
        assert_eq!(out.px[3], [1.0, 1.0, 1.0, 0.0]);
        assert!(close(out.px[1][3], 0.75));
    }

    #[test]
    fn box_resize_is_area_exact() {
        // 6x1 -> 4x1, data content: each output covers 1.5 source texels.
        let mut img = Rgba32F::new(6, 1, [0.0, 0.0, 0.0, 1.0]);
        img.px[2][0] = 1.0;
        img.px[3][0] = 1.0;
        let mut w = work(img, Domain::Stored, false);
        resize_work(&mut w, &profile(Subtype::GenericData));
        assert_eq!((w.img.width, w.img.height), (4, 1));
        // [0, 0 | 0.5 of 0] = 0; [0.5 of 0, 1] = 1/1.5; [1, 0.5 of 0] = 1/1.5; [0.5 of 0, 0] = 0.
        let want = [0.0, 2.0 / 3.0, 2.0 / 3.0, 0.0];
        for (px, e) in w.img.px.iter().zip(want) {
            assert!(close(px[0], e) && px[3] == 1.0, "{:?}", w.img.px);
        }
    }

    /// A power-of-two source keeps every bit: with no `resize`, and when the floor stops the halving (8 / 2 < 8).
    /// With floor 1 the same `eighth` does act, so the second case is the floor's doing.
    #[test]
    fn power_of_two_sources_are_untouched_bit_for_bit() {
        let img = rnd_image(8, 8, 11);
        let p = profile(Subtype::Diffuse);
        for (mode, floor) in [(ResizeMode::None, 1), (ResizeMode::Eighth, 8)] {
            let mut q = p.clone();
            (q.resize, q.resize_floor) = (mode, floor);
            let mut w = work(img.clone(), Domain::LinearLight, true);
            resize_work(&mut w, &q);
            assert_eq!(w.img, img, "{mode:?} floor {floor}");
        }
        let mut q = p.clone();
        (q.resize, q.resize_floor) = (ResizeMode::Eighth, 1);
        let mut w = work(img, Domain::LinearLight, true);
        resize_work(&mut w, &q);
        assert_eq!((w.img.width, w.img.height), (1, 1));
    }

    #[test]
    fn alpha_is_area_box_even_with_kaiser() {
        let img = rnd_image(12, 12, 3);
        let mut p = profile(Subtype::Diffuse);
        p.alpha.policy = AlphaPolicy::Data;
        p.mips.filter = MipFilter::Kaiser;
        let mut w = work(img.clone(), Domain::LinearLight, true);
        assert_eq!(mips::effective_filter(&w, &p), MipFilter::Kaiser);
        resize_work(&mut w, &p);
        let area = area_resample(&img, (8, 8), false);
        assert_eq!((w.img.width, w.img.height), (8, 8));
        assert!(w.img.px.iter().zip(&area.px).all(|(a, b)| a[3] == b[3]));
        assert!(w.img.px.iter().zip(&area.px).any(|(a, b)| a[0] != b[0])); // RGB is Kaiser
    }

    #[test]
    fn kaiser_resize_honours_the_clamp_edge() {
        let img = rnd_image(12, 12, 5);
        let mut p = profile(Subtype::Diffuse);
        p.alpha.policy = AlphaPolicy::Remove;
        p.mips.filter = MipFilter::Kaiser;
        p.mips.edge = MipEdge::Clamp;
        let mut w = work(img.clone(), Domain::LinearLight, false);
        resize_work(&mut w, &p);
        let clamp = crate::filter::resample_rgb(&img, (8, 8), MipFilter::Kaiser, MipEdge::Clamp);
        let wrap = crate::filter::resample_rgb(&img, (8, 8), MipFilter::Kaiser, MipEdge::Wrap);
        for (a, b) in w.img.px.iter().zip(&clamp.px) {
            for c in 0..3 {
                assert_eq!(a[c], b[c].clamp(0.0, 1.0));
            }
        }
        assert!(w
            .img
            .px
            .iter()
            .zip(&wrap.px)
            .any(|(a, b)| a[0] != b[0].clamp(0.0, 1.0)));
    }

    /// Task 4 review: an axis already a power of two is not filtered (one identity tap), so every output row is the
    /// 1-D Mitchell resample of its source row alone.
    #[test]
    fn mitchell_resize_leaves_a_power_of_two_axis_unfiltered() {
        let img = rnd_image(300, 512, 7);
        let mut p = profile(Subtype::Diffuse);
        p.alpha.policy = AlphaPolicy::Remove;
        p.mips.filter = MipFilter::Mitchell;
        let mut w = work(img.clone(), Domain::LinearLight, false);
        assert_eq!(mips::effective_filter(&w, &p), MipFilter::Mitchell);
        resize_work(&mut w, &p);
        assert_eq!((w.img.width, w.img.height), (256, 512));
        for y in 0..512 {
            let mut row = Rgba32F::new(300, 1, [0.0; 4]);
            row.px.copy_from_slice(&img.px[y * 300..(y + 1) * 300]);
            let want =
                crate::filter::resample_rgb(&row, (256, 1), MipFilter::Mitchell, p.mips.edge);
            for (a, b) in w.img.px[y * 256..(y + 1) * 256].iter().zip(&want.px) {
                for c in 0..3 {
                    assert_eq!(a[c], b[c].clamp(0.0, 1.0), "row {y}");
                }
            }
        }
    }

    /// M8: the box resize is area-exact and does not clamp, so a float LDR value above 1 survives until Quantise.
    #[test]
    fn box_resize_keeps_linear_light_values_above_one() {
        let img = Rgba32F::new(6, 1, [1.5, 1.5, 1.5, 1.0]);
        let mut p = profile(Subtype::Diffuse);
        p.alpha.policy = AlphaPolicy::Remove;
        let mut w = work(img, Domain::LinearLight, false);
        assert_eq!(mips::effective_filter(&w, &p), MipFilter::Box);
        resize_work(&mut w, &p);
        assert_eq!((w.img.width, w.img.height), (4, 1));
        assert!(w.img.px.iter().all(|px| px[0] > 1.0), "{:?}", w.img.px);
    }

    #[test]
    fn roughness_is_resized_on_r_squared() {
        // 3x1, R = [0, 1, 0] -> 2x1: each output covers texel 0 or 2 whole plus half of texel 1.
        let mut img = Rgba32F::new(3, 1, [0.0, 0.0, 0.0, 1.0]);
        img.px[1][0] = 1.0;
        let w = preprocess(img, &profile(Subtype::Rmaos));
        assert!(w.roughness_r_squared);
        assert_eq!((w.img.width, w.img.height), (2, 1));
        // mean r² = (1 * 0 + 0.5 * 1) / 1.5 = 1/3, R = √(1/3); the area mean of r would be 1/3.
        let want = (1.0f64 / 3.0).sqrt();
        assert!(
            close(w.img.px[0][0], want) && close(w.img.px[1][0], want),
            "{:?}",
            w.img.px
        );
    }

    #[test]
    fn weighted_colour_resize_ignores_invisible_colour() {
        // [red a=1, green a=0, blue a=1] -> 2x1: output 0 weights red by 1 * 1, green by 0.5 * 0.
        let mut img = Rgba32F::new(3, 1, [0.0; 4]);
        img.px[0] = [1.0, 0.0, 0.0, 1.0];
        img.px[1] = [0.0, 1.0, 0.0, 0.0];
        img.px[2] = [0.0, 0.0, 1.0, 1.0];
        let mut p = profile(Subtype::Diffuse);
        p.alpha.policy = AlphaPolicy::Cutout;
        let mut w = work(img, Domain::LinearLight, true);
        resize_work(&mut w, &p);
        let (o0, o1) = (w.img.px[0], w.img.px[1]);
        assert!(
            close(o0[0], 1.0) && close(o0[1], 0.0) && close(o0[2], 0.0),
            "{o0:?}"
        );
        assert!(
            close(o1[0], 0.0) && close(o1[1], 0.0) && close(o1[2], 1.0),
            "{o1:?}"
        );
        assert!(close(o0[3], 1.0 / 1.5) && close(o1[3], 1.0 / 1.5));
    }

    /// Note FM28: a weighted tent resize keeps the area-exact box alpha and ignores transparent colour.
    #[test]
    fn weighted_tent_resize_keeps_box_alpha_and_ignores_invisible_colour() {
        let mut img = Rgba32F::new(6, 1, [0.0, 1.0, 0.0, 1.0]);
        img.px[1] = [1.0, 0.0, 0.0, 0.0];
        img.px[4] = [1.0, 0.0, 0.0, 0.0];
        let mut p = profile(Subtype::Diffuse);
        p.alpha.policy = AlphaPolicy::Cutout;
        p.mips.filter = MipFilter::Tent;
        let mut w = work(img.clone(), Domain::LinearLight, true);
        assert_eq!(mips::effective_filter(&w, &p), MipFilter::Tent);
        resize_work(&mut w, &p);
        let boxed = area_resample(&img, (4, 1), true);
        assert_eq!(w.img.px.len(), 4);
        for (a, b) in w.img.px.iter().zip(&boxed.px) {
            assert_eq!(a[3], b[3]);
            assert!(
                close(a[0], 0.0) && close(a[1], 1.0) && close(a[2], 0.0),
                "{a:?}"
            );
        }
    }

    #[test]
    fn resized_normals_are_unit_length_at_mip_0_by_default() {
        let mut img = Rgba32F::new(6, 6, [0.0, 0.0, 0.0, 1.0]);
        for y in 0..6 {
            for x in 0..6 {
                let nx = 0.6 * (x as f32 * 1.3 + y as f32 * 0.7).sin();
                let ny = 0.6 * (x as f32 * 0.9 - y as f32 * 1.1).cos();
                let nz = (1.0 - nx * nx - ny * ny).sqrt();
                *img.at_mut(x, y) = [nx * 0.5 + 0.5, ny * 0.5 + 0.5, nz * 0.5 + 0.5, 1.0];
            }
        }
        let p = profile(Subtype::Normal);
        let w = preprocess(img, &p);
        let c = build_chain(&w, &p);
        assert_eq!((c[0].width, c[0].height), (4, 4));
        for v in &c[0].px {
            let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            assert!((len - 1.0).abs() < 1e-5, "{len}");
        }
    }
}
