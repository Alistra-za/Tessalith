//! Fast re-plan fixture (R2 §6): one generated cs project with every set shape the skeleton patch and the partial
//! plan must get right. Used by `engine::skeleton`'s lib tests (through `#[path]`) and by `tests/fast_replan.rs`.
#![allow(dead_code)]
use engine::api::BatchRequest;
use engine::config::Target;
use engine::presets::ConfigFile;
use image::{ImageBuffer, Luma, Rgb, Rgba};
use std::path::Path;

/// The project's base `[files]` table; keys are relative to `in/` (the first input).
pub const FAST_FILES_TOML: &str = r#"
[files."dir/b_mask"]
helper.role = "opacity"

[files."dir/c_px"]
subtype = "cubemap"
[files."dir/c_nx"]
subtype = "cubemap"
[files."dir/c_py"]
subtype = "cubemap"
[files."dir/c_ny"]
subtype = "cubemap"
[files."dir/c_pz"]
subtype = "cubemap"
[files."dir/c_nz"]
subtype = "cubemap"

[files."dir/s_px"]
subtype = "cubemap"
[files."dir/s_nx"]
subtype = "cubemap"
[files."dir/s_py"]
subtype = "cubemap"
[files."dir/s_ny"]
subtype = "cubemap"
[files."dir/s_pz"]
subtype = "cubemap"
[files."dir/s_nz"]
subtype = "cubemap"

[files."dir/h_rmaos".packing]
recipe = "rfaos"

[files."dir/q_rough"]
subtype = "diffuse"
"#;

/// A temp project and the request that plans it.
pub struct FastFixture {
    pub dir: tempfile::TempDir,
    pub req: BatchRequest,
}

fn put_rgb(dir: &Path, name: &str, seed: u8) {
    ImageBuffer::from_fn(16, 16, |x, y| {
        Rgb([(x * 9) as u8 ^ seed, (y * 7) as u8, seed.wrapping_add(40)])
    })
    .save(dir.join(name))
    .unwrap();
}

fn put_grey(dir: &Path, name: &str, seed: u8) {
    ImageBuffer::from_fn(16, 16, |x, _| Luma([seed.wrapping_add((x * 5) as u8)]))
        .save(dir.join(name))
        .unwrap();
}

/// A tangent-space-looking normal.
fn put_normal(dir: &Path, name: &str) {
    ImageBuffer::from_fn(16, 16, |x, y| {
        Rgb([128 + (x % 8) as u8, 128 + (y % 8) as u8, 250])
    })
    .save(dir.join(name))
    .unwrap();
}

