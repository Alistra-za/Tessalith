//! Per-stage timings for a single profiled batch. A thread without a scope records nothing.
use crate::{manifest, sysmem};
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    collections::HashMap,
    fmt, io,
    marker::PhantomData,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        atomic::{AtomicU32, AtomicU64, Ordering::Relaxed},
        Arc, Mutex, Weak,
    },
    thread,
    time::Instant,
};

macro_rules! stages {
    ($($variant:ident => $name:literal),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        #[repr(u8)]
        pub enum Stage { $($variant),+ }

        impl Stage {
            pub const ALL: [Stage; 53] = [$(Self::$variant),+];
            pub fn name(self) -> &'static str {
                match self { $(Self::$variant => $name),+ }
            }
        }
    };
}

stages! {
    Plan => "plan",
    PlanPrepare => "plan_prepare",
    PlanDiscover => "plan_discover",
    PlanFile => "plan_file",
    PlanLoad => "plan_load",
    PlanAnalyse => "plan_analyse",
    PlanResolve => "plan_resolve",
    PlanPack => "plan_pack",
    PlanFill => "plan_fill",
    PlanDerive => "plan_derive",
    PlanAdmitEstimate => "plan_admit_estimate",
    PlanAdmitWait => "plan_admit_wait",
    PoolBuild => "pool_build",
    Dispatch => "dispatch",
    ScopeJoin => "scope_join",
    Run => "run",
    RunAdmitEstimate => "run_admit_estimate",
    RunAdmitWait => "run_admit_wait",
    RunClassify => "run_classify",
    Currency => "currency",
    CurrencyHash => "currency_hash",
    File => "file",
    StampCheck => "stamp_check",
    HashSource => "hash_source",
    BakeLoad => "bake_load",
    Decode => "decode",
    PackAssemble => "pack_assemble",
    DeriveAssemble => "derive_assemble",
    Fill => "fill",
    CubePrepare => "cube_prepare",
    Preprocess => "preprocess",
    Dilate => "dilate",
    SrgbDecode => "srgb_decode",
    Resize => "resize",
    ResizeRows => "resize_rows",
    Mips => "mips",
    FilterRows => "filter_rows",
    RowJoin => "row_join",
    Coverage => "coverage",
    DetailCheck => "detail_check",
    Quantize => "quantize",
    Encode => "encode",
    EncodeGather => "encode_gather",
    EncodePack => "encode_pack",
    EncodeBand => "encode_band",
    EventsWait => "events_wait",
    Events => "events",
    DdsAssemble => "dds_assemble",
    Validate => "validate",
    ManifestRecord => "manifest_record",
    Write => "write",
    ManifestWait => "manifest_wait",
    ManifestAppend => "manifest_append",
}

impl Stage {
    pub fn is_wait(self) -> bool {
        matches!(
            self,
            Self::PlanAdmitWait
                | Self::RunAdmitWait
                | Self::RunClassify
                | Self::ScopeJoin
                | Self::RowJoin
                | Self::EventsWait
                | Self::ManifestWait
                | Self::Write
        )
    }

    pub fn is_row(self) -> bool {
        matches!(self, Self::EncodeBand | Self::ResizeRows | Self::FilterRows)
    }
}

type RawSpan = (u8, u32, u16, u64, u64, u64, u32);

