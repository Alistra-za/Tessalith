use encoders_sys::{ispc_target, IspcTarget};

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
