//! Shadows from the sun and the point light (Note IH3), through Community Shaders' own route: Lighting.hlsl reads a
//! screen-space shadow mask at t14 (`shadowColor`, Lighting.hlsl:1714-1719), the sun takes its R
//! (`dirDetailedShadow`, 2280-2281) and point light `i` its `ShadowLightMaskSelect[i]` channel (2401-2403).
//!
//! The renderer draws the casters (the depth pre-pass's shapes: not blended, writing depth, alpha test through their
//! own programs) into a sun depth map, orthographic and fitted to the model's bounding sphere (with the Gradient floor's quad, Note IH4), and, while the point
//! light is on, into a depth cube around it. This module's pass then turns the camera's depth into the mask: each
//! pixel back to its camera-relative position (inverse view-projection), into each light's map, 3×3 PCF. Every
//! [`View`] here is camera-relative like the camera's: `eye` is the camera's, so `World`, the point light and the
//! skinning pivot stay what the colour passes write. Repo-owned shader (not CS's), compiled with `D3DCompile`.
use crate::background::compile_source;
use crate::device::{create_tex, r32_srv, tex2d_desc, Gpu};
use crate::scene::{cross, dot, inverse4, mul4, norm, sub, Mat4, View};
use windows::Win32::Graphics::Direct3D::D3D11_SRV_DIMENSION_TEXTURECUBE;
use windows::Win32::Graphics::Direct3D11::{
    ID3D11Buffer, ID3D11DepthStencilState, ID3D11DepthStencilView, ID3D11PixelShader,
    ID3D11RasterizerState, ID3D11SamplerState, ID3D11ShaderResourceView, ID3D11Texture2D,
    ID3D11VertexShader, D3D11_BIND_CONSTANT_BUFFER, D3D11_BIND_DEPTH_STENCIL,
    D3D11_BIND_SHADER_RESOURCE, D3D11_BUFFER_DESC, D3D11_COMPARISON_LESS_EQUAL, D3D11_CULL_NONE,
    D3D11_DEPTH_STENCIL_DESC, D3D11_DEPTH_STENCIL_VIEW_DESC, D3D11_DEPTH_STENCIL_VIEW_DESC_0,
    D3D11_DSV_DIMENSION_TEXTURE2D, D3D11_DSV_DIMENSION_TEXTURE2DARRAY, D3D11_FILL_SOLID,
    D3D11_FILTER_COMPARISON_MIN_MAG_LINEAR_MIP_POINT, D3D11_FLOAT32_MAX, D3D11_RASTERIZER_DESC,
    D3D11_RESOURCE_MISC_TEXTURECUBE, D3D11_SAMPLER_DESC, D3D11_SHADER_RESOURCE_VIEW_DESC,
    D3D11_SHADER_RESOURCE_VIEW_DESC_0, D3D11_TEX2D_ARRAY_DSV, D3D11_TEXCUBE_SRV,
    D3D11_TEXTURE_ADDRESS_CLAMP, D3D11_USAGE_DEFAULT,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_D32_FLOAT, DXGI_FORMAT_R32_FLOAT, DXGI_FORMAT_R32_TYPELESS,
};

/// Side of the sun's depth map (D32).
pub const SUN_SIZE: u32 = 2048;
/// Side of each face of the point light's depth cube (D32).
pub const POINT_SIZE: u32 = 1024;
/// The cube's near plane as a fraction of its far plane (the light's falloff radius).
pub const POINT_NEAR: f32 = 0.01;
/// The sun map's square covers the bounding sphere's diameter plus this margin, so vertices on the sphere are kept.
pub const SUN_MARGIN: f32 = 1.02;
/// Depth bias of the mask's comparisons, in texels of the light's map at the receiver.
pub const BIAS_TEXELS: f32 = 1.5;
/// The light passes' rasteriser bias: `SlopeScaledDepthBias` and its `DepthBiasClamp` (depth units). 3 slopes cover
/// the 3×3 bilinear PCF's reach of two texels; at 2.0 a lit curved surface darkened in places (`tests/shadow.rs`
/// `a_lone_builtin_sphere_does_not_shadow_itself`). The clamp caps the push at 1 % of the map's depth range.
pub const SLOPE_BIAS: f32 = 3.0;
pub const SLOPE_BIAS_CLAMP: f32 = 0.01;
/// CS's `TexShadowMaskSampler` register (Lighting.hlsl:504).
pub const MASK_REGISTER: u32 = 14;