#[derive(Serialize, Deserialize)]
pub struct Report {
    pub schema: u32,
    pub meta: Meta,
    pub stages: Vec<StageInfo>,
    pub threads: Vec<ThreadInfo>,
    pub calls: Vec<Call>,
    pub inputs: Vec<Input>,
    pub jobs: Vec<Job>,
    pub spans: Vec<RawSpan>,
    pub process: Process,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Meta {
    pub engine_version: String,
    pub engine_commit: String,
    pub build_profile: String,
    pub isa: String,
    pub batch: String,
    pub args: Vec<String>,
    pub jobs: usize,
    pub logical_cores: usize,
    pub total_ram: u64,
    pub started_rfc3339: String,
    pub corpus: Option<PathBuf>,
    pub output_root: PathBuf,
    pub preset: String,
    pub target: String,
}

#[derive(Serialize, Deserialize)]
pub struct StageInfo {
    pub name: String,
    pub class: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ThreadInfo {
    pub id: u16,
    pub name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CallKind {
    Plan,
    Run,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Call {
    pub call: u32,
    pub kind: CallKind,
    pub pool_threads: usize,
    pub start_ns: u64,
    pub end_ns: u64,
    pub cpu_start_ns: u64,
    pub cpu_end_ns: u64,
    pub budget_limit: u64,
    pub budget_peak: u64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Input {
    pub id: u32,
    pub path: PathBuf,
    pub bytes: u64,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct JobNote {
    pub input: PathBuf,
    pub subtype: String,
    pub format: String,
    pub alpha_policy: String,
    pub packed: bool,
    pub baked: bool,
    pub width: u32,
    pub height: u32,
    pub levels: u32,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: u32,
    pub call: u32,
    pub output: String,
    #[serde(flatten)]
    pub note: JobNote,
}

#[derive(Serialize, Deserialize)]
pub struct Process {
    pub wall_ns: u64,
    pub user_cpu_ns: u64,
    pub kernel_cpu_ns: u64,
    pub peak_working_set: u64,
    pub peak_commit: u64,
    pub working_set_at_start: u64,
    pub working_set_at_end: u64,
    pub peak_is_process_lifetime: bool,
}

static CREATED: AtomicU64 = AtomicU64::new(0);

#[doc(hidden)]
pub fn recorders_created() -> u64 {
    CREATED.load(Relaxed)
}

#[derive(Clone)]
pub struct Recorder(Arc<Inner>);

impl fmt::Debug for Recorder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Recorder")
    }
}

struct Inner {
    id: u64,
    start: Instant,
    counters: sysmem::ProcessCounters,
    next_loop: AtomicU32,
    data: Mutex<Data>,
    shards: Mutex<Vec<Arc<Shard>>>,
}

struct Data {
    meta: Meta,
    calls: Vec<Call>,
    input_ids: HashMap<PathBuf, u32>,
    inputs: Vec<Input>,
    job_ids: HashMap<(u32, String), u32>,
    jobs: Vec<Job>,
}

struct Shard {
    thread: ThreadInfo,
    rows: Mutex<Rows>,
}

#[derive(Default)]
struct Rows {
    last_loop: u32,
    spans: Vec<RawSpan>,
}

struct Local {
    stack: Vec<Context>,
    shards: Vec<(u64, Weak<Shard>)>,
}

impl Local {
    fn prune_shards(&mut self) {
        self.shards.retain(|(_, shard)| shard.strong_count() != 0);
    }
}

thread_local! {
    static LOCAL: RefCell<Local> = const { RefCell::new(Local { stack: Vec::new(), shards: Vec::new() }) };
}

impl Default for Recorder {
    fn default() -> Self {
        Self::new()
    }
}

impl Recorder {
    pub fn new() -> Self {
        Self(Arc::new(Inner {
            id: CREATED.fetch_add(1, Relaxed),
            start: Instant::now(),
            counters: sysmem::process_counters(),
            next_loop: AtomicU32::new(1),
            shards: Mutex::default(),
            data: Mutex::new(Data {
                meta: Meta {
                    engine_version: manifest::ENGINE_VERSION.into(),
                    engine_commit: manifest::engine_commit().into(),
                    build_profile: if cfg!(debug_assertions) {
                        "debug"
                    } else {
                        "release"
                    }
                    .into(),
                    isa: manifest::dispatched_isa().into(),
                    batch: String::new(),
                    args: vec![],
                    jobs: 0,
                    logical_cores: thread::available_parallelism().map_or(1, usize::from),
                    total_ram: sysmem::total_ram(),
                    started_rfc3339: crate::events::now_rfc3339(),
                    corpus: None,
                    output_root: PathBuf::new(),
                    preset: String::new(),
                    target: String::new(),
                },
                calls: vec![],
                input_ids: HashMap::new(),
                inputs: vec![],
                job_ids: HashMap::new(),
                jobs: vec![],
            }),
        }))
    }

    fn now(&self) -> u64 {
        self.0.start.elapsed().as_nanos() as u64
    }

    pub fn begin_call(&self, kind: CallKind, pool_threads: usize) -> u32 {
        let mut data = self.0.data.lock().unwrap();
        let call = data.calls.len() as u32;
        let counters = sysmem::process_counters();
        data.calls.push(Call {
            call,
            kind,
            pool_threads,
            start_ns: self.now(),
            end_ns: 0,
            cpu_start_ns: counters.user_cpu_ns + counters.kernel_cpu_ns,
            cpu_end_ns: 0,
            budget_limit: 0,
            budget_peak: 0,
        });
        call
    }

    pub fn end_call(&self, call: u32, budget_limit: u64, budget_peak: u64) {
        let counters = sysmem::process_counters();
        let call = &mut self.0.data.lock().unwrap().calls[call as usize];
        call.end_ns = self.now();
        call.cpu_end_ns = counters.user_cpu_ns + counters.kernel_cpu_ns;
        call.budget_limit = budget_limit;
        call.budget_peak = budget_peak;
    }

    pub fn note_request(
        &self,
        corpus: Option<&Path>,
        output_root: &Path,
        jobs: usize,
        preset: &str,
        target: &str,
    ) {
        let meta = &mut self.0.data.lock().unwrap().meta;
        meta.corpus = corpus.map(Path::to_owned);
        meta.output_root = output_root.into();
        meta.jobs = jobs;
        meta.preset = preset.into();
        meta.target = target.into();
    }

    pub fn enter_batch(&self, call: u32) -> Scope {
        // Batch ids count down so a batch span cannot look like file zero.
        self.enter(u32::MAX - call, false)
    }

    pub fn enter_input(&self, path: &Path) -> Scope {
        let mut data = self.0.data.lock().unwrap();
        let id = if let Some(&id) = data.input_ids.get(path) {
            id
        } else {
            let id = data.inputs.len() as u32;
            data.inputs.push(Input {
                id,
                path: path.into(),
                bytes: path.metadata().map_or(0, |m| m.len()),
            });
            data.input_ids.insert(path.into(), id);
            id
        };
        self.enter(id, false)
    }

    pub fn enter_job(&self, call: u32, output: &str) -> Scope {
        let mut data = self.0.data.lock().unwrap();
        let key = (call, output.to_owned());
        let id = if let Some(&id) = data.job_ids.get(&key) {
            id
        } else {
            let id = data.jobs.len() as u32;
            data.jobs.push(Job {
                id,
                call,
                output: output.into(),
                note: JobNote::default(),
            });
            data.job_ids.insert(key, id);
            id
        };
        self.enter(id, true)
    }

    fn enter(&self, id: u32, job: bool) -> Scope {
        LOCAL.with_borrow_mut(|local| {
            local.stack.push(Context {
                rec: self.clone(),
                id,
                job,
            })
        });
        Scope(PhantomData)
    }

    fn shard(&self) -> Arc<Shard> {
        LOCAL.with_borrow_mut(|local| {
            local.prune_shards();
            let cache = &mut local.shards;
            if let Some(shard) = cache
                .iter()
                .find(|(id, _)| *id == self.0.id)
                .and_then(|(_, s)| s.upgrade())
            {
                return shard;
            }
            let mut shards = self.0.shards.lock().unwrap();
            let shard = Arc::new(Shard {
                thread: ThreadInfo {
                    id: u16::try_from(shards.len()).expect("profile thread id overflow"),
                    name: thread::current().name().unwrap_or("unnamed").into(),
                },
                rows: Mutex::default(),
            });
            shards.push(shard.clone());
            cache.push((self.0.id, Arc::downgrade(&shard)));
            shard
        })
    }

    pub(crate) fn push_raw(
        &self,
        stage: Stage,
        id: u32,
        row_loop: u32,
        thread: u16,
        start_ns: u64,
        end_ns: u64,
    ) {
        let shard = self.shard();
        let mut rows = shard.rows.lock().unwrap();
        let duration = end_ns - start_ns;
        if stage.is_row() && row_loop != 0 && rows.last_loop == row_loop {
            if let Some(last) = rows.spans.last_mut() {
                if (last.0, last.1, last.2) == (stage as u8, id, thread)
                    && last.4 <= start_ns
                    && start_ns - last.4 < 50_000
                {
                    last.4 = end_ns;
                    last.5 += duration;
                    last.6 += 1;
                    return;
                }
            }
        }
        rows.last_loop = row_loop;
        rows.spans
            .push((stage as u8, id, thread, start_ns, end_ns, duration, 1));
    }

    pub fn report(&self, batch: &str, args: Vec<String>) -> Report {
        let mut threads = vec![];
        let mut spans = vec![];
        for shard in self.0.shards.lock().unwrap().iter() {
            threads.push(shard.thread.clone());
            spans.extend_from_slice(&shard.rows.lock().unwrap().spans);
        }
        let data = self.0.data.lock().unwrap();
        let start = &self.0.counters;
        let end = sysmem::process_counters();
        Report {
            schema: 1,
            meta: Meta {
                batch: batch.into(),
                args,
                ..data.meta.clone()
            },
            stages: Stage::ALL
                .into_iter()
                .map(|s| StageInfo {
                    name: s.name().into(),
                    class: if s.is_wait() { "wait" } else { "work" }.into(),
                })
                .collect(),
            threads,
            spans,
            calls: data.calls.clone(),
            inputs: data.inputs.clone(),
            jobs: data.jobs.clone(),
            process: Process {
                wall_ns: self.now(),
                user_cpu_ns: end.user_cpu_ns.saturating_sub(start.user_cpu_ns),
                kernel_cpu_ns: end.kernel_cpu_ns.saturating_sub(start.kernel_cpu_ns),
                peak_working_set: end.peak_working_set,
                peak_commit: end.peak_commit,
                working_set_at_start: start.working_set,
                working_set_at_end: end.working_set,
                peak_is_process_lifetime: true,
            },
        }
    }

    pub fn write_report(&self, dir: &Path, batch: &str, args: Vec<String>) -> io::Result<PathBuf> {
        let name = format!("profile-{batch}.json");
        if Path::new(&name).file_name() != Some(name.as_ref()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid profile filename",
            ));
        }
        let path = dir.join(name);
        let mut temp = tempfile::NamedTempFile::new_in(dir)?;
        {
            use io::Write;
            let mut writer = io::BufWriter::new(temp.as_file_mut());
            serde_json::to_writer(&mut writer, &self.report(batch, args))?;
            writer.flush()?;
        }
        temp.as_file().sync_all()?;
        temp.persist_noclobber(&path)?;
        Ok(path)
    }
}

#[derive(Clone)]
struct Context {
    rec: Recorder,
    id: u32,
    job: bool,
}

#[must_use]
pub struct Scope(PhantomData<Rc<()>>);

impl Drop for Scope {
    fn drop(&mut self) {
        LOCAL.with_borrow_mut(|local| local.stack.pop());
    }
}

#[derive(Clone)]
pub struct Captured {
    ctx: Context,
    row_loop: u32,
}

pub fn capture() -> Option<Captured> {
    LOCAL.with_borrow(|local| {
        local.stack.last().map(|ctx| Captured {
            ctx: ctx.clone(),
            row_loop: ctx.rec.0.next_loop.fetch_add(1, Relaxed),
        })
    })
}

impl Captured {
    pub fn span(&self, stage: Stage) -> Span {
        self.ctx.clone().span(stage, self.row_loop)
    }
}

struct ActiveSpan {
    rec: Recorder,
    stage: Stage,
    id: u32,
    row_loop: u32,
    thread: u16,
    start_ns: u64,
}

impl Context {
    fn span(self, stage: Stage, row_loop: u32) -> Span {
        Span(
            Some(ActiveSpan {
                thread: self.rec.shard().thread.id,
                start_ns: self.rec.now(),
                rec: self.rec,
                stage,
                id: self.id,
                row_loop,
            }),
            PhantomData,
        )
    }
}

#[must_use]
pub struct Span(Option<ActiveSpan>, PhantomData<Rc<()>>);

impl Drop for Span {
    fn drop(&mut self) {
        if let Some(s) = &self.0 {
            s.rec
                .push_raw(s.stage, s.id, s.row_loop, s.thread, s.start_ns, s.rec.now());
        }
    }
}

pub fn span(stage: Stage) -> Span {
    let ctx = LOCAL.with_borrow_mut(|local| {
        if local.stack.is_empty() && !local.shards.is_empty() {
            local.prune_shards();
        }
        local.stack.last().cloned()
    });
    ctx.map_or(Span(None, PhantomData), |ctx| ctx.span(stage, 0))
}

/// The size Rayon actually built, including its environment overrides for an automatic pool.
pub(crate) fn note_pool_threads(threads: usize) {
    LOCAL.with_borrow(|local| {
        if let Some(ctx) = local.stack.last() {
            let mut data = ctx.rec.0.data.lock().unwrap();
            // enter_batch numbers batch scopes down from u32::MAX.
            if let Some(call) = data.calls.get_mut((u32::MAX - ctx.id) as usize) {
                call.pool_threads = threads;
            }
        }
    });
}

pub fn note_job(make: impl FnOnce() -> JobNote) {
    if let Some(ctx) = LOCAL.with_borrow(|local| local.stack.last().filter(|ctx| ctx.job).cloned())
    {
        let note = make();
        ctx.rec.0.data.lock().unwrap().jobs[ctx.id as usize].note = note;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::HashSet, sync::Barrier, thread, time::Duration};
    use Stage::*;

    #[test]
    fn stage_table_is_complete() {
        let names: HashSet<_> = Stage::ALL.map(Stage::name).into_iter().collect();
        assert_eq!(names.len(), 53);
        assert_eq!(PlanAdmitWait.name(), "plan_admit_wait");
        assert_eq!(ScopeJoin.name(), "scope_join");
        for (i, s) in Stage::ALL.into_iter().enumerate() {
            assert_eq!(s as usize, i);
            assert_eq!(
                s.is_wait(),
                [
                    PlanAdmitWait,
                    RunAdmitWait,
                    RunClassify,
                    ScopeJoin,
                    RowJoin,
                    EventsWait,
                    ManifestWait,
                    Write
                ]
                .contains(&s)
            );
            assert_eq!(
                s.is_row(),
                [EncodeBand, ResizeRows, FilterRows].contains(&s)
            );
        }
    }

    #[test]
    fn a_span_without_a_scope_records_nothing() {
        let rec = Recorder::new();
        {
            let _scope = rec.enter_batch(0);
            drop(span(Plan));
        }
        drop(span(Run));
        assert!(capture().is_none());
        let report = rec.report("test", vec![]);
        assert_eq!(report.spans.len(), 1);
        assert_eq!(report.spans[0].0, Plan as u8);
    }

    #[test]
    fn a_nested_scope_records_its_own_id_and_restores_the_outer() {
        let rec = Recorder::new();
        let _a = rec.enter_job(0, "a.dds");
        {
            let _join = span(RowJoin);
            let _b = rec.enter_job(0, "b.dds");
            drop(span(Decode));
        }
        drop(span(Validate));
        let r = rec.report("test", vec![]);
        let [decode, join, validate] = r.spans[..] else {
            panic!("expected three spans")
        };
        assert_eq!(
            (decode.0, join.0, validate.0),
            (Decode as u8, RowJoin as u8, Validate as u8)
        );
        assert_eq!(
            (decode.1, join.1, validate.1),
            (r.jobs[1].id, r.jobs[0].id, r.jobs[0].id)
        );
        assert_ne!(decode.1, join.1);
        assert_eq!(decode.2, join.2);
        assert!(join.3 <= decode.3 && decode.4 <= join.4 && join.4 <= validate.3);
    }

    #[test]
    fn a_captured_span_runs_on_a_controlled_thread_while_the_origin_waits() {
        let rec = Recorder::new();
        let _a = rec.enter_job(0, "a.dds");
        let cap = capture().unwrap();
        {
            let _join = span(RowJoin);
            thread::scope(|s| {
                s.spawn(|| {
                    let _band = cap.span(EncodeBand);
                    thread::sleep(Duration::from_millis(2));
                });
            });
        }
        let r = rec.report("test", vec![]);
        assert_eq!(r.spans.len(), 2);
        let join = r.spans.iter().find(|s| s.0 == RowJoin as u8).unwrap();
        let band = r.spans.iter().find(|s| s.0 == EncodeBand as u8).unwrap();
        assert_ne!(band.2, join.2);
        assert_eq!((band.1, join.1), (r.jobs[0].id, r.jobs[0].id));
        assert!(join.3 <= band.3 && band.4 <= join.4);
    }

    #[test]
    fn coalescing_sums_busy_time_and_counts_pieces() {
        let rec = Recorder::new();
        for start in [0, 20, 40, 200] {
            rec.push_raw(EncodeBand, 9, 1, 0, start * 1000, (start + 10) * 1000);
        }
        rec.push_raw(Quantize, 9, 1, 0, 211_000, 212_000);
        let spans = rec.report("test", vec![]).spans;
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].5, 30_000);
        assert_eq!(spans[0], (EncodeBand as u8, 9, 0, 0, 50_000, 30_000, 3));
        assert_eq!(
            spans[1],
            (EncodeBand as u8, 9, 0, 200_000, 210_000, 10_000, 1)
        );
        assert_eq!(spans[2], (Quantize as u8, 9, 0, 211_000, 212_000, 1_000, 1));
    }