/// The cs project of R2 §6, under `in/dir/`:
/// - PBR set `a` (`a`, `a_n`, `a_rmaos`);
/// - `b` and the hand-set helper `b_mask` (`helper.role = opacity`), joined to set `b`;
/// - six cube faces `c_px` .. `c_nz`, each `subtype = cubemap`;
/// - `d` (`d`, `d_n`);
/// - packed raw `e_ao`, `e_rough`, `e_metal` (key `dir/e_p` names their set);
/// - stem `f` over `f.png` and `f.dds`;
/// - duplicates `g.png` and `g.tga`;
/// - Advanced skin `h` (`h_sk`, raw `h_roughness` and `h_ao` with `packing.recipe = rfaos` on `dir/h_rmaos`), which
///   joins `h_wet`;
/// - `i` (`i`, `i_rmaos`) borrowing `a`'s normal;
/// - `j` and `k`, with a set edit moving `k.png` into set `dir/j`;
/// - non-PBR `q` with a raw height `q_height` (packs `_p`) and `q_rough`, typed `diffuse`: untyped, it would make
///   `q` pack `_rmaos` and read as PBR;
/// - `MixedCase.png`, a file name with upper-case letters;
/// - a BV2 writer collision: `n` (`n`, raw `n_ao`) and `o`, with a set edit moving raw `n_rough.png` into `o`, so both
///   sets pack a default `n_rmaos` (Note BV2);
/// - a second six-face cube `s_px` .. `s_nz` beside `s.png`, whose output the merged cube's clashes with;
/// - `z.png`, a corrupt source (a load failure);
/// - shared roughness on sets `e` and `q`.
pub fn fast_fixture() -> FastFixture {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path().join("in/dir");
    std::fs::create_dir_all(&d).unwrap();
    put_rgb(&d, "a.png", 1);
    put_normal(&d, "a_n.png");
    put_rgb(&d, "a_rmaos.png", 2);
    put_rgb(&d, "b.png", 3);
    put_grey(&d, "b_mask.png", 4);
    for (n, face) in ["px", "nx", "py", "ny", "pz", "nz"].iter().enumerate() {
        put_rgb(&d, &format!("c_{face}.png"), 10 + n as u8);
    }
    put_rgb(&d, "d.png", 20);
    put_normal(&d, "d_n.png");
    for (n, role) in ["ao", "rough", "metal"].iter().enumerate() {
        put_grey(&d, &format!("e_{role}.png"), 30 + n as u8 * 40);
    }
    put_rgb(&d, "f.png", 40);
    let desc = engine::dds::DdsDesc {
        width: 4,
        height: 4,
        format: engine::config::Format::Rgba8,
        srgb: false,
        header: engine::dds::ResolvedHeader::Dx10,
    };
    std::fs::write(
        d.join("f.dds"),
        engine::dds::dds_bytes(&desc, &[vec![0u8; 64]]).unwrap(),
    )
    .unwrap();
    put_rgb(&d, "g.png", 50);
    ImageBuffer::from_fn(16, 16, |x, y| Rgba([(x * 3) as u8, (y * 3) as u8, 9, 255]))
        .save(d.join("g.tga"))
        .unwrap();
    put_rgb(&d, "h.png", 60);
    put_grey(&d, "h_sk.png", 61);
    put_normal(&d, "h_wet.png");
    put_grey(&d, "h_roughness.png", 62);
    put_grey(&d, "h_ao.png", 63);
    put_rgb(&d, "i.png", 70);
    put_rgb(&d, "i_rmaos.png", 71);
    put_rgb(&d, "j.png", 80);
    put_rgb(&d, "k.png", 81);
    put_rgb(&d, "q.png", 90);
    put_grey(&d, "q_height.png", 91);
    put_grey(&d, "q_rough.png", 92);
    put_rgb(&d, "MixedCase.png", 100);
    put_rgb(&d, "n.png", 110);
    put_grey(&d, "n_ao.png", 111);
    put_grey(&d, "n_rough.png", 112);
    put_rgb(&d, "o.png", 113);
    for (n, face) in ["px", "nx", "py", "ny", "pz", "nz"].iter().enumerate() {
        put_rgb(&d, &format!("s_{face}.png"), 120 + n as u8);
    }
    put_rgb(&d, "s.png", 130);
    std::fs::write(d.join("z.png"), b"not a png").unwrap();

    let mut req = BatchRequest::new(
        vec![dir.path().join("in")],
        dir.path().join("out"),
        Target::Cs,
    );
    req.recursive = true;
    req.project_config_inline = Some(ConfigFile::parse(FAST_FILES_TOML, "fast fixture").unwrap());
    req.borrowed_normal = [("dir/i".to_string(), "dir/a".to_string())]
        .into_iter()
        .collect();
    req.set_edits = vec![
        engine::sets::SetEdit::Move {
            file: "dir/k.png".into(),
            to: "dir/j".into(),
        },
        engine::sets::SetEdit::Move {
            file: "dir/n_rough.png".into(),
            to: "dir/o".into(),
        },
    ];
    req.shared_roughness = ["dir/e", "dir/q"].map(String::from).into();
    FastFixture { dir, req }
}

/// `req` with `[files."<key>"]` set to `entry` (`None` removes it), as the GUI's in-memory document holds it.
pub fn with_entry(req: &BatchRequest, key: &str, entry: Option<&toml::Table>) -> BatchRequest {
    let mut r = req.clone();
    let files = &mut r.project_config_inline.as_mut().unwrap().files;
    match entry {
        Some(t) => {
            files.insert(key.to_string(), serde_json::to_value(t).unwrap());
        }
        None => {
            files.remove(key);
        }
    }
    r
}
