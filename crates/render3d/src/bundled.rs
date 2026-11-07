//! The pinned Community Shaders tree (`shaders/cs`, Note GT) embedded in the exe by `build.rs`.
include!(concat!(env!("OUT_DIR"), "/bundled.rs"));

/// The bytes of `rel`, folding ASCII case and `\` to `/` as the Windows folder the tree came from did.
pub(crate) fn read(rel: &str) -> Option<&'static [u8]> {
    let key = rel.replace('\\', "/").to_ascii_lowercase();
    let i = FILES
        .binary_search_by(|(k, _)| (*k).cmp(key.as_str()))
        .ok()?;
    Some(FILES[i].1)
}

/// Every embedded path (lowercased, `/`-separated).
#[cfg(feature = "test-hooks")]
pub(crate) fn names() -> impl Iterator<Item = &'static str> {
    FILES.iter().map(|(k, _)| *k)
}

#[cfg(test)]
mod tests {
    use super::read;

    // Unit test: `read` is crate-private.
    #[test]
    fn lookup_folds_case_and_slashes() {
        let a = read("common\\COLOR.hlsli").unwrap();
        assert!(!a.is_empty());
        assert_eq!(Some(a), read("Common/Color.hlsli"));
        assert!(read("Common/Nope.hlsli").is_none());
    }
}
