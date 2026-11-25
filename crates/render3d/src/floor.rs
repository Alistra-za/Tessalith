//! The Gradient background's floor (Note IH4): a horizontal quad at the model's lowest point that catches the sun's and
//! the point light's shadows and fades into the backdrop at its rim. Repo-owned shader (not CS's): the colour is the
//! floor colour times the light the sun, the ambient from above and the point light bring, with the shadow maps read
//! directly through [`crate::shadow`]'s lookups (the same PCF as the mask).
//!
//! The quad is background: drawn after the light passes and before the opaque shapes, blended over the gradient,
//! testing the pre-pass's depth and writing none, so every model pixel is drawn over it and CS's deferred composite
//! passes its pixels through as it does the gradient's.
use crate::background::{compile_source, gradient_floor_linear};
use crate::device::Gpu;
use crate::scene::{dot, sub, Scene, View};
use crate::shadow::{lookup_source, ShadowMaps, LOOKUP_FLOATS};
use windows::Win32::Graphics::Direct3D::{
    D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST, D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
};
use windows::Win32::Graphics::Direct3D11::{
    ID3D11BlendState, ID3D11Buffer, ID3D11DepthStencilState, ID3D11PixelShader,
    ID3D11RasterizerState, ID3D11VertexShader, D3D11_BIND_CONSTANT_BUFFER, D3D11_BLEND_DESC,
    D3D11_BLEND_INV_SRC_ALPHA, D3D11_BLEND_ONE, D3D11_BLEND_OP_ADD, D3D11_BLEND_SRC_ALPHA,
    D3D11_BUFFER_DESC, D3D11_COLOR_WRITE_ENABLE_ALL, D3D11_COMPARISON_LESS_EQUAL, D3D11_CULL_NONE,
    D3D11_DEPTH_STENCIL_DESC, D3D11_DEPTH_WRITE_MASK_ZERO, D3D11_FILL_SOLID, D3D11_RASTERIZER_DESC,
    D3D11_RENDER_TARGET_BLEND_DESC, D3D11_USAGE_DEFAULT,
};

/// The quad's half-size in bounding radii.
pub const HALF_RADII: f32 = 4.0;

const HEAD: &str = r#"
cbuffer Floor : register(b0)
{
    row_major float4x4 ViewProj;
    row_major float4x4 SunViewProj;
    float4 Point;     // the shadow lookups' fields, as ShadowMaps::lookup_floats lays them out
    float4 Params;
    float4 Bias;
    float4 Plane;     // xyz: the quad's centre (camera-relative); w: its half-size
    float4 Colour;    // rgb: the floor colour (linear); w: 1 when the readback does not sRGB-encode
    float4 SunDir;    // xyz: towards the sun; w: 1 when the shadow maps are bound
    float4 SunCol;    // rgb: the sun's colour
    float4 Ambient;   // rgb: the ambient from +Z
    float4 PointPos;  // xyz: the point light (camera-relative); w: its falloff radius
    float4 PointCol;  // rgb: its colour, 0 when off
};
"#;

const BODY: &str = r#"
struct VsOut { float4 pos : SV_Position; float3 p : TEXCOORD0; };

VsOut vs(uint id : SV_VertexID)
{
    float2 c = float2(id & 1, id >> 1) * 2.0 - 1.0;
    float3 w = Plane.xyz + float3(c * Plane.w, 0.0);
    VsOut o;
    o.pos = mul(ViewProj, float4(w, 1.0));
    o.p = w;
    return o;
}

float4 ps(VsOut i) : SV_Target
{
    float3 p = i.p;
    float sunLit = 1.0;
    float pointLit = 1.0;
    if (SunDir.w > 0.5)
    {
        sunLit = Sun(p);
        pointLit = PointShadow(p);
    }
    // The floor's normal is +Z: N.L of the sun is its z.
    float3 light = Ambient.rgb + SunCol.rgb * max(0.0, SunDir.z) * sunLit;
    float3 v = PointPos.xyz - p;
    float d = length(v);
    // CS's falloff: 1 - (d / radius)^2, gone at the radius (Lighting.hlsl, the point-light loop).
    float f = saturate(d / PointPos.w);
    light += PointCol.rgb * (1.0 - f * f) * max(0.0, v.z / max(d, 1e-4)) * pointLit;
    float3 c = Colour.rgb * light;
    if (Colour.w > 0.5)
        c = c <= 0.0031308 ? c * 12.92 : 1.055 * pow(c, 1.0 / 2.4) - 0.055;
    // Gone at the rim, so the floor meets the backdrop with no edge.
    float alpha = 1.0 - smoothstep(0.5, 1.0, length(p.xy - Plane.xy) / Plane.w);
    return float4(c, alpha);
}
"#;

/// The floor shader's whole source, as it is compiled (`tests/backend_baseline.rs` lists it).
pub fn source() -> String {
    lookup_source(HEAD, BODY)
}

/// The floor's place: world Z of the plane and the quad's centre and half-size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plane {
    /// World position of the quad's centre: under the model's bounds centre, at its lowest point.
    pub centre: [f32; 3],
    pub half: f32,
}

