//! `sdc uv` end to end (Note IB): NIFs are generated here with the NIF team's test builder, so no game asset is
//! needed. The geometry cases (conservative coverage, pad rule, 8-connected islands) are unit tests in `src/uv.rs`.
#[path = "../../nif/tests/builder/mod.rs"]
mod builder;
mod common;

use builder::scenes::{avobj, lsp, net, texset};
use builder::Nif;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// f32 to half float, rounded to nearest (the NIF stores UVs as halves); normal range only.
fn f16(v: f32) -> u16 {
    if v == 0.0 {
        return 0;
    }
    let b = v.to_bits();
    let e = ((b >> 23) & 0xff) as i32 - 127 + 15;
    assert!(
        (1..31).contains(&e),
        "{v} is outside the half range this helper handles"
    );
    let m = b & 0x7f_ffff;
    let mut h = ((e as u32) << 10) | (m >> 13);
    h += (m >> 12) & 1;
    ((b >> 16) & 0x8000) as u16 | h as u16
}

/// A BSTriShape with a quad over UV box `[u0, u1, v0, v1]` (two triangles); `None` has no vertices.
fn shape(shader: i32, quad: Option<[f32; 4]>) -> Vec<u8> {
    let w = avobj(net(-1)).fs(&[0.0; 4]).i32(-1).i32(shader).i32(-1);
    let Some([u0, u1, v0, v1]) = quad else {
        return w.u64(0).u16(0).u16(0).u32(0).u32(0).0;
    };
    // Vertex size 5 dwords: position and an unused float, then the UV as two halves.
    let mut w = w.u64(5 | (3u64 << 44)).u16(2).u16(4).u32(92);
    for (u, v) in [(u0, v0), (u1, v0), (u1, v1), (u0, v1)] {
        w = w.fs(&[0.0; 4]).u16(f16(u)).u16(f16(v));
    }
    for i in [0u16, 1, 2, 0, 2, 3] {
        w = w.u16(i);
    }
    w.u32(0).0
}

/// A NIF whose shapes (one per quad, all sharing a shader) name `tex` in texture-set slot `slot`.
fn nif(tex: &str, slot: usize, quads: &[Option<[f32; 4]>]) -> Vec<u8> {
    let mut n = Nif::new(100);
    let k = quads.len() as i32;
    for q in quads {
        n.block("BSTriShape", shape(k, *q));
    }
    n.block("BSLightingShaderProperty", lsp(0, k + 1));
    let mut slots = vec![""; slot];
    slots.push(tex);
    n.block("BSShaderTextureSet", texset(&slots));
    n.bytes()
}

/// The UV box of texels `x0..x1` by `y0..y1` of a 512 texture.
fn texels(x0: u32, x1: u32, y0: u32, y1: u32) -> Option<[f32; 4]> {
    let f = |v: u32| v as f32 / 512.0;
    Some([f(x0), f(x1), f(y0), f(y1)])
}

fn put(root: &Path, rel: &str, bytes: &[u8]) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, bytes).unwrap();
}

/// `sdc` with both user folders redirected, so the developer's own scan database is never read.
fn sdc(local: &Path) -> Command {
    let mut c = common::sdc();
    c.env("LOCALAPPDATA", local);
    c
}

