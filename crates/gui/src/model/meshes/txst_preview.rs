//! The shape card's Texture set dropdown (Note GO): the TXSTs that plugin records using this mesh assign to the
//! shape's 3D index, and on a Skin Tint shape the ARMA skin textures (Note GO4), previewed on that shape in the 3D
//! view. The game matches alternate textures by 3D index, so the record's stored shape name is shown, never used. Preview only: nothing is written, and the picks reset when
//! the shapes reload (another mesh, the In game / Source toggle, Edit opening or closing).
use super::{db_err, texture_sources, MeshesModel};
use crate::model::viewport3d::Content;
use modscan::store::ShapeView;
use render3d::ShapeId;
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
        // The same model: the camera and the skin card's picks stay.
        self.v3
            .set_content(Some(Content::Mesh(Arc::new(spec))), true);
        Ok(())
    }
}
