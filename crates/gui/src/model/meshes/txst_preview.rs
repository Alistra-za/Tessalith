//! The shape card's Texture set dropdown (Note GO): the TXSTs that plugin records using this mesh assign to the
//! shape's 3D index, and on a Skin Tint shape the ARMA skin textures (Note GO4), previewed on that shape in the 3D
//! view. The game matches alternate textures by 3D index, so the record's stored shape name is shown, never used. Preview only: nothing is written, and the picks reset when
//! the shapes reload (another mesh, the In game / Source toggle, Edit opening or closing).
use super::{db_err, texture_sources, MeshesModel};
use crate::model::csm_export::{memory_source, SetCsm};
use crate::model::viewport3d::{Content, MeshSpec};
use modscan::store::ShapeView;
use render3d::ShapeId;
use std::collections::BTreeMap;
use std::sync::Arc;

/// One TXST plugin records assign to a shape.
#[derive(Debug, Clone, PartialEq)]
pub struct TxstChoice {
    pub formid: u32,
    pub edid: Option<String>,
    /// Who assigns it: (index into `plugin_uses`, the shape name that record stores; None: the ARMA's skin texture).
    pub users: Vec<(usize, Option<String>)>,
}

impl MeshesModel {
    /// The TXSTs the scanned plugin records using this mesh assign to shape `shape`, each once, in `plugin_uses`
    /// order: alternate textures for its 3D index, and ARMA skin textures when it is a Skin Tint (5) shape. An
    /// unresolved TXST (its master not loaded) is left out: Plugin overrides lists it.
    pub fn txst_choices(&self, shape: &ShapeView) -> Vec<TxstChoice> {
        let skin_shape = shape.shader_kind == 1 && shape.shader_type == Some(5);
        let mut out: Vec<TxstChoice> = vec![];
        for (i, u) in self.plugin_uses.iter().enumerate() {
            for s in &u.swaps {
                let hit = match s.is_skin() {
                    true => skin_shape,
                    false => {
                        shape.index3d.is_some() && u32::try_from(s.index).ok() == shape.index3d
                    }
                };
                if s.txst_formid == 0 || !hit {
                    continue;
                }
                let user = (i, (!s.is_skin()).then(|| s.shape_name.clone()));
                match out.iter_mut().find(|c| c.formid == s.txst_formid) {
                    Some(c) => c.users.push(user),
                    None => out.push(TxstChoice {
                        formid: s.txst_formid,
                        edid: s.txst_edid.clone(),
                        users: vec![user],
                    }),
                }
            }
        }
        out
    }

    /// Shape `block`'s texture set pick: the TXST FormID and its 9 slot paths ("" = empty).
    pub fn txst_pick(&self, block: u32) -> Option<(u32, &[String])> {
        let f = *self.txst_picks.get(&block)?;
        match self.v3.content() {
            Some(Content::Mesh(spec)) => Some((f, spec.txst.get(&ShapeId(block))?.as_slice())),
            _ => None,
        }
    }

    /// Previews TXST `formid` on shape `block` (None: the NIF's own textures again). Err, with nothing changed,
    /// in Edit (the 3D view shows the edit session) or when the TXST is not in the latest scan.
    pub fn set_txst(&mut self, block: u32, formid: Option<u32>) -> Result<(), String> {
        let Some(Content::Mesh(spec)) = self.v3.content() else {
            return Err("Close Edit to preview texture sets".into());
        };
        let mut spec = (**spec).clone();
        let id = ShapeId(block);
        match formid {
            Some(f) => {
                let store = self.store.as_ref().ok_or("No scan")?;
                let slots = store
                    .txst_textures(f)
                    .map_err(db_err)?
                    .ok_or_else(|| format!("TXST {f:08X} is not in the latest scan"))?;
                let ty = self
                    .shapes
                    .iter()
                    .find(|s| s.block == block)
                    .and_then(|s| s.shader_type)
                    .unwrap_or(0);
                for (k, v) in texture_sources(store, &[(ty, slots.clone())]).map_err(db_err)? {
                    spec.textures.entry(k).or_insert(v);
                }
                spec.txst.insert(id, slots);
                self.txst_picks.insert(block, f);
            }
            None => {
                spec.txst.remove(&id);
                self.txst_picks.remove(&block);
            }
        }
        self.apply_set_csms(&mut spec);
        // The same model: the camera and the skin card's picks stay.
        self.v3
            .set_content(Some(Content::Mesh(Arc::new(spec))), true);
        Ok(())
    }

    /// Note IE12: the EDIDs of the TXSTs the plugin records using this mesh assign (any shape), each once: what the
    /// app looks up among the project's `cs_txst` blocks, so a pick finds its set's `.csm` at once.
    pub fn txst_edids(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .plugin_uses
            .iter()
            .flat_map(|u| &u.swaps)
            .filter(|s| s.txst_formid != 0)
            .filter_map(|s| s.txst_edid.clone())
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// Note IE12: the app's hand-over (`csm_export::txst_csms`), by TXST EDID. A change reaches the 3D view when a
    /// pick uses it: the spec gets the new `.csm` text, a new source id, so the view reloads (render3d's per-load
    /// cache starts empty).
    pub fn set_txst_csms(&mut self, csms: BTreeMap<String, SetCsm>) {
        if self.txst_csms == csms {
            return;
        }
        self.txst_csms = csms;
        if self.txst_picks.is_empty() {
            return;
        }
        let Some(Content::Mesh(spec)) = self.v3.content() else {
            return;
        };
        let mut spec = (**spec).clone();
        self.apply_set_csms(&mut spec);
        self.v3
            .set_content(Some(Content::Mesh(Arc::new(spec))), true);
    }

    /// The design's TXST-JSON rule (Note IE12): each picked TXST whose EDID a project set binds draws that set's
    /// in-memory `.csm` in place of the shape's name field; a blocked one draws the shape vanilla, with a row.
    fn apply_set_csms(&self, spec: &mut MeshSpec) {
        spec.csm.clear();
        spec.materials.clear();
        spec.material_notes.clear();
        for (&block, &f) in &self.txst_picks {
            let Some(c) = self.txst_edid(f).and_then(|e| self.txst_csms.get(e)) else {
                continue;
            };
            let id = ShapeId(block);
            match (&c.blocked, memory_source(c)) {
                (None, Some((key, src))) => {
                    spec.csm.insert(id, Some(key.clone()));
                    spec.materials.insert(key, src);
                }
                (blocked, _) => {
                    spec.csm.insert(id, None);
                    let why = blocked
                        .clone()
                        .unwrap_or_else(|| format!("{} is not a .csm under materials", c.path));
                    let row = format!("{why}; drawn vanilla");
                    if !spec.material_notes.contains(&row) {
                        spec.material_notes.push(row);
                    }
                }
            }
        }
    }

    /// The EDID the plugin records give TXST `formid`.
    fn txst_edid(&self, formid: u32) -> Option<&str> {
        self.plugin_uses
            .iter()
            .flat_map(|u| &u.swaps)
            .find(|s| s.txst_formid == formid)
            .and_then(|s| s.txst_edid.as_deref())
    }
}
