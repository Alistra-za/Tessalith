//! Generated project shared by the profile integration tests and fixture export.
use image::{DynamicImage, ImageBuffer, ImageFormat, Luma, Rgb, Rgba};
use std::{
    fs,
    io::{self, Write},
    path::Path,
};

const CONFIG: &str = r#"
[files."textures/clutter/plain.png"]
subtype = "diffuse"
mips.filter = "kaiser"
[files."textures/plants/leaf.png"]
subtype = "diffuse"
alpha.policy = "cutout"
[files."textures/clutter/onebit.png"]
subtype = "diffuse"
format = "bc1"
encoder.bc1_alpha = true
alpha.policy = "cutout"
[files."textures/clutter/glass.png"]
subtype = "diffuse"
alpha.policy = "blend"
[files."textures/clutter/keep_n.png"]
mips.normal_base = "keep"
[files."textures/landscape/cliff_h.png"]
subtype = "height"
[files."textures/clutter/wide.png"]
subtype = "diffuse"
"#;

pub fn write_project(root: &Path) -> io::Result<Vec<&'static str>> {
    fs::create_dir(root)?;
    let create = |name: &Path| {
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(name)
    };
    create(&root.join("sdc.toml"))?.write_all(CONFIG.as_bytes())?;
    let png = |name: &str, img: DynamicImage| -> io::Result<()> {
        let path = root.join(name);
        fs::create_dir_all(path.parent().unwrap())?;
        img.write_to(&mut create(&path)?, ImageFormat::Png)
            .map_err(io::Error::other)
    };
    let gradient = |w, h| {
        ImageBuffer::from_fn(w, h, |x, y| {
            Rgb([(x * 4) as u8, (y * 4) as u8, ((x + y) * 2) as u8])
        })
    };
    png("textures/clutter/plain.png", gradient(64, 64).into())?;
    png("textures/clutter/wide.png", gradient(31, 20).into())?;
    let leaf = ImageBuffer::from_fn(64, 64, |x, y| {
        let inside = (x as f32 - 31.5).powi(2) + (y as f32 - 30.5).powi(2) <= 21.0 * 21.0;
        Rgba(if inside {
            [32, (96 + y * 2) as u8, 16, 255]
        } else {
            [255, 0, 255, 0]
        })
    });
    png("textures/plants/leaf.png", leaf.clone().into())?;
    png("textures/clutter/onebit.png", leaf.into())?;
    png(
        "textures/clutter/glass.png",
        ImageBuffer::from_fn(64, 64, |x, y| {
            Rgba([40, (y * 4) as u8, 175, (x * 255 / 63) as u8])
        })
        .into(),
    )?;
    png("textures/pbr/fx/crate.png", gradient(64, 64).into())?;
    png(
        "textures/pbr/fx/crate_n.png",
        ImageBuffer::from_fn(64, 64, |x, y| {
            Rgb([
                if x % 2 == 0 { 68u8 } else { 188 },
                (115 + y % 26) as u8,
                236,
            ])
        })
        .into(),
    )?;
    for (name, seed) in [("roughness", 21), ("metallic", 95), ("ao", 148)] {
        png(
            &format!("textures/pbr/fx/crate_{name}.png"),
            ImageBuffer::from_fn(64, 64, |x, y| Luma([(seed + x + y) as u8])).into(),
        )?;
    }
    png(
        "textures/clutter/keep_n.png",
        ImageBuffer::from_fn(64, 64, |x, y| {
            Rgb(match (x + y) % 4 {
                0 => [255u8, 128, 128],
                1 => [0, 127, 128],
                2 => [128, 128, 180],
                _ => [155, 155, 241],
            })
        })
        .into(),
    )?;
    png("textures/effects/water_flow.png", gradient(64, 64).into())?;
    png(
        "textures/landscape/cliff_h.png",
        ImageBuffer::from_fn(512, 512, |x, y| Luma([((x + 2 * y) / 6) as u8])).into(),
    )?;
    Ok(vec![
        "textures/clutter/glass.dds",
        "textures/clutter/keep_n.dds",
        "textures/clutter/onebit.dds",
        "textures/clutter/plain.dds",
        "textures/clutter/wide.dds",
        "textures/effects/water_flow.dds",
        "textures/landscape/cliff_h.dds",
        "textures/pbr/fx/crate.dds",
        "textures/pbr/fx/crate_n.dds",
        "textures/pbr/fx/crate_rmaos.dds",
        "textures/plants/leaf.dds",
    ])
}
