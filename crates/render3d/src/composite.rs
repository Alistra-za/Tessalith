//! CS's DeferredCompositeCS (v2 Phase D, D4): compiled cs_5_0 from the user's install, run after the opaque DEFERRED
//! draws with the MRTs as SRVs (bound by reflected name) and RT0 as MainRW. Every A / A_RT golden capture binds only
//! t0/t2 (t4 null) with b5 + b12, so under Linear Lighting the composite is `MainRW.rgb += Specular; MainRW.a = 1`.
//! Reflected SRVs/UAVs outside [`INPUTS`], [`Inputs`] and MainRW stay unbound (reads 0, writes dropped); their names
//! are in [`Composite::unbound`].
//!
//! D5: DepthTexture, the Dynamic Cubemaps cubes (EnvTexture, EnvReflectionsTexture) and the sampler come from
//! [`Inputs`]: the GUI passes the deferred depth and the HDRI cube ([`EnvMaps`]), the replay the captured ones. A
//! reflected input that is not given is an error (fail closed).
use crate::compile::{compile, to_defines, Reflection, SrvKind};
use crate::cs_settings;
use crate::device::{Gpu, Target};
use crate::draw::{cbuffers, upload, Cb};
use crate::hdri::{self, Hdr};
use crate::scene::{Mat3x4, Scene, View};
use crate::source::SourceSet;
use std::collections::BTreeSet;
use std::path::Path;
use windows::Win32::Graphics::Direct3D11::{
    ID3D11ComputeShader, ID3D11SamplerState, ID3D11ShaderResourceView, ID3D11Texture2D,
    D3D11_COMPARISON_NEVER, D3D11_FEATURE_DATA_FORMAT_SUPPORT2, D3D11_FEATURE_FORMAT_SUPPORT2,
    D3D11_FILTER_MIN_MAG_MIP_LINEAR, D3D11_FLOAT32_MAX, D3D11_FORMAT_SUPPORT2_UAV_TYPED_LOAD,
    D3D11_SAMPLER_DESC, D3D11_TEXTURE_ADDRESS_CLAMP, D3D11_TEXTURE_ADDRESS_WRAP,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R16G16B16A16_FLOAT;

pub const SOURCE: &str = "DeferredCompositeCS.hlsl";
/// Reflected composite SRV name → MRT index (DeferredCompositeCS.hlsl:11-15,30; Lighting.hlsl:290-301).
pub const INPUTS: [(&str, usize); 6] = [
    ("SpecularTexture", 4),
    ("AlbedoTexture", 3),
    ("NormalRoughnessTexture", 2),
    ("MasksTexture", 6),
    ("ReflectanceTexture", 5),
    ("Masks2Texture", 7),
];
/// Reflected composite SRVs that come from [`Inputs`], with the kind each must have (DeferredCompositeCS.hlsl:23-35).
const NAMED: [(&str, SrvKind); 3] = [
    ("DepthTexture", SrvKind::Tex2D),
    ("EnvTexture", SrvKind::Cube),
    ("EnvReflectionsTexture", SrvKind::Cube),
];
const IDENT: Mat3x4 = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
];

/// The Dynamic Cubemaps cubes: `env` = EnvTexture, `reflection` = EnvReflectionsTexture.
pub struct EnvMaps {
    pub env: ID3D11ShaderResourceView,
    pub reflection: ID3D11ShaderResourceView,
}

impl EnvMaps {
    /// From the HDRI turned by `yaw`: [`hdri::cube_faces`] + [`hdri::specular_chain`] down to 1×1 (the composite
    /// divides by the level-15 luminance), RGBA16F. `env` and `reflection` are the same view (an HDRI has no sky/no-sky
    /// split).
    /// Err unless `size` is a power of two up to [`crate::textures::MAX_CUBE`].
    pub fn new(gpu: &Gpu, img: &Hdr, size: u32, yaw: f32) -> Result<EnvMaps, String> {
        check_env_size(size)?;
        let (mips, data) = f16_chains(hdri::specular_chain(
            &hdri::cube_faces(img, size, yaw),
            size,
        )?);
        let v =
            crate::textures::cube_srv(gpu, size, mips, DXGI_FORMAT_R16G16B16A16_FLOAT, 8, &data)?;
        Ok(EnvMaps {
            env: v.clone(),
            reflection: v,
        })
    }

