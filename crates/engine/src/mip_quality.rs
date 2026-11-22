//! Test-only measurements of the written DDS against a direct area reference.
use crate::api::{BatchRequest, CancelToken, Engine, Plan};
use crate::config::*;
use crate::coverage::passes;
use crate::dds::{read_dds, DdsFile};
use crate::decode::{decode_level, Decoded};
use crate::encode::encode_level;
use crate::events::{NullSink, Reporter};
use crate::image::Rgba32F;
use crate::manifest::{dispatched_isa, output_key, Manifest};
use crate::ops::Domain;
use crate::pipeline::{prepare, FileError, FileTracker, Keep, Prepared};
use crate::policy::{AcceptSuggestions, ResolvedJob};
use crate::quantize::{stored_rgb, unorm8, EncoderInput};
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, path::Path, sync::Arc};

#[derive(Clone, Copy)]
enum RefKind {
    LinearLight { weighted: bool },
    Stored { roughness_r: bool },
    Vector { normalise: bool },
    KeptNormal,
    SignedRg,
}

fn ref_kind(prep: &Prepared, p: &Profile) -> RefKind {
    match prep.domain {
        Domain::LinearLight => RefKind::LinearLight {
            weighted: prep.weighted,
        },
        Domain::Stored => RefKind::Stored {
            roughness_r: p.subtype.is_roughness_member(),
        },
        Domain::Normal
            if crate::mips::keeps_normal_lengths(
                p,
                p.subtype,
                prep.domain,
                prep.dims.len() as u32,
            ) =>
        {
            RefKind::KeptNormal
        }
        Domain::Normal | Domain::Msn => RefKind::Vector { normalise: true },
        Domain::SignedRg => RefKind::SignedRg,
        Domain::Hdr => unreachable!("HDR is not scored"),
    }
}

fn length([x, y, z]: [f64; 3]) -> f64 {
    (x * x + y * y + z * z).sqrt()
}

// Integer footprint endpoints avoid drift on odd dimensions. Overlaps are in units
// of 1/dest source texels and need only be divided by the footprint area once.
fn overlaps(i: u32, src: u32, dest: u32) -> impl Iterator<Item = (u32, f64)> {
    let step = u64::from(dest);
    let lo = u64::from(i) * u64::from(src);
    let hi = lo + u64::from(src);
    (lo / step..hi.div_ceil(step))
        .map(move |j| (j as u32, (hi.min((j + 1) * step) - lo.max(j * step)) as f64))
}

fn reference_level(base: &Rgba32F, to: (u32, u32), kind: RefKind) -> Vec<[f64; 4]> {
    area_means(base, to, kind, true)
}

fn area_means(
    base: &Rgba32F,
    to: (u32, u32),
    kind: RefKind,
    flat_on_cancel: bool,
) -> Vec<[f64; 4]> {
    if to == (base.width, base.height) {
        return base.px.iter().map(|p| p.map(f64::from)).collect();
    }
    let (w, h) = to;
    let area = f64::from(base.width) * f64::from(base.height);
    let mut level = Vec::with_capacity(w as usize * h as usize);
    for y in 0..h {
        for x in 0..w {
            let mut sum = [0.; 4];
            let mut weighted = [0.; 3];
            let mut radii = 0.;
            for (cy, wy) in overlaps(y, base.height, h) {
                for (cx, wx) in overlaps(x, base.width, w) {
                    let weight = wy * wx;
                    let mut p = base.at(cx, cy).map(f64::from);
                    match kind {
                        RefKind::LinearLight { weighted: true } => {
                            for c in 0..3 {
                                weighted[c] += weight * p[c] * p[3];
                            }
                        }
                        RefKind::Stored { roughness_r: true } => p[0] *= p[0],
                        RefKind::KeptNormal => radii += weight * length([p[0], p[1], p[2]]),
                        _ => {}
                    }
                    for c in 0..4 {
                        sum[c] += weight * p[c];
                    }
                }
            }
            let mut p = sum.map(|v| v / area);
            match kind {
                RefKind::LinearLight { weighted: true } if sum[3] > 0. => {
                    for c in 0..3 {
                        p[c] = weighted[c] / sum[3];
                    }
                }
                RefKind::Stored { roughness_r: true } => p[0] = p[0].sqrt(),
                RefKind::KeptNormal | RefKind::Vector { normalise: true } => {
                    let len = length([p[0], p[1], p[2]]);
                    let radius = if matches!(kind, RefKind::KeptNormal) {
                        radii / area
                    } else {
                        1.
                    };
                    if len > 1e-8 {
                        let max = p[0].abs().max(p[1].abs()).max(p[2].abs());
                        let scale = (radius / len).min(1. / max);
                        for v in &mut p[..3] {
                            *v *= scale;
                        }
                    } else if flat_on_cancel {
                        p[..3].copy_from_slice(&[0., 0., radius.min(1.)]);
                    }
                }
                _ => {}
            }
            level.push(p);
        }
    }
    level
}

fn stored_from_reference(level: &[[f64; 4]], kind: RefKind) -> Vec<[f64; 4]> {
    level
        .iter()
        .map(|&src| {
            let mut p = src;
            for v in &mut p[..3] {
                *v = match kind {
                    RefKind::LinearLight { .. } => {
                        let x = v.clamp(0., 1.);
                        if x <= 0.0031308 {
                            12.92 * x
                        } else {
                            1.055 * x.powf(1. / 2.4) - 0.055
                        }
                    }
                    RefKind::Vector { .. } | RefKind::KeptNormal | RefKind::SignedRg => {
                        0.5 * *v + 0.5
                    }
                    RefKind::Stored { .. } => *v,
                }
            }
            if matches!(kind, RefKind::SignedRg) {
                p[2] = 0.;
            }
            p
        })
        .collect()
}

