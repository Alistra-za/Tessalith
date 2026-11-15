//! Note IE8: the defines sdc picks for a golden subject's own NIF material are the ones CS 2.0 compiled in the capture
//! (`defines_match_capture`), and so are the lighting flags CS 2.0 reads at run time from cb4 `PixelShaderDescriptor`.
//! Needs SDC_GOLDEN (the CS PBR2Test1 root) and SDC_ACHERUS; runs alone (it loads each subject on the GPU).
#[path = "common/golden_nif.rs"]
mod golden_nif;

use golden_nif::{env, key, load_sdc, mapped_shape, renderer, M};
use render3d::golden;
use render3d::material::drawn;
use std::collections::BTreeSet;
use std::fmt::Write as _;

/// The profiles `golden.rs` replays.
const PROFILES: &[&str] = &["A", "A_RT", "A_RT2", "A_DC", "A_DC_N"];

/// cb4 `PixelShaderDescriptor` bits CS 2.0 reads at run time for a non-LOD, non-landscape shape: Specular, Soft, Rim and
/// Back lighting (cs2 `Lighting.hlsl` `RuntimeLightingFlags`, `Common/Permutation.hlsli` `LightingFlags`). The other
/// bits of the captured value are engine state (ShadowDir, DefShadow, CharacterLight) or the technique.
const LIGHTING_BITS: u32 = 0x1E00;

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
        let Some((sdc_defs, _, _)) = ren.draw_diag(sdc.id) else {
            failures.push(format!("{who}: no draw_diag ({:?})", sdc.errors));
            continue;
        };
        compared += 1;
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
        let cap_cb4 = u32::from_le_bytes(raw[4..8].try_into().unwrap()) & LIGHTING_BITS;
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
    assert!(compared >= 40, "only {compared} subjects compared");
    assert!(stale.is_empty(), "exceptions no subject needs: {stale:?}");
    assert!(
        mismatches.is_empty(),
        "sdc's defines differ from the capture: {mismatches:#?}"
    );
}
