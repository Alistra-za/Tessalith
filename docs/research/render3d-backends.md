# The 3D preview's graphics backend (2026-11-16)

Whether `render3d`, the crate that draws the 3D preview, should stay on Direct3D 11, and what D3D12, Vulkan, DXVK or wgpu would each make possible: Linux, speed, ray tracing, physics. This page is for whoever decides the preview's next step, and for authors of other tools built on `render3d`. It ranks the paths by what they enable and ignores what a switch would cost.

Measured on one NVIDIA RTX 4090 and AMD Ryzen 7 9800X3D on 2026-11-16, from `main` 40589a43. Sources were fetched the same day and are pinned under [Sources](#sources). No backend code was written: every statement about a path other than today's D3D11 is either cited or marked untested.

Short answer:
- **Frame time is not a backend question.** GPU time is under the 0.6 ms or so the probe can resolve, at every size. Reading the frame back to the CPU is 77% to 97% of a frame.
- **Cold load is shader compile.** FXC takes 0.83 to 1.34 s of a 0.87 to 1.39 s cold load.
- **The shaders decide the backend.** FXC compiles 98 of 98 permutations unmodified. DXC compiles 0 of 98. Community Shaders (CS) itself compiles with FXC on every branch I read.
- **The sources show Linux in use only through Wine or Proton.** Every native-Linux route rests on a link nobody has tested with these shaders.
- **My recommendation is provisional:** keep the D3D11 API and FXC bytecode, and test DXVK underneath them. [The first step](#recommendation-and-first-step) is two tests that could rule that out.

**Not delivered:** the DXC to SPIR-V trial. The Windows SDK's `dxc` lists `-spirv` but has no SPIR-V code generator, so the Vulkan and wgpu compile columns are empty. See [Limits](#limits).

## Why this comes down to the shaders

`render3d` exists to show what the game shows. It does that by compiling CS's own `Lighting.hlsl`, unmodified, with the compiler CS uses: FXC (`D3DCompile` from `d3dcompiler_47.dll`), shader model 5. The result is DXBC bytecode. The preview then binds the same registers the game binds, so a RenderDoc capture of Skyrim and a preview frame can be compared pixel by pixel.

So each path below is judged first by one question: **how does it run FXC's bytecode, and where does FXC come from on Linux?** What it adds (ray tracing, compute, zero-copy display) comes second.

What CS is doing, as of 2026-11-16:
- **CS compiles with FXC, `*_5_0` profiles, on its `dev` branch and on its Vulkan branch.** Its one attempt to compile HLSL to SPIR-V with DXC (PR 2558) was closed unmerged on 2026-07-21. That PR's description says shaders with samplers or groupshared memory had to fall back to FXC bytecode.
- **The "Vulkan" work is DXVK under the game.** Two draft PRs (2632 and 2670) load a CS fork of DXVK before Skyrim creates its D3D11 device. The fork is based on DXVK 3.1.0. The game still issues D3D11 calls; DXVK translates them, and the FXC bytecode with them, to Vulkan. Upscaling and frame generation then use DXVK's Vulkan device directly.
- **Nothing is decided.** Both are drafts, as is the ray tracing PR (2196), no maintainer has stated a timeline, and the Vulkan branch keeps a `CS_NATIVE_D3D11=1` switch back to plain D3D11. A related fix (PR 2662) was closed unmerged on 2026-11-14.
- **D3D12 points both ways.** PR 2632 deletes CS's D3D12 swap-chain bridge. The ray tracing draft (PR 2196) adds a D3D12 header and Streamline's D3D12 build.
- **Ray tracing is a separate plugin with a separate compiler.** [CreationEngineRaytracing](https://github.com/Gistix/CreationEngineRaytracing) (CER) takes a device from CS: either D3D12 handles, or DXVK's Vulkan device. It renders through NVRHI and compiles its own shaders with DXC, to DXIL on D3D12 and to SPIR-V on Vulkan. It needs `VK_KHR_ray_query` or ray tracing pipelines.

## What a frame costs today

The preview draws offscreen on D3D11, copies the frame to the CPU, and the GUI uploads it again as an egui texture. `Renderer::last_timing()` splits a frame into recording the draws (`draw_ms`), waiting for the GPU (`gpu_ms`) and the readback (`readback_ms`).

Hardware GPU, forward rendering, median of 200 frames, in ms. The `static` scene is shown; `skinned` and `pbr` are within 0.2 ms of it up to 1920×1080 and within 2 ms at 4K.

| Size | draw | GPU wait | readback | whole `render()` | readback share |
|---|---:|---:|---:|---:|---:|
| 1280×720 | 0.04 | 0.59 | 2.37 | 3.00 | 79% |
| 1600×900 | 0.04 | 0.59 | 3.62 | 4.27 | 85% |
| 1920×1080 | 0.04 | 0.60 | 5.26 | 5.91 | 89% |
| 3840×2160 | 0.10 | 0.66 | 22.69 | 23.45 | 97% |

- **The GPU wait is flat** at 0.56 to 0.66 ms for every scene and size. The probe polls an event query and sleeps 200 µs between polls, so I read this as the probe's floor, not GPU time. Real GPU time on this card is below it. That is an inference; timestamp queries would settle it.
- **Recording the draws is small:** 0.02 ms for 2 shapes, 0.10 to 0.15 ms for 147.
- **The readback is the frame.** At 4K it reads 66 MB of half floats and converts every channel to 8 bits on the CPU; its p95 was 23 to 34 ms, depending on the scene and the run. Copying the 33 MB result alone takes 0.85 ms, so the bytes are not the cost. The staging copy and the conversion were not timed apart.
- **Not measured:** the second half of the round trip, the GUI's upload of the frame into its own wgpu texture.

Software rasteriser (WARP, the same D3D11 code), whole `render()` median in ms:

| Size | static | skinned | pbr |
|---|---:|---:|---:|
| 1280×720 | 11.8 | 6.4 | 7.4 |
| 1600×900 | 17.0 | 9.3 | 13.7 |
| 1920×1080 | 20.3 | 12.0 | 13.4 |
| 3840×2160 | 72.6 | 40.3 | 43.3 |

On WARP the drawing dominates (53 ms of the 73 ms static 4K frame). The `pbr` scene cost slightly more at 1600×900 than at 1920×1080 in this run; I did not chase why. The meshes cover only 4% to 17% of the frame with the automatic camera, so a close-up costs more.

What this says about the backends:
- **A lower-overhead API has nothing to win here.** D3D12 and Vulkan cut CPU cost per draw; the preview spends 0.02 to 0.15 ms on draws. No other backend was run, so this is an inference from the D3D11 numbers.
- **Zero-copy display is the frame-time lever, and it does not need a new backend.** A D3D11 shared texture can be imported into the GUI's wgpu device: wgpu-hal 30.0.1 has `vulkan::Device::texture_from_d3d11_shared_handle` and `dx12::Device::texture_from_raw`, and egui-wgpu's `register_native_texture` shows the result. That texture must be `Rgba8Unorm`, so the half-to-8-bit step moves to the GPU. I found no project that does this import with wgpu.
- **The old gate still holds at its own size.** The October plan skipped zero-copy unless readback passed 4 ms at 1600×900. It measures 3.6 to 3.7 ms there. It passes 4 ms from 1920×1080 up (4.9 to 5.3 ms).

## What a load costs today

In ms, single shots; a second run agreed within 80 ms on cold loads and 10 ms on warm ones.

| Step | static | skinned | pbr |
|---|---:|---:|---:|
| Cold load, total | 949 | 910 | 1368 |
| of which FXC compile and draw build | 904 | 876 | 1319 |
| of which texture decode and upload | 44 | 33 | 49 |
| Warm load (programs from the disk cache) | 53 | 19 | 86 |
| First frame after a cold load (compiles the env-map compute shader) | 64 | 50 | 43 |

- **A cold load is 95% to 97% FXC** on the hardware GPU. The include-expanded `Lighting.hlsl` every compile parses is 4.6 MB.
- **WARP loads in the same time** (within 40 ms), because the compile is CPU work either way.
- **DXC is about twice as fast on pixel shaders**, but only on rewritten source: see the next section. A faster compiler that produces different bytecode is a different shader, so I don't count this as a win for any path.

## Which compilers take the shaders

98 distinct permutations were compiled: 70 from the golden captures (the define sets the game itself used), 24 from the three scenes in forward and deferred mode, and 4 built-in shaders. That is the set the captures, scenes and built-ins use, not every combination the define table can produce. The CS 2.0 tree from the unmerged `.csm` branch gave the same counts in every row.

| Compiler | Source | Compiles |
|---|---|---:|
| FXC, `*_5_0` | unmodified | 98 of 98 |
| DXC to DXIL, `*_6_0` | unmodified | 0 of 98 |
| DXC to DXIL, `-HV 2018` | unmodified | 2 of 98 (the backdrop shaders) |
| DXC to DXIL, `-HV 2018` | namespaced cbuffers rewritten | 98 of 98 |
| FXC | the same rewritten text | 98 of 98 |
| glslang to SPIR-V | unmodified or rewritten | 2 of 98 |
| DXC to SPIR-V | any | not run: no code generator |

Two things stop DXC:
- **Cbuffers inside namespaces (96 of 98).** CS declares `cbuffer SharedData : register(b5)` inside `namespace SharedData`, and the same for `Permutation`, `Skin` and `FrameBuffer`. FXC scopes the members to the namespace, so `SharedData::MipBias` resolves. DXC makes them global, so the name is not found, and `DirLightColor` then collides with `Lighting.hlsl`'s own. There are 410 `SharedData::` uses in 27 files. No flag I tried changes this.
- **A vector condition in `?:` (97 of 98).** DXC's default language version wants `select()`. `-HV 2018` restores FXC's behaviour.

The rewrite was a scratch experiment on expanded copies, to see whether anything else hides behind the first error. Nothing does. It turned each member `M` of a cbuffer declared inside namespace `N` into a global `N_M`, and added `static const T M = N_M;` inside the namespace right after the cbuffer. The script was not kept. It is not a proposed patch: the DXIL was validated by `dxc` but never run on a device, and its code was not compared with FXC's.

Compile time on the rewritten text, one process per shader, median ms: pixel shaders 123 (DXC) against 254 (FXC), with a worst case of 246 against 991. Vertex shaders are equal at 66 to 67.

glslang fails for a different, structural reason: it does not resolve an unqualified name declared earlier in the same namespace (`PI` in `Math.hlsli`), and CS does that throughout.

## The five paths

### 1. D3D11 as it is

- **Shaders:** FXC bytecode, the compiler and profile CS uses. This is the only path shown to run the CS shaders.
- **Enables:** rasterisation, compute shaders (`cs_5_0`, which the preview already uses for the deferred composite and the env map), GPU skinning, a software fallback (WARP). No ray tracing API.
- **Linux:** not natively. The Rust `windows` crate cannot target Linux. The whole Windows exe can run under Wine or Proton, where DXVK is the D3D11 runtime. Wine's built-in `d3dcompiler_47` is vkd3d's compiler, not FXC; people who run CS on Proton override it with Microsoft's DLL (the reporter of CS PR 3002 did).
- **Display:** zero-copy into the GUI by shared handle, as above.

### 2. D3D11 on DXVK

The same D3D11 calls and the same bytecode, with DXVK as the runtime: Microsoft's D3D11 stays the default on Windows, and DXVK's native build (its meson build names the library `dxvk_d3d11` off Windows) replaces it on Linux. DXVK translates the bytecode to SPIR-V with its own `dxbc-spirv` compiler.

- **Shaders:** unchanged where FXC exists. This is the path the CS drafts put the game itself on. A preview on CS's own fork would run the translator those drafts use; an upstream DXVK release is a different build (the fork's `interop` branch is 64 commits ahead of and 235 behind upstream `master`).
- **Enables:** everything in path 1, plus whatever Vulkan offers beside it. DXVK hands out its Vulkan instance, device and queue (`IDXGIVkInteropDevice`) and the Vulkan image behind a D3D11 texture. CER's Vulkan path does its ray tracing exactly this way.
- **Linux:** native. Two small Rust projects do it: they bind DXVK's headers with bindgen instead of the `windows` crate. The software fallback is Mesa's lavapipe; one of those projects reports rendering headless through it.
- **Open links, all untested here:**
  - **The compiler.** On native Linux `D3DCompile` comes from vkd3d-utils. Its output is not FXC's, and nobody has fed it this source. The namespace scoping that stops DXC and glslang is exactly the risk.
  - **Parity.** The golden replay has never run on DXVK.
  - **Headless.** DXVK-native throws at start-up unless `DXVK_WSI_DRIVER` names a window backend, even for offscreen work. Whether that backend needs a display is not in DXVK's sources.
  - **Display.** wgpu can wrap an existing Vulkan device (`device_from_raw`), which would let the GUI share DXVK's. I found no project doing it.

### 3. D3D12

- **Shaders:** D3D12 pipelines are created from shader model 5 bytecode in practice: SDL's GPU API lists "DXBC SM5_1 shaders for D3D12", and wgpu's DX12 backend can fall back to FXC. I found no Microsoft sentence saying FXC `*_5_0` blobs are accepted unchanged, so treat that as likely, not shown. Bindings move from fixed registers to a root signature.
- **Enables:** ray tracing pipelines and inline ray queries, mesh shaders, and bindless resources. All of those need DXIL from DXC, so the ray tracing shaders would be new code beside the CS raster. Microsoft's downloadable WARP has ray tracing since 1.0.8, which would allow ray tracing tests without a GPU. Jolt Physics has a D3D12 compute backend.
- **Linux:** through Wine or Proton only. vkd3d-proton has ray tracing but calls its native Linux build a development tool. Wine's own vkd3d reports no ray tracing and shader model 6.0 at most.
- **Display:** if the GUI's wgpu runs on DX12, `texture_from_raw` wraps a resource from the same device.

### 4. Vulkan

- **Shaders:** Vulkan takes SPIR-V. From FXC bytecode that means a translator (`dxbc-spirv`, or vkd3d-shader's DXBC to SPIR-V), and binding by that translator's conventions, which is rebuilding what DXVK already is. From source it means DXC with the namespace rewrite, a different compiler from the game's, and a route CS tried and closed. Neither route was measured.
- **Enables:** everything in D3D12's list through Vulkan extensions. Mesa lists ray queries and ray tracing pipelines as done on RADV, ANV and lavapipe, so software ray tracing exists on Linux. Jolt has a Vulkan compute backend. `ash` is the Rust binding.
- **Linux:** native, including the software rasteriser.
- **Display:** wgpu's Vulkan backend can wrap the same device and its images.

### 5. wgpu, on the GUI's device

- **Shaders:** naga has no HLSL input. FXC or DXC output can only be passed through unparsed (`PASSTHROUGH_SHADERS`), with explicit pipeline layouts, on DX12 for HLSL and DXIL and on Vulkan for SPIR-V. So this path inherits path 3's or path 4's shader problem and adds wgpu's binding model on top.
- **Enables:** one device for the GUI and the preview, so display is free. Ray queries are experimental; ray tracing pipelines are Vulkan only and experimental. Binding arrays and compute are available.
- **Linux:** native (Vulkan), with `force_fallback_adapter` for software.

## Physics

The graphics backend decides less here than it seems to:
- **Skyrim's own physics is Havok, inside the game.** A preview cannot run it.
- **HDT-SMP (FSMP) is Bullet on the CPU**, built with multithreading and TBB. It would run beside any backend.
- **Skinning and morphs** need vertex or compute shaders, which D3D11 has.
- **Jolt's GPU hair simulation** needs a D3D12, Vulkan or Metal device. It has no D3D11 backend. Paths 2, 3 and 4 can supply one.
- **PhysX GPU** needs CUDA, not a graphics API.

## Other tools on render3d

| Tool or use | What it needs | Paths that give it |
|---|---|---|
| Texalith's material ball and FaceGen Composer's head preview | CS raster, parity, compute | 1, 2 |
| The same tools on native Linux | the above plus a Linux build | 2, if its open links hold |
| Headless thumbnails or CI renders with no GPU | a software rasteriser | 1 (WARP, Windows), 2 or 4 (lavapipe) |
| A ray-traced reference view, like CER in game | a Vulkan or D3D12 device beside the raster | 2, 3, 4 |
| Cloth or hair preview on the CPU (Bullet) | nothing from the backend | all |
| Hair simulation on the GPU (Jolt) | a D3D12 or Vulkan device | 2, 3, 4 |

## Summary

"Shown" means run here with the CS shaders. "Cited" means a source says so and nothing was run.

| Path | CS shaders | Linux | Adds | Use for |
|---|---|---|---|---|
| 1. D3D11 as it is | shown: compiles 98 of 98; the repo's golden replay test pins its pixels (not re-run for this page) | Wine or Proton only (cited) | nothing new | the parity baseline; today's preview |
| 2. D3D11 on DXVK | untested; same bytecode, the route CS's drafts take | native (cited, third-party) | a Vulkan device beside the raster: ray tracing, compute, lavapipe | the path to test next |
| 3. D3D12 | likely, not shown; needs root signatures | Wine or Proton only (cited) | DXR, DXIL, bindless, software ray tracing in WARP | ray tracing on Windows if path 2 fails |
| 4. Vulkan | needs a translator or DXC plus a source rewrite (unmeasured) | native (cited) | the same as path 2, without D3D11 | new renderers that don't use CS shaders |
| 5. wgpu | passthrough only | native (cited) | one device with the GUI | drawing that never needed CS shaders |

## Recommendation and first step

I'd keep the D3D11 API and FXC bytecode as the contract, because that is what CS compiles on every branch, and it is the one thing measured to work. Among the ways to grow from there, path 2 is the only one that could add native Linux and a Vulkan device without changing what the shaders compile to. It is also where CS's own drafts point. That makes it the path to test, not yet the path to build: it rests on four untested links.

Two tests could rule it out. Both need a download, so both wait for the owner's word:
1. **Golden replay and the timing baseline on DXVK, on Windows.** Put DXVK's `d3d11.dll` and `dxgi.dll` beside the test binaries, then run the existing golden replay (parity) and `render_backend_baseline` (frame and load times on the same scenes). No code changes. Which DXVK is the owner's choice: upstream's release, or a build of CS's fork, which is what the game would run.
2. **A vkd3d row in the compile trial.** Add vkd3d's `D3DCompile` to `tools\r3d-dxc-trial.ps1` and run the 98 permutations. If it rejects namespaced cbuffers, native Linux needs another source of FXC bytecode. I have not checked whether a prebuilt Windows binary of vkd3d exists; a Linux VM is the fallback.

A third download, the Vulkan SDK's `dxc`, fills in the missing SPIR-V column for paths 4 and 5.

If you want frame time back before any of that, the zero-copy display on today's D3D11 is designed and its wgpu calls exist. It pays from 1920×1080 up.

## Method

- **Tests:** `crates/render3d/tests/backend_baseline.rs`, both ignored and gated on owner data. `render_backend_baseline` (needs `SDC_ACHERUS`) prints the timing tables; `dump_permutations` (`SDC_R3D_DUMP`, with `SDC_GOLDEN`) writes each permutation's expanded source and defines.
- **Script:** `tools\r3d-dxc-trial.ps1 -Dump <dir> -Out <dir>` runs each compiler over a dump and writes a CSV and a summary.
- **Build:** `cargo test --release --offline -p render3d -p sdc --test backend_baseline -- --ignored --nocapture --test-threads=1 <test>`.
- **Machine:** RTX 4090, driver 32.0.16.1088; Ryzen 7 9800X3D; Windows 11 build 26200; `fxc` 10.0.26100.7705; `dxc` 1.8.2502.11; glslang 16.5.0 (RenderDoc 1.43's copy).
- **Frames:** 20 warm-up frames, then 200 timed, per size, per scene, on the hardware GPU and on WARP. Forward mode, default CS features, black background, automatic camera. Three runs; the frame tables are the third, which added 1600×900. The first two ran up to 0.2 ms lower at 1280×720, 0.5 ms lower at 1920×1080 and 3 ms lower at 4K on the hardware GPU. Load numbers are the first run.
- **Golden root:** `cs-1.9.0-d2026.10.11.0`, the one `tests/fixtures/golden-baseline.txt` matches.

| Scene | Mesh | Shapes | Triangles | Textures |
|---|---|---:|---:|---:|
| static | `architecture\whiterun\wrinteriors\wrinthousedrygoods01.nif` (vanilla) | 33 | 31,157 | 29 (56 MiB) |
| skinned | `armor\ebony\f\cuirass_1.nif` (vanilla) | 2 | 6,015 | 7 (14 MiB) |
| pbr | `srider\srex\statics\srexmarkarthfrontwall.nif` (Faultier's PBR Windhelm), 14 TruePBR shapes, multilayer parallax | 147 | 41,509 | 46 (78 MiB), 8 missing |

## Limits

- **Only D3D11 ran.** No D3D12, Vulkan, DXVK or wgpu frame was drawn.
- **GPU time is below the probe's resolution** on this card, and the meshes cover 4% to 17% of the frame.
- **Deferred frames were not timed.** Their permutations are in the compile trial.
- **DXC to SPIR-V was not run.** The SDK's `dxc` and seven other copies on the machine report no SPIR-V code generator. The two SPIR-V files that exist (the backdrop shaders, from glslang) parse and validate in naga 30.0.1; that says nothing about `Lighting.hlsl`.
- **Load times are single shots,** and textures came from memory, so disk and BSA time is excluded.
- **Compile times in the trial include process start** (about 25 ms each).
- **The CS 2.0 tree** was compiled with the CS 1.9 define sets and the bundled tree's default features.

## Sources

Read at the commit shown, on 2026-11-16. The Mesa, WARP, PhysX and Vulkan reference pages were read through a page summariser, so what this page says from them is a paraphrase; check the page before relying on a detail.

| What | Where |
|---|---|
| CS compiles with FXC `*_5_0` (`src/ShaderCache.cpp`, `dev` efe34f9ff5 lines 156-158 and 1852-1854; `codex/vulkan-clean` 29726c766e lines 109-111 and 1491) | https://github.com/community-shaders/skyrim-community-shaders |
| CS Vulkan drafts, the closed DXC attempt, the Proton reports | PRs [2632](https://github.com/community-shaders/skyrim-community-shaders/pull/2632), [2670](https://github.com/community-shaders/skyrim-community-shaders/pull/2670), [2196](https://github.com/community-shaders/skyrim-community-shaders/pull/2196), [2558](https://github.com/community-shaders/skyrim-community-shaders/pull/2558), [2662](https://github.com/community-shaders/skyrim-community-shaders/pull/2662), [3002](https://github.com/community-shaders/skyrim-community-shaders/pull/3002) |
| CS's DXVK fork (`interop` a6c294487f: `FORK.md`, `meson.build` version 3.1.0) | https://github.com/community-shaders/dxvk |
| CER: devices, DXC arguments, extensions (55ccb60a12: `src/API.h`, `src/Renderer.cpp`, `src/ShaderUtils.cpp`, `cmake/NVRHI.cmake`) | https://github.com/Gistix/CreationEngineRaytracing |
| DXVK native build, the WSI requirement (`src/wsi/wsi_platform.cpp` line 37), interop interfaces (`src/dxgi/dxgi_interfaces.h`), `dxbc-spirv` since 3.0 (1f1dfc65fc) | https://github.com/doitsujin/dxvk, https://github.com/doitsujin/dxbc-spirv |
| Rust on DXVK-native (small third-party projects) | https://github.com/MrJox/warscape_community_editor, https://github.com/VIRUXE/vespucci |
| The `windows` crate does not target Linux | https://github.com/microsoft/windows-rs/issues/3361, https://github.com/microsoft/windows-rs/issues/3565 |
| vkd3d: `D3DCompile` in vkd3d-utils, shader source and target types, device caps; Wine's `d3dcompiler_47` (wine 9f8c4ae4d3, vkd3d 2.1) | https://github.com/wine-mirror/wine |
| vkd3d-proton: native build, DXR (f2963ee23c) | https://github.com/HansKristian-Work/vkd3d-proton |
| D3D12: DXR libraries, mesh shaders, SM 6.6 heaps | https://github.com/microsoft/DirectX-Specs |
| WARP with ray tracing (release notes, 1.0.8 and later) | https://www.nuget.org/packages/Microsoft.Direct3D.WARP |
| DXC SPIR-V binding flags and unsupported HLSL (`docs/SPIR-V.rst`, e82d6adf3c) | https://github.com/microsoft/DirectXShaderCompiler |
| Mesa: ray tracing extensions per driver, lavapipe | https://docs.mesa3d.org/features.txt |
| wgpu 30.0.1: backends, naga inputs, passthrough, ray tracing flags, `texture_from_d3d11_shared_handle` (`wgpu-hal/src/vulkan/device.rs` line 453), `texture_from_raw`, `device_from_raw` | https://github.com/gfx-rs/wgpu/tree/v30.0.1 |
| egui-wgpu 0.36 `register_native_texture` (`crates/egui-wgpu/src/renderer.rs`) | https://github.com/emilk/egui |
| SDL GPU shader formats (8f706210e8, `include/SDL3/SDL_gpu.h`) | https://github.com/libsdl-org/SDL |
| FSMP builds on Bullet (2215ec3ba0, `vcpkg.json`) | https://github.com/DaymareOn/hdtSMP64 |
| Jolt compute backends and GPU hair (v5.7.0) | https://github.com/jrouwe/JoltPhysics |
| PhysX GPU needs CUDA | https://nvidia-omniverse.github.io/PhysX/physx/5.6.1/docs/GPURigidBodies.html |

Not found, so not claimed: a maintainer statement on CS's timeline or Linux plans; a list of what vkd3d's HLSL compiler lacks for shader model 5; any measurement of D3D11 against D3D12 or Vulkan overhead for scenes this small; any project importing a D3D11 texture into wgpu.

Next: decide on the two downloads in [the first step](#recommendation-and-first-step).