impl Plane {
    /// Under `bounds` (centre, radius), at `min_z`, [`HALF_RADII`] radii to the half-size.
    pub fn under(bounds: ([f32; 3], f32), min_z: f32) -> Plane {
        Plane {
            centre: [bounds.0[0], bounds.0[1], min_z],
            half: HALF_RADII * bounds.1.max(1e-3),
        }
    }

    /// The eye is above the plane: from below nothing is drawn.
    pub fn seen_from(&self, eye: [f32; 3]) -> bool {
        eye[2] > self.centre[2]
    }

    /// The bounding sphere (centre, radius) of the model's bounds and the quad's corners, which the sun's depth map
    /// fits so a slanted sun's shadow landing past the model still falls inside it.
    pub fn sun_bounds(&self, bounds: ([f32; 3], f32)) -> ([f32; 3], f32) {
        let corner = self.half * std::f32::consts::SQRT_2;
        let d = sub(bounds.0, self.centre);
        (self.centre, corner.max(dot(d, d).sqrt() + bounds.1))
    }
}

/// The compiled pass and its states.
pub struct Floor {
    vs: ID3D11VertexShader,
    ps: ID3D11PixelShader,
    cb: ID3D11Buffer,
    depth: ID3D11DepthStencilState,
    blend: ID3D11BlendState,
    raster: ID3D11RasterizerState,
}

impl Floor {
    pub fn new(gpu: &Gpu) -> Result<Floor, String> {
        let source = source();
        let vs_code = compile_source(&source, c"floor.hlsl", "vs", "vs_5_0")?;
        let ps_code = compile_source(&source, c"floor.hlsl", "ps", "ps_5_0")?;
        let bd = D3D11_BUFFER_DESC {
            ByteWidth: (FLOATS * 4) as u32,
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
            StructureByteStride: 0,
        };
        // Tests the pre-pass's depth (where the model is in front, nothing to draw) and writes none: the floor is
        // background, so every model pixel drawn after it wins, and a deferred frame's composite sees depth 1 there.
        let dd = D3D11_DEPTH_STENCIL_DESC {
            DepthEnable: true.into(),
            DepthWriteMask: D3D11_DEPTH_WRITE_MASK_ZERO,
            DepthFunc: D3D11_COMPARISON_LESS_EQUAL,
            ..Default::default()
        };
        let mut blend = D3D11_BLEND_DESC::default();
        blend.RenderTarget[0] = D3D11_RENDER_TARGET_BLEND_DESC {
            BlendEnable: true.into(),
            SrcBlend: D3D11_BLEND_SRC_ALPHA,
            DestBlend: D3D11_BLEND_INV_SRC_ALPHA,
            BlendOp: D3D11_BLEND_OP_ADD,
            SrcBlendAlpha: D3D11_BLEND_ONE,
            DestBlendAlpha: D3D11_BLEND_INV_SRC_ALPHA,
            BlendOpAlpha: D3D11_BLEND_OP_ADD,
            RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
        };
        // No depth clip: the quad reaches past the camera's far plane (4 radii out), where its depth clamps to 1; a
        // clipped rim would be an edge.
        let rd = D3D11_RASTERIZER_DESC {
            FillMode: D3D11_FILL_SOLID,
            CullMode: D3D11_CULL_NONE,
            DepthClipEnable: false.into(),
            ..Default::default()
        };
        let (mut vs, mut ps, mut cb, mut ds, mut bs, mut rs) = (None, None, None, None, None, None);
        // SAFETY: descriptors and bytecode outlive the calls; out-pointers are locals.
        unsafe {
            gpu.device
                .CreateVertexShader(&vs_code, None, Some(&mut vs))
                .map_err(|e| gpu.fail("CreateVertexShader(floor)", e))?;
            gpu.device
                .CreatePixelShader(&ps_code, None, Some(&mut ps))
                .map_err(|e| gpu.fail("CreatePixelShader(floor)", e))?;
            gpu.device
                .CreateBuffer(&bd, None, Some(&mut cb))
                .map_err(|e| gpu.fail("CreateBuffer(floor)", e))?;
            gpu.device
                .CreateDepthStencilState(&dd, Some(&mut ds))
                .map_err(|e| gpu.fail("CreateDepthStencilState(floor)", e))?;
            gpu.device
                .CreateBlendState(&blend, Some(&mut bs))
                .map_err(|e| gpu.fail("CreateBlendState(floor)", e))?;
            gpu.device
                .CreateRasterizerState(&rd, Some(&mut rs))
                .map_err(|e| gpu.fail("CreateRasterizerState(floor)", e))?;
        }
        Ok(Floor {
            vs: vs.ok_or("no floor vertex shader")?,
            ps: ps.ok_or("no floor pixel shader")?,
            cb: cb.ok_or("no floor buffer")?,
            depth: ds.ok_or("no floor depth state")?,
            blend: bs.ok_or("no floor blend state")?,
            raster: rs.ok_or("no floor rasteriser state")?,
        })
    }

