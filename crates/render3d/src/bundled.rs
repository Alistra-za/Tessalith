//! The two pinned Community Shaders trees embedded in the exe by `build.rs`: A (`shaders/cs`, Note GT) and B
//! (`shaders/cs2`, CS TruePBR 2.0, Note ID).
include!(concat!(env!("OUT_DIR"), "/bundled.rs"));

/// Which embedded shader tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tree {
    A,
    B,
}

fn table(tree: Tree) -> &'static [(&'static str, &'static [u8])] {
    match tree {
        Tree::A => FILES,
        Tree::B => FILES_B,
    }
}

/// The bytes of `rel` in tree A.
pub(crate) fn read(rel: &str) -> Option<&'static [u8]> {
    read_in(Tree::A, rel)
}

/// The bytes of `rel` in `tree`, folding ASCII case and `\` to `/` as the Windows folder the tree came from did.
pub(crate) fn read_in(tree: Tree, rel: &str) -> Option<&'static [u8]> {
    let key = rel.replace('\\', "/").to_ascii_lowercase();
    let t = table(tree);
    let i = t.binary_search_by(|(k, _)| (*k).cmp(key.as_str())).ok()?;
    Some(t[i].1)
}

/// Every embedded path of `tree` (lowercased, `/`-separated).
#[cfg(feature = "test-hooks")]
pub(crate) fn names_in(tree: Tree) -> impl Iterator<Item = &'static str> {
    table(tree).iter().map(|(k, _)| *k)
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
