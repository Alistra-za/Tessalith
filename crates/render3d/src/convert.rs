//! The display readback's half → RGBA8 conversion, on the GPU (Note IF). A compute pass takes each colour channel's
//! half bits and looks them up in a 256×256 texture holding the same 65536-entry table the CPU conversion used, so
//! the bytes are identical; alpha is written as 255. The CPU then copies 4 bytes per pixel instead of converting 8.

use crate::device::Gpu;
use windows::Win32::Graphics::Direct3D11::{
    ID3D11ComputeShader, ID3D11DepthStencilView, ID3D11RenderTargetView, ID3D11ShaderResourceView,
    ID3D11UnorderedAccessView, D3D11_BIND_SHADER_RESOURCE, D3D11_SUBRESOURCE_DATA,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_IMMUTABLE,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_R8_UINT, DXGI_SAMPLE_DESC};

/// Thread-group side of [`SOURCE`].
const GROUP: u32 = 8;

const SOURCE: &str = r#"
Texture2D<float4> Src : register(t0);
Texture2D<uint> Lut : register(t1);
RWTexture2D<uint4> Dst : register(u0);

uint look(float v)
{
    // Half → float → half is exact, so these are the bits the target holds (every NaN stays a NaN; the table maps
    // them all to 0).
    uint b = f32tof16(v);
    return Lut.Load(int3(b & 255, b >> 8, 0));
}

[numthreads(8, 8, 1)]
void cs(uint3 id : SV_DispatchThreadID)
{
    float4 c = Src.Load(int3(id.xy, 0));
    Dst[id.xy] = uint4(look(c.r), look(c.g), look(c.b), 255);
}
"#;

/// The compiled pass and the two lookup tables. One per [`Gpu`], made on the first readback.
pub(crate) struct Convert {
    cs: ID3D11ComputeShader,
    unorm: ID3D11ShaderResourceView,
    srgb: ID3D11ShaderResourceView,
}

/// A 256×256 R8_UINT texture of `table` (65536 bytes): texel (bits & 255, bits >> 8) is `table[bits]`.
fn lut(gpu: &Gpu, table: &[u8]) -> Result<ID3D11ShaderResourceView, String> {
    assert_eq!(
        table.len(),
        1 << 16,
        "the readback table has one byte per half bit pattern"
    );
    let desc = D3D11_TEXTURE2D_DESC {
        Width: 256,
        Height: 256,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_R8_UINT,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_IMMUTABLE,
        BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
        CPUAccessFlags: 0,
        MiscFlags: 0,
    };
    let data = D3D11_SUBRESOURCE_DATA {
        pSysMem: table.as_ptr() as _,
        SysMemPitch: 256,
        SysMemSlicePitch: 0,
    };
    let (mut tex, mut srv) = (None, None);
    // SAFETY: `table` holds 256 rows of 256 bytes and outlives the call; out-pointers are locals.
    unsafe {
        gpu.device
            .CreateTexture2D(&desc, Some(&data), Some(&mut tex))
            .map_err(|e| gpu.fail("CreateTexture2D (readback table)", e))?;
        let tex = tex.ok_or_else(|| gpu.fail("CreateTexture2D (readback table)", "no texture"))?;
        gpu.device
            .CreateShaderResourceView(&tex, None, Some(&mut srv))
            .map_err(|e| gpu.fail("CreateShaderResourceView (readback table)", e))?;
    }
    srv.ok_or_else(|| gpu.fail("CreateShaderResourceView (readback table)", "no view"))
}

impl Convert {
    /// Compiles the pass and uploads the tables: `unorm` and `srgb` are the CPU conversion's 65536-entry tables.
    pub(crate) fn new(gpu: &Gpu, unorm: &[u8], srgb: &[u8]) -> Result<Convert, String> {
        let code = crate::background::compile_source(SOURCE, c"readback.hlsl", "cs", "cs_5_0")
            .map_err(|e| format!("readback conversion shader: {e}"))?;
        let mut cs = None;
        // SAFETY: `code` is live bytecode; the out-pointer is a local.
        unsafe { gpu.device.CreateComputeShader(&code, None, Some(&mut cs)) }
            .map_err(|e| gpu.fail("CreateComputeShader (readback)", e))?;
        Ok(Convert {
            cs: cs.ok_or_else(|| gpu.fail("CreateComputeShader (readback)", "no shader"))?,
            unorm: lut(gpu, unorm)?,
            srgb: lut(gpu, srgb)?,
        })
    }

    /// Converts `src` (the `w`×`h` half-float colour target) into `dst` (R8G8B8A8_UINT). Unbinds the render
    /// targets for the pass (D3D11 hands a shader a null view of a texture that is still bound for output) and
    /// binds them again after it. The compute stage is not restored: its shader, t0, t1 and u0-u7 are left
    /// unbound, as every compute pass in this crate binds what it uses before it dispatches.
    pub(crate) fn run(
        &self,
        gpu: &Gpu,
        src: &ID3D11ShaderResourceView,
        dst: &ID3D11UnorderedAccessView,
        (w, h): (u32, u32),
        srgb: bool,
    ) {
        let table = if srgb { &self.srgb } else { &self.unorm };
        let ctx = &gpu.ctx;
        let mut rtvs: [Option<ID3D11RenderTargetView>; 8] = Default::default();
        let mut dsv: Option<ID3D11DepthStencilView> = None;
        // SAFETY: every view belongs to `gpu.device` and outlives the calls; the out-array and the slices are live
        // locals.
        unsafe {
            // Put back afterwards: a frame has always left its targets bound, and callers may draw again.
            ctx.OMGetRenderTargets(Some(&mut rtvs), Some(&mut dsv));
            ctx.OMSetRenderTargets(None, None);
            // Before the input is bound: a compute UAV of the colour texture left in any slot (the deferred
            // composite writes it through one) would null the input the same way.
            let none: [Option<ID3D11UnorderedAccessView>; 8] = Default::default();
            ctx.CSSetUnorderedAccessViews(0, 8, Some(none.as_ptr()), None);
            ctx.CSSetShader(&self.cs, None);
            ctx.CSSetShaderResources(0, Some(&[Some(src.clone()), Some(table.clone())]));
            ctx.CSSetUnorderedAccessViews(0, 1, Some(&Some(dst.clone())), None);
            ctx.Dispatch(w.div_ceil(GROUP), h.div_ceil(GROUP), 1);
            ctx.CSSetUnorderedAccessViews(0, 1, Some(&None), None);
            ctx.CSSetShaderResources(0, Some(&[None, None]));
            ctx.CSSetShader(None, None);
            ctx.OMSetRenderTargets(Some(&rtvs), dsv.as_ref());
        }
    }
}