fn stored_reference(
    base: &Rgba32F,
    to: (u32, u32),
    kind: RefKind,
    domain: Domain,
) -> Vec<[f64; 4]> {
    // Tangent space has a flat +Z. A cancelling model-space footprint has no preferred axis.
    let level = if domain == Domain::Msn {
        area_means(base, to, kind, false)
    } else {
        reference_level(base, to, kind)
    };
    stored_from_reference(&level, kind)
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum AlphaScore {
    None,
    Values,
    Coverage,
}
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum NormalScore {
    None,
    Xyz { keep: bool },
    RebuiltZ,
}
struct Scoring {
    rgb: Vec<usize>,
    alpha: AlphaScore,
    normal: NormalScore,
}

fn scored_channels(p: &Profile, alpha_present: bool, kind: RefKind) -> Scoring {
    use Format::*;
    let rgb = match p.format {
        Bc4 | R8 => vec![0],
        Bc5 | Rg8 => vec![0, 1],
        Bc1 | Bc3 | Bc7 | Rgb8 | Rgba8 => vec![0, 1, 2],
        Bc6hUf16 | Rgba16f => unreachable!("HDR is not scored"),
    };
    let alpha = match p.format {
        Bc1 if alpha_present && p.bc1_one_bit_alpha() => AlphaScore::Coverage,
        Bc3 | Bc7 | Rgba8 if alpha_present => AlphaScore::Values,
        _ => AlphaScore::None,
    };
    let normal = if matches!(crate::ops::domain_for(p), Domain::Normal | Domain::Msn) {
        match rgb.len() {
            2 => NormalScore::RebuiltZ,
            3 => NormalScore::Xyz {
                keep: matches!(kind, RefKind::KeptNormal),
            },
            _ => NormalScore::None,
        }
    } else {
        NormalScore::None
    };
    Scoring { rgb, alpha, normal }
}

fn rebuilt_z(x: f64, y: f64) -> f64 {
    (1. - x * x - y * y).max(0.).sqrt()
}
fn vectors(px: &[[f64; 4]], mode: NormalScore) -> Vec<[f64; 3]> {
    px.iter()
        .map(|p| {
            let [x, y, z, _] = p.map(|c| 2. * c - 1.);
            let z = match mode {
                NormalScore::RebuiltZ => rebuilt_z(x, y),
                _ => z,
            };
            [x, y, z]
        })
        .collect()
}

fn mse_codes(a: &[[f64; 4]], b: &[[f64; 4]], ch: &[usize], mask: Option<&[bool]>) -> f64 {
    assert_eq!(a.len(), b.len());
    if let Some(mask) = mask {
        assert_eq!(mask.len(), a.len());
    }
    let mut sum = 0.;
    let mut count = 0usize;
    for (i, (a, b)) in a.iter().zip(b).enumerate() {
        if mask.is_some_and(|m| !m[i]) {
            continue;
        }
        for &c in ch {
            sum += ((a[c] - b[c]) * 255.).powi(2);
        }
        count += ch.len();
    }
    sum / count.max(1) as f64
}

fn psnr(mse: f64) -> String {
    if mse == 0. {
        "exact".into()
    } else {
        format!("{:.2} dB", 10. * (255f64.powi(2) / mse).log10())
    }
}

fn coverage(alpha: impl Iterator<Item = u8>, threshold: u8, compare: AlphaCompare) -> f64 {
    let (mut pass, mut total) = (0usize, 0usize);
    for a in alpha {
        pass += usize::from(passes(a, threshold, compare));
        total += 1;
    }
    pass as f64 / total.max(1) as f64
}

fn coverage_baseline(prep: &Prepared, p: &Profile) -> Option<f64> {
    prep.base_coverage.or_else(|| {
        (prep.alpha_present && p.alpha.policy == AlphaPolicy::Blend).then(|| {
            coverage(
                prep.chain[0].px.iter().map(|q| unorm8(q[3])),
                p.alpha.threshold,
                p.alpha.compare,
            )
        })
    })
}

fn stats(mut values: Vec<f64>) -> (f64, f64, f64) {
    assert!(!values.is_empty());
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    values.sort_unstable_by(f64::total_cmp);
    let percentile = |pc: usize| values[(values.len() * pc).div_ceil(100) - 1];
    (mean, percentile(1), percentile(99))
}
fn normal_lengths(px: &[[f64; 3]]) -> (f64, f64, f64) {
    stats(px.iter().copied().map(length).collect())
}
fn angle_stats(a: &[[f64; 3]], b: &[[f64; 3]]) -> (f64, f64) {
    assert_eq!(a.len(), b.len());
    // A zero vector has no direction. An all-undefined level writes null angles.
    let angles: Vec<_> = a
        .iter()
        .zip(b)
        .filter_map(|(&a, &b)| {
            let denom = length(a) * length(b);
            let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
            (denom > 0.).then(|| (dot / denom).clamp(-1., 1.).acos().to_degrees())
        })
        .collect();
    if angles.is_empty() {
        return (f64::NAN, f64::NAN);
    }
    let (mean, _, p99) = stats(angles);
    (mean, p99)
}
fn normal_metrics(d: &[[f64; 4]], r: &[[f64; 4]], mode: NormalScore) -> Value {
    let (d, r) = (vectors(d, mode), vectors(r, mode));
    let (mean, p99) = angle_stats(&d, &r);
    let mut row = json!({"angle_mean_degrees":mean,"angle_p99_degrees":p99});
    if let NormalScore::Xyz { keep } = mode {
        let (mean, p1, p99) = normal_lengths(&d);
        let target = if keep {
            r.iter().copied().map(length).sum::<f64>() / r.len() as f64
        } else {
            1.
        };
        row["length_mean"] = json!(mean);
        row["length_p1"] = json!(p1);
        row["length_p99"] = json!(p99);
        row["length_target_mean"] = json!(target);
        row["length_error_mean"] = json!(mean - target);
        let absolute_error = d
            .iter()
            .zip(&r)
            .map(|(&d, &r)| (length(d) - if keep { length(r) } else { 1. }).abs())
            .sum::<f64>();
        row["length_mean_absolute_error"] = json!(absolute_error / d.len() as f64);
    }
    row
}

fn edge_band(
    alpha: &[u8],
    w: u32,
    h: u32,
    threshold: u8,
    compare: AlphaCompare,
    radius: u32,
) -> (Vec<usize>, Vec<usize>) {
    assert_eq!(alpha.len(), w as usize * h as usize);
    let (mut edge, mut inside) = (vec![], vec![]);
    for (i, &a) in alpha.iter().enumerate() {
        if !passes(a, threshold, compare) {
            continue;
        }
        let (x, y) = ((i % w as usize) as u32, (i / w as usize) as u32);
        let near = (y.saturating_sub(radius)..=y.saturating_add(radius).min(h - 1)).any(|cy| {
            (x.saturating_sub(radius)..=x.saturating_add(radius).min(w - 1)).any(|cx| {
                !passes(
                    alpha[cy as usize * w as usize + cx as usize],
                    threshold,
                    compare,
                )
            })
        });
        if near {
            edge.push(i);
        } else {
            inside.push(i);
        }
    }
    (edge, inside)
}
fn edge_metrics(
    d: &[[f64; 4]],
    r: &[[f64; 4]],
    (band, inside): &(Vec<usize>, Vec<usize>),
) -> Value {
    let mse = |indices: &[usize]| {
        let mut sum = 0.;
        for &i in indices {
            for c in 0..3 {
                sum += ((d[i][c] - r[i][c]) * 255.).powi(2);
            }
        }
        sum / (indices.len() * 3).max(1) as f64
    };
    json!({"band_texels":band.len(),"interior_texels":inside.len(),"band_mse":mse(band),"interior_mse":mse(inside)})
}

fn reporter() -> Reporter {
    Reporter::new(Arc::new(NullSink), "mip-quality")
}
fn plan(input: &Path, out: &Path, sets: &str) -> Plan {
    let mut req = BatchRequest::new(vec![input.to_owned()], out.to_owned(), Target::Cs);
    req.target = None;
    req.recursive = true;
    req.accept = AcceptSuggestions::High;
    req.overrides = sets
        .split(';')
        .filter(|s| !s.is_empty())
        .map(|s| crate::overrides::parse_set(s).unwrap())
        .collect();
    Engine::new()
        .unwrap()
        .plan(req, &reporter(), &CancelToken::default())
        .unwrap()
}
fn prepare_job(job: &ResolvedJob) -> Result<Prepared, FileError> {
    prepare(
        job,
        &CancelToken::default(),
        &FileTracker::default(),
        Keep::Preview { max_size: u32::MAX },
    )
}
type ScoreResult<T> = Result<T, (&'static str, String)>;

fn encoded(prep: &Prepared, i: usize, p: &Profile) -> ScoreResult<Vec<u8>> {
    let (w, h) = prep.dims[i];
    encode_level(&prep.inputs[i], w, h, p, &Default::default())
        .map_err(|e| ("encode failed", format!("level {i}: {e}")))
}
fn ldr(input: &EncoderInput) -> &[[u8; 4]] {
    let EncoderInput::Ldr(px) = input else {
        unreachable!("HDR is not scored")
    };
    px
}
fn codes(px: &[[u8; 4]]) -> Vec<[f64; 4]> {
    px.iter().map(|p| p.map(|c| f64::from(c) / 255.)).collect()
}
fn engine_stored(img: &Rgba32F, domain: Domain) -> Vec<[f64; 4]> {
    img.px
        .iter()
        .map(|p| {
            let [r, g, b] = stored_rgb(p, domain);
            [r, g, b, p[3]].map(f64::from)
        })
        .collect()
}
fn decode_codes(
    fmt: crate::dds::DdsPixelFormat,
    w: u32,
    h: u32,
    bytes: &[u8],
) -> ScoreResult<Vec<[u8; 4]>> {
    let Decoded::Ldr(px) =
        decode_level(fmt, w, h, bytes).map_err(|e| ("decode failed", e.to_string()))?
    else {
        unreachable!("HDR is not scored")
    };
    Ok(px)
}

fn score_level(
    job: &ResolvedJob,
    prep: &Prepared,
    i: usize,
    dds: &DdsFile,
    baseline: Option<f64>,
) -> ScoreResult<Value> {
    let p = &job.profile;
    let kind = ref_kind(prep, p);
    let (w, h, range) = &dds.levels[i];
    let r = stored_reference(&prep.chain[0], (*w, *h), kind, prep.domain);
    let e = engine_stored(&prep.chain[i], prep.domain);
    let raw_q = ldr(&prep.inputs[i]);
    let raw_d = decode_codes(dds.format, *w, *h, &dds.data[range.clone()])?;
    let q = codes(raw_q);
    let d = codes(&raw_d);
    let mut scoring = scored_channels(p, prep.alpha_present, kind);
    if i > 0 && job.bake.is_some() {
        if p.subtype.is_roughness_member() {
            scoring.rgb.retain(|&c| c != 0);
        }
        if p.subtype == Subtype::ComplexMaterial {
            scoring.rgb.retain(|&c| c != 1);
        }
    }
    if i > 0 && crate::mips::coat_self_bake(p, prep.domain, prep.alpha_present).is_some() {
        scoring.alpha = AlphaScore::None;
    }
    let mask = (scoring.alpha == AlphaScore::Coverage)
        .then(|| raw_q.iter().map(|p| p[3] != 0).collect::<Vec<bool>>());
    let texels = mask
        .as_ref()
        .map_or(r.len(), |m| m.iter().filter(|&&pass| pass).count());
    let mut row = json!({"level":i,"texels":texels,"level_texels":r.len(),"mse":{},"delta_mse":{}});
    let mut group = |name: &str, channels: &[usize], mask: Option<&[bool]>| {
        if channels.is_empty() || mask.is_some_and(|m| !m.iter().any(|&pass| pass)) {
            return;
        }
        let er = mse_codes(&e, &r, channels, mask);
        let qr = mse_codes(&q, &r, channels, mask);
        let dr = mse_codes(&d, &r, channels, mask);
        for (step, value) in [
            ("e", er),
            ("q", qr),
            ("d", dr),
            ("q_vs_e", mse_codes(&q, &e, channels, mask)),
            ("d_vs_q", mse_codes(&d, &q, channels, mask)),
        ] {
            row["mse"][format!("{step}_{name}")] = json!(value);
        }
        row["delta_mse"][format!("q_minus_e_{name}")] = json!(qr - er);
        row["delta_mse"][format!("d_minus_q_{name}")] = json!(dr - qr);
        row["psnr"][name] = json!(psnr(dr));
    };
    group("rgb", &scoring.rgb, mask.as_deref());
    if scoring.alpha == AlphaScore::Values {
        group("alpha", &[3], None);
    }
    if let Some(base) = baseline.filter(|_| scoring.alpha != AlphaScore::None) {
        let measured = coverage(
            raw_d.iter().map(|q| q[3]),
            p.alpha.threshold,
            p.alpha.compare,
        );
        row["coverage"] = json!({"decoded":measured,"baseline":base,"drift_pp":100.*(measured-base),
            "step":if prep.base_coverage.is_some() {"coverage"} else {"no coverage step"}});
    }
    if scoring.normal != NormalScore::None {
        row["normal"] = normal_metrics(&d, &r, scoring.normal);
    }
    if prep.weighted && prep.alpha_present {
        let alpha: Vec<_> = prep.chain[i].px.iter().map(|p| unorm8(p[3])).collect();
        let masks = edge_band(&alpha, *w, *h, p.alpha.threshold, p.alpha.compare, 2);
        row["edge"] = edge_metrics(&d, &r, &masks);
    }
    Ok(row)
}

struct JobScore {
    row: Value,
    isa_skipped: bool,
}

fn score_prepared(
    job: &ResolvedJob,
    prep: &Prepared,
    dds: &DdsFile,
    isa: &str,
    score: &mut JobScore,
) -> ScoreResult<()> {
    let p = &job.profile;
    let layout_matches = dds
        .format
        .as_format()
        .is_some_and(|(format, _)| format == p.format)
        && prep.dims.len() == dds.mip_count as usize
        && dds
            .levels
            .iter()
            .map(|(w, h, _)| (*w, *h))
            .eq(prep.dims.iter().copied());
    if !layout_matches {
        score.row["why"] = json!("layout mismatch");
        return Ok(());
    }
    score.isa_skipped = p.format == Format::Bc7 && isa != dispatched_isa();
    if !score.isa_skipped {
        let mut mismatches = Vec::new();
        let last = prep.inputs.len() - 1;
        let first = 1.min(last);
        for i in std::iter::once(first).chain((last != first).then_some(last)) {
            if encoded(prep, i, p)? != dds.data[dds.levels[i].2.clone()] {
                mismatches.push(i);
            }
        }
        if !mismatches.is_empty() {
            score.row["mismatched_levels"] = json!(mismatches);
            score.row["why"] = json!("rebuild mismatch");
            return Ok(());
        }
    }
    let baseline = coverage_baseline(prep, p);
    score.row["per_level"] = (0..dds.levels.len())
        .map(|i| score_level(job, prep, i, dds, baseline))
        .collect::<ScoreResult<Value>>()?;
    Ok(())
}

fn score_record(
    job: &ResolvedJob,
    output: &str,
    record: &Value,
    prepare_job: impl FnOnce(&ResolvedJob) -> Result<Prepared, FileError>,
) -> JobScore {
    let p = &job.profile;
    // The manifest retains the validated mip count even when the DDS is missing.
    let mut score = JobScore {
        row: json!({"output":output,"format":p.format,"subtype":p.subtype,
        "levels":record["validation"]["levels"].as_u64()}),
        isa_skipped: false,
    };
    let excluded = if job.cube.is_some() || p.subtype == Subtype::Cubemap {
        Some("cube")
    } else if p.subtype == Subtype::ComplexGrass {
        Some("complex grass")
    } else if p.format.is_hdr() {
        Some("hdr")
    } else {
        None
    };
    if let Some(why) = excluded {
        score.row["why"] = json!(why);
        return score;
    }
    let attempt = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let bytes = fs::read(&job.output).map_err(|e| ("read failed", e.to_string()))?;
        let dds = read_dds(&bytes).map_err(|e| ("invalid dds", e.to_string()))?;
        drop(bytes);
        score.row["levels"] = json!(dds.mip_count);
        if dds.is_cubemap {
            score.row["why"] = json!("cube");
            return Ok(());
        }
        let isa = record["isa"]
            .as_str()
            .ok_or_else(|| ("missing isa", "manifest has no ISA string".into()))?;
        let prep = prepare_job(job).map_err(|e| ("prepare failed", e.to_string()))?;
        score_prepared(job, &prep, &dds, isa, &mut score)
    }));
    let (why, detail) = match attempt {
        Ok(Ok(())) => return score,
        Ok(Err(error)) => error,
        Err(payload) => (
            "scoring panic",
            payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "non-string panic payload".into()),
        ),
    };
    score.row["why"] = json!(why);
    score.row["detail"] = json!(detail);
    score
}
fn score_job(job: &ResolvedJob, out_root: &Path) -> JobScore {
    let key = output_key(out_root, &job.output);
    let records = Manifest::at(out_root).read().unwrap();
    score_record(job, &key, &records[&key].raw, prepare_job)
}

