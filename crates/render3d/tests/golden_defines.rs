//! Note IE8: the defines sdc picks for a golden subject's own NIF material are the ones CS 2.0 compiled in the capture
//! (`defines_match_capture`), and so are the lighting flags CS 2.0 reads at run time from cb4 `PixelShaderDescriptor`.
//! The captures record the PS defines (decoded from the PSO) but not the VS ones, only the VS bytecode's hash: the
//! test holds sdc to one set of the VS-read defines ([`VS_READ`]) per captured VS instead.
//! Needs SDC_GOLDEN (the CS PBR2Test1 root) and SDC_ACHERUS; runs alone (it loads each subject on the GPU).
#[path = "common/golden_nif.rs"]
mod golden_nif;

use golden_nif::{env, json, key, load_sdc, mapped_shape, renderer, M};
use render3d::golden::{self, PROFILES};
use render3d::material::drawn;
use render3d::permutation::{LIGHTING_BACK, LIGHTING_RIM, LIGHTING_SOFT, LIGHTING_SPECULAR};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

/// cb4 `PixelShaderDescriptor` bits CS 2.0 reads at run time for a non-LOD, non-landscape shape: Specular, Soft, Rim and
/// Back lighting (cs2 `Lighting.hlsl` `RuntimeLightingFlags`, `Common/Permutation.hlsli` `LightingFlags`). The other
/// bits of the captured value are engine state (ShadowDir, DefShadow, CharacterLight) or the technique.
const LIGHTING_BITS: u32 = LIGHTING_SPECULAR | LIGHTING_SOFT | LIGHTING_RIM | LIGHTING_BACK;

/// The subjects compared: the replayed subjects whose shape `NIFS` maps. Update it when a capture or a `NIFS` row is
/// added or removed (the table the test writes lists the unmapped shapes).
const COMPARED: usize = 43;

/// VS defines compared across subjects that share a captured VS: the material defines cs2's VS code reads
/// (`Lighting.hlsl` `VSHADER` section, lines 35-286). It reads no technique define but EYE, so CS 2.0 draws Default,
/// EnvMap and MLP shapes with one VS (all 43 subjects, the coffin, Object174, SOVStairsPiece01 and BlackreachCrystalL01
/// among them, use two VS hashes); sdc's ENVMAP and MULTI_LAYER_PARALLAX on the VS change its compile key, not its code.
const VS_READ: &[&str] = &["EYE", "MODELSPACENORMALS", "PROJECTED_UV", "SKINNED", "VC"];

/// Capture-side differences: (subject folder name, the define or `cb4`, why). None so far.
const EXCEPTIONS: &[(&str, &str, &str)] = &[];