    /// [`EnvMaps::new`] with the GGX levels from CS's own SpecularIrradianceCS on the GPU (Note GM1): the faces and
    /// their [`hdri::mip_chain`] on the CPU (the pass's input and the output's mip 0), then `pre` writes levels 1..
    /// in place of the box mips. ~40 ms → a few ms per yaw change.
    pub fn new_gpu(
        gpu: &Gpu,
        pre: &mut Prefilter,
        img: &Hdr,
        size: u32,
        yaw: f32,
    ) -> Result<EnvMaps, String> {
        check_env_size(size)?;
        let chains = hdri::cube_faces(img, size, yaw)
            .iter()
            .map(|f| hdri::mip_chain(f, size))
            .collect::<Result<Vec<_>, _>>()?;
        let (mips, data) = f16_chains(chains);
        let f = DXGI_FORMAT_R16G16B16A16_FLOAT;
        let input = crate::textures::cube_srv(gpu, size, mips, f, 8, &data)?;
        let (out, v) = crate::textures::cube_rw(gpu, size, mips, f, 8, &data)?;
        pre.run(gpu, &input, &out, size, mips)?;
        Ok(EnvMaps {
            env: v.clone(),
            reflection: v,
        })
    }
}

fn check_env_size(size: u32) -> Result<(), String> {
    if !size.is_power_of_two() || size > crate::textures::MAX_CUBE {
        return Err(format!(
            "env cube size {size} is not a power of two up to {}",
            crate::textures::MAX_CUBE
        ));
    }
    Ok(())
}

/// Per-face mip chains as RGBA16F bytes, face-major (the cube upload layout), and the level count.
fn f16_chains(chains: impl IntoIterator<Item = Vec<Vec<[f32; 4]>>>) -> (u32, Vec<u8>) {
    let mut data = Vec::new();
    let mut mips = 0;
    for chain in chains {
        mips = chain.len() as u32;
        for px in chain.iter().flatten() {
            // As the backdrop: clamped below f16's maximum, so a hot pixel never becomes infinity.
            data.extend(
                px.iter()
                    .flat_map(|v| half::f16::from_f32(v.min(65000.0)).to_le_bytes()),
            );
        }
    }
    (mips, data)
}

/// CS's Dynamic Cubemaps GGX prefilter ([`Prefilter::SOURCE`], cs_5_0) from the user's install (Note GM1): level L of the
/// output = the input cube filtered at roughness L / (levels − 1), the same pass CS runs in game. Fails closed on an
/// interface it does not know (one cube SRV, one UAV, one sampler, a `roughness` constant); the renderer then keeps
/// the CPU [`hdri::specular_chain`].
pub struct Prefilter {
    cs: ID3D11ComputeShader,
    cb: Cb,
    sampler: ID3D11SamplerState,
    input_slot: u32,
    output_slot: u32,
    sampler_slot: u32,
    threads: [u32; 3],
    /// What it was compiled with (the renderer rebuilds when its wanted list differs).
    pub defines: Vec<String>,
}

impl Prefilter {
    pub const SOURCE: &'static str = "DynamicCubemaps/SpecularIrradianceCS.hlsl";

    pub fn new(
        gpu: &Gpu,
        src: &SourceSet,
        defines: &[String],
        cache_dir: &Path,
    ) -> Result<Prefilter, String> {
        let name = Prefilter::SOURCE;
        let s = src.get(Path::new(name))?;
        let c = compile(&s, &to_defines(defines), "cs_5_0", cache_dir)?;
        let r = &c.reflection;
        let one = |what: &str, n: usize| match n {
            1 => Ok(()),
            _ => Err(format!("{name}: {n} {what}, expected 1")),
        };
        one("SRVs", r.srvs.len())?;
        one("UAVs", r.uavs.len())?;
        one("samplers", r.samplers.len())?;
        one("cbuffers", r.cbuffers.len())?;
        let (input, input_slot, kind) = &r.srvs[0];
        if *kind != SrvKind::Cube {
            return Err(format!("{name}: {input} is {kind:?}, not Cube"));
        }
        if r.threads.contains(&0) {
            return Err(format!("{name}: no thread-group size"));
        }
        let mut cb = cbuffers(gpu, r)?.remove(0);
        if !cb.writer.set("roughness", &[0.0]) {
            return Err(format!("{name}: no roughness constant"));
        }
        let mut cs = None;
        // SAFETY: the bytecode slice is valid for the call; the out-pointer is a local.
        unsafe {
            gpu.device
                .CreateComputeShader(&c.bytecode, None, Some(&mut cs))
        }
        .map_err(|e| gpu.fail("CreateComputeShader", e))?;
        let sd = D3D11_SAMPLER_DESC {
            Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
            AddressU: D3D11_TEXTURE_ADDRESS_WRAP,
            AddressV: D3D11_TEXTURE_ADDRESS_WRAP,
            AddressW: D3D11_TEXTURE_ADDRESS_WRAP,
            MipLODBias: 0.0,
            MaxAnisotropy: 1,
            ComparisonFunc: D3D11_COMPARISON_NEVER,
            BorderColor: [0.0; 4],
            MinLOD: 0.0,
            MaxLOD: D3D11_FLOAT32_MAX,
        };
        let mut sampler = None;
        // SAFETY: descriptor and out-pointer are locals.
        unsafe { gpu.device.CreateSamplerState(&sd, Some(&mut sampler)) }
            .map_err(|e| gpu.fail("CreateSamplerState", e))?;
        Ok(Prefilter {
            cs: cs.ok_or("no compute shader")?,
            cb,
            sampler: sampler.ok_or("no sampler")?,
            input_slot: *input_slot,
            output_slot: r.uavs[0].1,
            sampler_slot: r.samplers[0].1,
            threads: r.threads,
            defines: defines.to_vec(),
        })
    }

