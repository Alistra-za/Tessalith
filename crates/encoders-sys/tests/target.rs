use encoders_sys::{ispc_target, IspcTarget};

#[test]
fn the_process_target_is_supported_and_named() {
    let t = ispc_target();
    assert!(t.supported());
    assert!(t <= IspcTarget::best());
    let names: Vec<_> = IspcTarget::ALL.iter().map(|t| t.name()).collect();
    assert_eq!(names, ["sse2", "sse4", "avx", "avx2"]);
}
