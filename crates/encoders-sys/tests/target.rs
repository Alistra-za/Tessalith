use encoders_sys::{
    bc6h_compress_rows_routed, bc7e_compress_blocks_routed, gather_blocks, ispc_target, Bc6hLevel,
    Bc7Level, Bc7Params, IspcTarget, ShimRoute,
};
use half::f16;

#[test]
fn the_process_target_is_supported_and_named() {
    let t = ispc_target();
    assert!(t.supported());
    assert!(t <= IspcTarget::best());
    let names: Vec<_> = IspcTarget::ALL.iter().map(|t| t.name()).collect();
    assert_eq!(names, ["sse2", "avx2", "avx512"]);
}

/// The shim's switches (`bc7e_blocks_on`, `sdc_bc6h_compress`) take these numbers; its `default:` would silently
/// dispatch on any other. The behaviour-level guard is `level_identity`, which sees a mismatch only on a CPU whose
/// `best()` is above AVX2 (there a forced AVX2 encode that falls to the dispatcher writes AVX-512 bytes).
#[test]
fn the_shim_numbers_are_kept() {
    let n: Vec<i32> = IspcTarget::ALL.iter().map(|t| *t as i32).collect();
    assert_eq!(n, [0, 3, 4]);
}

#[test]
fn a_test_build_encodes_on_avx2_at_most_unless_lifted() {
    if std::env::var("SDC_TEST_AVX512").as_deref() == Ok("1") {
        assert_eq!(ispc_target(), IspcTarget::best());
    } else {
        assert!(
            ispc_target() <= IspcTarget::Avx2,
            "this test binary was built without encoders-sys/test-hooks"
        );
    }
    eprintln!("ispc_target = {}", ispc_target().name());
}

fn xorshift32(s: &mut u32) -> u32 {
    *s ^= *s << 13;
    *s ^= *s >> 17;
    *s ^= *s << 5;
    *s
}

/// Which of the three outputs the dispatcher's equals, for the `eprintln!`.
fn which(d: &[u8], s: &[u8], a: Option<&[u8]>) -> &'static str {
    if d == s {
        "sse2"
    } else if a == Some(d) {
        "avx2"
    } else {
        "neither direct object's"
    }
}

