# Where the time goes outside the encoders

How much of a `sdc convert` run is encoding, and whether the rest is worth speeding up. Measured 2026-11-26 with the release build of commit e315ac28 (engine 3.2.0, stage recorder on), against the build before the recorder (a1eabd9c) for the overhead check. The mip quality side of the same runs is in [Result 7 of the mip page](mipmaps-and-alpha.md#result-7-measured-mip-quality-2026-11-26). Rulings are in Note LW.

## The verdict

Encode is 93.9 % of the work on the 10,000-image corpus and 95.0 % on the mixed corpus. Both are over the 90 % line set before the runs. The largest group of stages outside the encoders is the plan phase's decode and analysis, at 2.1 % of the work on T10K. The recorder costs less than 1 % of wall time.

The share counts busy thread-seconds in the work stages, with waits left out, so idle threads cannot move it. Stage seconds on this page are thread-seconds across the pool of 16 threads, never wall time. The wall split, encode's union of the wall and idle cores sit beside the share; they do not decide it.

| | 10,000-image corpus (T10K) | Mixed corpus (MIX) |
|---|---|---|
| Content | 10,000 PNGs at 1024 by 1024, 23.87 GB; 9,228 output keys ready | 417 files, 2.84 GB, source images and re-encoded DDS; 264 output keys ready |
| Cohort | warm-fresh, 5 runs | warm-fresh, 5 runs |
| Wall, median [min, max] | 1,247.5 s [1,246.3, 1,251.0] | 248.8 s [244.3, 252.5] |
| Encode share of all work | 93.9 % [93.9, 94.0] | 95.0 % [94.8, 95.1] |
| Encode's share of pool capacity | 86.9 % | 88.7 % |
| Wall split: encode only, no encode, mixed | 73.7 %, 9.4 %, 16.9 % | 73.7 %, 8.8 %, 17.5 % |
| Encode's union of the wall (error bound) | 90.6 % (199.7 s) | 91.2 % (13.9 s) |
| Idle cores, mean per run | 1.56 [1.54, 1.59] of 16 | 1.37 [1.08, 1.60] of 16 |
| Plan phase, median | 27.9 s (2.2 % of wall) | 6.8 s (2.7 % of wall) |
| Peak working set, median | 487 MB | 5.7 GB |

The wall split cuts the run into intervals with an encode thread and nothing else (encode only), with no encode thread (no encode) and with both (mixed). The 9.4 % of T10K wall with no encode thread is about 117 s, more than the 27.9 s plan phase; the data does not say where the rest falls.

Waits are reported apart and excluded from every share: on T10K the median `run_admit_wait` is 1,204 s and `row_join` 682 s.

## Method

- Machine. One Windows machine, 16 hardware threads, 31.6 GiB of RAM, AVX-512. Inputs on one drive, outputs on a local NVMe drive. The pool has 16 threads.
- Builds. The base build (a1eabd9c), the same code with the recorder compiled in (e315ac28, release) and a profiling profile of it for sampled runs. The recorder writes a report only when `SDC_PROFILE` names a folder.
- Commands. `sdc convert <corpus> --recursive --accept-suggestions high --jobs 0`, with no `--validate` unless stated. The default flow leaves some inputs unresolved: T10K plans 10,000 jobs and writes 9,228 keys, so 772 jobs write nothing. MIX has 417 inputs but plans 416 jobs, because one input is only packed into another job's set and has no job of its own; 264 jobs write a key and 152 write nothing. The unresolved set is the same in every run. MIX under-represents foliage (20 of 60 ready) and source images (36 of 100 ready). Its ready inputs by category are normal maps 40 of 40, PBR sets 81 of 90, raw maps 16 of 19, 1k and 2k pairs 71 of 100, foliage 20 of 60, source images 36 of 100 and icons 1 of 8. They sum to 265, the 264 written keys plus the packing-only input.
- Checks on every run. Each run's DDS bytes were hashed and compared with the base build's reference run, and every expected key had to be written. All runs on this page passed. Outputs were deleted after the comparison, except the reference runs.
- Quiet machine. A run counts only when other processes stay near the idle baseline: other I/O may not exceed it by 200 MB/s for more than 10 s in total or 3 s in a row, the run's mean other I/O may not exceed it by more than 20 MB/s, and its mean other CPU may not exceed it by more than 0.5 cores. Three T10K attempts were dropped for other I/O under the earlier single-spike rule, before the seconds rule replaced it. The one spike left in the reported runs is 1 s at 211.5 MB/s (the Windows search indexer, in the first stale run).
- Cohorts. Warm: a repeat right after a completed run of the same corpus. Cold: before the run, each input's cached pages were dropped by opening it unbuffered; the program, its DLLs and file system metadata stay cached. Fresh: no output exists, so every ready file is encoded. Noop: every output exists and is current. Stale: every output exists, copied from a fresh output written with the default (`wrap`); the stale run sets `mips.edge = clamp`, so it finds them out of date. The box filter never reads the edge, so the bytes come out the same. Validate: a fresh run with `--validate`.
- T10K warm-fresh is five runs of the overhead matrix. The recorder-on runs of rounds 1 to 4 and one more from round 5 serve as the warm-fresh cohort.
- MIX had its own setup. It ran under a shorter output path, with its own idle baseline, expected key set, warm-up and base reference, because three outputs overflowed the Windows path limit under the longer one (below).

## Recorder overhead

Within 1 %. Four rounds, each a base-build run, a recorder-off run (the e315ac28 build with `SDC_PROFILE` unset) and a recorder-on run, back to back on T10K:

| Round | Base | Off | On | Off against base | On against off |
|---|---|---|---|---|---|
| 1 | 1,245.3 s | 1,246.0 s | 1,246.3 s | +0.06 % | +0.02 % |
| 2 | 1,251.8 s | 1,249.8 s | 1,247.5 s | -0.16 % | -0.18 % |
| 3 | 1,249.8 s | 1,250.3 s | 1,248.8 s | +0.04 % | -0.12 % |
| 4 | 1,247.5 s | 1,248.8 s | 1,247.3 s | +0.10 % | -0.12 % |

A fifth round was cut after outside I/O spoiled its base run twice; every round is under 0.2 %, so a fifth cannot move a verdict near 1 %. The bytes matched the base build in every run. Validation was off in these runs, so their records hold only skipped markers; the one `--validate` run passed 9,228 with 0 failed. That the verdicts agree with the recorder on and off is pinned by a test with validation running (`profile_on_and_off_agree_when_validation_runs`).

## The other cohorts

| Corpus and cohort | Runs | Wall, median [min, max] | Encode share of work | Corpus disk read |
|---|---|---|---|---|
| T10K warm-fresh | 5 | 1,247.5 s [1,246.3, 1,251.0] | 93.9 % | 45.95 to 46.03 GB |
| T10K cold-fresh | 1 | 1,250.5 s | 94.0 % | 45.96 GB |
| T10K validate | 1 | 1,286.8 s | 93.5 % | 45.97 GB |
| T10K stale | 3 | 1,257.0 s [1,256.5, 1,257.8] | 93.2 % | 64.0 to 65.6 GB |
| T10K noop | 5 | 37.5 s [36.3, 37.5] | none | 40.5 to 43.4 GB |
| MIX stale | 3 | 245.5 s [244.0, 247.0] | 94.7 % | not separable, see below |
| MIX noop | 5 | 7.8 s [7.5, 7.8] | none | not separable, see below |

- Cold matches warm on T10K. The cold run took 0.2 % longer than the warm median, inside the warm spread, and read the same 46 GB. The cold method is the per-file eviction above, so the cold figure says nothing about the program or file system metadata, and this page makes no claim about what the cache held at any point. Cold ran once; MIX has no cold run.
- Validate costs 3.1 % of wall on T10K. The run took 1,286.8 s against the 1,247.5 s warm median, 39.3 s more and far outside the 4.75 s warm spread. `--validate` found 9,228 passed, 0 failed, 0 skipped. The `validate` stage is 109.8 thread-seconds, 0.6 % of the work and about 7 s of wall if it spread evenly over 16 threads. This was one fresh run. The path that validates an existing output (`--validate` with `--on-existing if-changed`) was not run and has no stage of its own: its time is unattributed, or charged to a concurrent encode job.
- Stale runs cost 0.8 % more wall than fresh on T10K (1,257.0 s against 1,247.5 s) and read 64.0 to 65.6 GB, about 19 GB more than a fresh run.
- A noop run on T10K takes 37.5 s, of which 27.7 s is the plan phase. Its work is 38.2 % `plan_analyse`, 32.9 % `plan_load` and 23.8 % `currency_hash`.
- MIX disk counters. MIX inputs and outputs share one drive, so its disk columns do not separate reads of inputs from reads of outputs. A MIX warm-fresh run read 0.6 to 1.75 GB from that drive and wrote 2.82 to 2.83 GB.

## Findings for later plans

Nothing here is adopted, and no gain is measured: every gain is an `upper bound`, the thread-seconds of the work a fix would remove. A fix that could move a byte or a verdict, or that touches ruled behaviour, needs an owner ruling before it is planned.

| Finding | Stage | Cost, T10K warm-fresh median | Cost, MIX | Upper bound | Bytes change expected | Ruling needed |
|---|---|---|---|---|---|---|
| Every input is read twice in a run | none (disk read) | 46.0 GB read against 23.87 GB of inputs | not measured | no stage covers it | no | no |
| The plan phase decodes and analyses what the run phase decodes again | `plan_load`, `plan_analyse`, plan assembly | 392.2 s | 66.6 s | 2.1 % of work on T10K, 1.8 % on MIX; at most the 27.9 s plan wall | no | no |
| The source is hashed twice: currency, then `hash_source` | `hash_source` | 45.0 s is the single `hash_source`: a fresh run has no output to check, so it runs no `currency_hash` (stale runs hash twice: `currency_hash` 141.2 s, `hash_source` 43.4 s) | 2.3 s | 0.2 % of work on T10K, 0.1 % on MIX | no | yes, the currency rules (Notes Y1, JA) |
| The bake inputs are loaded for the run | `bake_load` | 65.1 s | 0.0 s | 0.4 % of work on T10K | no | no |
| The write waits on the disk | `write` (wait) | 62.9 s | 5.0 s | 62.9 thread-seconds of waiting, 0.3 % of pool capacity (16 threads times the wall) on T10K | no | yes, `sync_all` (Note BC10) |
| Manifest appends and the waits for them | `manifest_append`, `manifest_wait` | 44.5 s | 0.7 s | 44.5 thread-seconds, about half of it waiting, 0.2 % of pool capacity on T10K | no | no |
| Admission estimates | `plan_admit_estimate`, `run_admit_estimate` | 20.4 s | 0.2 s | 0.1 % of work on T10K | no | no |

- The double read. The plan phase reads every input (23.87 GB) and the run phase reads the ready ones again (21.94 GB), so a T10K run reads about 46 GB. The machine has 31.6 GiB of RAM and the corpus is 23.87 GB with about 12.4 GB written per run, so the second read comes from disk. A noop run reads 40.5 to 43.4 GB for the same reason. A 0.4 GB slice of the corpus read at most 0.25 GB when warm, so the second read depends on the RAM limit. A survey of where the reads happen is running.
- The double hash. A noop run spends 132.4 thread-seconds in `currency_hash`, 23.8 % of its work. A stale run hashes the source in `currency_hash` and again in `hash_source`.
- Long output paths. Outputs whose full path was 260 characters or more failed to write with os error 3 (path not found) although the system allows long paths. The MIX corpus found it, under the longer path: three outputs at 260, 265 and 270 characters failed and the longest that wrote was 259. The cause was the temp file's rename, which handed Win32 the raw path; Note LV fixes it.
- Not worth a row. Events (0.5 s) and the pool build (under 0.001 s) cost nothing measurable.

## What this does not show

- No sampled profile. samply was not run (it needs elevation), so copies of pixel data and time inside a stage are not attributed. The span stages are the only attribution.
- MIX has no cold run and no mip quality run, and T10K cold and validate are one run each.
- One machine. The shares move with the thread count, the drive and the CPU. Other machines can differ.
- Unrecorded CPU is information only. Process CPU time not covered by a span averages -0.35 cores on T10K warm-fresh, printed with its sign, and is never an input to the verdict. Where the idle 1.56 cores fall is not attributed.
- Waits are not work. `run_admit_wait` and `row_join` are listed above but excluded from every share.