/// Where a pass that reads the maps finds them: the sun map, the point cube and the comparison sampler. The mask pass
/// and the floor share these (and [`LOOKUP`]), so a register moves in one place.
pub const SUN_REGISTER: u32 = 1;
pub const POINT_REGISTER: u32 = 2;
pub const SAMPLER_REGISTER: u32 = 0;
const _: () = assert!(POINT_REGISTER == SUN_REGISTER + 1);

/// The lookups every reader of the maps shares, as HLSL: the map declarations and `Sun` / `PointShadow` (1 = lit,
/// 3×3 PCF). The caller's `cbuffer` comes first and declares `SunViewProj`, `Point`, `Params` and `Bias` as
/// [`ShadowMaps::lookup_floats`] lays them out; positions are camera-relative.
const LOOKUP: &str = r#"
Texture2D<float> SunMap : register(SUN_T);
TextureCube<float> PointMap : register(POINT_T);
SamplerComparisonState Cmp : register(CMP_S);

float Sun(float3 p)
{
    float4 s = mul(SunViewProj, float4(p, 1.0));
    float2 uv = s.xy * float2(0.5, -0.5) + 0.5;
    if (any(uv < 0.0) || any(uv > 1.0) || s.z > 1.0)
        return 1.0;
    // One texel of the orthographic map is 1 / size in depth too (the box is as deep as it is wide).
    float z = s.z - Bias.x * Params.z;
    float lit = 0.0;
    [unroll] for (int y = -1; y <= 1; y++)
        [unroll] for (int x = -1; x <= 1; x++)
            lit += SunMap.SampleCmpLevelZero(Cmp, uv + float2(x, y) * Params.z, z);
    return lit / 9.0;
}

float PointShadow(float3 p)
{
    float3 v = p - Point.xyz;
    float3 a = abs(v);
    float ma = max(a.x, max(a.y, a.z));
    if (Point.w < 0.5 || ma >= Params.y)
        return 1.0;
    float n = Params.x;
    float f = Params.y;
    // A texel of a face at distance ma spans 2 ma / size; the compared depth is the face's own projected depth.
    float d = max(ma * (1.0 - 2.0 * Bias.x * Params.w), n);
    float z = f / (f - n) * (1.0 - n / d);
    float3 t1 = float3(1.0, 0.0, 0.0);
    float3 t2 = float3(0.0, 1.0, 0.0);
    if (a.x >= a.y && a.x >= a.z)
        t1 = float3(0.0, 0.0, 1.0);
    else if (a.y >= a.z)
        t2 = float3(0.0, 0.0, 1.0);
    float step = 2.0 * ma * Params.w;
    float lit = 0.0;
    [unroll] for (int y = -1; y <= 1; y++)
        [unroll] for (int x = -1; x <= 1; x++)
            lit += PointMap.SampleCmpLevelZero(Cmp, v + (x * t1 + y * t2) * step, z);
    return lit / 9.0;
}
"#;

/// A shader that reads the maps: `head` (its `cbuffer`, which `LOOKUP` needs) then the lookups then `body`.
pub(crate) fn lookup_source(head: &str, body: &str) -> String {
    format!(
        "#define SUN_T t{SUN_REGISTER}\n#define POINT_T t{POINT_REGISTER}\n#define CMP_S s{SAMPLER_REGISTER}\n{head}{LOOKUP}{body}"
    )
}

/// The mask pass. `Depth` is the camera depth copy; positions are camera-relative. R = sun, G = point light 0, B = A
/// = 1; a pixel with no geometry (depth 1) is 1 everywhere.
const MASK_HEAD: &str = r#"
cbuffer Mask : register(b0)
{
    row_major float4x4 InvViewProj;
    row_major float4x4 SunViewProj;
    float4 Point;   // xyz: the light (camera-relative); w: 1 when the cube was drawn
    float4 Params;  // x, y: the cube's near and far; z: 1 / sun size; w: 1 / cube size
    float4 Bias;    // x: texels of depth bias
};
Texture2D<float> Depth : register(t0);
"#;