// Seeded ordering within each sub-type, then one from each nonempty stratum per round.
fn sample<'a>(
    jobs: &'a [ResolvedJob],
    out: &Path,
    records: &BTreeMap<String, crate::manifest::ManifestEntry>,
    limit: usize,
    seed: u64,
) -> Vec<(&'a ResolvedJob, String)> {
    let mut groups = BTreeMap::<_, Vec<_>>::new();
    for j in jobs {
        let key = output_key(out, &j.output);
        if !records.contains_key(&key) {
            continue;
        }
        use sha2::{Digest, Sha256};
        let rank = Sha256::new()
            .chain_update(seed.to_le_bytes())
            .chain_update(key.as_bytes())
            .finalize();
        groups
            .entry(j.profile.subtype)
            .or_default()
            .push((rank, j, key));
    }
    for group in groups.values_mut() {
        group.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.2.cmp(&a.2)));
    }
    let mut selected = vec![];
    while selected.len() < limit {
        let before = selected.len();
        for group in groups.values_mut() {
            if let Some((_, job, key)) = group.pop() {
                selected.push((job, key));
            }
            if selected.len() == limit {
                break;
            }
        }
        if selected.len() == before {
            break;
        }
    }
    selected
}

#[test]
#[ignore]
fn mip_quality_on_a_corpus() {
    use std::{env, io::Write, path::PathBuf};
    let path = |name| PathBuf::from(env::var_os(name).unwrap_or_else(|| panic!("missing {name}")));
    let input = path("SDC_MIPQ_INPUT");
    let out = path("SDC_MIPQ_OUT");
    let report = path("SDC_MIPQ_REPORT");
    let plan = plan(&input, &out, &env::var("SDC_MIPQ_SET").unwrap_or_default());
    let records = Manifest::at(&plan.request.output_root).read().unwrap();
    assert!(
        !records.is_empty(),
        "no manifest records in {}",
        plan.request.output_root.display()
    );
    let limit = env::var("SDC_MIPQ_SAMPLE").map_or(300, |s| s.parse().unwrap());
    let seed = env::var("SDC_MIPQ_SEED").map_or(20261121, |s| s.parse().unwrap());
    let mut jobs = vec![];
    let mut skipped = vec![];
    let mut isa_skips = 0;
    for (job, key) in sample(&plan.jobs, &plan.request.output_root, &records, limit, seed) {
        let score = score_record(job, &key, &records[&key].raw, prepare_job);
        isa_skips += usize::from(score.isa_skipped);
        if score.row.get("why").is_some() {
            skipped.push(score.row);
        } else {
            jobs.push(score.row);
        }
    }
    let result = json!({"isa":dispatched_isa(),"self_check":{"isa_skipped":isa_skips,
        "rebuild_mismatches":skipped.iter().filter(|s|s["why"]=="rebuild mismatch").count()},"jobs":jobs,"skipped":skipped});
    let mut file = std::io::BufWriter::new(fs::File::create_new(&report).unwrap());
    serde_json::to_writer_pretty(&mut file, &result).unwrap();
    file.flush().unwrap();
    eprintln!(
        "{} scored, {} skipped; {}",
        jobs.len(),
        skipped.len(),
        report.display()
    );
}

