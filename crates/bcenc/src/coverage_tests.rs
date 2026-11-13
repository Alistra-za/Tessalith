//! Every `hit!` branch of the ported code is taken by the class generators.
use crate::hits::counts;
use crate::{bc4_encode_alpha, bc4_encode_block, bc5_encode_block};

#[path = "../tests/support/gen.rs"]
mod gen;

#[test]
fn every_ported_branch_is_taken() {
    for v in gen::bc4_classes(0xC0DE_0001, 600) {
        let px = v.map(|a| [a, a ^ 0x5A, 0, a]);
        for rad in [4, 5, 10] {
            bc4_encode_block(&px, 0, rad);
            bc5_encode_block(&px, rad);
            for cut in gen::CUTS {
                bc4_encode_alpha(&v, rad, cut);
            }
        }
    }
    let never: Vec<_> = counts()
        .into_iter()
        .filter(|&(_, n)| n == 0)
        .map(|(h, _)| h)
        .collect();
    assert!(never.is_empty(), "branches never taken: {never:?}");
}
