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

/// [`parse_in`] for tree A.
pub fn parse(nif_bytes: &[u8]) -> ParsedNif {
    parse_in(nif_bytes, crate::Tree::A)
}

/// The warning of a type-21 shape on tree A, which draws its `to_v1` stand-in (Note ID33).
pub const APPROXIMATE: &str = "Approximate: tick CS PBR 2.0 to preview CS 2.0 shading";

/// Scans the NIF and decodes the geometry of every BSLightingShaderProperty shape, in block order. A type-21 shape is
/// always kept as read (Note ID33): on tree A with the [`APPROXIMATE`] warning (`material::drawn` draws its v1
/// stand-in); on tree B, when it breaks a CS 2.0 rule, hidden with its first Error as the warning, as CS does not draw it.
/// shortcut: `hidden` is decided here, so a renderer switched to the other tree keeps this tree's verdict; parse again
/// after a tree switch to upgrade.
pub fn parse_in(nif_bytes: &[u8], tree: crate::Tree) -> ParsedNif {
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
    let mut model: Option<Option<nif::model::NifModel>> = None;
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
            match tree {
                crate::Tree::A => out.warnings.push(format!("{short}: {APPROXIMATE}")),
                crate::Tree::B => {
                    // read once, only for a type-21 shape on tree B
                    let m = model.get_or_insert_with(|| nif::model::read_model(nif_bytes).ok());
                    let first = m.as_ref().and_then(|m| {
                        let shader = u32::try_from(row.head.shader).ok()?;
                        nif::edit::validate::type_21_warnings(m, shader)
                            .into_iter()
                            .find(|w| w.level == nif::edit::validate::Level::Error)
                    });
                    if let Some(w) = first {
                        material.hidden = true;
                        out.warnings.push(format!("{short}: {}", w.text));
                    }
                }
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