fn fixture(convert: bool) -> (tempfile::TempDir, Vec<ResolvedJob>) {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in");
    let expected = crate::profile_fixture::write_project(&input).unwrap();
    let plan = plan(&input, &dir.path().join("out"), "");
    let jobs = plan.jobs.clone();
    assert_eq!(jobs.len(), expected.len());
    if convert {
        let result = Engine::new()
            .unwrap()
            .run(plan, &reporter(), &CancelToken::default())
            .unwrap();
        assert_eq!(result.converted as usize, expected.len());
    }
    (dir, jobs)
}

fn prepared(job: &ResolvedJob) -> Prepared {
    prepare_job(job).unwrap()
}

fn named<'a>(jobs: &'a [ResolvedJob], name: &str) -> &'a ResolvedJob {
    jobs.iter().find(|j| j.output.ends_with(name)).unwrap()
}

fn pair(a: [f32; 4], b: [f32; 4], kind: RefKind) -> [f64; 4] {
    reference_level(
        &Rgba32F {
            width: 2,
            height: 1,
            px: vec![a, b],
        },
        (1, 1),
        kind,
    )[0]
}

#[test]
fn box_reference_equals_the_engine_chain_on_a_power_of_two_stored_map() {
    let p = crate::presets::PresetRegistry::builtin()
        .unwrap()
        .profile("cs-quality", Target::Cs, Subtype::Rmaos)
        .unwrap();
    let mut seed = 20261121u32;
    let mut img = Rgba32F::new(64, 64, [0.; 4]);
    for c in img.px.iter_mut().flatten() {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        *c = f32::from((seed >> 24) as u8) / 255.;
    }
    let work = crate::ops::preprocess(img, &p);
    let chain = crate::mips::build_chain(&work, &p);
    for level in &chain {
        let r = reference_level(
            &chain[0],
            (level.width, level.height),
            RefKind::Stored { roughness_r: true },
        );
        assert_eq!(r.len(), level.px.len());
        for (a, b) in level.px.iter().flatten().zip(r.iter().flatten()) {
            assert!(
                (f64::from(*a) - b).abs() < 1e-6,
                "{a} vs {b} at width {}",
                level.width
            );
        }
    }
}