/// Fixtures (xorshift32, seed `0x9E37_79B9` for each; differing 16-byte blocks measured on the 1.31 objects on a Ryzen
/// whose `best()` is AVX-512, as SSE2 vs AVX2 / dispatcher vs SSE2 / dispatcher vs AVX2): BC7 Veryslow, 128x128 (1024 blocks), alpha 255,
/// texel (x, y) channel c = clamp(200 * (x + y) / 256 + 20 * c + n, 0, 255), one n in [-8, 8] per texel (28 / 62 / 59).
/// BC6H Veryslow, 64x256 (1024 blocks), b = (x + y) * 0.01, R = b + 0.3 u, G = 0.7 b + 0.3 u, B = 0.4 b + 0.3 u with
/// u uniform in [0, 1] drawn per channel in that order, alpha 1 (19 / 26 / 15). White noise does not separate the paths
/// (the targets converge on it); smooth content with small noise does. Each run prints its counts.
///
/// ISPC's dispatcher runs the object of `best()`: on an AVX2-only CPU the AVX2 object, on an AVX-512 CPU the AVX-512
/// object, which no direct route reaches. Forced SSE2 and AVX2 call their own objects, and only `best()` goes through
/// the dispatcher (`forced`), so this test reaches both routes with `ShimRoute` and compares the bytes: SSE2 and AVX2
/// must differ on the fixture (else it proves nothing), and the dispatcher must equal the object of `best()`, or on
/// AVX-512 differ from both. It needs no `SDC_TEST_AVX512` (it uses `best()`, not `ispc_target()`) and no byte pin, so
/// it runs on Intel too. It is the only test in this file that encodes: `Direct(Avx2)` on an AVX2-only CPU re-runs AVX2's table
/// init beside the dispatcher's (`sdc_shim.cpp`, `bc7e_blocks_on`), which is safe only while nothing else encodes.
#[test]
fn the_dispatcher_runs_the_best_target() {
    let best = IspcTarget::best();
    let avx2 = IspcTarget::Avx2.supported();

    // BC7: 128x128 RGBA8, a gradient with one noise draw per texel.
    let mut seed = 0x9E37_79B9u32;
    let (w, h) = (128u32, 128u32);
    let mut px = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            let mut t = [0u8, 0, 0, 255];
            let n = (xorshift32(&mut seed) % 17) as i32 - 8;
            for (c, v) in t.iter_mut().enumerate().take(3) {
                *v =
                    (200 * (x + y) as i32 / (2 * w as i32) + 20 * c as i32 + n).clamp(0, 255) as u8;
            }
            px.push(t);
        }
    }
    let blocks = gather_blocks(&px, w, h);
    let p = Bc7Params {
        level: Bc7Level::Veryslow,
        perceptual: false,
        exact_opaque: false,
    };
    let enc7 = |route| {
        let mut out = vec![0u8; blocks.len()];
        bc7e_compress_blocks_routed(&blocks, &mut out, p, route);
        out
    };
    let d = enc7(ShimRoute::Dispatcher);
    let s = enc7(ShimRoute::Direct(IspcTarget::Sse2));
    let a = avx2.then(|| enc7(ShimRoute::Direct(IspcTarget::Avx2)));
    check("bc7e", best, &d, &s, a.as_deref());

    // BC6H: a 64x256 strip of halves, a gradient with noise.
    let mut seed = 0x9E37_79B9u32;
    let (w, rows) = (64u32, 256u32);
    let mut u = || xorshift32(&mut seed) as f32 / u32::MAX as f32;
    let mut hp: Vec<[u16; 4]> = Vec::with_capacity((w * rows) as usize);
    for y in 0..rows {
        for x in 0..w {
            let b = (x + y) as f32 * 0.01;
            let c = [b + u() * 0.3, b * 0.7 + u() * 0.3, b * 0.4 + u() * 0.3];
            hp.push([
                f16::from_f32(c[0]).to_bits(),
                f16::from_f32(c[1]).to_bits(),
                f16::from_f32(c[2]).to_bits(),
                f16::ONE.to_bits(),
            ]);
        }
    }
    let enc6 = |route| {
        let mut out = vec![0u8; (w / 4 * rows / 4 * 16) as usize];
        bc6h_compress_rows_routed(&hp, w, rows, &mut out, Bc6hLevel::Veryslow, route);
        out
    };
    let d = enc6(ShimRoute::Dispatcher);
    let s = enc6(ShimRoute::Direct(IspcTarget::Sse2));
    let a = avx2.then(|| enc6(ShimRoute::Direct(IspcTarget::Avx2)));
    check("bc6h", best, &d, &s, a.as_deref());
}

fn blocks_differing(x: &[u8], y: &[u8]) -> usize {
    x.chunks(16)
        .zip(y.chunks(16))
        .filter(|(p, q)| p != q)
        .count()
}

fn check(what: &str, best: IspcTarget, d: &[u8], s: &[u8], a: Option<&[u8]>) {
    eprintln!(
        "{what}: differing blocks of {}: sse2 vs avx2 {}, dispatcher vs sse2 {}, dispatcher vs avx2 {}",
        d.len() / 16,
        a.map_or("n/a".into(), |a| blocks_differing(s, a).to_string()),
        blocks_differing(d, s),
        a.map_or("n/a".into(), |a| blocks_differing(d, a).to_string()),
    );
    if let Some(a) = a {
        assert!(
            a != s,
            "{what}: the fixture does not separate SSE2 and AVX2"
        );
    }
    match best {
        IspcTarget::Sse2 => assert!(d == s, "{what}: the dispatcher is not the SSE2 object"),
        IspcTarget::Avx2 => assert!(
            d == a.expect("best() above SSE2 implies AVX2 runs"),
            "{what}: the dispatcher is not the AVX2 object"
        ),
        IspcTarget::Avx512 => {
            assert!(
                d != a.expect("best() above SSE2 implies AVX2 runs"),
                "{what}: the dispatcher equals the AVX2 object"
            );
            assert!(d != s, "{what}: the dispatcher equals the SSE2 object");
        }
    }
    eprintln!(
        "best = {}: the dispatcher wrote {} bytes ({what})",
        best.name(),
        which(d, s, a)
    );
}