/// The mask shader's whole source, as it is compiled (`tests/backend_baseline.rs` lists it).
pub fn mask_source() -> String {
    lookup_source(MASK_HEAD, MASK_BODY)
}

const MASK_BODY: &str = r#"
struct VsOut { float4 pos : SV_Position; float2 ndc : TEXCOORD0; };

VsOut vs(uint id : SV_VertexID)
{
    float2 p = float2((id << 1) & 2, id & 2) * 2.0 - 1.0;
    VsOut o;
    o.pos = float4(p, 0.0, 1.0);
    o.ndc = p;
    return o;
}

float4 ps(VsOut i) : SV_Target
{
    float d = Depth.Load(int3(i.pos.xy, 0));
    if (d >= 1.0)
        return 1.0;
    float4 h = mul(InvViewProj, float4(i.ndc, d, 1.0));
    float3 p = h.xyz / h.w;
    return float4(Sun(p), PointShadow(p), 1.0, 1.0);
}
"#;

/// The sun's view: orthographic along `-dir` (`dir` points towards the sun, as `DirLight::direction`), centred on
/// the bounding sphere `bounds` and as wide and as deep as it (× [`SUN_MARGIN`]). Camera-relative: `eye` is the
/// camera's, so a position the camera passes write (`World` minus `eye`) lands in the map. Depth 0 faces the sun.
/// `width` and `height` are 1: the renderer sets the map's size.
pub fn sun_view(dir: [f32; 3], bounds: ([f32; 3], f32), eye: [f32; 3]) -> View {
    let (centre, radius) = bounds;
    let r = radius.max(1e-3) * SUN_MARGIN;
    let f = if dot(dir, dir) > 0.0 {
        norm(dir.map(|c| -c))
    } else {
        [0.0, 0.0, -1.0]
    };
    // `View::new`'s basis (right = forward × up), with its fallback up for a vertical view.
    let up = if f[2].abs() > 0.999 {
        [0.0, 1.0, 0.0]
    } else {
        [0.0, 0.0, 1.0]
    };
    let x = norm(cross(f, up));
    let y = cross(x, f);
    let c = sub(centre, eye);
    let view = [
        [x[0], x[1], x[2], -dot(x, c)],
        [y[0], y[1], y[2], -dot(y, c)],
        [f[0], f[1], f[2], -dot(f, c)],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let proj = [
        [1.0 / r, 0.0, 0.0, 0.0],
        [0.0, 1.0 / r, 0.0, 0.0],
        [0.0, 0.0, 0.5 / r, 0.5],
        [0.0, 0.0, 0.0, 1.0],
    ];
    View {
        eye,
        view,
        proj,
        view_proj: mul4(&proj, &view),
        near: 0.0,
        far: 2.0 * r,
        width: 1,
        height: 1,
    }
}

/// Per cube face (+X −X +Y −Y +Z −Z) its (right, up, forward) axes: D3D's cube addressing, as
/// `texcore::cube::dir_at` reads it (u = NDC x, v = NDC y), so the hardware's lookup by direction finds what each
/// face drew.
const FACES: [[[f32; 3]; 3]; 6] = [
    [[0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]],
    [[0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [-1.0, 0.0, 0.0]],
    [[1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]],
    [[1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, -1.0, 0.0]],
    [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
    [[-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0]],
];

/// The six 90° views of the point light at world `pos` out to `far` (near `far` × [`POINT_NEAR`]), camera-relative
/// like [`sun_view`], in cube face order. `width` and `height` are 1: the renderer sets the face's size.
pub fn point_views(pos: [f32; 3], far: f32, eye: [f32; 3]) -> [View; 6] {
    let far = far.max(1e-3);
    let near = far * POINT_NEAR;
    let q = far / (far - near);
    let l = sub(pos, eye);
    let proj = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, q, -q * near],
        [0.0, 0.0, 1.0, 0.0],
    ];
    FACES.map(|[r, u, f]| {
        let view = [
            [r[0], r[1], r[2], -dot(r, l)],
            [u[0], u[1], u[2], -dot(u, l)],
            [f[0], f[1], f[2], -dot(f, l)],
            [0.0, 0.0, 0.0, 1.0],
        ];
        View {
            eye,
            view,
            proj,
            view_proj: mul4(&proj, &view),
            near,
            far,
            width: 1,
            height: 1,
        }
    })
}

/// The point light's depth cube: six D32 faces of `size`², a DSV per face and one cube SRV.
pub struct PointCube {
    pub size: u32,
    _tex: ID3D11Texture2D,
    pub dsvs: Vec<ID3D11DepthStencilView>,
    pub srv: ID3D11ShaderResourceView,
}

/// The light maps of the current frame, for the mask pass and any other pass that samples them (the floor). The
/// view-projection and the point light's place are this frame's, camera-relative. [`ShadowMaps::bind`] binds the sun
/// map at [`SUN_REGISTER`], the cube at [`POINT_REGISTER`] and `sampler` at [`SAMPLER_REGISTER`].
pub struct ShadowMaps {
    pub sun_size: u32,
    _sun_tex: ID3D11Texture2D,
    pub sun_dsv: ID3D11DepthStencilView,
    pub sun_srv: ID3D11ShaderResourceView,
    pub sun_view_proj: Mat4,
    /// Only while the point light is on.
    pub point: Option<PointCube>,
    /// The point light minus the camera's eye.
    pub point_pos: [f32; 3],
    /// The cube's far plane: the light's falloff radius.
    pub point_far: f32,
    /// LessEqual comparison, bilinear, clamped: `SampleCmpLevelZero(…, depth) = 1` where lit.
    pub sampler: ID3D11SamplerState,
}

/// The floats of [`LOOKUP`]'s cbuffer fields: `SunViewProj` (16), then `Point`, `Params` and `Bias` (4 each).
pub(crate) const LOOKUP_FLOATS: usize = 28;

impl ShadowMaps {
    /// The values of `SunViewProj`, `Point`, `Params` and `Bias`, in [`LOOKUP`]'s order.
    pub(crate) fn lookup_floats(&self) -> [f32; LOOKUP_FLOATS] {
        let (point_on, cube) = match &self.point {
            Some(c) => (1.0, c.size as f32),
            None => (0.0, 1.0),
        };
        let p = self.point_pos;
        let mut f = [0.0; LOOKUP_FLOATS];
        for (d, s) in f.iter_mut().zip(self.sun_view_proj.iter().flatten()) {
            *d = *s;
        }
        f[16..].copy_from_slice(&[
            p[0],
            p[1],
            p[2],
            point_on,
            self.point_far * POINT_NEAR,
            self.point_far,
            1.0 / self.sun_size as f32,
            1.0 / cube,
            BIAS_TEXELS,
            0.0,
            0.0,
            0.0,
        ]);
        f
    }

    /// Binds the sun map, the cube (none while the light is off) and the sampler where [`LOOKUP`] reads them.
    pub(crate) fn bind(&self, gpu: &Gpu) {
        let cube = self.point.as_ref().map(|c| c.srv.clone());
        // SAFETY: every object belongs to `gpu.device`.
        unsafe {
            gpu.ctx
                .PSSetShaderResources(SUN_REGISTER, Some(&[Some(self.sun_srv.clone()), cube]));
            gpu.ctx
                .PSSetSamplers(SAMPLER_REGISTER, Some(&[Some(self.sampler.clone())]));
        }
    }

    /// Unbinds the maps: the next light pass writes them.
    pub(crate) fn unbind(&self, gpu: &Gpu) {
        // SAFETY: plain context call.
        unsafe {
            gpu.ctx
                .PSSetShaderResources(SUN_REGISTER, Some(&[None, None]));
        }
    }
}

/// The mask pass's cbuffer: `InvViewProj` (16), then the lookups' fields.
const MASK_FLOATS: usize = 16 + LOOKUP_FLOATS;

/// The maps and the mask pass, made on the first shadow frame and dropped on a frame without.
pub struct Shadows {
    pub maps: ShadowMaps,
    vs: ID3D11VertexShader,
    ps: ID3D11PixelShader,
    cb: ID3D11Buffer,
    no_depth: ID3D11DepthStencilState,
    /// The light passes' rasteriser: no culling (a cube face's basis is not mirrored like the camera's, so its
    /// winding differs) and the slope-scaled bias.
    pub raster: ID3D11RasterizerState,
}

fn depth_desc(size: u32, faces: u32) -> windows::Win32::Graphics::Direct3D11::D3D11_TEXTURE2D_DESC {
    let mut d = tex2d_desc(size, size, DXGI_FORMAT_R32_TYPELESS);
    d.ArraySize = faces;
    d.Usage = D3D11_USAGE_DEFAULT;
    d.BindFlags = (D3D11_BIND_DEPTH_STENCIL.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32;
    if faces == 6 {
        d.MiscFlags = D3D11_RESOURCE_MISC_TEXTURECUBE.0 as u32;
    }
    d
}

fn dsv(
    gpu: &Gpu,
    tex: &ID3D11Texture2D,
    desc: &D3D11_DEPTH_STENCIL_VIEW_DESC,
) -> Result<ID3D11DepthStencilView, String> {
    let mut v = None;
    // SAFETY: live resource; descriptor and out-pointer are locals.
    unsafe {
        gpu.device
            .CreateDepthStencilView(tex, Some(desc), Some(&mut v))
    }
    .map_err(|e| gpu.fail("CreateDepthStencilView (shadow)", e))?;
    v.ok_or_else(|| gpu.fail("CreateDepthStencilView (shadow)", "no view returned"))
}

impl PointCube {
    fn new(gpu: &Gpu, size: u32) -> Result<PointCube, String> {
        let tex = create_tex(gpu, &depth_desc(size, 6))?;
        let dsvs = (0..6)
            .map(|i| {
                let d = D3D11_DEPTH_STENCIL_VIEW_DESC {
                    Format: DXGI_FORMAT_D32_FLOAT,
                    ViewDimension: D3D11_DSV_DIMENSION_TEXTURE2DARRAY,
                    Flags: 0,
                    Anonymous: D3D11_DEPTH_STENCIL_VIEW_DESC_0 {
                        Texture2DArray: D3D11_TEX2D_ARRAY_DSV {
                            MipSlice: 0,
                            FirstArraySlice: i,
                            ArraySize: 1,
                        },
                    },
                };
                dsv(gpu, &tex, &d)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let sd = D3D11_SHADER_RESOURCE_VIEW_DESC {
            Format: DXGI_FORMAT_R32_FLOAT,
            ViewDimension: D3D11_SRV_DIMENSION_TEXTURECUBE,
            Anonymous: D3D11_SHADER_RESOURCE_VIEW_DESC_0 {
                TextureCube: D3D11_TEXCUBE_SRV {
                    MostDetailedMip: 0,
                    MipLevels: 1,
                },
            },
        };
        let mut srv = None;
        // SAFETY: live resource; descriptor and out-pointer are locals.
        unsafe {
            gpu.device
                .CreateShaderResourceView(&tex, Some(&sd), Some(&mut srv))
        }
        .map_err(|e| gpu.fail("CreateShaderResourceView (shadow cube)", e))?;
        Ok(PointCube {
            size,
            srv: srv.ok_or_else(|| gpu.fail("CreateShaderResourceView", "no view returned"))?,
            dsvs,
            _tex: tex,
        })
    }
}

impl Shadows {
    /// The sun map of `sun_size`², no cube, the mask pass's shaders and states.
    pub fn new(gpu: &Gpu, sun_size: u32) -> Result<Shadows, String> {
        let sun_tex = create_tex(gpu, &depth_desc(sun_size, 1))?;
        let sun_dsv = dsv(
            gpu,
            &sun_tex,
            &D3D11_DEPTH_STENCIL_VIEW_DESC {
                Format: DXGI_FORMAT_D32_FLOAT,
                ViewDimension: D3D11_DSV_DIMENSION_TEXTURE2D,
                ..Default::default()
            },
        )?;
        let sun_srv = r32_srv(gpu, &sun_tex)?;
        let source = mask_source();
        let vs_code = compile_source(&source, c"shadow.hlsl", "vs", "vs_5_0")?;
        let ps_code = compile_source(&source, c"shadow.hlsl", "ps", "ps_5_0")?;
        let bd = D3D11_BUFFER_DESC {
            ByteWidth: (MASK_FLOATS * 4) as u32,
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
            StructureByteStride: 0,
        };
        let smd = D3D11_SAMPLER_DESC {
            Filter: D3D11_FILTER_COMPARISON_MIN_MAG_LINEAR_MIP_POINT,
            AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
            AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
            AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
            MipLODBias: 0.0,
            MaxAnisotropy: 1,
            ComparisonFunc: D3D11_COMPARISON_LESS_EQUAL,
            BorderColor: [1.0; 4],
            MinLOD: 0.0,
            MaxLOD: D3D11_FLOAT32_MAX,
        };
        let rd = D3D11_RASTERIZER_DESC {
            FillMode: D3D11_FILL_SOLID,
            CullMode: D3D11_CULL_NONE,
            FrontCounterClockwise: true.into(),
            DepthBias: 0,
            DepthBiasClamp: SLOPE_BIAS_CLAMP,
            SlopeScaledDepthBias: SLOPE_BIAS,
            DepthClipEnable: true.into(),
            ..Default::default()
        };
        let dd = D3D11_DEPTH_STENCIL_DESC::default(); // DepthEnable = false
        let (mut vs, mut ps, mut cb, mut smp, mut rs, mut ds) =
            (None, None, None, None, None, None);
        // SAFETY: descriptors and bytecode outlive the calls; out-pointers are locals.
        unsafe {
            gpu.device
                .CreateVertexShader(&vs_code, None, Some(&mut vs))
                .map_err(|e| gpu.fail("CreateVertexShader(shadow)", e))?;
            gpu.device
                .CreatePixelShader(&ps_code, None, Some(&mut ps))
                .map_err(|e| gpu.fail("CreatePixelShader(shadow)", e))?;
            gpu.device
                .CreateBuffer(&bd, None, Some(&mut cb))
                .map_err(|e| gpu.fail("CreateBuffer(shadow)", e))?;
            gpu.device
                .CreateSamplerState(&smd, Some(&mut smp))
                .map_err(|e| gpu.fail("CreateSamplerState(shadow)", e))?;
            gpu.device
                .CreateRasterizerState(&rd, Some(&mut rs))
                .map_err(|e| gpu.fail("CreateRasterizerState(shadow)", e))?;
            gpu.device
                .CreateDepthStencilState(&dd, Some(&mut ds))
                .map_err(|e| gpu.fail("CreateDepthStencilState(shadow)", e))?;
        }
        Ok(Shadows {
            maps: ShadowMaps {
                sun_size,
                _sun_tex: sun_tex,
                sun_dsv,
                sun_srv,
                sun_view_proj: [[0.0; 4]; 4],
                point: None,
                point_pos: [0.0; 3],
                point_far: 1.0,
                sampler: smp.ok_or("no shadow sampler")?,
            },
            vs: vs.ok_or("no shadow vertex shader")?,
            ps: ps.ok_or("no shadow pixel shader")?,
            cb: cb.ok_or("no shadow buffer")?,
            no_depth: ds.ok_or("no shadow depth state")?,
            raster: rs.ok_or("no shadow rasteriser state")?,
        })
    }

    /// The cube at `size` (made, or made again at a new size), or none: the point light is off.
    pub fn set_point(&mut self, gpu: &Gpu, size: Option<u32>) -> Result<(), String> {
        match size {
            None => self.maps.point = None,
            Some(n) if self.maps.point.as_ref().is_none_or(|c| c.size != n) => {
                self.maps.point = Some(PointCube::new(gpu, n)?);
            }
            Some(_) => {}
        }
        Ok(())
    }

    /// Draws the mask into the bound render target (the target's mask RTV, its viewport, no depth) from `depth` (the
    /// camera depth copy) and the maps drawn this frame; `view` is the camera's. Leaves t0 and the map registers
    /// of the pixel stage unbound, the default depth state and no input layout.
    pub fn draw_mask(&self, gpu: &Gpu, depth: &ID3D11ShaderResourceView, view: &View) {
        let m = &self.maps;
        let mut floats = [0.0f32; MASK_FLOATS];
        for (d, s) in floats
            .iter_mut()
            .zip(inverse4(&view.view_proj).iter().flatten())
        {
            *d = *s;
        }
        floats[16..].copy_from_slice(&m.lookup_floats());
        // SAFETY: every object belongs to `gpu.device`; `floats` holds the buffer's `MASK_FLOATS` × 4 bytes
        // (little-endian `f32`, as the GPU reads them).
        unsafe {
            let ctx = &gpu.ctx;
            ctx.UpdateSubresource(&self.cb, 0, None, floats.as_ptr() as _, 0, 0);
            ctx.OMSetDepthStencilState(&self.no_depth, 0);
            ctx.OMSetBlendState(None, None, 0xffff_ffff);
            ctx.IASetInputLayout(None);
            ctx.VSSetShader(&self.vs, None);
            ctx.PSSetShader(&self.ps, None);
            ctx.PSSetConstantBuffers(0, Some(&[Some(self.cb.clone())]));
            ctx.PSSetShaderResources(0, Some(&[Some(depth.clone())]));
            m.bind(gpu);
            ctx.Draw(3, 0);
            ctx.PSSetShaderResources(0, Some(&[None]));
            m.unbind(gpu);
            ctx.OMSetDepthStencilState(None, 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(v: &View, world: [f32; 3]) -> [f32; 3] {
        let c = [
            world[0] - v.eye[0],
            world[1] - v.eye[1],
            world[2] - v.eye[2],
            1.0,
        ];
        let clip: Vec<f32> = (0..4)
            .map(|r| (0..4).map(|k| v.view_proj[r][k] * c[k]).sum())
            .collect();
        [clip[0] / clip[3], clip[1] / clip[3], clip[2] / clip[3]]
    }

    /// Every texel of every face sees the direction D3D's cube addressing gives it (`texcore::cube::cube_dir`), so a
    /// lookup by direction reads the face and texel that drew it.
    #[test]
    fn cube_faces_follow_d3d_addressing() {
        let (pos, far, eye, size) = ([3.0, -4.0, 5.0], 50.0, [10.0, 20.0, -30.0], 8u32);
        let views = point_views(pos, far, eye);
        for (f, v) in views.iter().enumerate() {
            for y in 0..size {
                for x in 0..size {
                    let d = texcore::cube::cube_dir(f, x, y, size);
                    let p = [0, 1, 2].map(|k| pos[k] + 20.0 * d[k]);
                    let ndc = project(v, p);
                    let px = ((ndc[0] + 1.0) * 0.5 * size as f32).floor() as u32;
                    let py = ((1.0 - ndc[1]) * 0.5 * size as f32).floor() as u32;
                    assert_eq!((px, py), (x, y), "face {f}");
                    assert!(ndc[2] > 0.0 && ndc[2] < 1.0, "face {f}: depth {}", ndc[2]);
                }
            }
        }
    }

    /// The sun's box holds the bounding sphere: its centre at the middle, the point nearest the sun at depth 0 and
    /// the farthest at 1, the sides at ±1; straight down (`View::new`'s vertical fallback) included.
    #[test]
    fn the_sun_box_fits_the_bounding_sphere() {
        let (c, r, eye) = ([5.0, 6.0, 7.0], 10.0, [100.0, -50.0, 20.0]);
        for dir in [[0.0, 0.0, 1.0], norm([0.5, 0.8, 0.9]), [1.0, 0.0, 0.0]] {
            let v = sun_view(dir, (c, r), eye);
            let at = |k: f32| [0, 1, 2].map(|i| c[i] + k * r * dir[i]);
            let mid = project(&v, c);
            assert!(mid[0].abs() < 1e-4 && mid[1].abs() < 1e-4 && (mid[2] - 0.5).abs() < 1e-4);
            let near = project(&v, at(1.0))[2];
            let far = project(&v, at(-1.0))[2];
            let k = 1.0 / SUN_MARGIN;
            assert!((near - 0.5 * (1.0 - k)).abs() < 1e-4, "{dir:?}: {near}");
            assert!((far - 0.5 * (1.0 + k)).abs() < 1e-4, "{dir:?}: {far}");
            let side = norm(cross(dir, [0.3, 0.2, 0.1]));
            let s = project(&v, [0, 1, 2].map(|i| c[i] + r * side[i]));
            assert!(
                ((s[0] * s[0] + s[1] * s[1]).sqrt() - k).abs() < 1e-4,
                "{dir:?}: {s:?}"
            );
        }
    }
}