    /// Writes levels 1..`mips` of every face of `out` from `input` (both `size`² RGBA16F cubes with `mips` levels),
    /// one dispatch per level, and unbinds everything again.
    fn run(
        &mut self,
        gpu: &Gpu,
        input: &ID3D11ShaderResourceView,
        out: &ID3D11Texture2D,
        size: u32,
        mips: u32,
    ) -> Result<(), String> {
        let f = DXGI_FORMAT_R16G16B16A16_FLOAT;
        let uavs = (1..mips)
            .map(|l| crate::textures::uav_level(gpu, out, f, l, 6))
            .collect::<Result<Vec<_>, _>>()?;
        let [tx, ty, _] = self.threads;
        // SAFETY (whole block): every bound object belongs to `gpu.device`; slices outlive the calls.
        unsafe {
            let ctx = &gpu.ctx;
            ctx.CSSetShader(&self.cs, None);
            ctx.CSSetConstantBuffers(self.cb.slot, Some(&[Some(self.cb.buffer.clone())]));
            ctx.CSSetShaderResources(self.input_slot, Some(&[Some(input.clone())]));
            ctx.CSSetSamplers(self.sampler_slot, Some(&[Some(self.sampler.clone())]));
            for (l, uav) in (1..mips).zip(&uavs) {
                self.cb
                    .writer
                    .set("roughness", &[l as f32 / (mips - 1) as f32]);
                upload(gpu, &self.cb);
                ctx.CSSetUnorderedAccessViews(self.output_slot, 1, Some(&Some(uav.clone())), None);
                let n = (size >> l).max(1);
                ctx.Dispatch(n.div_ceil(tx), n.div_ceil(ty), 6);
            }
            ctx.CSSetUnorderedAccessViews(self.output_slot, 1, Some(&None), None);
            ctx.CSSetShaderResources(self.input_slot, Some(&[None]));
            ctx.CSSetSamplers(self.sampler_slot, Some(&[None]));
            ctx.CSSetConstantBuffers(self.cb.slot, Some(&[None]));
            ctx.CSSetShader(None, None);
        }
        Ok(())
    }
}

/// What the composite reads besides the target: the depth SRV, the env cubes, and its sampler (`None` = the
/// composite's own LinearSampler, CS's captured state).
pub struct Inputs<'a> {
    pub depth: Option<&'a ID3D11ShaderResourceView>,
    pub env: Option<&'a EnvMaps>,
    pub sampler: Option<&'a ID3D11SamplerState>,
}

pub struct Composite {
    cs: ID3D11ComputeShader,
    /// MIN_MAG_MIP_LINEAR, clamp, LOD 0..max: the captured LinearSampler (composite/extras.json samplers.cs).
    linear: ID3D11SamplerState,
    pub reflection: Reflection,
    /// What it was compiled with (render rebuilds when the wanted list differs).
    pub defines: Vec<String>,
    /// One per reflected cbuffer ([`cbuffers`]); replay swaps `buffer` for the captured bytes.
    pub cbs: Vec<Cb>,
    /// Reflected UAV "MainRW".
    main_slot: u32,
    /// Reflected SRVs/UAVs left unbound ("t4 DepthTexture", "u1 NormalTAAMaskSpecularMaskRW").
    pub unbound: Vec<String>,
}

