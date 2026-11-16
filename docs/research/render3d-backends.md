# The 3D preview's graphics backend (2026-11-16)

Whether `render3d`, the crate that draws the 3D preview, should stay on Direct3D 11, and what D3D12, Vulkan, DXVK or wgpu would each make possible: Linux, speed, ray tracing, physics. This page is for whoever decides the preview's next step, and for authors of other tools built on `render3d`. It ranks the paths by what they enable and ignores what a switch would cost.

Measured on one NVIDIA RTX 4090 and AMD Ryzen 7 9800X3D on 2026-11-16, from `main` 40589a43. Sources were fetched the same day and are pinned under [Sources](#sources). No backend code was written. The unchanged renderer was run on DXVK on Windows and under Wine on Linux, and the shaders were fed to vkd3d's compiler on Linux; every other statement about a path other than today's D3D11 is either cited or marked untested.

Short answer:
- **Frame time is not a backend question.** GPU time is under the 0.6 ms or so the probe can resolve, at every size. Reading the frame back to the CPU was 77% to 97% of a frame. Its conversion has since moved to the GPU, which cut the readback from 5.0 to 1.6 ms at 1920×1080 ([the split](#what-a-frame-costs-today)).
- **Cold load is shader compile.** FXC takes 0.83 to 1.34 s of a 0.87 to 1.39 s cold load.
- **The shaders decide the backend.** FXC compiles 98 of 98 permutations unmodified. DXC compiles 0 of 98, to DXIL or to SPIR-V. Community Shaders (CS) itself compiles with FXC on every branch I read.
- **DXVK runs the preview unchanged, on Windows.** With CS's own DXVK build under the same exe, the three scenes' frames match Microsoft's D3D11 (one pixel of 921,600 differs by 1 in one scene), and frame and load times are the same.
- **The golden replay fails on DXVK,** and that was the parity line I set before the run: 123 of 149 captured draws are over its limits. Both causes I found are in how the replay uses the game's captures (8× anisotropic samplers, captured depth), not in the shaders. See [what DXVK did](#what-dxvk-did-on-windows).
- **vkd3d cannot compile the shaders.** vkd3d 2.1's HLSL compiler, the one Wine's `d3dcompiler_47` is built on, stops at the first `namespace` in 96 of 98 permutations. On Linux the bytecode has to come from Microsoft's compiler.
- **The Windows build's renderer tests pass under Wine.** With DXVK and Microsoft's `d3dcompiler_47.dll`, 349 of 352 `render3d` tests pass on a Linux machine with software Vulkan. Two of the three failures need `cargo`; the third is a frame hash pinned on another rasteriser. The GUI needs a font in place of Segoe UI to start. See [what Wine did](#what-wine-did-on-linux).

My recommendation: keep the D3D11 API and FXC bytecode, with DXVK as the runtime to build towards. What is left for Linux is under [Recommendation and next step](#recommendation-and-next-step).

## Why this comes down to the shaders

`render3d` exists to show what the game shows. It does that by compiling CS's own `Lighting.hlsl`, unmodified, with the compiler CS uses: FXC (`D3DCompile` from `d3dcompiler_47.dll`), shader model 5. The result is DXBC bytecode. The preview then binds the same registers the game binds, so a RenderDoc capture of Skyrim and a preview frame can be compared pixel by pixel.

So each path below is judged first by one question: **how does it run FXC's bytecode, and where does FXC come from on Linux?** What it adds (ray tracing, compute, zero-copy display) comes second.

What CS is doing, as of 2026-11-16:
- **CS compiles with FXC, `*_5_0` profiles, on its `dev` branch and on its Vulkan branch.** Its one attempt to compile HLSL to SPIR-V with DXC (PR 2558) was closed unmerged on 2026-07-21. That PR's description says shaders with samplers or groupshared memory had to fall back to FXC bytecode.
- **The "Vulkan" work is DXVK under the game.** Two draft PRs (2632 and 2670) load a CS fork of DXVK before Skyrim creates its D3D11 device. The fork's build file reports version 3.1.0. The game still issues D3D11 calls; DXVK translates them, and the FXC bytecode with them, to Vulkan. Upscaling and frame generation then use DXVK's Vulkan device directly.
- **Nothing is decided.** Both are drafts, as is the ray tracing PR (2196), no maintainer has stated a timeline, and the Vulkan branch keeps a `CS_NATIVE_D3D11=1` switch back to plain D3D11. A related fix (PR 2662) was closed unmerged on 2026-11-14.
- **D3D12 points both ways.** PR 2632 deletes CS's D3D12 swap-chain bridge. The ray tracing draft (PR 2196) adds a D3D12 header and Streamline's D3D12 build.
- **Ray tracing is a separate plugin with a separate compiler.** [CreationEngineRaytracing](https://github.com/Gistix/CreationEngineRaytracing) (CER) takes a device from CS: either D3D12 handles, or DXVK's Vulkan device. It renders through NVRHI and compiles its own shaders with DXC, to DXIL on D3D12 and to SPIR-V on Vulkan. It needs `VK_KHR_ray_query` or ray tracing pipelines.

## What a frame costs today

The preview draws offscreen on D3D11, copies the frame to the CPU, and the GUI uploads it again as an egui texture. `Renderer::last_timing()` splits a frame into recording the draws (`draw_ms`), waiting for the GPU (`gpu_ms`) and the readback (`readback_ms`).

The table and the bullets under it are the state before the conversion moved to the GPU (Note IF); [the table after it](#after-the-gpu-conversion) is the preview today. Hardware GPU, forward rendering, median of 200 frames, in ms. The `static` scene is shown; `skinned` and `pbr` are within 0.2 ms of it up to 1920×1080 and within 2 ms at 4K.

| Size | draw | GPU wait | readback | whole `render()` | readback share |
|---|---:|---:|---:|---:|---:|
| 1280×720 | 0.04 | 0.59 | 2.37 | 3.00 | 79% |
| 1600×900 | 0.04 | 0.59 | 3.62 | 4.27 | 85% |
| 1920×1080 | 0.04 | 0.60 | 5.26 | 5.91 | 89% |
| 3840×2160 | 0.10 | 0.66 | 22.69 | 23.45 | 97% |

- **The GPU wait is flat** at 0.56 to 0.66 ms for every scene and size. The probe polls an event query and sleeps 200 µs between polls, so I read this as the probe's floor, not GPU time. Real GPU time on this card is below it. That is an inference; timestamp queries would settle it.
- **Recording the draws is small:** 0.02 ms for 2 shapes, 0.10 to 0.16 ms for 147.
- **The readback was the frame.** At 4K it read 66 MB of half floats and converted every channel to 8 bits on the CPU; its p95 was 23 to 34 ms, depending on the scene and the run. Copying the 33 MB result alone takes about 1 ms (0.7 to 1.1 over the three runs), so the bytes are not the cost.
- **Three quarters of the readback is the CPU conversion.** A later run timed the two halves apart; see the table below.
- **Not measured:** the second half of the round trip, the GUI's upload of the frame into its own wgpu texture.

The readback split, hardware GPU, `static` scene, median ms (a fourth run, so the totals are a little below the table above). "Copy and `Map`" is the staging copy and the transfer to CPU memory. "Convert" is the half-to-8-bit table lookup, one thread, into a new buffer.

| Size | readback | copy and `Map` | convert | convert share | the same conversion on ordinary memory |
|---|---:|---:|---:|---:|---:|
| 1280×720 | 2.23 | 0.61 | 1.61 | 72% | 1.17 |
| 1600×900 | 3.45 | 0.93 | 2.51 | 73% | 1.81 |
| 1920×1080 | 4.99 | 1.32 | 3.66 | 73% | 2.56 |
| 3840×2160 | 20.11 | 5.11 | 14.98 | 74% | 10.40 |

- **The other two scenes agree** within 0.1 ms up to 1920×1080 and 0.2 ms at 4K.
- **WARP converts in the same time** (1.60, 2.50, 3.60 and 14.8 ms), so the conversion cost is CPU work, not slow GPU memory.
- **The 4K spikes are in the copy, not the conversion:** the p95 of copy and `Map` is 19.9 ms against a 5.1 ms median, on all three scenes.
- **So a smaller fix existed, and it is now in.** The conversion runs on the GPU and the readback copies 8-bit pixels (Note IF). The bytes are identical: a test compares every half value with the old CPU table on both devices. It needs no shared texture; it was not re-run on DXVK.

#### After the GPU conversion

Every table on this page except the next one was measured before that change. After it, same machine and scene, median ms:

| Size | readback before | readback after | whole `render()` before | after |
|---|---:|---:|---:|---:|
| 1280×720 | 2.23 | 0.73 | 2.85 | 1.35 |
| 1600×900 | 3.45 | 1.09 | 4.06 | 1.69 |
| 1920×1080 | 4.99 | 1.57 | 5.63 | 2.18 |
| 3840×2160 | 20.11 | 6.20 | 20.80 | 6.85 |

- **A second run agrees:** readback 0.68, 0.93, 1.41 and 6.60 ms at the committed code.
- **The golden replay still passes** at that commit on Microsoft's D3D11: 0 of 149 over the limit.
- **The 4K spikes are gone:** the readback's p95 is 7.0 ms, from 34.6.
- **WARP gains less:** its readback is 1.33, 2.12, 2.96 and 12.0 ms, from 1.70, 2.62, 3.77 and 17.8, because the conversion pass itself runs on the CPU there.
- **What is left at 4K** is 2.7 ms for the pass, the copy and `Map`, and 3.5 ms to copy the rows into a new buffer.

Software rasteriser (WARP, the same D3D11 code), whole `render()` median in ms:

| Size | static | skinned | pbr |
|---|---:|---:|---:|
| 1280×720 | 11.8 | 6.4 | 7.4 |
| 1600×900 | 17.0 | 9.3 | 13.7 |
| 1920×1080 | 20.3 | 12.0 | 13.4 |
| 3840×2160 | 72.6 | 40.3 | 43.3 |

On WARP the drawing dominates (53 ms of the 73 ms static 4K frame). The `pbr` scene cost slightly more at 1600×900 than at 1920×1080 in this run; I did not chase why. The meshes cover only 4% to 17% of the frame with the automatic camera, so a close-up costs more.

What this says about the backends:
- **A lower-overhead API has nothing to win here.** D3D12 and Vulkan cut CPU cost per draw; the preview spends 0.02 to 0.16 ms on draws. No other backend was run, so this is an inference from the D3D11 numbers.
- **Zero-copy display is the frame-time lever, and it does not need a new backend.** A D3D11 shared texture can be imported into the GUI's wgpu device: wgpu-hal 30.0.1 has `vulkan::Device::texture_from_d3d11_shared_handle` and `dx12::Device::texture_from_raw`, and egui-wgpu's `register_native_texture` shows the result. That texture must be `Rgba8Unorm`, so the half-to-8-bit step moves to the GPU. I found no project that does this import with wgpu.
- **The October gate now holds everywhere but 4K.** That plan skipped zero-copy unless readback passed 4 ms at 1600×900. Before the GPU conversion it measured 3.6 to 3.7 ms there and passed 4 ms from 1920×1080 up. After it, readback is 1.1 ms at 1600×900 and 1.6 ms at 1920×1080, and passes 4 ms only at 3840×2160 (6.2 ms).

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

98 distinct permutations were compiled: 70 from the golden captures (the define sets the game itself used), 24 from the three scenes in forward and deferred mode, and 4 built-in shaders. That is the set the captures, scenes and built-ins use, not every combination the define table can produce. The CS 2.0 tree from the unmerged `.csm` branch gave the same counts in every row but vkd3d's, which ran on the bundled tree only.

| Compiler | Source | Compiles |
|---|---|---:|
| FXC, `*_5_0` | unmodified | 98 of 98 |
| DXC to DXIL, `*_6_0` | unmodified | 0 of 98 |
| DXC to DXIL, `-HV 2018` | unmodified | 2 of 98 (the backdrop shaders) |
| DXC to SPIR-V, with or without `-HV 2018` | unmodified | the same: 0, then 2 of 98 |
| DXC to DXIL, `-HV 2018` | namespaced cbuffers rewritten | 98 of 98 |
| DXC to SPIR-V, `-HV 2018` | namespaced cbuffers rewritten | 98 of 98 |
| naga 30.0.1 reading that SPIR-V | namespaced cbuffers rewritten | 98 parse, 41 validate |
| FXC | the same rewritten text | 98 of 98 |
| glslang to SPIR-V | unmodified or rewritten | 2 of 98 |
| vkd3d 2.1 to DXBC or to SPIR-V, `*_5_0` | unmodified | 2 of 98 (the backdrop shaders) |

DXC is 1.9.2609 from Microsoft's release, because the Windows SDK's 1.8 has no SPIR-V code generator. The SDK's 1.8 gave the same DXIL counts.

Two things stop DXC:
- **Cbuffers inside namespaces (96 of 98).** CS declares `cbuffer SharedData : register(b5)` inside `namespace SharedData`, and the same for `Permutation`, `Skin` and `FrameBuffer`. FXC scopes the members to the namespace, so `SharedData::MipBias` resolves. DXC makes them global, so the name is not found, and `DirLightColor` then collides with `Lighting.hlsl`'s own. There are 410 `SharedData::` uses in 27 files. No flag I tried changes this.
- **A vector condition in `?:` (97 of 98).** DXC's default language version wants `select()`. `-HV 2018` restores FXC's behaviour.

The rewrite was a scratch experiment on expanded copies, to see whether anything else hides behind the first error. Nothing does. It turned each member `M` of a cbuffer declared inside namespace `N` into a global `N_M`, and added `static const T M = N_M;` inside the namespace right after the cbuffer. The script is `tools\r3d-ns-rewrite.py`. It is not a proposed patch: the DXIL and SPIR-V were validated by `dxc` but never run on a device, and their code was not compared with FXC's.

The SPIR-V that compiles is still not ready for wgpu. naga rejects 57 of the 98 files at its first check: all 55 `Lighting.hlsl` pixel shaders and 2 of the 3 deferred composite compute shaders. The error is a uniform alignment rule: inside the `FeatureData` cbuffer a struct member sits at offset 4 where naga requires a multiple of 16. I compiled with HLSL's own packing (`-fvk-use-dx-layout`) so the cbuffer bytes the renderer already builds would still fit, and suspected that flag. It is not the flag: without it the same 57 fail with the same error. I did not find which member it is, or look past that first error.

Compile time on the rewritten text, one process per shader, median ms: pixel shaders 125 (DXC to DXIL) and 155 (DXC to SPIR-V) against 293 (FXC). Vertex shaders are 60 to 74 on all three. Other tests were running at the same time, so read these as rough.

glslang fails for a different, structural reason: it does not resolve an unqualified name declared earlier in the same namespace (`PI` in `Math.hlsli`), and CS does that throughout.

vkd3d fails before any of that: it has no namespaces. This is the compiler behind Wine's built-in `d3dcompiler_47`, run here as `vkd3d-compiler` 2.1 on Linux. All 96 failures are one error at the first `namespace` in the file, `E5000: syntax error, unexpected KW_NAMESPACE`, whether the target is DXBC or SPIR-V. The two that compile are the backdrop shaders, which use no namespace.
- **It is the keyword, not CS's code.** A two-line shader with one function inside a `namespace` fails with the same error, and compiles once the function is moved out.
- **It is the same in Wine's current source.** The lexer returns `KW_NAMESPACE` and the grammar declares the token, but no rule uses it.
- **What else vkd3d would reject in the CS shaders is unknown.** The parser stops at that first error. The cbuffer rewrite above keeps the namespaces, so it does not help here, and I did not write one that removes them. The Wine run found [two more gaps](#what-wine-did-on-linux) in shaders that have no namespace.

## The five paths

### 1. D3D11 as it is

- **Shaders:** FXC bytecode, the compiler and profile CS uses. This is the only path shown to run the CS shaders.
- **Enables:** rasterisation, compute shaders (`cs_5_0`, which the preview already uses for the deferred composite and the env map), GPU skinning, a software fallback (WARP). No ray tracing API.
- **Linux:** not natively. The Rust `windows` crate cannot target Linux. The whole Windows exe runs under Wine with DXVK as the D3D11 runtime ([measured](#what-wine-did-on-linux)). Wine's built-in `d3dcompiler_47` is vkd3d's compiler, not FXC, and [it does not take these shaders](#which-compilers-take-the-shaders); people who run CS on Proton override it with Microsoft's DLL (the reporter of CS PR 3002 did).
- **Display:** zero-copy into the GUI by shared handle, as above.

### 2. D3D11 on DXVK

The same D3D11 calls and the same bytecode, with DXVK as the runtime: Microsoft's D3D11 stays the default on Windows, and DXVK's native build (its meson build names the library `dxvk_d3d11` off Windows) replaces it on Linux. DXVK translates the bytecode to SPIR-V with its own `dxbc-spirv` compiler.

- **Shaders:** unchanged where FXC exists. This is the path the CS drafts put the game itself on. I ran the DXVK build CS's draft pins; an upstream DXVK release is a different build (the fork's `interop` branch is 64 commits ahead of and 235 behind upstream `master`).
- **Enables:** everything in path 1, plus whatever Vulkan offers beside it. DXVK hands out its Vulkan instance, device and queue (`IDXGIVkInteropDevice`) and the Vulkan image behind a D3D11 texture. CER's Vulkan path does its ray tracing exactly this way.
- **Linux:** native. Two small Rust projects do it: they bind DXVK's headers with bindgen instead of the `windows` crate. The software fallback is Mesa's lavapipe; one of those projects reports rendering headless through it.

Open links for a native Linux build. Of these only the compiler was tested on Linux; the software fallback was seen on Windows:
- **The compiler.** On native Linux `D3DCompile` comes from vkd3d-utils, and vkd3d 2.1 [rejects 96 of the 98 permutations](#which-compilers-take-the-shaders). The bytecode has to be FXC's, compiled somewhere else.
- **Headless.** DXVK-native throws at start-up unless `DXVK_WSI_DRIVER` names a window backend, even for offscreen work. Whether that backend needs a display is not in DXVK's sources.
- **Software fallback.** DXVK has none of its own: asked for WARP, it logs "Unsupported driver type 5" and returns the hardware GPU (measured). lavapipe would have to be chosen on the Vulkan side.
- **Display.** wgpu can wrap an existing Vulkan device (`device_from_raw`), which would let the GUI share DXVK's. I found no project doing it.

#### What DXVK did on Windows

CS's DXVK build sat beside the unchanged test exes, so the same Rust code and the same FXC bytecode ran on Vulkan. DXVK's log confirms it created the device on the RTX 4090 for every run.

The pass and fail lines were written down before any DXVK output was read:

| Line | Pass means | Result |
|---|---|---|
| Loaded | DXVK's log names the Vulkan adapter | pass |
| Parity | every golden replay case that passes on Microsoft's D3D11 passes on DXVK, test unchanged | **fail**: 123 of 149 over the limit |
| Frame time | DXVK's median is at most 1.5× Microsoft's in the same session | pass: at most 1.07× |
| Warm load | at most 2× | pass |
| Cold load, first frame | recorded, no verdict | recorded |

The comparison of the preview's own frames was not one of those lines, so it carries no verdict set in advance.

Frames at 1280×720, hardware GPU, 8-bit output, compared byte by byte:

| Scene | Microsoft D3D11, two runs | Microsoft D3D11 against DXVK | Hardware against WARP, for scale |
|---|---|---|---|
| static | identical | 1 pixel of 921,600 differs, by 1 | 68,885 pixels differ, most by 1 |
| skinned | identical | identical | 13,492 pixels differ |
| pbr | identical | identical | 14,108 pixels differ |

Whole `render()` median in ms, three runs back to back: Microsoft's D3D11, DXVK, Microsoft's D3D11 again.

| Size | static | skinned | pbr |
|---|---|---|---|
| 1280×720 | 2.82, **2.83**, 2.78 | 2.77, **2.80**, 2.80 | 2.90, **2.88**, 2.87 |
| 1600×900 | 4.13, **3.99**, 3.97 | 3.97, **4.01**, 3.99 | 4.07, **4.24**, 4.01 |
| 1920×1080 | 5.62, **5.52**, 5.44 | 5.44, **5.55**, 5.42 | 5.67, **5.86**, 5.49 |
| 3840×2160 | 20.43, **20.79**, 20.28 | 20.22, **20.53**, 20.14 | 20.33, **21.26**, 20.64 |

- **Frames cost the same.** DXVK is within 0.2 ms of the nearer Microsoft run up to 1920×1080 and within 0.7 ms at 4K. The readback dominates on both.
- **Loads cost the same.** Cold load 868 to 1436 ms on DXVK against 910 to 1416 ms; warm load 36 to 95 ms against 38 to 134 ms. The compile is FXC either way.
- **No first-frame stall.** The first frame after a cold load took 40 to 56 ms on DXVK against 39 to 61 ms. I expected DXVK to pay for building its pipelines there; it did not show. Neither DXVK's nor the driver's shader cache was cleared first, so this is not a cold-cache figure.
- **The test suite: 345 of 348 pass** (348 of 348 on Microsoft's D3D11). The three failures are exact-value pins: two composite tests off by one half-float step (0.7495 for 0.75), and one frame hash. All three ask for WARP and their values were pinned there. DXVK gave them the RTX 4090 instead, so each failure mixes two changes, the runtime and the rasteriser. I did not separate them.

The golden replay does not pass on DXVK, and the reason matters more than the count. It replays 149 draws captured from the game and compares all eight render targets with the capture. Its limit is a small tolerance per pixel format plus the noise measured between captures of two neighbouring frames, and no pixel may be over it. On Microsoft's D3D11 none is. On DXVK 123 of the 149 draws have pixels over it. Two causes, each confirmed by changing one thing on a few subjects:
- **8× anisotropic sampling differs between the two runtimes.** The captures use the game's 8× anisotropic samplers. With those samplers three of the eight targets differ, by 0.0005 to 0.007 on average and by up to 0.15. With them swapped for trilinear, three subjects are bit-identical on all eight targets, bar one value in one of them.
- **A captured depth buffer does not match DXVK's depth exactly.** 76 subjects test `Equal` against the depth the game's prepass wrote. On DXVK 61 of them come out at least 90% wrong, and the one I opened had drawn nothing at all. With `LessEqual` instead, two subjects match Microsoft's D3D11 except for at most 9 values by one step. I did not separate DXVK's rasterised depth from the copy that seeds the captured depth.

Neither touches the preview: it samples trilinear and tests against its own depth, which is why its frames match. What it does mean:
- **The replay cannot gate DXVK as it stands.** Its limits assume the runtime the captures came from.
- **The translation held up where I probed it.** With sampling and depth taken out, the game's deferred `Lighting.hlsl` through DXVK's `dxbc-spirv` is identical to Microsoft's D3D11 on three subjects and within one step on two. That is five subjects of 149, so the other 144 failures are explained by pattern, not checked one by one.
- **If the game moves to DXVK, captures should too.** Captures taken on Microsoft's D3D11 would no longer be what players see wherever anisotropic filtering shows.

#### What Wine did on Linux

The same Windows test exes, built on Windows and not changed, ran under Wine 11.0 on a Linux virtual machine. The machine has no GPU, so Vulkan was Mesa's software driver (llvmpipe). This shows what runs, not how fast it is. The suite has 352 tests here and 348 in the DXVK run on Windows, because the GPU readback conversion added four in between.

| D3D11 runtime | `d3dcompiler_47` | `render3d` tests passing |
|---|---|---:|
| Microsoft's, on Windows (the baseline) | Microsoft's | 352 of 352 |
| DXVK 3.1.1 under Wine | Microsoft's | 349 of 352 |
| DXVK 3.1.1 under Wine | Wine's own (vkd3d) | 320 of 352 |
| Wine's own (wined3d) | Microsoft's | 317 of 352 |

- **DXVK with Microsoft's compiler passes all but three tests.** The passing ones cover the forward and deferred paths, the compute passes and the GPU readback conversion. Two failures are tests that call `cargo`, which that machine does not have. The third compares a frame hash that was pinned on WARP: the frame's bytes differ here, and I did not look at how. Both the D3D11 runtime and the rasteriser differ from the pinned run, so it says nothing about either alone ([the same case on Windows](#what-dxvk-did-on-windows)).
- **Wine's own compiler fails wherever CS source is compiled.** This is `D3DCompile` through Wine's DLL, the route [the compile trial](#which-compilers-take-the-shaders) did not cover. 30 more tests fail: 22 report the same `namespace` error, 4 report two further gaps in small test and built-in shaders (`GetDimensions` on a `RWTexture2DArray`, and stores to a structured buffer), and 4 fail an assertion that I read as following from a shader that did not compile.
- **Wine's own D3D11 ran the forward path only, in this run.** With wined3d the deferred preview stops with "this GPU cannot read R16G16B16A16_FLOAT through a UAV", which the composite needs. 32 of its 33 extra failures are that message; the other is the WARP hash. This was on the software driver only, and I did not check which of wined3d's renderers was in use, so a GPU driver may differ.
- **Tessalith starts, once it has a font.** `tessalith.exe` stops at start-up with "cannot load the system font C:\windows\Fonts\segoeui.ttf": the GUI requires Segoe UI, and Wine has none. With another font copied to that name, the main window draws. I did not check which wgpu backend drew it, and I did not open a project or a 3D preview in it.

So on Linux through Wine the unchanged Windows build passes its renderer tests and opens its window when three things are there: DXVK; Microsoft's `d3dcompiler_47.dll`, which the user has to add; and a font where the GUI looks for Segoe UI. I take Proton to ship DXVK as its default D3D11 runtime, but did not check that for this page.

### 3. D3D12

- **Shaders:** D3D12 pipelines are created from shader model 5 bytecode in practice: SDL's GPU API lists "DXBC SM5_1 shaders for D3D12", and wgpu's DX12 backend can fall back to FXC. I found no Microsoft sentence saying FXC `*_5_0` blobs are accepted unchanged, so treat that as likely, not shown. Bindings move from fixed registers to a root signature.
- **Enables:** ray tracing pipelines and inline ray queries, mesh shaders, and bindless resources. All of those need DXIL from DXC, so the ray tracing shaders would be new code beside the CS raster. Microsoft's downloadable WARP has ray tracing since 1.0.8, which would allow ray tracing tests without a GPU. Jolt Physics has a D3D12 compute backend.
- **Linux:** through Wine or Proton only. vkd3d-proton has ray tracing but calls its native Linux build a development tool. Wine's own vkd3d reports no ray tracing and shader model 6.0 at most.
- **Display:** if the GUI's wgpu runs on DX12, `texture_from_raw` wraps a resource from the same device.

### 4. Vulkan

- **Shaders:** Vulkan takes SPIR-V. From FXC bytecode that means a translator (`dxbc-spirv`, or vkd3d-shader's DXBC to SPIR-V), and binding by that translator's conventions, which is rebuilding what DXVK already is. From source it means DXC with the namespace rewrite, a different compiler from the game's, and a route CS tried and closed. The source route compiles (98 of 98 after the rewrite) but was never run on a device; the translator route is what path 2 measured.
- **Enables:** everything in D3D12's list through Vulkan extensions. Mesa lists ray queries and ray tracing pipelines as done on RADV, ANV and lavapipe, so software ray tracing exists on Linux. Jolt has a Vulkan compute backend. `ash` is the Rust binding.
- **Linux:** native, including the software rasteriser.
- **Display:** wgpu's Vulkan backend can wrap the same device and its images.

### 5. wgpu, on the GUI's device

- **Shaders:** naga has no HLSL input. FXC or DXC output can only be passed through unparsed (`PASSTHROUGH_SHADERS`), with explicit pipeline layouts, on DX12 for HLSL and DXIL and on Vulkan for SPIR-V. So this path inherits path 3's or path 4's shader problem and adds wgpu's binding model on top. Its own checks already object: naga validates only 41 of the 98 SPIR-V files DXC produced, stopping on a cbuffer alignment rule.
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
| 1. D3D11 as it is | shown: compiles 98 of 98; the repo's golden replay test pins its pixels (re-run for this page: 0 of 149 failing) | under Wine, with DXVK and Microsoft's compiler (shown on software Vulkan) | nothing new | the parity baseline; today's preview |
| 2. D3D11 on DXVK | shown on Windows: same bytecode, same preview frames, same speed; the golden replay fails on captured samplers and depth; the route CS's drafts take | native (cited, third-party); vkd3d rejects the shaders, so the bytecode must be FXC's | a Vulkan device beside the raster: ray tracing, compute, lavapipe | the path to build towards |
| 3. D3D12 | likely, not shown; needs root signatures | Wine or Proton only (cited) | DXR, DXIL, bindless, software ray tracing in WARP | ray tracing on Windows if path 2's Linux links fail |
| 4. Vulkan | a translator (which is path 2), or DXC plus a source rewrite: compiles 98 of 98, never run | native (cited) | the same as path 2, without D3D11 | new renderers that don't use CS shaders |
| 5. wgpu | passthrough only; naga validates 41 of 98 | native (cited) | one device with the GUI | drawing that never needed CS shaders |

## Recommendation and next step

I'd keep the D3D11 API and FXC bytecode as the contract, because that is what CS compiles on every branch, and it is what the measurements support. Among the ways to grow from there, path 2 is the only one that adds native Linux and a Vulkan device without changing what the shaders compile to, and it is where CS's own drafts point. Its first test gave a split result. On Windows the unchanged renderer gives the same frames at the same speed on DXVK. The golden replay failed the parity line I set for it. I read that failure as a limit of the replay, because both causes I isolated are in the captured samplers and depth, and the five draws I probed match once those are taken out. That reading rests on five draws of 149. If you'd rather hold path 2 to the line as written, the test to add first is a replay that passes on DXVK: its own captures, taken with DXVK under the game.

What is left is Linux, in this order:
1. **Run the Windows build under Wine.** This is the route I'd take first, because [the renderer's tests already pass there](#what-wine-did-on-linux) with no backend code. Skyrim has no Linux build, so I take Wine or Proton to be how Linux players already run the game and its tools; I did not look for a source. It keeps Microsoft's compiler, so any CS shader tree still compiles. What it needs from Tessalith is small: a fallback when Segoe UI is missing, and a line in the docs naming DXVK and `d3dcompiler_47.dll`. It still wants a run on a real GPU, with a project open.
2. **A native Linux build of the renderer on DXVK-native,** only if Wine turns out not to be enough. vkd3d cannot be its compiler, so its bytecode would be compiled on Windows and shipped or cached; the preview's disk cache already stores it per permutation, but a CS tree the user installs later could not be compiled. This is also where the `windows` crate, `DXVK_WSI_DRIVER` and lavapipe get answered. It is backend code, so it is outside this page.

The frame-time fix that needed no backend is done: the readback converts on the GPU and takes 1.6 ms at 1920×1080 and 6.2 ms at 4K. The zero-copy display removes the rest, but it is Windows-only work with a fallback path, and it pays only if that is still too much.

## Method

- **Tests:** `crates/render3d/tests/backend_baseline.rs`, both ignored and gated on owner data. `render_backend_baseline` (needs `SDC_ACHERUS`) prints the timing tables; `dump_permutations` (`SDC_R3D_DUMP`, with `SDC_GOLDEN`) writes each permutation's expanded source and defines.
- **Scripts:** `tools\r3d-dxc-trial.ps1 -Dump <dir> -Out <dir> [-Dxc <dxc.exe>]` runs each compiler over a dump and writes a CSV and a summary. `tools\r3d-ns-rewrite.py <dump> <out>` writes the rewritten copies.
- **Build:** `cargo test --release --offline -p render3d -p sdc --test backend_baseline -- --ignored --nocapture --test-threads=1 <test>`.
- **Machine:** RTX 4090, driver 32.0.16.1088; Ryzen 7 9800X3D; Windows 11 build 26200; `fxc` 10.0.26100.7705; `dxc` 1.8.2502.11 (Windows SDK) and 1.9.2609.5 (Microsoft's release `dxc_2026_09_29.zip`); glslang 16.5.0 (RenderDoc 1.43's copy).
- **Frames:** 20 warm-up frames, then 200 timed, per size, per scene, on the hardware GPU and on WARP. Forward mode, default CS features, black background, automatic camera. Three runs; the frame tables are the third, which added 1600×900. The first two ran up to 0.2 ms lower at 1280×720, 0.5 ms lower at 1920×1080 and 3 ms lower at 4K on the hardware GPU. Load numbers are the first run.
- **Readback split:** `FrameTiming::map_ms` is the time until `Map` returns. In the split table the rest of `readback_ms` was the CPU conversion; since Note IF the conversion pass is inside `map_ms` and the rest is the rows copied out. The test also times the old table loop on an ordinary buffer.
- **Golden root:** `cs-1.9.0-d2026.10.11.0`, the one `tests/fixtures/golden-baseline.txt` matches.
- **DXVK:** CS's fork at b926a24c, the commit PR 2632 pins as `extern/dxvk`; the build is the fork's own Actions artifact `dxvk-interop-b926a24c…` (DXVK 3.1.0, x64, gcc 16.2). The fork names its files `dxvk_d3d11.dll` and `dxvk_dxgi.dll` and CS loads them by path. For these runs they were copied beside copies of the test exes as `d3d11.dll` and `dxgi.dll`, with `dxvk_dxgi.dll` kept too because the first imports it by name. Only `DXVK_LOG_PATH` and `DXVK_LOG_LEVEL` were set; CS also sets `DXVK_HDR` and `DXVK_DEBUG`, which these runs did not.
- **File identities (SHA-256):** `dxvk_d3d11.dll`, 7,495,694 bytes, `9B44DA108260B27360DB0C0294324583B5E7CAA2F427EFEE3695A0D528FEE421`. `dxvk_dxgi.dll`, 5,427,214 bytes, `795736BDE0267ED9649E7A7E3D37C2C9CAB571489C09CDDEF46E5CEEF3227747`. `dxc_2026_09_29.zip`, 32,287,592 bytes, `AD31B1FC8443175D204F77A611FDB3EF2EC42759BDC2F1167368DE24A4A7E7F1`. An Actions artifact expires, so the hashes are what identifies the DXVK build later.
- **vkd3d:** `tools\r3d-vkd3d-trial.py --dump <dir> --out <dir> --cmd "<vkd3d-compiler>"` ran `vkd3d-compiler -x hlsl -p <target> -e <entry> -b dxbc-tpf` and `-b spirv-binary` over a dump written at `main` d9131939 (the same 98 permutations). The tool has no `-D`, so each define went in as a `#define` line ahead of the source. The compiler is Debian testing's `vkd3d-compiler` 2.1+ds-1 (58,396 bytes, SHA-256 `be68e5acace335a6da4f7f95bd89381e0f1926901899f8e14be95f3b29406f1e`) with `libvkd3d-shader1` 2.1+ds-1 (636,872 bytes, SHA-256 `af2260d53778c2e86c104d8d31d5c20feaa34c795ddc28ff1467cc6d0d0041f3`), in a Debian testing root built with debootstrap 1.0.145 on an Ubuntu 22.04 virtual machine.
- **Wine:** Wine 11.0 from WineHQ's repository (`wine-stable` 11.0.0.0~forky-1; Debian's own Wine package could not be installed that day), DXVK from Debian unstable (`dxvk-wine64` 3.1.1+ds-2; `d3d11.dll` SHA-256 `820be3bf29f6fdd481cad4b5da9fdf8109f12174dca4ad72a32d19283e02077e`, `dxgi.dll` `3f1060ec9f0bd10966f1f5690b5ea4756f18bfc456ffdb6c50db2501e0fc8969`), Mesa 26.1.6 llvmpipe, an Xvfb display, all in the same Debian testing root. Microsoft's `d3dcompiler_47.dll` is this PC's System32 copy (10.0.26100.9974, 4,669,440 bytes, SHA-256 `81494ADC53AAE6A9B2CDA729DA5D20B0D15BFFCB663DFF7AE0242CAB2E1F1A58`). The three DLLs were placed in the prefix's `system32` and chosen per run with `WINEDLLOVERRIDES`. The 44 test exes of `render3d` are a debug build of `main` 1382b7d9, run one at a time with `--test-threads=1`; the source tree was mapped as drive `F:` so the paths compiled into the tests resolve. The same exes gave the Windows baseline. The scripts are not in the repo.
- **naga:** a scratch program on naga 30.0.1 from the local cargo registry parsed and validated each `.spv` with every capability on. It is not in the repo.
- **DXVK frames:** `SDC_R3D_FRAME_OUT=<dir>` makes `render_backend_baseline` write each scene's frame as raw RGBA. The DXVK timing run used a 10 s WARP budget; its "WARP" rows are the hardware GPU.
- **Replay probes:** a throwaway test replayed five golden subjects on both runtimes, with the samplers or the depth test changed as described. It is not in the repo.

| Scene | Mesh | Shapes | Triangles | Textures |
|---|---|---:|---:|---:|
| static | `architecture\whiterun\wrinteriors\wrinthousedrygoods01.nif` (vanilla) | 33 | 31,157 | 29 (56 MiB) |
| skinned | `armor\ebony\f\cuirass_1.nif` (vanilla) | 2 | 6,015 | 7 (14 MiB) |
| pbr | `srider\srex\statics\srexmarkarthfrontwall.nif` (Faultier's PBR Windhelm), 14 TruePBR shapes, multilayer parallax | 147 | 41,509 | 46 (78 MiB), 8 missing |

## Limits

- **Only the D3D11 API ran,** on Microsoft's runtime and on DXVK. No D3D12, native Vulkan or wgpu frame was drawn.
- **DXVK ran on Windows on one GPU and one driver, and on Linux on software Vulkan only.** Nothing ran on a Linux GPU driver, on AMD or Intel, or under Proton.
- **The Wine runs are pass counts.** No frame from Linux was compared with a Windows frame beyond what the tests assert, and nothing was timed. The DXVK there is upstream 3.1.1 as Debian builds it, not CS's fork. The GUI was only started.
- **vkd3d ran as its command-line compiler (2.1) and as Wine 11.0's `d3dcompiler_47`,** not as `D3DCompile` through vkd3d-utils, which I take to share the same HLSL parser (`vkd3d-shader`). Which vkd3d release Wine 11.0 bundles was not checked: 2.1 is the command-line compiler's version and the one in Wine's `master`. I read Wine's bundled copy of the grammar, not vkd3d's own repository, which may be ahead of release 2.1.
- **DXVK frames were compared in forward mode at one size.** The deferred path was compared only through the five replay probes.
- **GPU time is below the probe's resolution** on this card, and the meshes cover 4% to 17% of the frame.
- **Deferred frames were not timed.** Their permutations are in the compile trial.

Also:
- **No DXIL or SPIR-V from DXC was run on a device.** Compiling and validating is as far as it went.
- **Load times are single shots,** and textures came from memory, so disk and BSA time is excluded.
- **Compile times in the trial include process start** (about 25 ms each).
- **The CS 2.0 tree** was compiled with the CS 1.9 define sets and the bundled tree's default features.

## Sources

Read at the commit shown, on 2026-11-16. The Mesa, WARP, PhysX and Vulkan reference pages were read through a page summariser, so what this page says from them is a paraphrase; check the page before relying on a detail.

| What | Where |
|---|---|
| CS compiles with FXC `*_5_0` (`src/ShaderCache.cpp`, `dev` efe34f9ff5 lines 156-158 and 1852-1854; `codex/vulkan-clean` 29726c766e lines 109-111 and 1491) | https://github.com/community-shaders/skyrim-community-shaders |
| CS Vulkan drafts, the closed DXC attempt, the Proton reports | PRs [2632](https://github.com/community-shaders/skyrim-community-shaders/pull/2632), [2670](https://github.com/community-shaders/skyrim-community-shaders/pull/2670), [2196](https://github.com/community-shaders/skyrim-community-shaders/pull/2196), [2558](https://github.com/community-shaders/skyrim-community-shaders/pull/2558), [2662](https://github.com/community-shaders/skyrim-community-shaders/pull/2662), [3002](https://github.com/community-shaders/skyrim-community-shaders/pull/3002) |
| CS's DXVK fork (`interop` a6c294487f: `FORK.md`, `meson.build` version 3.1.0; the build that was run: b926a24c, Actions run 34335226070) | https://github.com/community-shaders/dxvk |
| How CS loads DXVK (`src/DxvkLoader.cpp`, `codex/vulkan-clean` 29726c766e) | https://github.com/community-shaders/skyrim-community-shaders |
| DXC 1.9.2609 with SPIR-V (`dxc_2026_09_29.zip`) | https://github.com/microsoft/DirectXShaderCompiler/releases/tag/v1.9.2609 |
| CER: devices, DXC arguments, extensions (55ccb60a12: `src/API.h`, `src/Renderer.cpp`, `src/ShaderUtils.cpp`, `cmake/NVRHI.cmake`) | https://github.com/Gistix/CreationEngineRaytracing |
| DXVK native build, the WSI requirement (`src/wsi/wsi_platform.cpp` line 37), interop interfaces (`src/dxgi/dxgi_interfaces.h`), `dxbc-spirv` since 3.0 (1f1dfc65fc) | https://github.com/doitsujin/dxvk, https://github.com/doitsujin/dxbc-spirv |
| Rust on DXVK-native (small third-party projects) | https://github.com/MrJox/warscape_community_editor, https://github.com/VIRUXE/vespucci |
| The `windows` crate does not target Linux | https://github.com/microsoft/windows-rs/issues/3361, https://github.com/microsoft/windows-rs/issues/3565 |
| vkd3d: `D3DCompile` in vkd3d-utils, shader source and target types, device caps; Wine's `d3dcompiler_47` (wine 9f8c4ae4d3, which bundles vkd3d 2.1; not the Wine 11.0 that was run) | https://github.com/wine-mirror/wine |
| vkd3d's HLSL grammar has no namespace rule (`libs/vkd3d/libs/vkd3d-shader/hlsl.y` line 7117 and `hlsl.l` line 116, Wine `master` on 2026-11-16; `hlsl.y` last changed in 9226b10cd1, the import of vkd3d 2.1) | https://github.com/wine-mirror/wine |
| Debian testing's `vkd3d-compiler` 2.1+ds-1 | https://packages.debian.org/forky/vkd3d-compiler |
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

Next: give the GUI a fallback for a missing Segoe UI, then run Tessalith under Wine on a Linux machine with a GPU ([the next step](#recommendation-and-next-step)).
