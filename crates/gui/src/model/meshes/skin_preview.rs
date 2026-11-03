//! The Meshes view's Advanced Skin card (Note GN): preview-only RFAOS / wetness map picks and a wetness slider for
//! FaceGen (4) and FaceGen RGB tint (5) shapes. They go to the 3D preview only: nothing is written to the NIF, the
//! project, gui.toml or the game, and they reset when the preview shows another model.
use super::MeshesModel;
use crate::model::viewport3d::{FileSource, SkinEntry};
use nif::material::SkinPreview;
use render3d::ShapeId;
use std::path::Path;

/// Which of the card's maps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SkinMap {
    Rfaos,
    Wetness,
}

/// Where a picked texture comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SkinPick {
    /// A scanned winning texture of the mod list (any spelling of its game path).
    Mod(String),
    /// A DDS under the Textures export folder, by its `/`-separated path under that folder.
    Project(String),
}

/// The renderer key of a project pick: never a game path, so it cannot take the mod list copy's cache entry.
pub const PROJECT_KEY: &str = "project:";

/// Project DDS files listed at most.
const PROJECT_LIMIT: usize = 20_000;

/// The shapes that get the card: Lighting shapes of type 4 or 5.
pub fn has_skin_card(shader_kind: i32, shader_type: Option<u32>) -> bool {
    shader_kind == 1 && matches!(shader_type, Some(4 | 5))
}

impl MeshesModel {
    /// Shape `block`'s card picks (None: the shape as loaded).
    pub fn skin_entry(&self, block: u32) -> Option<&SkinEntry> {
        self.v3.skin_preview(ShapeId(block))
    }

    /// Picks (Some) or returns to Auto (None) one map of shape `block`. Err, with nothing changed, when the pick
    /// cannot be read: not in the latest scan, or no Textures export folder.
    pub fn set_skin_map(
        &mut self,
        block: u32,
        map: SkinMap,
        pick: Option<SkinPick>,
    ) -> Result<(), String> {
        let mut e = self.skin_entry(block).cloned().unwrap_or(SkinEntry {
            preview: SkinPreview::default(),
            sources: vec![],
        });
        let key = match pick {
            None => None,
            Some(p) => {
                let (key, src) = self.skin_source(&p)?;
                e.sources.retain(|(k, _)| *k != key);
                e.sources.push((key.clone(), src));
                Some(key)
            }
        };
        match map {
            SkinMap::Rfaos => e.preview.rfaos = key,
            SkinMap::Wetness => e.preview.wet_map = key,
        }
        let p = &e.preview;
        let used = |k: &String| p.rfaos.as_ref() == Some(k) || p.wet_map.as_ref() == Some(k);
        e.sources.retain(|(k, _)| used(k));
        self.put_skin(block, e);
        Ok(())
    }

    /// Shape `block`'s wetness, 0 (dry) … 1 (soaked).
    pub fn set_skin_wet(&mut self, block: u32, wet: f32) {
        let mut e = self.skin_entry(block).cloned().unwrap_or(SkinEntry {
            preview: SkinPreview::default(),
            sources: vec![],
        });
        e.preview.wet = wet.clamp(0.0, 1.0);
        self.put_skin(block, e);
    }

    fn put_skin(&mut self, block: u32, e: SkinEntry) {
        let shape_as_loaded = e.preview == SkinPreview::default();
        self.v3
            .set_skin_preview(ShapeId(block), (!shape_as_loaded).then_some(e));
    }

    /// The renderer key and the file of a pick.
    fn skin_source(&self, p: &SkinPick) -> Result<(String, FileSource), String> {
        match p {
            SkinPick::Mod(path) => {
                let norm = nif::path::canon_texture(path.as_bytes()).norm;
                let found = self
                    .store
                    .as_ref()
                    .and_then(|s| s.texture_location(&norm).ok().flatten());
                match found {
                    Some((kind, root)) => {
                        let src = FileSource::from_store(kind, &root, &norm);
                        Ok((norm, src))
                    }
                    None => Err(format!("{norm} is not in the latest scan")),
                }
            }
            SkinPick::Project(rel) => {
                let root = self
                    .nif_export_root
                    .as_ref()
                    .ok_or("No Textures export folder is set")?;
                let src = FileSource::Loose(root.join(rel.replace('/', "\\")));
                Ok((format!("{PROJECT_KEY}{rel}"), src))
            }
        }
    }

    /// The first Auto guess of `map` the latest scan has (the texture the preview uses), for the card's hover.
    pub fn skin_auto(
        &self,
        normal: &str,
        specular: &str,
        shader_type: u32,
        map: SkinMap,
    ) -> Option<String> {
        let guesses = match map {
            SkinMap::Rfaos => render3d::textures::rfaos_candidates(shader_type, normal, specular),
            SkinMap::Wetness => render3d::textures::wet_candidates(shader_type, normal, specular),
        };
        let store = self.store.as_ref()?;
        guesses
            .into_iter()
            .find(|g| matches!(store.texture_location(g), Ok(Some(_))))
    }

    /// Every DDS under the Textures export folder, `/`-separated paths under it, sorted (empty without a folder).
    pub fn project_textures(&self) -> Vec<String> {
        self.nif_export_root
            .as_deref()
            .map(dds_under)
            .unwrap_or_default()
    }
}

fn dds_under(root: &Path) -> Vec<String> {
    let mut out = vec![];
    let mut dirs = vec![root.to_path_buf()];
    while let Some(d) = dirs.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let (path, Ok(ty)) = (e.path(), e.file_type()) else {
                continue;
            };
            if ty.is_dir() {
                dirs.push(path);
            } else if path
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("dds"))
            {
                if let Ok(rel) = path.strip_prefix(root) {
                    out.push(rel.to_string_lossy().replace('\\', "/"));
                }
                if out.len() == PROJECT_LIMIT {
                    dirs.clear();
                    break;
                }
            }
        }
    }
    out.sort();
    out
}