#[test]
fn linear_light_box_reference_matches_the_stored_engine_chain() {
    let mut p = crate::presets::PresetRegistry::builtin()
        .unwrap()
        .profile("cs-quality", Target::Cs, Subtype::Diffuse)
        .unwrap();
    p.mips.filter = MipFilter::Box;
    p.alpha.policy = AlphaPolicy::Remove;
    let mut img = Rgba32F::new(8, 8, [0.; 4]);
    for (i, px) in img.px.iter_mut().enumerate() {
        *px = [
            i as f32 / 63.,
            (i % 3) as f32 / 255.,
            (i % 5) as f32 / 4.,
            1.,
        ];
    }
    let work = crate::ops::preprocess(img, &p);
    assert_eq!(work.domain, Domain::LinearLight);
    let chain = crate::mips::build_chain(&work, &p);
    assert_eq!(chain.len(), 4);
    for level in &chain {
        let r = stored_reference(
            &chain[0],
            (level.width, level.height),
            RefKind::LinearLight { weighted: false },
            work.domain,
        );
        let e = engine_stored(level, work.domain);
        assert_eq!(r.len(), e.len());
        for (r, e) in r.iter().flatten().zip(e.iter().flatten()) {
            assert!((r - e).abs() < 1e-6, "{r} vs {e} at width {}", level.width);
        }
    }
}

#[test]
fn a_failed_corpus_job_is_recorded_and_the_other_jobs_are_scored() {
    let (dir, jobs) = fixture(true);
    let out = dir.path().join("out");
    let partial = dir.path().join("partial");
    let manifest = Manifest::at(&partial);
    fs::create_dir_all(manifest.path().parent().unwrap()).unwrap();
    fs::copy(Manifest::at(&out).path(), manifest.path()).unwrap();
    let missing = named(&jobs, "plain.dds");
    for j in jobs.iter().filter(|j| j.output != missing.output) {
        let path = partial.join(j.output.strip_prefix(&out).unwrap());
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::copy(&j.output, path).unwrap();
    }
    let report = dir.path().join("quality.json");
    let run = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "mip_quality::mip_quality_on_a_corpus",
            "--ignored",
            "--nocapture",
        ])
        .env("SDC_MIPQ_INPUT", dir.path().join("in"))
        .env("SDC_MIPQ_OUT", &partial)
        .env("SDC_MIPQ_REPORT", &report)
        .env_remove("SDC_MIPQ_SET")
        .env_remove("SDC_MIPQ_SAMPLE")
        .env_remove("SDC_MIPQ_SEED")
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "a missing DDS aborted the corpus: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    let result: Value = serde_json::from_slice(&fs::read(report).unwrap()).unwrap();
    let scored = result["jobs"].as_array().unwrap();
    let skipped = result["skipped"].as_array().unwrap();
    assert_eq!((scored.len(), skipped.len()), (10, 2));
    let ids: std::collections::BTreeSet<_> = scored
        .iter()
        .chain(skipped)
        .map(|row| row["output"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        ids,
        jobs.iter().map(|j| output_key(&out, &j.output)).collect()
    );
    let bad = skipped
        .iter()
        .find(|j| j["output"] == output_key(&out, &missing.output))
        .unwrap();
    assert_eq!(bad["why"], "read failed");
    assert_eq!(bad["format"], json!(missing.profile.format));
    assert_eq!(bad["subtype"], json!(missing.profile.subtype));
    assert_eq!(bad["levels"], 7);
    assert!(bad["detail"].as_str().is_some_and(|s| !s.is_empty()));
    assert_eq!(result["self_check"]["rebuild_mismatches"], 0);
}

#[test]
fn unscored_classes_do_not_require_a_readable_source() {
    let (dir, jobs) = fixture(true);
    for (name, reason) in [
        ("plain.dds", "cube"),
        ("glass.dds", "complex grass"),
        ("lamp_g.dds", "hdr"),
    ] {
        let mut job = named(&jobs, name).clone();
        job.input = dir.path().join("absent.png");
        match reason {
            "cube" => {
                job.cube = Some(crate::cube::CubeJob {
                    layout: CubeLayout::Faces,
                    sources: vec![],
                })
            }
            "complex grass" => job.profile.subtype = Subtype::ComplexGrass,
            _ => {}
        }
        let score = score_job(&job, &dir.path().join("out"));
        assert_eq!(score.row["why"], reason);
        assert!(score.row["levels"].as_u64().unwrap() > 0);
        assert!(!score.isa_skipped);
    }
}

#[test]
fn layout_mismatches_are_distinct_from_rebuilt_bytes() {
    let (dir, jobs) = fixture(true);
    let mut job = named(&jobs, "plain.dds").clone();
    job.profile.format = Format::Rgba8;
    let row = score_job(&job, &dir.path().join("out")).row;
    assert_eq!(row["why"], "layout mismatch");
    assert!(row.get("mismatched_levels").is_none());
}

#[test]
fn model_space_keep_does_not_enable_kept_length_scoring() {
    let (_dir, jobs) = fixture(true);
    let mut job = named(&jobs, "keep_n.dds").clone();
    job.profile.subtype = Subtype::Msn;
    let prep = prepared(&job);
    assert_eq!(prep.domain, Domain::Msn);
    let dds = read_dds(&fs::read(&job.output).unwrap()).unwrap();
    let row = score_level(&job, &prep, 0, &dds, None).unwrap();
    assert_eq!(row["normal"]["length_target_mean"], 1.);
}