    #[test]
    fn coalescing_never_joins_two_loops() {
        let rec = Recorder::new();
        rec.push_raw(EncodeBand, 9, 1, 0, 0, 10_000);
        rec.push_raw(EncodeBand, 9, 2, 0, 11_000, 20_000);
        let spans = rec.report("test", vec![]).spans;
        assert_eq!(spans.len(), 2);
        assert!(spans.iter().all(|s| s.6 == 1));
    }

    #[test]
    fn capture_assigns_a_new_loop_each_time() {
        let rec = Recorder::new();
        let _scope = rec.enter_batch(0);
        let a = capture().unwrap();
        let b = capture().unwrap();
        assert_ne!(a.row_loop, b.row_loop);
        assert_ne!(a.row_loop, 0);
    }

    #[test]
    fn coalescing_requires_the_same_stage_id_thread_and_a_gap_under_50_us() {
        let rec = Recorder::new();
        for (i, (stage, id, thread, start)) in [
            (ResizeRows, 0, 0, 0),
            (ResizeRows, 0, 0, 51_000),
            (FilterRows, 0, 0, 52_000),
            (FilterRows, 1, 0, 53_000),
            (FilterRows, 1, 1, 54_000),
        ]
        .into_iter()
        .enumerate()
        {
            rec.push_raw(stage, id, 1, thread, start, start + 1_000);
            assert_eq!(rec.report("test", vec![]).spans.len(), i + 1);
        }
    }

