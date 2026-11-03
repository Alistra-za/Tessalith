use encoders_sys::{init, versions};

#[test]
fn init_is_idempotent() {
    init();
    init();
    let v = versions();
    assert_eq!(v.bc7enc_rdo, "b943862");
    assert_eq!(v.ispctc, "79ddbc9");
    assert_eq!(v.ispc, "1.31.0");
    assert_eq!(
        v.rgbcx_patch,
        Some("bc4-round-to-nearest+bc4-search+bc1-dual+bc4-guard-cut")
    );
}
