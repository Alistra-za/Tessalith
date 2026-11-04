//! Shape heads up to the shader/alpha refs (plan 01 Q3; nif.xml 3840-3870, 8228-8246).
use crate::cursor::Cursor;
use crate::header::Header;
use crate::NifError;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeKind {
    BSTriShape,
    BSSubIndexTriShape,
    BSDynamicTriShape,
    BSMeshLODTriShape,
    NiTriShape,
    NiTriStrips,
    BSLODTriShape,
    BSSegmentedTriShape,
    NiParticleSystem,
    BSStripParticleSystem,
}

impl ShapeKind {
    pub fn from_name(n: &[u8]) -> Option<ShapeKind> {
        use ShapeKind::*;
        Some(match n {
            b"BSTriShape" => BSTriShape,
            b"BSSubIndexTriShape" => BSSubIndexTriShape,
            b"BSDynamicTriShape" => BSDynamicTriShape,
            b"BSMeshLODTriShape" => BSMeshLODTriShape,
            b"NiTriShape" => NiTriShape,
            b"NiTriStrips" => NiTriStrips,
            b"BSLODTriShape" => BSLODTriShape,
            b"BSSegmentedTriShape" => BSSegmentedTriShape,
            b"NiParticleSystem" => NiParticleSystem,
            b"BSStripParticleSystem" => BSStripParticleSystem,
            _ => return None,
        })
    }
    fn bs_layout(self, bsver: u32) -> bool {
        use ShapeKind::*;
        match self {
            BSTriShape | BSSubIndexTriShape | BSDynamicTriShape | BSMeshLODTriShape => true,
            NiParticleSystem | BSStripParticleSystem => bsver >= 100,
            _ => false,
        }
    }
    fn has_vertex_head(self) -> bool {
        use ShapeKind::*;
        matches!(
            self,
            BSTriShape | BSSubIndexTriShape | BSDynamicTriShape | BSMeshLODTriShape
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShapeHead {
    pub kind: ShapeKind,
    /// Shared with every block that names the same string-table entry ([`Header::name_arc`]).
    pub name: Option<Arc<str>>,
    pub skinned: bool,
    pub shader: i32,
    pub alpha: i32,
    pub vertex_desc: Option<u64>,
    pub num_triangles: Option<u32>,
    pub num_vertices: Option<u32>,
}

pub fn decode_head(
    h: &Header,
    b: &[u8],
    block: usize,
    kind: ShapeKind,
) -> Result<ShapeHead, NifError> {
    decode_head_at(h, b, block, kind).map(|(s, _)| s)
}

/// Absolute file offsets of the shader and alpha refs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefAt {
    pub shader: usize,
    pub alpha: usize,
}

pub fn decode_head_at(
    h: &Header,
    b: &[u8],
    block: usize,
    kind: ShapeKind,
) -> Result<(ShapeHead, RefAt), NifError> {
    let start = h.offsets()[block];
    // Reads stay inside the block; offsets stay absolute, so a read past its end is a `BlockSize` error.
    let end = start + h.sizes[block] as usize;
    let b = b.get(..end).ok_or_else(|| NifError::Truncated {
        at: b.len(),
        need: end - b.len(),
    })?;
    head_in(h, b, kind, start).map_err(|e| match e {
        NifError::Truncated { at, need } => NifError::BlockSize {
            block: block as u32,
            kind: crate::text::lossy(h.kind(block)),
            declared: h.sizes[block],
            consumed: u32::try_from(at.saturating_add(need).saturating_sub(start))
                .unwrap_or(u32::MAX),
        },
        e => e,
    })
}

fn head_in(
    h: &Header,
    b: &[u8],
    kind: ShapeKind,
    start: usize,
) -> Result<(ShapeHead, RefAt), NifError> {
    let mut c = Cursor::new(b, start);
    let name = h.name_arc(c.i32()?).filter(|s| !s.is_empty());
    let n = c.u32()? as usize;
    c.skip(4 * n)?;
    c.i32()?; // controller
    c.u32()?; // NiAVObject flags (u32 at BSVER > 26)
    c.skip(12 + 36 + 4)?; // translation, rotation, scale
    c.i32()?; // collision
    let (skinned, shader, alpha);
    let mut at = RefAt {
        shader: 0,
        alpha: 0,
    };
    let (mut vertex_desc, mut num_triangles, mut num_vertices) = (None, None, None);
    if kind.bs_layout(h.bsver) {
        c.skip(16)?; // bounding sphere
        skinned = c.i32()? != -1;
        at.shader = c.pos();
        shader = c.i32()?;
        at.alpha = c.pos();
        alpha = c.i32()?;
        if kind.has_vertex_head() {
            vertex_desc = Some(c.u64()?);
            num_triangles = Some(c.u16()? as u32); // u16 at BSVER 100 (u32 only from 130)
            num_vertices = Some(c.u16()? as u32);
        }
    } else {
        c.i32()?; // data
        skinned = c.i32()? != -1;
        let m = c.u32()? as usize;
        c.skip(8 * m)?; // material names + extra data
        c.i32()?; // active material
        c.u8()?; // needs update
        at.shader = c.pos();
        shader = c.i32()?;
        at.alpha = c.pos();
        alpha = c.i32()?;
    }
    Ok((
        ShapeHead {
            kind,
            name,
            skinned,
            shader,
            alpha,
            vertex_desc,
            num_triangles,
            num_vertices,
        },
        at,
    ))
}