    #[test]
    fn shards_drop_with_the_recorder_and_expired_handles_are_pruned() {
        let dead_id = {
            let rec = Recorder::new();
            rec.push_raw(Plan, 0, 0, 0, 0, 1);
            rec.0.id
        };
        LOCAL.with_borrow(|local| {
            let (_, weak) = local.shards.iter().find(|(id, _)| *id == dead_id).unwrap();
            assert!(weak.upgrade().is_none());
        });
        drop(span(Plan));
        assert!(LOCAL.with_borrow(|local| local.shards.iter().all(|(id, _)| *id != dead_id)));
        let next = Recorder::new();
        let _scope = next.enter_batch(0);
        drop(span(Run));
        assert!(LOCAL.with_borrow(|local| local.shards.iter().all(|(id, _)| *id != dead_id)));
        assert_eq!(next.report("test", vec![]).spans.len(), 1);
    }

    #[test]
    fn creating_a_recorder_increments_the_process_counter() {
        let before = recorders_created();
        let rec = Recorder::new();
        assert!(recorders_created() > before);
        assert_eq!(format!("{rec:?}"), "Recorder");
    }

    #[test]
    fn note_job_runs_its_closure_only_with_a_scope() {
        let rec = Recorder::new();
        let count = std::cell::Cell::new(0);
        let make = || {
            count.set(count.get() + 1);
            JobNote {
                input: "image.png".into(),
                subtype: "diffuse".into(),
                format: "BC7".into(),
                alpha_policy: "blend".into(),
                packed: true,
                baked: true,
                width: 8,
                height: 16,
                levels: 5,
            }
        };
        note_job(make);
        assert_eq!(count.get(), 0);
        let _scope = rec.enter_job(0, "a.dds");
        note_job(make);
        assert_eq!(count.get(), 1);
        let job = &serde_json::to_value(rec.report("test", vec![])).unwrap()["jobs"][0];
        assert_eq!(
            *job,
            serde_json::json!({"id": job["id"], "call": 0, "output":"a.dds", "input":"image.png",
            "subtype":"diffuse","format":"BC7","alpha_policy":"blend","packed":true,"baked":true,
            "width":8,"height":16,"levels":5})
        );
    }