fn run(c: &mut Command) -> (i32, String, String) {
    let o: Output = c.output().unwrap();
    (
        o.status.code().unwrap(),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

struct Fx {
    d: tempfile::TempDir,
}

impl Fx {
    fn new() -> Fx {
        Fx {
            d: tempfile::tempdir().unwrap(),
        }
    }
    fn meshes(&self) -> PathBuf {
        self.d.path().join("meshes")
    }
    fn out(&self) -> PathBuf {
        self.d.path().join("out")
    }
    fn local(&self) -> PathBuf {
        self.d.path().join("local")
    }
    /// `sdc uv <pattern> <args>` over the meshes folder, writing to `out`.
    fn uv(&self, pattern: &str, args: &[&str]) -> (i32, String, String) {
        let mut c = sdc(&self.local());
        c.args(["uv", pattern, "--meshes"])
            .arg(self.meshes())
            .arg("--out")
            .arg(self.out())
            .args(args);
        run(&mut c)
    }
    fn png(&self, rel: &str) -> image::GrayImage {
        image::open(self.out().join(rel)).unwrap().into_luma8()
    }
}

#[test]
fn one_quad_island_has_a_band_of_exactly_pad_texels() {
    let f = Fx::new();
    put(
        &f.meshes(),
        "a/wall.nif",
        &nif("textures\\a\\wall.dds", 0, &[texels(128, 256, 128, 256)]),
    );
    let (code, out, err) = f.uv("a\\wall", &["--s", "512"]);
    assert_eq!(code, 0, "{err}");
    assert!(
        out.contains("islands 1, pad 2, too close 0, meshes 1"),
        "{out}"
    );
    assert!(out.contains("wall_uvmask.png"), "{out}");
    let img = f.png("wall_uvmask.png");
    assert_eq!(img.dimensions(), (512, 512));
    let px = |x, y| img.get_pixel(x, y).0[0];
    assert_eq!((px(128, 128), px(255, 255), px(200, 200)), (255, 255, 255));
    assert_eq!(
        (px(126, 200), px(127, 200), px(256, 200), px(257, 200)),
        (127, 127, 127, 127)
    );
    assert_eq!(
        (px(125, 200), px(258, 200), px(200, 125), px(200, 258)),
        (0, 0, 0, 0)
    );
    assert!(img.pixels().all(|p| [0, 127, 255].contains(&p.0[0])));
}

#[test]
fn islands_closer_than_two_pads_get_191_between_them() {
    let near = Fx::new();
    put(
        &near.meshes(),
        "wall.nif",
        &nif(
            "textures/wall.dds",
            0,
            &[texels(128, 256, 128, 256), texels(259, 387, 128, 256)],
        ),
    );
    let (code, out, err) = near.uv("wall", &["--s", "512"]);
    assert_eq!(code, 0, "{err}");
    assert!(
        out.contains("islands 2, pad 2, too close 132, meshes 1"),
        "{out}"
    );
    let img = near.png("wall_uvmask.png");
    assert_eq!(img.get_pixel(257, 200).0[0], 191);
    assert_eq!(img.get_pixel(256, 200).0[0], 127);

    let far = Fx::new();
    put(
        &far.meshes(),
        "wall.nif",
        &nif(
            "textures/wall.dds",
            0,
            &[texels(128, 256, 128, 256), texels(260, 388, 128, 256)],
        ),
    );
    let (code, out, err) = far.uv("wall", &["--s", "512"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("islands 2, pad 2, too close 0"), "{out}");
    assert!(!far.png("wall_uvmask.png").pixels().any(|p| p.0[0] == 191));
}

#[test]
fn the_pad_follows_the_longer_side() {
    let f = Fx::new();
    put(
        &f.meshes(),
        "wall.nif",
        &nif("textures/wall.dds", 0, &[texels(128, 256, 128, 256)]),
    );
    for (args, pad) in [
        (["--s", "512", "", ""], 2),
        (["--s", "4096", "", ""], 16),
        (["--w", "2048", "--l", "4096"], 16),
    ] {
        let args: Vec<&str> = args.into_iter().filter(|a| !a.is_empty()).collect();
        let (code, out, err) = f.uv("wall", &[&args[..], &["--overwrite"]].concat());
        assert_eq!(code, 0, "{err}");
        assert!(out.contains(&format!("pad {pad},")), "{args:?}: {out}");
    }
    assert_eq!(f.png("wall_uvmask.png").dimensions(), (2048, 4096));
}

#[test]
fn a_sliver_narrower_than_a_texel_is_drawn() {
    let f = Fx::new();
    let sliver = Some([10.2 / 512.0, 10.5 / 512.0, 100.0 / 512.0, 200.0 / 512.0]);
    put(
        &f.meshes(),
        "wall.nif",
        &nif("textures/wall.dds", 0, &[sliver]),
    );
    let (code, out, err) = f.uv("wall", &["--s", "512"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("islands 1,"), "{out}");
    let img = f.png("wall_uvmask.png");
    assert_eq!(
        (img.get_pixel(10, 150).0[0], img.get_pixel(10, 100).0[0]),
        (255, 255)
    );
    assert_eq!(img.get_pixel(10, 99).0[0], 127);
}

#[test]
fn uvs_outside_0_to_1_wrap() {
    let f = Fx::new();
    let [u0, u1, v0, v1] = texels(128, 256, 128, 256).unwrap();
    put(
        &f.meshes(),
        "a.nif",
        &nif("textures/a.dds", 0, &[Some([u0, u1, v0, v1])]),
    );
    put(
        &f.meshes(),
        "b.nif",
        &nif(
            "textures/b.dds",
            0,
            &[Some([u0 + 1.0, u1 + 1.0, v0 - 2.0, v1 - 2.0])],
        ),
    );
    assert_eq!(f.uv("a", &["--s", "512"]).0, 0);
    assert_eq!(f.uv("b", &["--s", "512"]).0, 0);
    assert_eq!(f.png("a_uvmask.png"), f.png("b_uvmask.png"));
}

#[test]
fn pattern_spellings_and_folders() {
    let f = Fx::new();
    put(
        &f.meshes(),
        "m/one.nif",
        &nif(
            "Data\\Textures\\Harry\\Wall.dds",
            0,
            &[texels(128, 256, 128, 256)],
        ),
    );
    put(
        &f.meshes(),
        "m/two.nif",
        &nif(
            "textures\\harry\\deep\\floor.dds",
            2,
            &[texels(128, 256, 128, 256)],
        ),
    );
    // The texture is named in slot 2 of a set; every spelling finds it.
    for p in ["harry\\floor_x", "nothing"] {
        assert_eq!(f.uv(p, &["--s", "512"]).0, 2, "{p} matches nothing");
    }
    for p in [
        "harry/deep/floor",
        "Textures\\Harry\\Deep\\Floor.dds",
        "HARRY\\DEEP\\FL*",
        "harry/deep/fl?or",
        "harry\\deep",
    ] {
        let f = Fx::new();
        put(
            &f.meshes(),
            "m/two.nif",
            &nif(
                "textures\\harry\\deep\\floor.dds",
                2,
                &[texels(128, 256, 128, 256)],
            ),
        );
        let (code, _, err) = f.uv(p, &["--s", "512"]);
        assert_eq!(code, 0, "{p}: {err}");
        assert!(f.out().join("floor_uvmask.png").is_file(), "{p}");
    }
    // A folder matches the textures directly in it; --recursive takes those below too, mirrored below the folder.
    let (code, out, err) = f.uv("harry", &["--s", "512"]);
    assert_eq!(code, 0, "{err}");
    assert!(
        out.contains("wall_uvmask.png") && !out.contains("floor"),
        "{out}"
    );
    assert!(f.out().join("wall_uvmask.png").is_file());
    let (code, _, err) = f.uv("harry", &["--s", "512", "--recursive", "--overwrite"]);
    assert_eq!(code, 0, "{err}");
    assert!(f.out().join("deep").join("floor_uvmask.png").is_file());
}

#[test]
fn an_existing_output_is_kept_without_overwrite() {
    let f = Fx::new();
    put(
        &f.meshes(),
        "wall.nif",
        &nif("textures/wall.dds", 0, &[texels(128, 256, 128, 256)]),
    );
    put(&f.out(), "wall_uvmask.png", b"mine");
    let (code, out, err) = f.uv("wall", &["--s", "512"]);
    assert_eq!((code, out.as_str()), (1, ""), "{err}");
    assert!(err.contains("--overwrite"), "{err}");
    assert_eq!(
        std::fs::read(f.out().join("wall_uvmask.png")).unwrap(),
        b"mine"
    );
    let (code, _, err) = f.uv("wall", &["--s", "512", "--overwrite"]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(f.png("wall_uvmask.png").dimensions(), (512, 512));
}

#[test]
fn size_flags_are_checked() {
    let f = Fx::new();
    put(
        &f.meshes(),
        "wall.nif",
        &nif("textures/wall.dds", 0, &[texels(128, 256, 128, 256)]),
    );
    for args in [
        &["--s", "512", "--w", "512"][..],
        &["--s", "512", "--l", "512"],
        &["--w", "512"],
        &["--l", "512"],
        &[],
        &["--s", "0"],
        &["--s", "16385"],
    ] {
        assert_eq!(f.uv("wall", args).0, 2, "{args:?}");
    }
    // The long spellings are aliases.
    assert_eq!(f.uv("wall", &["--size", "512"]).0, 0);
    assert_eq!(
        f.uv(
            "wall",
            &["--width", "512", "--length", "256", "--overwrite"]
        )
        .0,
        0
    );
    assert_eq!(f.png("wall_uvmask.png").dimensions(), (512, 256));
}

#[test]
fn a_bad_nif_is_reported_and_skipped_and_a_texture_without_triangles_writes_nothing() {
    let f = Fx::new();
    put(
        &f.meshes(),
        "good.nif",
        &nif("textures/wall.dds", 0, &[texels(128, 256, 128, 256)]),
    );
    put(&f.meshes(), "bad.nif", b"not a nif");
    put(
        &f.meshes(),
        "empty.nif",
        &nif("textures/hollow.dds", 0, &[None]),
    );
    let (code, out, err) = f.uv("wall", &["--s", "512"]);
    assert_eq!(code, 0, "{err}");
    assert!(err.contains("bad.nif") && err.contains("skipped"), "{err}");
    assert!(out.contains("meshes 1"), "{out}");
    let (code, out, err) = f.uv("hollow", &["--s", "512"]);
    assert_eq!((code, out.as_str()), (1, ""), "{err}");
    assert!(err.contains("nothing written"), "{err}");
    assert!(!f.out().join("hollow_uvmask.png").exists());
}

#[test]
fn the_scan_database_is_the_other_source() {
    let f = Fx::new();
    let data = f.d.path().join("data");
    put(
        &data,
        "meshes/m/wall.nif",
        &nif("textures/wall.dds", 0, &[texels(128, 256, 128, 256)]),
    );
    let db = f.d.path().join("scan.db");
    let mut s = modscan::store::Store::open(&db).unwrap();
    modscan::scan::run(
        &modscan::scan::Source::Folder(data),
        &mut s,
        &std::sync::atomic::AtomicBool::new(false),
        &|_| {},
        &|| vec![],
    )
    .unwrap();
    drop(s);
    let mut c = sdc(&f.local());
    c.args(["uv", "wall", "--s", "512", "--scan"])
        .arg(&db)
        .arg("--out")
        .arg(f.out());
    let (code, out, err) = run(&mut c);
    assert_eq!(code, 0, "{err}");
    assert!(
        out.contains("islands 1, pad 2, too close 0, meshes 1"),
        "{out}"
    );
    // The same picture as reading the folder.
    put(
        &f.meshes(),
        "wall.nif",
        &nif("textures/wall.dds", 0, &[texels(128, 256, 128, 256)]),
    );
    let direct = Fx::new();
    put(
        &direct.meshes(),
        "wall.nif",
        &nif("textures/wall.dds", 0, &[texels(128, 256, 128, 256)]),
    );
    assert_eq!(direct.uv("wall", &["--s", "512"]).0, 0);
    assert_eq!(f.png("wall_uvmask.png"), direct.png("wall_uvmask.png"));
}

#[test]
fn a_missing_scan_database_names_the_gui_scan_and_meshes() {
    let f = Fx::new();
    let mut c = sdc(&f.local());
    c.args(["uv", "wall", "--s", "512"])
        .arg("--out")
        .arg(f.out());
    let (code, _, err) = run(&mut c);
    assert_eq!(code, 2);
    assert!(
        err.contains("Meshes scan") && err.contains("--meshes"),
        "{err}"
    );
    assert!(!f.out().exists());
}

#[test]
fn the_default_output_folder_is_the_current_one() {
    let f = Fx::new();
    put(
        &f.meshes(),
        "wall.nif",
        &nif("textures/wall.dds", 0, &[texels(128, 256, 128, 256)]),
    );
    std::fs::create_dir_all(f.out()).unwrap();
    let mut c = sdc(&f.local());
    c.current_dir(f.out())
        .args(["uv", "wall", "--s", "512", "--meshes"])
        .arg(f.meshes());
    let (code, _, err) = run(&mut c);
    assert_eq!(code, 0, "{err}");
    assert!(f.out().join("wall_uvmask.png").is_file());
}
