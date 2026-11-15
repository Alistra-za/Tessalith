//! NIF bytes → owned per-shape data (03 §1.1): pure, `Send`, no GPU, so a worker can parse while the render
//! thread keeps drawing. A shape's `world` is the root..parent NiNode chain x its own transform (`Mesh.transform`, local); a block
//! with no parent entry keeps its own transform.
use nif::material::MaterialState;
use std::sync::Arc;

/// Stable for the life of a load: the NIF block index of the shape (`nif::scan::ShapeRow.block`). Built-in
/// shapes use `ShapeId(0)` (block 0 is the root node, never a shape). Storage order is not draw order.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ShapeId(pub u32);

/// One drawable shape: a BSLightingShaderProperty shape whose geometry decoded and passed `mesh::validate`.
#[derive(Debug, Clone)]
pub struct ParsedShape {
    pub id: ShapeId,
    /// The NIF name, or `"block N"` when it has none (P0's label). Shared with the NIF string table (Note GA1).
    pub name: Arc<str>,
    pub mesh: Arc<nif::geom::Mesh>,
    /// Root … parent chain × `mesh.transform` (3×4 row-major, the `Mesh.transform` convention). The draw's model
    /// matrix and the bounds use it. Skinned shapes carry it too, but their VS ignores World.
    pub world: [[f32; 4]; 3],
    pub material: MaterialState,
}

/// A whole NIF, parsed. `skipped` lists every scanned shape that is not in `shapes`, with its reason; the same
/// text is also a `warnings` line (not a Lighting shape) or an `errors` line (geometry), exactly as P0's
/// `LoadReport` printed them, so a load can copy `warnings` / `errors` unchanged.
#[derive(Debug, Clone, Default)]
pub struct ParsedNif {
    /// BSVER 83: nothing is parsed further (LE meshes are not rendered, 00 §6.5).
    pub le_mesh: bool,
    pub shapes: Vec<ParsedShape>,
    pub skipped: Vec<(ShapeId, String)>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    /// The NIF is a head part (Note BA: a facegendata NIF or an HDPT model): every shape is CS "head". `parse`
    /// cannot know it (it needs the NIF path and the plugins); the caller sets it before loading.
    pub head_part: bool,
}

/// Scans the NIF and decodes the geometry of every BSLightingShaderProperty shape, in block order. The same for both
/// shader trees (Note ID38): a type-21 shape is kept as read, never hidden, with its first CS 2.0 Error
/// (`edit::validate`'s type-21 rules) in `material.pbr2_error`; the renderer decides per tree what that means
/// (`material::skipped`, `Renderer::tree_notes`). When the NIF does not read as a model the rules are not run: the
/// shape draws, with a "CS 2.0 rules not checked" warning.
pub fn parse(nif_bytes: &[u8]) -> ParsedNif {
    let mut out = ParsedNif::default();
    let scan = match nif::scan::scan(nif_bytes) {
        Ok(s) => s,
        Err(e) => {
            out.errors.push(format!("NIF: {e}"));
            return out;
        }
    };
    out.warnings
        .extend(scan.problems.iter().map(|p| format!("NIF: {p}")));
    if scan.le_mesh {
        out.le_mesh = true;
        return out;
    }
    let header = match nif::header::Header::parse(nif_bytes) {
        Ok(h) => h,
        Err(e) => {
            out.errors.push(format!("NIF header: {e}"));
            return out;
        }
    };
    let graph = nif::graph::read_graph(&header, nif_bytes);
    out.warnings
        .extend(graph.problems.iter().map(|p| format!("NIF: {p}")));
    let mut model: Option<Result<nif::model::NifModel, String>> = None;
    for row in &scan.shapes {
        let id = ShapeId(row.block);
        let label: Arc<str> = row
            .head
            .name
            .clone()
            .unwrap_or_else(|| format!("block {}", row.block).into());
        // messages are kept per shape: a short label, not the whole name (Note GA7)
        let short = nif::text::label(&label);
        let nif::scan::Shader::Lighting(lighting, slots) = &row.shader else {
            let why = format!("{short}: not a BSLightingShaderProperty shape, not drawn in P0");
            out.warnings.push(why.clone());
            out.skipped.push((id, why));
            continue;
        };
        let mesh = match nif::geom::read_shape(&header, nif_bytes, row.block as usize) {
            Ok(m) => m,
            Err(e) => {
                let why = format!("{short}: {e}");
                out.errors.push(why.clone());
                out.skipped.push((id, why));
                continue;
            }
        };
        if let Err(e) = crate::mesh::validate(&mesh) {
            let why = format!("{short}: {e}");
            out.errors.push(why.clone());
            out.skipped.push((id, why));
            continue;
        }
        if row.block != 0 && !graph.parent.contains_key(&row.block) {
            out.warnings.push(format!(
                "{short}: not under any known node; placed by its own transform"
            ));
        }
        let world = nif::graph::world(&graph, row.block, &mesh.transform);
        let mut material = MaterialState::from_lighting(&row.head, lighting, slots, row.alpha);
        if lighting.shader_type == 21 {
            // read once, only for a NIF with a type-21 shape
            let m = model.get_or_insert_with(|| {
                nif::model::read_model(nif_bytes).map_err(|e| e.to_string())
            });
            match first_cs2_error(m.as_ref(), row.head.shader) {
                Ok(e) => material.pbr2_error = e,
                Err(e) => out.warnings.push(format!("{short}: {e}")),
            }
        }
        out.shapes.push(ParsedShape {
            id,
            name: label,
            world,
            mesh: Arc::new(mesh),
            material,
        });
    }
    out
}

/// The first CS 2.0 Error of the type-21 shader `shader` (Note ID38): `Ok(None)` when it breaks no rule, `Err` (the
/// warning text) when the rules could not run because the NIF did not read as a model.
fn first_cs2_error(
    model: Result<&nif::model::NifModel, &String>,
    shader: i32,
) -> Result<Option<String>, String> {
    let m = model.map_err(|e| format!("CS 2.0 rules not checked: {e}"))?;
    let Ok(shader) = u32::try_from(shader) else {
        return Ok(None);
    };
    Ok(nif::edit::validate::type_21_warnings(m, shader)
        .into_iter()
        .find(|w| w.level == nif::edit::validate::Level::Error)
        .map(|w| w.text))
}

#[cfg(test)]
mod tests {
    /// Note ID38: a NIF that scans but does not read as a model draws its type-21 shape unchecked, with a warning.
    #[test]
    fn an_unread_model_is_a_warning_not_a_verdict() {
        let e = "block 3: truncated".to_string();
        assert_eq!(
            super::first_cs2_error(Err(&e), 3),
            Err("CS 2.0 rules not checked: block 3: truncated".to_string())
        );
    }
}