fn input(name: &str) -> Option<usize> {
    INPUTS.iter().find(|i| i.0 == name).map(|i| i.1)
}

impl Composite {
    /// The composite reads MainRW (RGBA16F) through a UAV: optional at FL11_0.
    pub fn typed_uav_load(gpu: &Gpu) -> bool {
        let mut d = D3D11_FEATURE_DATA_FORMAT_SUPPORT2 {
            InFormat: DXGI_FORMAT_R16G16B16A16_FLOAT,
            OutFormatSupport2: 0,
        };
        // SAFETY: `d` is a live local of the size passed.
        let ok = unsafe {
            gpu.device.CheckFeatureSupport(
                D3D11_FEATURE_FORMAT_SUPPORT2,
                &mut d as *mut _ as _,
                std::mem::size_of_val(&d) as u32,
            )
        };
        ok.is_ok() && d.OutFormatSupport2 & D3D11_FORMAT_SUPPORT2_UAV_TYPED_LOAD.0 as u32 != 0
    }

    /// Compiles `SOURCE` (cs_5_0, `defines` as "K" or "K=V") and checks what render binds: thread-group size,
    /// MainRW, every [`INPUTS`] name it reflects a 2D texture, DepthTexture 2D and the env cubes cubes.
    pub fn new(
        gpu: &Gpu,
        src: &SourceSet,
        defines: &[String],
        cache_dir: &Path,
    ) -> Result<Composite, String> {
        if !Composite::typed_uav_load(gpu) {
            return Err(
                "this GPU cannot read R16G16B16A16_FLOAT through a UAV (typed UAV load)".into(),
            );
        }
        let s = src.get(Path::new(SOURCE))?;
        let c = compile(&s, &to_defines(defines), "cs_5_0", cache_dir)?;
        let r = c.reflection;
        if r.threads == [0; 3] {
            return Err(format!("{SOURCE}: no thread-group size"));
        }
        let main_slot = r
            .uavs
            .iter()
            .find(|u| u.0 == "MainRW")
            .ok_or_else(|| format!("{SOURCE}: no MainRW UAV"))?
            .1;
        let mut unbound = vec![];
        for (name, reg, kind) in &r.srvs {
            let want = match (input(name), NAMED.iter().find(|n| n.0 == name)) {
                (Some(_), _) => Some(SrvKind::Tex2D),
                (None, Some(n)) => Some(n.1),
                (None, None) => None,
            };
            match want {
                Some(k) if *kind != k => {
                    return Err(format!("{SOURCE}: {name} is {kind:?}, not {k:?}"))
                }
                Some(_) => {}
                None => unbound.push(format!("t{reg} {name}")),
            }
        }
        // Fail closed like the SRVs: the only sampler there is an input for is the captured LinearSampler.
        if let Some((name, slot)) = r.samplers.iter().find(|s| s.0 != "LinearSampler") {
            return Err(format!(
                "{SOURCE}: sampler s{slot} {name} has no input (only LinearSampler is bound)"
            ));
        }
        unbound.extend(
            r.uavs
                .iter()
                .filter(|u| u.0 != "MainRW")
                .map(|(n, reg)| format!("u{reg} {n}")),
        );
        let mut cs = None;
        // SAFETY: the bytecode slice is valid for the call; the out-pointer is a local.
        unsafe {
            gpu.device
                .CreateComputeShader(&c.bytecode, None, Some(&mut cs))
        }
        .map_err(|e| gpu.fail("CreateComputeShader", e))?;
        let sd = D3D11_SAMPLER_DESC {
            Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
            AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
            AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
            AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
            MipLODBias: 0.0,
            MaxAnisotropy: 1,
            ComparisonFunc: D3D11_COMPARISON_NEVER,
            BorderColor: [0.0; 4],
            MinLOD: 0.0,
            MaxLOD: D3D11_FLOAT32_MAX,
        };
        let mut linear = None;
        // SAFETY: descriptor and out-pointer are locals.
        unsafe { gpu.device.CreateSamplerState(&sd, Some(&mut linear)) }
            .map_err(|e| gpu.fail("CreateSamplerState", e))?;
        Ok(Composite {
            cs: cs.ok_or("no compute shader")?,
            linear: linear.ok_or("no sampler")?,
            cbs: cbuffers(gpu, &r)?,
            reflection: r,
            defines: defines.to_vec(),
            main_slot,
            unbound,
        })
    }