#[test]
fn linear_light_reference_averages_light_not_codes() {
    let k = RefKind::LinearLight { weighted: false };
    let mean = pair([1.; 4], [0.; 4], k);
    assert!((stored_from_reference(&[mean], k)[0][0] - 0.7354).abs() < 1e-4);
}

#[test]
fn weighted_reference_ignores_transparent_colour() {
    assert_eq!(
        pair(
            [1., 0., 0., 1.],
            [0., 0., 1., 0.],
            RefKind::LinearLight { weighted: true }
        ),
        [1., 0., 0., 0.5]
    );
}

#[test]
fn prepared_carries_the_weighting_facts() {
    let (_dir, jobs) = fixture(false);
    let leaf = prepared(named(&jobs, "leaf.dds"));
    assert!(leaf.alpha_present && leaf.weighted);
    for name in ["plain.dds", "keep_n.dds"] {
        assert!(!prepared(named(&jobs, name)).weighted);
    }
}

#[test]
fn kept_reference_follows_la2() {
    let mean = |a, b| pair(a, b, RefKind::KeptNormal);
    assert_eq!(mean([1., 0., 0., 1.], [-1., 0., 0., 1.]), [0., 0., 1., 1.]);
    assert_eq!(
        mean([0.5, 0., 0., 1.], [1., 0., 0., 1.]),
        [0.75, 0., 0., 1.]
    );
    assert_eq!(mean([1., 1., 0., 1.], [1., -1., 0., 1.]), [1., 0., 0., 1.]);
    let (dir, jobs) = fixture(true);
    let job = named(&jobs, "keep_n.dds");
    let prep = prepared(job);
    let score = score_job(job, &dir.path().join("out"));
    let last = score.row["per_level"].as_array().unwrap().last().unwrap();
    let r = reference_level(&prep.chain[0], (1, 1), RefKind::KeptNormal)[0];
    let expected = length([r[0], r[1], r[2]]);
    assert!((expected - 1.).abs() > 0.01);
    let n = &last["normal"];
    assert!(
        (n["length_error_mean"].as_f64().unwrap()
            - (n["length_mean"].as_f64().unwrap() - expected))
            .abs()
            < 1e-10
    );
}

#[test]
fn bc1_one_bit_alpha_scores_rgb_on_opaque_texels_and_alpha_as_coverage() {
    let (dir, jobs) = fixture(true);
    let job = named(&jobs, "onebit.dds");
    let prep = prepared(job);
    let channels = scored_channels(
        &job.profile,
        prep.alpha_present,
        ref_kind(&prep, &job.profile),
    );
    assert_eq!(channels.rgb, [0, 1, 2]);
    assert_eq!(channels.alpha, AlphaScore::Coverage);
    let q = ldr(&prep.inputs[0]);
    assert!(q.iter().any(|px| px[3] == 0));
    for (px, src) in q.iter().zip(&prep.chain[0].px) {
        if !passes(
            unorm8(src[3]),
            job.profile.alpha.threshold,
            job.profile.alpha.compare,
        ) {
            assert_eq!(*px, [0; 4]);
        }
    }
    let score = score_job(job, &dir.path().join("out"));
    let lv = &score.row["per_level"][0];
    let e = engine_stored(&prep.chain[0], prep.domain);
    let mut sum = 0.;
    let mut count = 0;
    for (q, e) in q.iter().zip(e) {
        if q[3] != 0 {
            for c in 0..3 {
                sum += (f64::from(q[c]) - e[c] * 255.).powi(2);
                count += 1;
            }
        }
    }
    assert!(
        (lv["mse"]["q_vs_e_rgb"].as_f64().unwrap() - sum / f64::from(count)).abs() < 1e-8,
        "RGB must score only texels Q keeps opaque"
    );
    assert!(lv["coverage"].is_object() && lv["mse"]["d_alpha"].is_null());
}

#[test]
fn bc5_normals_rebuild_z_and_skip_length() {
    let mut p = crate::presets::PresetRegistry::builtin()
        .unwrap()
        .profile("cs-quality", Target::Cs, Subtype::Normal)
        .unwrap();
    p.format = Format::Bc5;
    let mode = scored_channels(&p, false, RefKind::Vector { normalise: true }).normal;
    assert_eq!(mode, NormalScore::RebuiltZ);
    let px = codes(&[[160, 150, 0, 255]]);
    let x = 2. * px[0][0] - 1.;
    let y = 2. * px[0][1] - 1.;
    let v = vectors(&px, mode);
    let stats = angle_stats(&v, &[[x, y, (1. - x * x - y * y).sqrt()]]);
    assert!(
        stats.0 < 1e-5 && stats.1 < 1e-5,
        "decoded BC5 uses rebuilt Z: {stats:?}"
    );
    assert_eq!(rebuilt_z(1., 1.), 0.);
    assert!(normal_metrics(&px, &px, mode).get("length_mean").is_none());
}

#[test]
fn a_blend_output_uses_the_chains_level_0_coverage() {
    let (_dir, jobs) = fixture(false);
    let job = named(&jobs, "glass.dds");
    let prep = prepared(job);
    assert_eq!(prep.base_coverage, None);
    let a = &job.profile.alpha;
    let expected = coverage(
        prep.chain[0].px.iter().map(|q| unorm8(q[3])),
        a.threshold,
        a.compare,
    );
    assert!(expected > 0. && expected < 1.);
    assert_eq!(coverage_baseline(&prep, &job.profile), Some(expected));
}

#[test]
fn edge_band_finds_the_two_texel_ring() {
    let alpha: Vec<_> = (0..144)
        .map(|i| {
            if (2..10).contains(&(i % 12)) && (2..10).contains(&(i / 12)) {
                255
            } else {
                0
            }
        })
        .collect();
    let (edge, inside) = edge_band(&alpha, 12, 12, 127, AlphaCompare::Gt, 2);
    assert_eq!((edge.len(), inside.len()), (48, 16));
    assert!(inside
        .iter()
        .all(|i| (4..8).contains(&(i % 12)) && (4..8).contains(&(i / 12))));
}

