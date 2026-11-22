//! Allocation budget for one settled GUI frame (Note LF2): how many allocation calls the real `SdcApp` makes drawing
//! one frame of the demo project once nothing is pending. The counter is per thread (a frame is drawn on the test's own
//! thread; worker threads for plans and thumbnails are left out on purpose), and it is a global allocator, so this
//! file is a `[[test]]` of its own, kept apart from `it`.
mod app_harness;
#[allow(dead_code)] // fields only the doc shots read
#[path = "doc_shots/manifest.rs"]
mod manifest;
mod support;

use egui_kittest::kittest::Queryable;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static CALLS: Cell<u64> = const { Cell::new(0) };
    static BYTES: Cell<u64> = const { Cell::new(0) };
}

/// Adds one allocation of `n` bytes to this thread's counter. `try_with`: a thread that is starting or ending has no
/// counter, and the allocator must not panic there.
fn add(n: usize) {
    let _ = CALLS.try_with(|c| c.set(c.get() + 1));
    let _ = BYTES.try_with(|b| b.set(b.get() + n as u64));
}

struct Counting;
// SAFETY: forwards every call to the system allocator; only adds two per-thread counters.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        add(l.size());
        // SAFETY: the caller's layout contract passes straight through to `System`.
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        // SAFETY: `p` and `l` come from this allocator, which only forwards to `System`.
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        add(n);
        // SAFETY: `p`, `l` and `n` come from this allocator's caller and pass straight through to `System`.
        unsafe { System.realloc(p, l, n) }
    }
}
#[global_allocator]
static COUNTING: Counting = Counting;

/// The allocation calls and bytes `f` made on this thread (freed bytes still count), and its result.
fn counted<R>(f: impl FnOnce() -> R) -> (R, u64, u64) {
    let (c0, b0) = (CALLS.with(Cell::get), BYTES.with(Cell::get));
    let r = f();
    let (c1, b1) = (CALLS.with(Cell::get), BYTES.with(Cell::get));
    (r, c1 - c0, b1 - b0)
}

// Measured 2026-11-21, 5 runs of the binary, 5 frames each: 4071 calls in all 25 frames (634260 bytes). 4071 * 1.25 =
// 5089, rounded up to two significant figures.
const FRAME_CALLS: u64 = 5_100;

#[test]
fn the_counter_counts_each_allocation() {
    let mut keep: Vec<Box<u64>> = Vec::with_capacity(1000);
    let (_, calls, _) = counted(|| {
        for _ in 0..1000 {
            keep.push(Box::new(0u64));
        }
    });
    assert_eq!(keep.len(), 1000);
    eprintln!("1,000 boxes counted as {calls} allocation calls");
    assert!(
        (1000..=1010).contains(&calls),
        "1,000 boxes counted as {calls} allocation calls"
    );
}

#[test]
fn a_settled_frame_stays_within_its_allocation_budget() {
    let tmp = tempfile::tempdir().unwrap();
    app_harness::redirect_app_data(tmp.path());
    let root = app_harness::demo(tmp.path());
    let m = support::opened_as_the_app(root, tmp.path().join("export"));
    let mut h = app_harness::harness(m);
    app_harness::quiet(&mut h);
    assert!(
        h.query_by_label("stonewall.png").is_some(),
        "the demo project is not on screen: no \"stonewall.png\" row, so an empty frame would pass as a cheap one"
    );
    let frames: Vec<(u64, u64)> = (0..5)
        .map(|_| {
            let ((), calls, bytes) = counted(|| {
                h.step();
            });
            (calls, bytes)
        })
        .collect();
    let calls: Vec<u64> = frames.iter().map(|f| f.0).collect();
    let (largest, bytes) = frames.iter().copied().max_by_key(|f| f.0).unwrap();
    eprintln!("settled frame: calls {calls:?}, largest {largest} calls, {bytes} bytes");
    assert!(
        largest <= FRAME_CALLS,
        "a settled frame made {largest} allocation calls, budget {FRAME_CALLS}"
    );
}