    /// Draws the quad into the bound target (viewport, render target RT0 alone and depth already set), lit by
    /// `scene`'s lights and, with `maps`, shadowed by them. `encode`: the readback does not sRGB-encode. Restores
    /// the default blend and depth states and the triangle-list topology and unbinds the shadow registers; its
    /// rasteriser stays bound (every shape binds its own).
    pub fn draw(
        &self,
        gpu: &Gpu,
        plane: &Plane,
        scene: &Scene,
        view: &View,
        maps: Option<&ShadowMaps>,
        encode: bool,
    ) {
        let rel = sub(plane.centre, view.eye);
        let sun = scene.dir_light.direction;
        let sun_dir = {
            let l = dot(sun, sun).sqrt();
            if l > 0.0 {
                sun.map(|c| c / l)
            } else {
                [0.0, 0.0, 1.0]
            }
        };
        let (sc, c) = (scene.sun_color(), gradient_floor_linear());
        let ambient = scene.ambient()[4];
        let p = scene.point_light;
        let point_pos = sub(p.position, view.eye);
        let point_col = [p.brightness.max(0.0); 3];
        let own: [f32; OWN_FLOATS] = [
            rel[0],
            rel[1],
            rel[2],
            plane.half,
            c[0],
            c[1],
            c[2],
            if encode { 1.0 } else { 0.0 },
            sun_dir[0],
            sun_dir[1],
            sun_dir[2],
            if maps.is_some() { 1.0 } else { 0.0 },
            sc[0],
            sc[1],
            sc[2],
            0.0,
            ambient[0],
            ambient[1],
            ambient[2],
            0.0,
            point_pos[0],
            point_pos[1],
            point_pos[2],
            p.radius.max(1e-3),
            point_col[0],
            point_col[1],
            point_col[2],
            0.0,
        ];
        let mut floats = [0.0f32; FLOATS];
        for (d, s) in floats.iter_mut().zip(view.view_proj.iter().flatten()) {
            *d = *s;
        }
        floats[16..16 + LOOKUP_FLOATS]
            .copy_from_slice(&maps.map_or([0.0; LOOKUP_FLOATS], ShadowMaps::lookup_floats));
        floats[16 + LOOKUP_FLOATS..].copy_from_slice(&own);
        // SAFETY: every object belongs to `gpu.device`; `floats` holds the buffer's `FLOATS` × 4 bytes (little-endian
        // `f32`, as the GPU reads them).
        unsafe {
            let ctx = &gpu.ctx;
            ctx.UpdateSubresource(&self.cb, 0, None, floats.as_ptr() as _, 0, 0);
            ctx.OMSetDepthStencilState(&self.depth, 0);
            ctx.OMSetBlendState(&self.blend, None, 0xffff_ffff);
            ctx.RSSetState(&self.raster);
            ctx.IASetInputLayout(None);
            ctx.IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP);
            ctx.VSSetShader(&self.vs, None);
            ctx.PSSetShader(&self.ps, None);
            ctx.VSSetConstantBuffers(0, Some(&[Some(self.cb.clone())]));
            ctx.PSSetConstantBuffers(0, Some(&[Some(self.cb.clone())]));
            if let Some(m) = maps {
                m.bind(gpu);
            }
            ctx.Draw(4, 0);
            if let Some(m) = maps {
                m.unbind(gpu);
            }
            ctx.IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            ctx.OMSetBlendState(None, None, 0xffff_ffff);
            ctx.OMSetDepthStencilState(None, 0);
        }
    }
}

/// The floor's own fields: the seven float4s from `Plane` to `PointCol`.
const OWN_FLOATS: usize = 7 * 4;
/// The cbuffer: the view-projection, the lookups' fields, then the floor's own.
const FLOATS: usize = 16 + LOOKUP_FLOATS + OWN_FLOATS;

#[cfg(test)]
mod tests {
    use super::*;

    /// The quad sits under the bounds centre at the lowest point, 4 radii to the half-size; seen only from above.
    #[test]
    fn the_plane_is_under_the_model_and_seen_from_above() {
        let p = Plane::under(([1.0, 2.0, 3.0], 10.0), -4.0);
        assert_eq!(p.centre, [1.0, 2.0, -4.0]);
        assert_eq!(p.half, 40.0);
        assert!(p.seen_from([0.0, 0.0, -3.9]));
        assert!(!p.seen_from([0.0, 0.0, -4.0]));
        assert!(!p.seen_from([0.0, 0.0, -9.0]));
    }

    /// The sun's fit holds the model's sphere and every corner of the quad.
    #[test]
    fn the_sun_fit_holds_the_model_and_the_quad() {
        let bounds = ([1.0, 2.0, 3.0], 10.0);
        let p = Plane::under(bounds, -7.0);
        let (c, r) = p.sun_bounds(bounds);
        let dist = |a: [f32; 3], b: [f32; 3]| {
            let d = sub(a, b);
            dot(d, d).sqrt()
        };
        for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
            let corner = [
                p.centre[0] + sx * p.half,
                p.centre[1] + sy * p.half,
                p.centre[2],
            ];
            assert!(dist(c, corner) <= r * 1.0001);
        }
        assert!(dist(c, bounds.0) + bounds.1 <= r * 1.0001);
    }
}