#[test]
fn the_bleed_control_changes_only_the_colour_under_zero_alpha() {
    let (_dir, jobs) = fixture(false);
    let job = named(&jobs, "leaf.dds");
    let first = prepared(job);
    let second = without_dilation(|| prepared(job));
    let base = &first.chain[0];
    let mut changed = false;
    for (a, b) in base.px.iter().zip(&second.chain[0].px) {
        assert_eq!(a[3], b[3]);
        if a[3] > 0. {
            assert_eq!(a, b);
        } else {
            changed |= a != b;
        }
    }
    assert!(changed, "at least one alpha-0 texel differs");
    let decode = |prep: &Prepared| {
        decode_codes(
            job.profile.format.pixel_format(false),
            base.width,
            base.height,
            &encoded(prep, 0, &job.profile).unwrap(),
        )
        .unwrap()
    };
    let (a, b) = (decode(&first), decode(&second));
    let w = base.width as usize;
    assert!(
        w.is_multiple_of(4) && base.height.is_multiple_of(4),
        "leaf fixture needs complete BC blocks"
    );
    let mut solid_blocks = 0;
    let mut has_mixed_block = false;
    for y in (0..base.height as usize).step_by(4) {
        for x in (0..w).step_by(4) {
            let block = (0..4).flat_map(|dy| (0..4).map(move |dx| (y + dy) * w + x + dx));
            if block.clone().all(|i| base.px[i][3] > 0.) {
                solid_blocks += 1;
                for i in block {
                    assert_eq!(a[i], b[i]);
                }
            } else if block.clone().any(|i| base.px[i][3] > 0.) {
                has_mixed_block = true;
            }
        }
    }
    assert!(solid_blocks > 0);
    assert!(has_mixed_block, "the alpha edge must cross a BC block");
    let k = ref_kind(&first, &job.profile);
    let r = stored_from_reference(&reference_level(base, first.dims[0], k), k);
    let alpha: Vec<_> = base.px.iter().map(|q| unorm8(q[3])).collect();
    let masks = edge_band(
        &alpha,
        base.width,
        base.height,
        job.profile.alpha.threshold,
        job.profile.alpha.compare,
        2,
    );
    let control = edge_metrics(&codes(&a), &r, &masks);
    let disabled = edge_metrics(&codes(&b), &r, &masks);
    eprintln!("leaf level 0 with dilation {control}, without dilation {disabled}");
    assert!(disabled["band_mse"].as_f64().unwrap() > control["band_mse"].as_f64().unwrap());
}

#[test]
fn the_self_check_catches_a_wrong_middle_level() {
    let (dir, jobs) = fixture(true);
    let job = named(&jobs, "plain.dds");
    let bytes = fs::read(&job.output).unwrap();
    let dds = read_dds(&bytes).unwrap();
    let mut prep = prepared(job);
    let EncoderInput::Ldr(q) = &mut prep.inputs[1] else {
        panic!("expected LDR")
    };
    for code in q.iter_mut().flatten() {
        *code = !*code;
    }
    let last = prep.inputs.len() - 1;
    assert_ne!(
        encoded(&prep, 1, &job.profile).unwrap(),
        dds.data[dds.levels[1].2.clone()]
    );
    assert_eq!(
        encoded(&prep, last, &job.profile).unwrap(),
        dds.data[dds.levels[last].2.clone()]
    );
    let out = dir.path().join("out");
    let key = output_key(&out, &job.output);
    let records = Manifest::at(&out).read().unwrap();
    let score = score_record(job, &key, &records[&key].raw, |_| Ok(prep));
    assert_eq!(score.row["why"], "rebuild mismatch");
    assert_eq!(score.row["mismatched_levels"], json!([1]));
}

#[test]
fn every_fixture_job_scores_or_says_why() {
    let (dir, jobs) = fixture(true);
    let out = dir.path().join("out");
    for job in jobs {
        let row = score_job(&job, &out).row;
        assert_eq!(row["subtype"], json!(job.profile.subtype));
        assert_eq!(row["format"], json!(job.profile.format));
        assert_eq!(row["output"], output_key(&out, &job.output));
        let levels = row["levels"].as_u64().unwrap() as usize;
        assert!(levels > 0);
        if job.profile.format.is_hdr() {
            assert_eq!(row["why"], "hdr");
        } else {
            let per_level = row["per_level"]
                .as_array()
                .unwrap_or_else(|| panic!("{row}"));
            assert_eq!(per_level.len(), levels);
            let prep = prepared(&job);
            for (i, lv) in per_level.iter().enumerate() {
                assert_eq!(lv["level"], i);
                let q = ldr(&prep.inputs[i]);
                let scored = if job.profile.bc1_one_bit_alpha() && prep.alpha_present {
                    q.iter().filter(|p| p[3] != 0).count()
                } else {
                    q.len()
                };
                assert_eq!(lv["level_texels"], q.len());
                assert_eq!(lv["texels"], scored);
                assert_eq!(lv["mse"]["d_rgb"].is_number(), scored != 0);
            }
        }
    }
}

#[test]
fn psnr_from_mse_and_exact() {
    assert_eq!(psnr(1.), "48.13 dB");
    assert_eq!(psnr(0.), "exact");
}

#[test]
fn coverage_counts_the_threshold_by_its_compare() {
    let a = [127, 128, 200, 0];
    assert_eq!(coverage(a.into_iter(), 127, AlphaCompare::Gt), 0.5);
    assert_eq!(coverage(a.into_iter(), 127, AlphaCompare::Ge), 0.75);
}

#[test]
fn odd_footprints_take_their_area_directly_from_mip_zero() {
    let mut base = Rgba32F::new(5, 1, [0.; 4]);
    base.px[2] = [1.; 4];
    let r = reference_level(&base, (2, 1), RefKind::Stored { roughness_r: false });
    assert_eq!(r, [[0.2; 4]; 2]);
    let r = reference_level(&base, (1, 1), RefKind::SignedRg);
    assert_eq!(
        stored_from_reference(&r, RefKind::SignedRg),
        [[0.6, 0.6, 0., 0.2]]
    );
}

#[test]
fn attribution_measures_direct_steps_even_when_errors_cancel() {
    let (_dir, jobs) = fixture(true);
    let job = named(&jobs, "glass.dds");
    let prep = prepared(job);
    let dds = read_dds(&fs::read(&job.output).unwrap()).unwrap();
    let i = 1;
    let (w, h, range) = &dds.levels[i];
    let d = decode_codes(dds.format, *w, *h, &dds.data[range.clone()]).unwrap();
    let q = ldr(&prep.inputs[i]);
    let r = reference_level(&prep.chain[0], (*w, *h), ref_kind(&prep, &job.profile));
    let mut sums = [0.; 5];
    for (((r, e), q), d) in r.iter().zip(&prep.chain[i].px).zip(q).zip(&d) {
        let r = r[3] * 255.;
        let e = f64::from(e[3]) * 255.;
        let (q, d) = (f64::from(q[3]), f64::from(d[3]));
        for (sum, error) in sums.iter_mut().zip([e - r, q - r, d - r, q - e, d - q]) {
            *sum += error * error;
        }
    }
    let expected = sums.map(|s| s / d.len() as f64);
    let row = score_level(job, &prep, i, &dds, coverage_baseline(&prep, &job.profile)).unwrap();
    let close = |v: &Value, x: f64| assert!((v.as_f64().unwrap() - x).abs() < 1e-9, "{v} vs {x}");
    for (name, value) in [
        "e_alpha",
        "q_alpha",
        "d_alpha",
        "q_vs_e_alpha",
        "d_vs_q_alpha",
    ]
    .into_iter()
    .zip(expected)
    {
        close(&row["mse"][name], value);
    }
    close(
        &row["delta_mse"]["d_minus_q_alpha"],
        expected[2] - expected[1],
    );
    let a = &job.profile.alpha;
    let baseline = coverage(
        prep.chain[0].px.iter().map(|p| unorm8(p[3])),
        a.threshold,
        a.compare,
    );
    let decoded = coverage(d.iter().map(|p| p[3]), a.threshold, a.compare);
    close(&row["coverage"]["drift_pp"], 100. * (decoded - baseline));
    assert_eq!(row["coverage"]["step"], "no coverage step");
}