    /// The composite reads an env cube (DYNAMIC_CUBEMAPS): render builds [`EnvMaps`] for it.
    pub fn needs_env(&self) -> bool {
        self.reflection
            .srvs
            .iter()
            .any(|s| s.0 == "EnvTexture" || s.0 == "EnvReflectionsTexture")
    }

    /// The reflected cbuffer field names, namespace-stripped (what `cs_settings::feature_data` keys on).
    pub fn fields(&self) -> BTreeSet<String> {
        cs_settings::reflected_fields(&self.reflection.cbuffers)
    }

    /// The scene by name into every cbuffer (BufferDim, the camera inverses, DynamicResolutionParams2), uploaded.
    pub fn write_scene(
        &mut self,
        gpu: &Gpu,
        scene: &Scene,
        view: &View,
        feature_values: &[(String, Vec<f64>)],
    ) {
        for cb in &mut self.cbs {
            scene.write(&mut cb.writer, view, &IDENT, feature_values);
            upload(gpu, cb);
        }
    }

    /// Thread groups covering `w × h`.
    pub fn groups(&self, w: u32, h: u32) -> [u32; 3] {
        let [tx, ty, _] = self.reflection.threads;
        [w.div_ceil(tx.max(1)), h.div_ceil(ty.max(1)), 1]
    }

    /// Runs the composite on `t` (deferred): unbinds the OM (RT0 becomes the UAV, the MRTs and depth SRVs), binds by
    /// reflected name (MRTs, `inputs`, every reflected sampler), dispatches and unbinds again. A reflected input that
    /// `inputs` lacks is an Err before anything is bound. The caller rebinds its render targets.
    pub fn dispatch(&self, gpu: &Gpu, t: &Target, inputs: &Inputs) -> Result<(), String> {
        let uav = t
            .main_uav
            .as_ref()
            .filter(|_| t.deferred())
            .ok_or("the composite needs a deferred target")?;
        let missing =
            |name: &str, what: &str| format!("composite reads {name} but no {what} was given");
        let mut srvs: Vec<(u32, ID3D11ShaderResourceView)> = vec![];
        for (name, reg, _) in &self.reflection.srvs {
            let v = match (input(name), name.as_str()) {
                (Some(rt), _) => t.mrt[rt].2.clone(),
                (_, "DepthTexture") => inputs.depth.ok_or_else(|| missing(name, "depth"))?.clone(),
                (_, "EnvTexture") => inputs
                    .env
                    .ok_or_else(|| missing(name, "env cube"))?
                    .env
                    .clone(),
                (_, "EnvReflectionsTexture") => inputs
                    .env
                    .ok_or_else(|| missing(name, "env cube"))?
                    .reflection
                    .clone(),
                _ => continue,
            };
            srvs.push((*reg, v));
        }
        let sampler = inputs.sampler.unwrap_or(&self.linear);
        let [x, y, z] = self.groups(t.width, t.height);
        // SAFETY (whole block): every bound object belongs to `gpu.device`; slices outlive the calls.
        unsafe {
            let ctx = &gpu.ctx;
            // First: the depth SRV and RT0's UAV must not stay bound as DSV / RTV.
            ctx.OMSetRenderTargets(None, None);
            ctx.CSSetShader(&self.cs, None);
            for cb in &self.cbs {
                ctx.CSSetConstantBuffers(cb.slot, Some(&[Some(cb.buffer.clone())]));
            }
            for (reg, v) in &srvs {
                ctx.CSSetShaderResources(*reg, Some(&[Some(v.clone())]));
            }
            for (_, slot) in &self.reflection.samplers {
                ctx.CSSetSamplers(*slot, Some(&[Some(sampler.clone())]));
            }
            ctx.CSSetUnorderedAccessViews(self.main_slot, 1, Some(&Some(uav.clone())), None);
            ctx.Dispatch(x, y, z);
            for (reg, _) in &srvs {
                ctx.CSSetShaderResources(*reg, Some(&[None]));
            }
            for (_, slot) in &self.reflection.samplers {
                ctx.CSSetSamplers(*slot, Some(&[None]));
            }
            ctx.CSSetUnorderedAccessViews(self.main_slot, 1, Some(&None), None);
            ctx.CSSetShader(None, None);
        }
        Ok(())
    }
}
