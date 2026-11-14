//! The only test of this file, so its forced encodes are the first of the process: the per-target init of the
//! shim must run once under concurrent first calls.
use encoders_sys::{bc7e_compress_blocks_on, gather_blocks, Bc7Level, Bc7Params, IspcTarget};
use std::sync::{Arc, Barrier};

#[test]
fn first_forced_encodes_from_many_threads_agree() {
    // The highest supported target below the best one; nothing to force when there is none.
    let Some(target) = IspcTarget::ALL
        .into_iter()
        .rfind(|t| t.supported() && *t < IspcTarget::best())
    else {
        return;
    };
    let (w, h) = (64u32, 64u32);
    let mut px = Vec::new();
    for y in 0..h {
        for x in 0..w {
            px.push([
                (x * 4 + y) as u8,
                ((y * 3) ^ x) as u8,
                ((x * y) >> 3) as u8,
                255 - (x as u8),
            ]);
        }
    }
    let blocks = Arc::new(gather_blocks(&px, w, h));
    let p = Bc7Params {
        level: Bc7Level::Veryslow,
        perceptual: false,
        exact_opaque: false,
    };
    let barrier = Arc::new(Barrier::new(8));
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let (blocks, barrier) = (blocks.clone(), barrier.clone());
            std::thread::spawn(move || {
                let mut out = vec![0u8; blocks.len()];
                barrier.wait();
                bc7e_compress_blocks_on(&blocks, &mut out, p, target);
                out
            })
        })
        .collect();
    let outs: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let mut want = vec![0u8; blocks.len()];
    bc7e_compress_blocks_on(&blocks, &mut want, p, target);
    for (i, out) in outs.iter().enumerate() {
        assert_eq!(*out, want, "thread {i}");
    }
}