#[test]
fn sampling_is_seeded_stratified_and_requires_a_record() {
    let (dir, jobs) = fixture(true);
    let out = dir.path().join("out");
    let mut records = Manifest::at(&out).read().unwrap();
    let missing = output_key(&out, &named(&jobs, "wide.dds").output);
    records.remove(&missing).unwrap();
    let order = |jobs: &[ResolvedJob], limit, seed| {
        sample(jobs, &out, &records, limit, seed)
            .into_iter()
            .map(|(j, key)| (j.profile.subtype, key))
            .collect::<Vec<_>>()
    };
    let full = order(&jobs, 300, 20261121);
    assert_eq!(full.len(), records.len());
    assert!(full.iter().all(|(_, key)| key != &missing));
    let types = |s: &[(Subtype, String)]| {
        s.iter()
            .map(|(st, _)| *st)
            .collect::<std::collections::BTreeSet<_>>()
    };
    let k = types(&full).len();
    assert_eq!(types(&order(&jobs, k, 20261121)), types(&full));
    let mut reversed = jobs.clone();
    reversed.reverse();
    assert_eq!(order(&reversed, 300, 20261121), full);
    assert!((1..8).any(|seed| order(&jobs, 300, seed) != full));
}

#[test]
fn self_check_skips_only_isa_dependent_encoders_on_a_different_isa() {
    let (dir, jobs) = fixture(true);
    for (name, skip) in [
        ("plain.dds", true),
        ("onebit.dds", false),
        ("lamp_g.dds", false),
    ] {
        let mut job = named(&jobs, name).clone();
        let mut bytes = fs::read(&job.output).unwrap();
        let dds = read_dds(&bytes).unwrap();
        let at = bytes.len() - dds.data.len() + dds.levels[1].2.start;
        bytes[at] ^= 1;
        job.output = dir.path().join(name);
        fs::write(&job.output, bytes).unwrap();
        for isa in ["other ISA", dispatched_isa()] {
            let score = score_record(
                &job,
                name,
                &json!({"isa":isa,"validation":{"levels":dds.mip_count}}),
                prepare_job,
            );
            assert_eq!(score.isa_skipped, skip && isa != dispatched_isa());
            if job.profile.format.is_hdr() {
                assert_eq!(score.row["why"], "hdr");
            } else if score.isa_skipped {
                assert!(score.row["per_level"].is_array());
            } else {
                assert_eq!(score.row["why"], "rebuild mismatch");
            }
        }
    }
}

fn without_dilation<T>(run: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            crate::ops::SKIP_DILATE.set(self.0);
        }
    }
    let _restore = Restore(crate::ops::SKIP_DILATE.replace(true));
    run()
}

#[test]
fn the_dilation_control_restores_the_previous_value_on_unwind() {
    assert!(!crate::ops::SKIP_DILATE.get());
    let panic = std::panic::catch_unwind(|| {
        without_dilation(|| {
            without_dilation(|| assert!(crate::ops::SKIP_DILATE.get()));
            assert!(crate::ops::SKIP_DILATE.get());
            panic!("test unwind")
        })
    });
    assert_eq!(
        panic.unwrap_err().downcast_ref::<&str>(),
        Some(&"test unwind")
    );
    assert!(!crate::ops::SKIP_DILATE.get());
}

#[test]
fn scoring_failures_preserve_the_job_and_explain_the_skip() {
    let (dir, jobs) = fixture(true);
    let out = dir.path().join("out");
    let mut job = named(&jobs, "plain.dds").clone();
    let key = output_key(&out, &job.output);
    let records = Manifest::at(&out).read().unwrap();
    let record = &records[&key].raw;
    let check = |score: JobScore, reason: &str| {
        assert_eq!(score.row["why"], reason);
        assert_eq!(score.row["output"], key);
        assert_eq!(score.row["levels"], 7);
        assert!(score.row["detail"].as_str().is_some_and(|s| !s.is_empty()));
        assert!(score.row.get("per_level").is_none());
    };
    check(
        score_record(&job, &key, &json!({}), prepare_job),
        "missing isa",
    );
    let panicked = score_record(&job, &key, record, |_| panic!("fixture engine panic"));
    assert_eq!(panicked.row["detail"], "fixture engine panic");
    check(panicked, "scoring panic");
    job.input = dir.path().join("absent.png");
    check(
        score_record(&job, &key, record, prepare_job),
        "prepare failed",
    );
    job.output = dir.path().join("invalid.dds");
    fs::write(&job.output, b"not a DDS").unwrap();
    check(score_record(&job, &key, record, prepare_job), "invalid dds");
}

#[test]
fn normal_statistics_use_degrees_and_nearest_rank_percentiles() {
    let lengths: Vec<_> = (1..=100).map(|n| [f64::from(n), 0., 0.]).collect();
    assert_eq!(normal_lengths(&lengths), (50.5, 1., 99.));
    let reference = vec![[0., 0., 1.]; 100];
    let mut decoded = reference.clone();
    decoded[98] = [0., 1., 0.];
    decoded[99] = [0., 0., -1.];
    assert_eq!(angle_stats(&decoded, &reference), (2.7, 90.));
}

#[test]
fn a_cancelling_model_space_normal_has_no_flat_direction() {
    let base = Rgba32F {
        width: 2,
        height: 1,
        px: vec![[1., 0., 0., 1.], [-1., 0., 0., 1.]],
    };
    let kind = RefKind::Vector { normalise: true };
    assert_eq!(
        stored_reference(&base, (1, 1), kind, Domain::Normal),
        [[0.5, 0.5, 1., 1.]]
    );
    assert_eq!(
        stored_reference(&base, (1, 1), kind, Domain::Msn),
        [[0.5, 0.5, 0.5, 1.]]
    );
}
