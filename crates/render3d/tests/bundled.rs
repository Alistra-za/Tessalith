//! The pinned CS tree embedded in the crate (Note GT2): reads, closure, and equality with the folder it came from.
use render3d::includes::expand;
use render3d::source::SourceSet;
use render3d::ShaderSource;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const ENTRIES: [&str; 3] = [
    "Lighting.hlsl",
    "DeferredCompositeCS.hlsl",
    "DynamicCubemaps/SpecularIrradianceCS.hlsl",
];

fn unexpanded(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|l| {
            let t = l.trim_start();
            t.starts_with('#') && t[1..].trim_start().starts_with("include")
        })
        .collect()
}

#[test]
fn every_entry_expands_from_the_bundle() {
    for e in ENTRIES {
        let x = expand(&ShaderSource::default(), e).unwrap();
        assert!(
            unexpanded(&x.text).is_empty(),
            "{e}: {:?}",
            unexpanded(&x.text)
        );
        assert!(x.files.len() > 1, "{e}");
    }
}

#[test]
fn the_bundle_holds_only_the_closure() {
    let mut reached = BTreeSet::new();
    for e in ENTRIES {
        for f in expand(&ShaderSource::Bundled, e).unwrap().files {
            reached.insert(f.to_string_lossy().to_ascii_lowercase());
        }
    }
    reached.insert("settingsdefault.json".to_string());
    let held: BTreeSet<String> = render3d::bundled_names()
        .into_iter()
        .map(str::to_string)
        .collect();
    assert_eq!(held, reached);
    assert_eq!(held.len(), 58);
}

#[test]
fn a_missing_file_is_an_error_naming_it() {
    let e = SourceSet::new(ShaderSource::Bundled)
        .get(Path::new("nope.hlsli"))
        .err()
        .unwrap();
    assert!(e.contains("nope.hlsli"), "{e}");
    // A missing include stays verbatim, so the compiler names it.
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("a.hlsl"), "#include \"nope.hlsli\"\n").unwrap();
    let x = expand(&ShaderSource::Folder(d.path().to_path_buf()), "a.hlsl").unwrap();
    assert!(x.text.contains("nope.hlsli"));
}

#[test]
fn bundled_expansion_equals_the_folder() {
    let Some(root) = std::env::var_os("SDC_CS_SHADERS").map(PathBuf::from) else {
        eprintln!("skipped: set SDC_CS_SHADERS");
        return;
    };
    let bundled = SourceSet::new(ShaderSource::Bundled);
    let folder = SourceSet::new(ShaderSource::Folder(root));
    for e in ENTRIES {
        let (b, f) = (
            bundled.get(Path::new(e)).unwrap(),
            folder.get(Path::new(e)).unwrap(),
        );
        assert!(b.text == f.text, "{e}: text differs");
        assert_eq!(b.sha256, f.sha256, "{e}");
        eprintln!("{e}: {} bytes, sha256 equal", b.text.len());
    }
}

// ---- Tree B: CS TruePBR 2.0 (Note ID) ----

/// Git blob sha1 (`blob <len>\0<bytes>`), so the test can name a commit's file by the id git gives it.
fn git_blob_sha1(data: &[u8]) -> String {
    let mut msg = format!("blob {}\0", data.len()).into_bytes();
    msg.extend_from_slice(data);
    let bit_len = (msg.len() as u64) * 8;
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    let mut h: [u32; 5] = [
        0x6745_2301,
        0xEFCD_AB89,
        0x98BA_DCFE,
        0x1032_5476,
        0xC3D2_E1F0,
    ];
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 80];
        for (i, b) in chunk.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A82_7999),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6u32),
            };
            let t = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = t;
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e]) {
            *x = x.wrapping_add(y);
        }
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}

#[test]
fn git_blob_sha1_matches_git() {
    // `printf '' | git hash-object --stdin` and `printf 'hello\n' | git hash-object --stdin`.
    assert_eq!(
        git_blob_sha1(b""),
        "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391"
    );
    assert_eq!(
        git_blob_sha1(b"hello\n"),
        "ce013625030ba8dba906f756967f9e9ca394464a"
    );
}

/// The files the bundler wrote into `shaders/cs2` (including `SettingsDefault.json`).
const TREE_B_FILES: usize = 54;

#[test]
fn tree_b_holds_only_its_closure() {
    let mut reached = BTreeSet::new();
    for e in ENTRIES {
        for f in expand(&ShaderSource::BundledB, e).unwrap().files {
            reached.insert(f.to_string_lossy().to_ascii_lowercase());
        }
    }
    reached.insert("settingsdefault.json".to_string());
    let held: BTreeSet<String> = render3d::bundled_names_in(render3d::Tree::B)
        .into_iter()
        .map(str::to_string)
        .collect();
    assert_eq!(held, reached);
    assert_eq!(held.len(), TREE_B_FILES);
}

#[test]
fn tree_b_skin_is_advanced_skin() {
    let skin =
        render3d::bundled_read_in(render3d::Tree::B, "Skin/Skin.hlsli").expect("tree B Skin.hlsli");
    // jiayev/advanced-skin be6afe5.
    assert_eq!(
        git_blob_sha1(skin),
        "be6afe5e165a1a2dd6f1ed08f2aa0cea8b8508a8"
    );
}

#[test]
fn tree_a_unchanged() {
    assert_eq!(render3d::bundled_names().len(), 58);
    let skin = render3d::bundled_read_in(render3d::Tree::A, "Skin/Skin.hlsli").unwrap();
    assert_eq!(
        git_blob_sha1(skin),
        "be6afe5e165a1a2dd6f1ed08f2aa0cea8b8508a8"
    );
}

/// Compiles tree B's `Lighting.hlsl` with the default PS and VS permutations and with `TRUE_PBR`. Not GPU (GT5).
#[test]
fn tree_b_lighting_compiles() {
    use render3d::permutation::{defines, Material, Stage};
    let m = |slsf2: u32| Material {
        shader_type: 0,
        slsf1: 0,
        slsf2,
        skinned: false,
        alpha_test: false,
        has_vertex_colors: false,
        specular_strength: 1.0,
        projected: false,
        projected_glint: false,
        wetness: false,
    };
    let src = SourceSet::new(ShaderSource::BundledB)
        .get(Path::new("Lighting.hlsl"))
        .unwrap();
    let cache = tempfile::tempdir().unwrap();
    for (name, slsf2, stage, target) in [
        ("ps default", 0, Stage::Pixel, "ps_5_0"),
        ("vs default", 0, Stage::Vertex, "vs_5_0"),
        ("ps TRUE_PBR", 1 << 23, Stage::Pixel, "ps_5_0"),
    ] {
        let d = defines(&m(slsf2), stage, &[], false);
        let c = render3d::compile::compile(&src, &d, target, cache.path())
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(!c.reflection.cbuffers.is_empty(), "{name}");
    }
}
