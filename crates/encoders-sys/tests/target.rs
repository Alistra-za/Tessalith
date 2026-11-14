use encoders_sys::{ispc_target, IspcTarget};

#[test]
fn the_process_target_is_supported_and_named() {
    let t = ispc_target();
    assert!(t.supported());
    assert!(t <= IspcTarget::best());
    let names: Vec<_> = IspcTarget::ALL.iter().map(|t| t.name()).collect();
    assert_eq!(names, ["sse2", "sse4", "avx", "avx2", "avx512"]);
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
