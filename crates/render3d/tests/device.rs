use render3d::device::{Gpu, Target};

#[test]
fn clear_and_readback() {
    let gpu = Gpu::new().unwrap();
    eprintln!("adapter: {} (warp: {})", gpu.adapter, gpu.warp);
    assert!(!gpu.is_removed());
    let t = Target::new(&gpu, 4, 2).unwrap();
    t.clear(&gpu, [1.0, 0.5, 0.0, 1.0]);
    let px = t.readback_display(&gpu, false).unwrap();
    assert_eq!(px.len(), 4 * 2 * 4);
    assert_eq!(&px[0..4], &[255, 128, 0, 255]);
}

#[test]
fn warp_fallback_renders() {
    let gpu = render3d::device::Gpu::new_warp().unwrap();
    assert!(gpu.warp);
    let t = Target::new(&gpu, 2, 2).unwrap();
    t.clear(&gpu, [0.0, 1.0, 0.0, 1.0]);
    assert_eq!(
        &t.readback_display(&gpu, false).unwrap()[0..4],
        &[0, 255, 0, 255]
    );
}

/// The display readback is opaque whatever alpha the target holds.
#[test]
fn display_readback_is_opaque() {
    let gpu = Gpu::new_warp().unwrap();
    let t = Target::new(&gpu, 2, 1).unwrap();
    t.clear(&gpu, [0.25, 0.0, 1.0, 0.5]);
    assert_eq!(
        t.readback_display(&gpu, false).unwrap(),
        [64, 0, 255, 255].repeat(2)
    );
    assert_eq!(t.readback_display(&gpu, true).unwrap()[3], 255);
}

/// The GPU conversion gives the bytes the CPU table gives, for every half bit pattern in every colour channel, on
/// both devices, forward and deferred, sRGB on and off (Note IF1). The bits are written raw, so every NaN payload
/// and denormal is there.
#[test]
fn gpu_conversion_matches_the_cpu_table_for_every_half() {
    // Odd multipliers, so each channel still walks all 65536 patterns, in a different order.
    let raw: Vec<u8> = (0..=u16::MAX)
        .flat_map(|b| [b, b.wrapping_mul(3), b.wrapping_mul(5).wrapping_add(77), b])
        .flat_map(u16::to_le_bytes)
        .collect();
    let hardware = Gpu::new().unwrap();
    eprintln!("adapter: {} (warp: {})", hardware.adapter, hardware.warp);
    for gpu in [hardware, Gpu::new_warp().unwrap()] {
        for deferred in [false, true] {
            let t = if deferred {
                Target::new_deferred(&gpu, 256, 256).unwrap()
            } else {
                Target::new(&gpu, 256, 256).unwrap()
            };
            // SAFETY: `raw` holds 256 rows of 256 × 8 bytes, the whole colour texture.
            unsafe {
                gpu.ctx
                    .UpdateSubresource(&t.color, 0, None, raw.as_ptr() as _, 256 * 8, 0)
            };
            for srgb in [false, true] {
                let (got, want) = (
                    t.readback_display(&gpu, srgb).unwrap(),
                    t.readback_display_cpu(&gpu, srgb).unwrap(),
                );
                let who = format!("{} deferred {deferred} srgb {srgb}", gpu.adapter);
                assert_eq!(got.len(), want.len(), "{who}");
                let bad = got.iter().zip(&want).position(|(a, b)| a != b);
                assert_eq!(bad, None, "{who}: first differing byte");
                // Not a table of zeros: mid grey is there.
                assert!(want.contains(&128), "{who}");
            }
        }
    }
}

/// The conversion pass binds the render targets again after it runs (Note IF3), and a target whose sides are not
/// multiples of the pass's 8×8 groups reads back whole.
#[test]
fn readback_leaves_the_render_targets_bound() {
    use windows::Win32::Graphics::Direct3D11::{ID3D11DepthStencilView, ID3D11RenderTargetView};
    let gpu = Gpu::new_warp().unwrap();
    let t = Target::new(&gpu, 3, 5).unwrap();
    t.clear(&gpu, [1.0, 0.5, 0.0, 1.0]);
    // SAFETY: live views of this device.
    unsafe {
        gpu.ctx
            .OMSetRenderTargets(Some(&[Some(t.rtv.clone())]), &t.dsv)
    };
    assert_eq!(
        t.readback_display(&gpu, false).unwrap(),
        [255, 128, 0, 255].repeat(15)
    );
    let mut rtvs: [Option<ID3D11RenderTargetView>; 8] = Default::default();
    let mut dsv: Option<ID3D11DepthStencilView> = None;
    // SAFETY: the out-pointers are live locals.
    unsafe { gpu.ctx.OMGetRenderTargets(Some(&mut rtvs), Some(&mut dsv)) };
    assert_eq!(rtvs[0].as_ref(), Some(&t.rtv));
    assert!(rtvs[1..].iter().all(Option::is_none));
    assert_eq!(dsv.as_ref(), Some(&t.dsv));
}

/// Deferred target: RT0 (`color`, RTV|SRV|UAV) + RT1-7 in the capture's formats, and RT0 as a UAV (D3 step 1).
#[test]
fn new_deferred_makes_eight_targets_and_a_main_uav() {
    use render3d::device::MRT_FORMATS;
    let gpu = Gpu::new_warp().unwrap();
    let t = Target::new_deferred(&gpu, 4, 2).unwrap();
    assert!(t.deferred());
    assert_eq!(t.mrt.len(), 8);
    assert!(t.main_uav.is_some());
    assert_eq!(t.rtvs().len(), 8);
    for (i, (tex, _, _)) in t.mrt.iter().enumerate() {
        let mut d = windows::Win32::Graphics::Direct3D11::D3D11_TEXTURE2D_DESC::default();
        unsafe { tex.GetDesc(&mut d) };
        assert_eq!(d.Format, MRT_FORMATS[i].0, "RT{i}");
    }
    assert!(!Target::new(&gpu, 4, 2).unwrap().deferred());
}
