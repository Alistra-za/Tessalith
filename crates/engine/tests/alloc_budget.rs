//! Allocation budgets (Note LF1): how many allocation calls a warm full plan and a one-edit re-plan of the fast
//! fixture make, and enum ids that do not allocate (Note LH4). The plan counter is process-wide (the engine plans on a rayon pool), so every request runs with
//! `jobs = 1` to keep the count the same from run to run. The tests share the counter, so each measuring test holds
//! one static `Mutex` (`MEASURE`) for its whole body; no two run their setup or measuring at the same time.
//! `enum_ids_do_not_allocate` counts per thread instead (`counted_here`): its work runs on the test's own thread, and
//! the harness's threads must not reach an exact-zero count.
#[path = "common/fast_fixture.rs"]
mod fast_fixture;

use engine::api::{BatchRequest, CancelToken};
use engine::config::{Format, RawMap, Subtype};
use engine::events::{Reporter, VecSink};
use engine::session::Session;
use fast_fixture::{fast_fixture, with_entry};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

static CALLS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);
static MEASURE: Mutex<()> = Mutex::new(());

thread_local! {
    static HERE_CALLS: Cell<u64> = const { Cell::new(0) };
    static HERE_BYTES: Cell<u64> = const { Cell::new(0) };
}

/// Adds one allocation of `n` bytes to both the process-wide and this thread's counters. `try_with`: a thread that is
/// starting or ending has no counter, and the allocator must not panic there.
fn add(n: usize) {
    CALLS.fetch_add(1, Ordering::Relaxed);
    BYTES.fetch_add(n as u64, Ordering::Relaxed);
    let _ = HERE_CALLS.try_with(|c| c.set(c.get() + 1));
    let _ = HERE_BYTES.try_with(|b| b.set(b.get() + n as u64));
}

// The sibling counter in `crates/gui/tests/alloc_frame.rs` is per-thread only; this file counts process-wide for the
// plans (rayon) and per thread for `enum_ids_do_not_allocate`. A fix to one is checked against the other.
struct Counting;
// SAFETY: forwards every call to the system allocator; only adds process-wide and per-thread counters.
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

/// Each test takes this for its whole body (setup included), so no other test's allocations land in its count.
fn exclusive() -> std::sync::MutexGuard<'static, ()> {
    MEASURE.lock().unwrap_or_else(|e| e.into_inner())
}

/// The allocation calls and bytes `f` made (freed bytes still count), and its result.
fn counted<R>(f: impl FnOnce() -> R) -> (R, u64, u64) {
    let (c0, b0) = (CALLS.load(Ordering::Relaxed), BYTES.load(Ordering::Relaxed));
    let r = f();
    let (c1, b1) = (CALLS.load(Ordering::Relaxed), BYTES.load(Ordering::Relaxed));
    (r, c1 - c0, b1 - b0)
}

/// The allocation calls and bytes `f` made on this thread only (freed bytes still count), and its result.
fn counted_here<R>(f: impl FnOnce() -> R) -> (R, u64, u64) {
    let (c0, b0) = (HERE_CALLS.with(Cell::get), HERE_BYTES.with(Cell::get));
    let r = f();
    let (c1, b1) = (HERE_CALLS.with(Cell::get), HERE_BYTES.with(Cell::get));
    (r, c1 - c0, b1 - b0)
}

// A run with captured output reads 2 calls higher; the cause is not traced. The counter is process-wide, so allocations
// by the test harness's own threads can land in the measured window (`MEASURE` keeps out only this file's tests).
// Measured 2026-11-21 with `--nocapture`, 5 runs, jobs = 1, after the suffix and vocabulary splits stopped
// allocating (Note LF3): 112683 calls in all five (11090098 bytes, 34 jobs). 112683 * 1.10 = 123952, rounded up to two
// significant figures.
const WARM_PLAN_CALLS: u64 = 130_000;
// Measured 2026-11-21 with `--nocapture`, 5 runs, jobs = 1, after the same change and `no_set_paths` building its
// index only for a `NoSet` edit: 27376 calls in all five (2531208 bytes, 5 jobs). 27376 * 1.10 = 30114, rounded up to
// two significant figures.
const REPLAN_CALLS: u64 = 31_000;

fn reporter() -> Reporter {
    Reporter::new(Arc::new(VecSink::default()), "t")
}

/// A session that planned the fast fixture in full once (not measured), and the one-job-at-a-time request it planned.
fn planned() -> (fast_fixture::FastFixture, Session, BatchRequest) {
    let fx = fast_fixture();
    let session = Session::new().unwrap();
    let mut req = fx.req.clone();
    req.jobs = 1;
    session
        .plan_and_keep(req.clone(), &reporter(), &CancelToken::default())
        .unwrap();
    (fx, session, req)
}

#[test]
fn the_counter_counts_each_allocation() {
    let _x = exclusive();
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

/// Note LH4: an enum id is a name, not a new string. Counted on this thread only: the ids run here, and the
/// harness's own threads must not reach an exact-zero count.
#[test]
fn enum_ids_do_not_allocate() {
    let _x = exclusive();
    let (len, calls, _) = counted_here(|| {
        let mut len = 0;
        for r in RawMap::INPUTS {
            len += std::hint::black_box(r.id()).len();
        }
        for s in Subtype::ALL {
            len += std::hint::black_box(s.id()).len();
        }
        len + std::hint::black_box(Format::Bc6hUf16.id()).len()
    });
    assert!(len > 0);
    assert_eq!(calls, 0, "enum ids made {calls} allocation calls");
}

#[test]
fn a_warm_full_plan_stays_within_its_allocation_budget() {
    let _x = exclusive();
    let (_fx, session, req) = planned();
    let cancel = CancelToken::default();
    let rep = reporter();
    let (plan, calls, bytes) = counted(|| {
        session
            .engine()
            .plan_cached(req, &rep, &cancel, session.cache())
            .unwrap()
    });
    let jobs = plan.jobs.len();
    assert!(
        jobs > 0,
        "the warm plan held no job, so its allocation count would mean nothing"
    );
    eprintln!(
        "warm full plan: {calls} calls, {bytes} bytes, {jobs} jobs, {} calls per job",
        calls / jobs as u64
    );
    assert!(
        calls <= WARM_PLAN_CALLS,
        "a warm full plan made {calls} allocation calls, budget {WARM_PLAN_CALLS}"
    );
}

#[test]
fn a_one_edit_replan_stays_within_its_allocation_budget() {
    let _x = exclusive();
    let (_fx, session, base) = planned();
    let cancel = CancelToken::default();
    let rep = reporter();
    let edit = vec![(
        "dir/a.png".to_string(),
        Some(toml::from_str::<toml::Table>("alpha.policy = \"cutout\"").unwrap()),
    )];
    let req = with_entry(&base, &edit[0].0, edit[0].1.as_ref());
    let (partial, calls, bytes) =
        counted(|| session.replan_sets(&req, &edit, &rep, &cancel).unwrap());
    let jobs = partial.plan.jobs.len();
    assert!(
        jobs > 0,
        "the one-edit re-plan held no job, so its allocation count would mean nothing"
    );
    eprintln!(
        "one-edit re-plan: {calls} calls, {bytes} bytes, {jobs} jobs, {} calls per job",
        calls / jobs as u64
    );
    assert!(
        calls <= REPLAN_CALLS,
        "a one-edit re-plan made {calls} allocation calls, budget {REPLAN_CALLS}"
    );
}