#[test]
#[ignore]
fn defines_match_capture() {
    let Some(env) = env() else { return };
    // Fails before any work on a root the bundled shaders did not draw (Note IE5).
    golden::noise_path(&env.root).unwrap_or_else(|e| panic!("{e}"));
    let mut ren = renderer(&env);
    let mut t = String::from("subject | shape | defines | cb4 lighting bits (capture / sdc)\n");
    let (mut compared, mut mismatches, mut failures) = (0usize, Vec::new(), Vec::new());
    let mut unmapped: BTreeSet<String> = BTreeSet::new();
    let mut matched_exceptions = BTreeSet::new();
    // Captured VS hash → sdc's VS define set → the subjects that got it.
    let mut by_vs: BTreeMap<String, BTreeMap<Vec<String>, Vec<String>>> = BTreeMap::new();
    for s in golden::subjects(&env.root, PROFILES).unwrap() {
        let name = s.dir.file_name().unwrap().to_string_lossy().into_owned();
        let who = format!("{}/{name}", s.capture);
        // The subjects the golden replay runs (its skip rules), and of those the shapes sdc can load.
        let Ok(ready) = golden::prepare(&s.dir) else {
            continue;
        };
        let Some(shape) = mapped_shape(&s.dir) else {
            unmapped.insert(s.shape.clone());
            continue;
        };
        let r = ready.replay;
        let sdc = match load_sdc(&mut ren, &env, &s.dir, &r) {
            Ok(x) => x,
            Err(e) => {
                failures.push(format!("{who}: {e}"));
                continue;
            }
        };
        let Some((sdc_defs, Some(refl), _)) = ren.draw_diag(sdc.id) else {
            failures.push(format!(
                "{who}: no draw_diag or PS reflection ({:?})",
                sdc.errors
            ));
            continue;
        };
        // The capture's PixelShaderDescriptor at its reflected offset (the same cs2 source compiled both).
        let Some(desc_at) = refl
            .cbuffers
            .iter()
            .filter(|c| c.slot == 4)
            .flat_map(|c| &c.vars)
            // fxc names it with its namespace (`Permutation::PixelShaderDescriptor`).
            .find(|v| v.name.rsplit("::").next() == Some("PixelShaderDescriptor"))
            .map(|v| v.offset as usize)
        else {
            failures.push(format!(
                "{who}: the PS reflects no cb4 PixelShaderDescriptor"
            ));
            continue;
        };
        compared += 1;
        let vs_sha = json(&s.dir.join("state.json"))["shaders"]["vs"]["sha256"]
            .as_str()
            .unwrap_or("?")
            .to_string();
        let vs_set: Vec<String> = ren
            .draw_vs_defines(sdc.id)
            .unwrap_or_default()
            .into_iter()
            .map(|d| d.0)
            .filter(|d| VS_READ.contains(&d.as_str()))
            .collect();
        by_vs
            .entry(vs_sha)
            .or_default()
            .entry(vs_set)
            .or_default()
            .push(who.clone());
        let cap: BTreeSet<&str> = r.defines[0]
            .iter()
            .map(|d| key(d))
            .filter(|k| M.contains(k))
            .collect();
        let mine: BTreeSet<&str> = sdc_defs
            .iter()
            .map(|d| d.0.as_str())
            .filter(|k| M.contains(k))
            .collect();
        let mut diffs: Vec<(String, String)> = cap
            .difference(&mine)
            .map(|d| (d.to_string(), format!("{d} capture only")))
            .chain(
                mine.difference(&cap)
                    .map(|d| (d.to_string(), format!("{d} sdc only"))),
            )
            .collect();
        let raw = std::fs::read(s.dir.join("cb_ps_b4.bin")).unwrap();
        let cap_cb4 =
            u32::from_le_bytes(raw[desc_at..desc_at + 4].try_into().unwrap()) & LIGHTING_BITS;
        let sdc_cb4 = ren
            .draw_cb_floats(sdc.id, "PixelShaderDescriptor")
            .map(|f| f[0].to_bits() & LIGHTING_BITS);
        if sdc_cb4 != Some(cap_cb4) {
            diffs.push((
                "cb4".into(),
                format!("cb4 lighting bits capture {cap_cb4:#x} sdc {sdc_cb4:x?}"),
            ));
        }
        let m = drawn(
            &ren.shapes()
                .into_iter()
                .find(|s| s.id == sdc.id)
                .unwrap()
                .material,
        )
        .into_owned();
        let _ = writeln!(
            t,
            "{who} | {shape}: {}; drawn SLSF1 {:#010x} SLSF2 {:#010x} | {} | {cap_cb4:#x} / {sdc_cb4:x?}",
            sdc.rule,
            m.slsf1,
            m.slsf2,
            if diffs.is_empty() {
                format!("EQUAL {cap:?}")
            } else {
                format!("DIFFER capture {cap:?} sdc {mine:?}")
            }
        );
        for (what, line) in diffs {
            match EXCEPTIONS.iter().find(|(n, w, _)| *n == name && *w == what) {
                Some((_, _, why)) => {
                    matched_exceptions.insert((name.clone(), what));
                    let _ = writeln!(t, "  expected: {line} ({why})");
                }
                None => mismatches.push(format!("{who}: {line}")),
            }
        }
    }
    // One captured VS was compiled from one define set, so sdc must give its subjects one VS set too.
    let split_vs: Vec<String> = by_vs
        .iter()
        .filter(|(_, sets)| sets.len() > 1)
        .map(|(sha, sets)| format!("VS {sha}: {sets:?}"))
        .collect();
    let _ = writeln!(
        t,
        "\ncaptured VS shaders {}; split across sdc VS define sets {}",
        by_vs.len(),
        split_vs.len()
    );
    for v in &split_vs {
        let _ = writeln!(t, "  {v}");
    }
    let stale: Vec<_> = EXCEPTIONS
        .iter()
        .filter(|(n, w, _)| !matched_exceptions.contains(&(n.to_string(), w.to_string())))
        .collect();
    let _ = writeln!(
        t,
        "\ncompared {compared}; mismatches {}; load failures {}; stale exceptions {stale:?}\nshapes the replay runs \
         that NIFS does not map (not compared): {unmapped:?}",
        mismatches.len(),
        failures.len()
    );
    for m in &mismatches {
        let _ = writeln!(t, "  {m}");
    }
    for f in &failures {
        let _ = writeln!(t, "  FAIL {f}");
    }
    print!("{t}");
    let out = std::env::temp_dir().join("golden-defines.txt");
    std::fs::write(&out, &t).unwrap();
    println!("table written to {}", out.display());
    assert!(
        failures.is_empty(),
        "subjects sdc did not load: {failures:#?}"
    );
    assert_eq!(compared, COMPARED, "subjects compared");
    assert!(stale.is_empty(), "exceptions no subject needs: {stale:?}");
    assert!(
        split_vs.is_empty(),
        "subjects sharing a captured VS get different sdc VS defines: {split_vs:#?}"
    );
    assert!(
        mismatches.is_empty(),
        "sdc's defines differ from the capture: {mismatches:#?}"
    );
}
