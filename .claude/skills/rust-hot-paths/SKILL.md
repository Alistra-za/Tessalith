---
name: rust-hot-paths
description: Use when writing or reviewing Rust code that runs once per input file or per frame at project scale (planning, grouping, matching, drawing a GUI frame), or when a change touches code an allocation budget test covers.
---

# Rust hot paths

Code that runs once per input file or per frame multiplies its cost by the project size: 10,000 files, or 60 frames a second. In Tessalith, one line of that kind caused 64 % of a warm plan's allocation calls (Note LF3). These are the shapes measured there, and what to write instead. The examples, test paths and goldens rule are Tessalith's.

## Shapes to avoid on a per-file or per-frame path

1. **Building a `String` only to compare it.** `format!("_{s}")` or `to_lowercase()` inside a loop, then `ends_with` or `==`.
   - Where the existing rule is ASCII-only, compare bytes: `stem.as_bytes()` and `eq_ignore_ascii_case` on the tail. Example: `cut_tail` in `crates/engine/src/group.rs`.
   - Where the code folds Unicode case (set ids and `[files]` keys go through `str::to_lowercase`, as `fold_id` in `crates/engine/src/sets.rs` does; Note BF), keep that fold and its `String`. Lowering one char at a time is not the same fold: `str::to_lowercase` turns a word-final `Σ` into `ς`, `char::to_lowercase` gives `σ`. Changing the fold rule is a behaviour change, not an optimisation.
   - Slice the original `&str` only at or just after an ASCII byte you have checked, and make sure the check runs before the slice: `.then(|| &stem[..cut])`, never `.then_some(&stem[..cut])`, which slices first and panics when `cut` lands inside a multi-byte char.
2. **Building an index or collection before checking that anything needs it.** Return early when no input can use it (for example, no edit of the kind the index serves). Example: `no_set_paths` in `crates/engine/src/sets.rs`.
3. **Serialising a value to get its name.** `serde_json::to_value(e)` to read a unit enum's id. Write a `match` that returns `&'static str`, with a test that checks it against the serde name of every variant. Example: `RawMap::id` and `ids_are_the_serde_names_of_every_variant` in `crates/engine/src/config.rs` (Note LH4).
4. **Cloning a whole structure to change one part of it,** on every re-plan or frame. Note LH5 shares the unchanged parts of the re-plan's skeleton through their own `Arc`s.
   - `Arc::make_mut` clones the value whenever another `Arc` to it is alive. It saves work only when the unchanged parts sit behind their own `Arc`s; one `Arc` around the whole structure, made mutable each frame while the GUI holds the last one, is the same full clone.
   - Call it once per change, not once per element.

Returning an owned `String` once, as the result, is fine. The cost is what runs on every call, matched or not.

## Rules

- **Copy the intent of the code around you, not its cost.** When the function next to yours allocates in its loop, don't repeat that in the new one. Don't rewrite the old function either, unless the task asks for it.
- **Shapes 1 to 3 in new code need no measurement first:** write the cheap version.
- **Shape 4 changes a type.** Do it only when the task changes that type, and with an allocation budget test. Otherwise leave it and mention it in your report.
- **For a change under `crates/engine/src/` or to GUI frame code, run both budget tests** (a counting global allocator: `crates/engine/tests/alloc_budget.rs`, `crates/gui/tests/alloc_frame.rs`). Never raise a budget to make it pass. Report the new count and why it grew.
- **A smaller count in a cold path is not worth a harder read.** Code that runs once per plan or once per click is not a hot path.
- **Float rewrites for speed** (`mul_add`, reordered sums) change bytes, so they are a planned task of their own under CLAUDE.md's "A measured speed change may alter bytes" rule, with timing, quality and Intel/AMD parity measured. Never make one in passing.
- **No scope widening.** This skill applies to the code the task changes. If you see a costly shape elsewhere, leave it and mention it in your report if it matters.