    #[test]
    fn the_same_input_or_output_gets_the_same_id_and_calls_keep_jobs_apart() {
        let rec = Recorder::new();
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("source");
        std::fs::write(&input, b"123").unwrap();
        for _ in 0..2 {
            let _scope = rec.enter_input(&input);
            drop(span(PlanFile));
        }
        for call in [0, 0, 1] {
            let _scope = rec.enter_job(call, "x.dds");
            drop(span(File));
        }
        let r = rec.report("test", vec![]);
        assert_eq!(r.inputs.len(), 1);
        assert_eq!(r.inputs[0].bytes, 3);
        assert_eq!(r.inputs[0].path, input);
        assert_eq!(r.jobs.len(), 2);
        assert_eq!((r.jobs[0].call, r.jobs[1].call), (0, 1));
        assert_ne!(r.jobs[0].id, r.jobs[1].id);
        assert_eq!(
            r.spans.iter().map(|s| s.1).collect::<Vec<_>>(),
            [
                r.inputs[0].id,
                r.inputs[0].id,
                r.jobs[0].id,
                r.jobs[0].id,
                r.jobs[1].id
            ]
        );
    }

    #[test]
    fn the_report_round_trips() {
        let rec = Recorder::new();
        let call = rec.begin_call(CallKind::Plan, 4);
        rec.note_request(Some(Path::new("in")), Path::new("out"), 4, "balanced", "cs");
        {
            let _scope = rec.enter_batch(call);
            drop(span(Plan));
        }
        rec.end_call(call, 10, 7);
        let json = serde_json::to_value(rec.report("batch", vec!["convert".into()])).unwrap();
        assert_eq!(json["spans"][0].as_array().unwrap().len(), 7);
        let r: Report = serde_json::from_value(json).unwrap();
        assert_eq!(r.schema, 1);
        assert_eq!(r.stages.len(), 53);
        for (stage, row) in Stage::ALL.iter().zip(&r.stages) {
            assert_eq!(row.name, stage.name());
            assert_eq!(row.class, if stage.is_wait() { "wait" } else { "work" });
        }
        assert_eq!(r.calls[0].pool_threads, 4);
        assert_eq!((r.calls[0].budget_limit, r.calls[0].budget_peak), (10, 7));
        assert_eq!(r.calls[0].call, call);
        assert!(r.calls[0].start_ns <= r.spans[0].3 && r.spans[0].4 <= r.calls[0].end_ns);
        assert_eq!(r.meta.isa, crate::manifest::dispatched_isa());
        assert_eq!(r.meta.corpus.as_deref(), Some(Path::new("in")));
        assert_eq!(r.meta.output_root, Path::new("out"));
        assert_eq!(
            (r.meta.jobs, r.meta.preset.as_str(), r.meta.target.as_str()),
            (4, "balanced", "cs")
        );
        assert_eq!(r.threads[0].id, r.spans[0].2);
        assert!(r.process.peak_is_process_lifetime);
    }

    #[test]
    fn write_report_never_overwrites_even_in_a_race() {
        let dir = tempfile::tempdir().unwrap();
        let barrier = Barrier::new(2);
        let outcomes = thread::scope(|scope| {
            let workers = ["first", "second"].map(|arg| {
                let barrier = &barrier;
                let dir = dir.path();
                scope.spawn(move || {
                    let rec = Recorder::new();
                    barrier.wait();
                    (arg, rec.write_report(dir, "x", vec![arg.into()]))
                })
            });
            workers.map(|w| w.join().unwrap())
        });
        assert_eq!(
            outcomes.iter().filter(|(_, result)| result.is_ok()).count(),
            1
        );
        let (arg, path) = outcomes.iter().find(|(_, result)| result.is_ok()).unwrap();
        let error = outcomes
            .iter()
            .find_map(|(_, result)| result.as_ref().err())
            .unwrap();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(*path.as_ref().unwrap(), dir.path().join("profile-x.json"));
        let report: Report =
            serde_json::from_reader(std::fs::File::open(path.as_ref().unwrap()).unwrap()).unwrap();
        assert_eq!(report.meta.args, [*arg]);
    }
}
