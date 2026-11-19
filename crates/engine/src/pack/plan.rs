//! Plan-time wiring of channel packing (spec §6.4, plan M2.5, DECISIONS Note M). `Engine::plan` calls
//! [`plan_packing`] once every file is resolved (pass 1: recipes, packing settings, conflicts, consumed and unused
//! raw jobs), then assembles, analyses and resolves each [`Candidate`] with [`finish_recipe`] (pass 2).
use super::assemble::{check_file_sizes, check_sizes, sizes, PackError};
use super::{
    file_name as name, recipes_for_sel, recipes_for_with, resolve_channels, ChannelSource, PackJob,
    PackSource, RawInput, RecipeKind, Slot,
};
use crate::analyse::{analyse, Analysis};
use crate::api::SkippedFile;
use crate::codes::{Code, Warning};
use crate::config::{GraySource, PackPrefer, PackRecipe, Profile, RawMap, Subtype, Target};
use crate::discover::{InputFile, InputKind};
use crate::events::{RecipeReport, RecipeStatus};
use crate::group::{member_split, split_suffix_with, split_vocab, TextureSet, VocabRole};
use crate::image::{Rgba32F, SourceInfo};
use crate::policy::{
    layered_profile_placeholder, resolve_decided, FileFacts, JobStatus, ResolveCtx, ResolvedJob,
    SourceStamp, SubtypeDecision,
};
use crate::presets::Provenance;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const CHANNELS: [&str; 4] = ["R", "G", "B", "A"];

// `name` is `pack::file_name` (imported above), the file-name text used in every message of this module.

fn slash(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// The input of a job that writes (or will, once decided or acknowledged: not a raw input or an
/// error), or of another recipe, that also writes `output` (case-insensitive, either separator). The recipe's own
/// raw sources are raw inputs (Note M8). `outputs` holds every recipe's output (explicit or default), input and
/// whether it writes, in planning order, collected before any recipe is planned, so a later recipe counts too
/// (DECISIONS AF7); `me` is this recipe's index in it.
fn other_output(
    output: &Path,
    jobs: &[ResolvedJob],
    outputs: &[(PathBuf, PathBuf, bool)],
    me: usize,
) -> Option<PathBuf> {
    let fold = |p: &Path| slash(p).to_lowercase();
    let want = fold(output);
    jobs.iter()
        .filter(|j| {
            !matches!(
                j.status,
                JobStatus::PackingInput { .. } | JobStatus::Error { .. }
            )
        })
        .find(|j| fold(&j.output) == want)
        .map(|j| j.input.clone())
        .or_else(|| {
            outputs
                .iter()
                .enumerate()
                .find(|(i, (o, _, writes))| *i != me && *writes && fold(o) == want)
                .map(|(_, (_, input, _))| input.clone())
        })
}

fn list(paths: &[PathBuf]) -> String {
    paths.iter().map(|p| name(p)).collect::<Vec<_>>().join(", ")
}

/// The raw-map role of an input file, from its name (spec §4.4).
pub fn raw_role(f: &InputFile) -> Option<RawMap> {
    if f.kind != InputKind::Image {
        return None;
    }
    let stem = f.rel.file_stem()?.to_string_lossy().into_owned();
    match split_vocab(&stem)?.1.role {
        VocabRole::PackingInput(r) => Some(r),
        _ => None,
    }
}

/// The packed output as an input file: the first source's `path` (so `job.input` is a real file, Note M8), with
/// the output's name `<base>_<suffix>.<ext>` in `rel` and `game_path` (rule globs and `file:` scopes match it)
/// and the packed `output` next to the source's.
pub fn virtual_input(first: &InputFile, kind: RecipeKind) -> InputFile {
    virtual_input_as(first, kind.suffix())
}

/// [`virtual_input`] for an output whose suffix is `suffix` (a derived `_m`, Note CN).
pub(crate) fn virtual_input_as(first: &InputFile, suffix: &str) -> InputFile {
    let stem = virtual_stem_as(&first.rel, suffix);
    let ext = first
        .rel
        .extension()
        .map_or_else(|| "png".to_string(), |e| e.to_string_lossy().into_owned());
    let file = format!("{stem}.{ext}");
    let lower = file.to_lowercase();
    let game_path = match first.game_path.rsplit_once('/') {
        Some((dir, _)) => format!("{dir}/{lower}"),
        None => lower,
    };
    InputFile {
        path: first.path.clone(),
        rel: first.rel.with_file_name(&file),
        game_path,
        output: first.output.with_file_name(format!("{stem}.dds")),
        kind: InputKind::Image,
        real_rel: None,
    }
}

/// The packed output's stem, from its first source's name: `<base>_<suffix>` (`rock01_roughness` → `rock01_rmaos`,
/// and `rock01_rmaos` → `rock01_rmaos` for a source that already carries a §6.1 suffix, audit 08a-4). A shared or
/// joined helper's output takes its set's name instead; [`files_stem`] gives the stem its `[files]` key uses.
/// The base is `group::base_of`, which carries the alias `shortcut:`.
pub fn virtual_stem(first_source: &Path, kind: RecipeKind) -> String {
    virtual_stem_as(first_source, kind.suffix())
}

/// Note HS2: the stem of a packed job's `[files]` key, as policy reads it (the key is this stem in the first source's
/// folder, wherever an output path puts the file): the stem of the job's own output name (`game_path`), so a shared
/// helper's output in another set (`slab_rmaos` from `rock_roughness`) and a joined helper's (`x_rmaos` from
/// `x_mask`) key under their set's name. [`virtual_stem`] when the two agree but for letter case, so existing keys
/// keep theirs.
pub fn files_stem(job: &ResolvedJob, kind: RecipeKind) -> String {
    let own = virtual_stem(&job.input, kind);
    let named = Path::new(&job.game_path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    // Both sides folded alike: `game_path` may be folded for ASCII only (`discover::game_path`).
    if named.is_empty() || own.to_lowercase() == named.to_lowercase() {
        own
    } else {
        named
    }
}

pub(crate) fn virtual_stem_as(first_source: &Path, suffix: &str) -> String {
    format!("{}_{suffix}", source_base(first_source))
}

/// The base a source names a packed output after: its stem without a recognised suffix (`group::base_of`).
fn source_base(p: &Path) -> String {
    let stem = p
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    crate::group::base_of(&stem)
}

/// Note DY: a hand-role helper that `group::join_helpers` moved into set `x` (`x_mask`) names a packed output as the
/// set's own members do (`x_rmaos`, with its `[files]` key): each such member with `rel` renamed to the stem of a
/// member whose base is the set's. The whole stem, never the bare base: `virtual_stem_as` parses the name again, and
/// a base that itself ends in a term (`x_ao`) would be cut twice. Empty for every set without a joined helper, so
/// other names never change.
fn joined_helper_names(
    ctx: &ResolveCtx,
    set: &TextureSet,
    files: &[InputFile],
) -> Vec<(PathBuf, InputFile)> {
    let key_base = set
        .key
        .rsplit_once('/')
        .map_or(set.key.as_str(), |(_, b)| b);
    let own = |f: &InputFile| source_base(&f.rel).to_lowercase() == key_base;
    let Some(stem) = set
        .members
        .iter()
        .map(|&i| &files[i])
        .find(|f| own(f))
        .map(|f| f.rel.file_stem().unwrap_or_default().to_string_lossy())
    else {
        return vec![];
    };
    set.members
        .iter()
        .map(|&i| &files[i])
        .filter(|f| {
            f.kind == InputKind::Image
                && !own(f)
                && crate::policy::helper_requested(ctx, f)
                    .role
                    .is_some_and(|r| r != crate::helper::HelperRole::NotHelper)
        })
        .map(|f| {
            let ext = f.rel.extension().unwrap_or_default().to_string_lossy();
            let mut g = f.clone();
            g.rel = f.rel.with_file_name(format!("{stem}.{ext}"));
            (f.path.clone(), g)
        })
        .collect()
}

/// Note HH2: the file as set `set` reads it. A recipe's output is built from its first source, so a shared helper
/// stands in under the set's folder, base and game folder, as `joined_helper_names` does for a hand-set helper in
/// the same folder. `path` stays the real file's (sizes, `consumed`, job removal).
fn shared_stand_in(set: &TextureSet, files: &[InputFile], f: &InputFile) -> InputFile {
    let key_base = set
        .key
        .rsplit_once('/')
        .map_or(set.key.as_str(), |(_, b)| b);
    let mut members = set.members.iter().map(|&i| &files[i]);
    let Some(anchor) = members
        .clone()
        .find(|m| source_base(&m.rel).to_lowercase() == key_base)
        .or_else(|| members.clone().find(|m| m.kind == InputKind::Image))
        .or_else(|| members.next())
    else {
        return f.clone();
    };
    let stem = anchor.rel.file_stem().unwrap_or_default().to_string_lossy();
    let ext = f.rel.extension().unwrap_or_default().to_string_lossy();
    let file = format!("{stem}.{ext}");
    let lower = file.to_lowercase();
    let mut g = f.clone();
    g.rel = anchor.rel.with_file_name(&file);
    g.output = anchor.output.with_file_name(format!("{stem}.dds"));
    g.game_path = match anchor.game_path.rsplit_once('/') {
        Some((dir, _)) => format!("{dir}/{lower}"),
        None => lower,
    };
    g
}

/// A recipe that passed pass 1: assembled, analysed and resolved in pass 2.
pub struct Candidate {
    pub set_key: String,
    pub kind: RecipeKind,
    pub virtual_input: InputFile,
    /// Carries `gray`, the `channels.grayscale_source` every later step uses (Note M6).
    pub job: PackJob,
    /// Channels that took a neutral default (index, slot): one `PACK-DEFAULTED` each.
    pub defaulted: Vec<(usize, Slot)>,
    /// A `packing.recipe` notice: `PACK-RECIPE-CONFLICT` (the `_rfaos` and `_rmaos` keys disagree) or
    /// `PACK-RECIPE-UNBUILDABLE` (`rfaos` was asked for on a set that cannot build it).
    pub notes: Vec<(Code, String)>,
    /// `PACK-SOURCE-RESIZED` when a source is smaller than the largest (Note FM30), read from the headers.
    pub resized: Option<Warning>,
    /// Why a defaulted fuzz channel has no source although a fuzz map is shared in (`plan_packing`'s `shortcut:`):
    /// added to its `PACK-DEFAULTED` message and as its `data.why`.
    pub fuzz_why: Option<String>,
}

pub struct PackPlan {
    pub candidates: Vec<Candidate>,
    /// Recipe reports per set key, for `set_resolved`.
    pub reports: BTreeMap<String, Vec<RecipeReport>>,
    /// Generated AO maps skipped `GEN-REPLACED` because a pre-packed image's B holds AO (Note CO103); the caller
    /// drops them from their sets, as step 3b does its losers.
    pub gen_replaced: Vec<PathBuf>,
    /// Backlog T17: (set key, recipe, source, its output) of every source of a recipe pass 1 failed or cancelled; pass 2 adds its own.
    pub lost: Vec<(String, RecipeKind, PathBuf, PathBuf)>,
}

/// Marks the report of `c` `Failed` with `reason`: pass 2 could not plan it (a load or assembly failure, a panic, or
/// a cancel before its sources were loaded; DECISIONS Note M11), so `set_resolved` must not say it runs.
pub fn fail_report(
    reports: &mut BTreeMap<String, Vec<RecipeReport>>,
    c: &Candidate,
    reason: String,
) {
    let output = slash(&c.virtual_input.output);
    if let Some(r) = reports
        .get_mut(&c.set_key)
        .and_then(|v| v.iter_mut().find(|r| r.output == output))
    {
        r.status = RecipeStatus::Failed;
        r.reason = Some(reason);
    }
}

/// Backlog T17: a recipe that failed or was cancelled has no job, so its sources besides the one that carries the
/// `file_failed` would get no terminal event. After pass 2, ends each of `lost` (set key, recipe, source, its output)
/// as not converted unless a job reads it, or a failure, a cancel or a skip already ends it. Each source ends once.
pub fn end_lost_sources(
    lost: &[(String, RecipeKind, PathBuf, PathBuf)],
    jobs: &[ResolvedJob],
    failed: &[crate::api::FailedFile],
    cancelled: &[PathBuf],
    skipped: &mut Vec<SkippedFile>,
) {
    for (set_key, kind, path, output) in lost {
        let ended = failed.iter().any(|f| &f.path == path)
            || cancelled.contains(path)
            || skipped.iter().any(|s| &s.path == path)
            || jobs.iter().any(|j| {
                j.pack
                    .as_ref()
                    .is_some_and(|p| p.sources.iter().any(|x| &x.path == path))
            });
        if ended {
            continue;
        }
        skipped.push(SkippedFile {
            path: path.clone(),
            output: output.clone(),
            set: Some(set_key.clone()),
            code: Some(Code::DeriveNotConverted),
            reason: format!(
                "{} is not converted: its {} recipe failed",
                name(path),
                kind.suffix()
            ),
        });
    }
}
/// The analysis pass 1 resolves a packed profile with: the packed image does not exist yet (DECISIONS Note M6).
/// Pass 1 ignores rules that match on `alpha` (`layered_profile_placeholder`), so its alpha never decides anything.
fn placeholder() -> (Analysis, SourceInfo) {
    let info = SourceInfo {
        bit_depth: 8,
        channels: 4,
        has_alpha: true,
        is_float: false,
    };
    (analyse(&Rgba32F::new(4, 4, [0.5; 4]), &info), info)
}

/// Replaces a waiting raw job's packing-input warning (`policy::packing_input_job` gives it exactly one).
fn retag(jobs: &mut [ResolvedJob], path: &Path, code: Code, message: String) {
    if let Some(j) = jobs
        .iter_mut()
        .find(|j| j.input == path && matches!(j.status, JobStatus::PackingInput { .. }))
    {
        let data = j
            .warnings
            .first()
            .map(|w| w.data.clone())
            .unwrap_or_default();
        j.warnings = vec![Warning::new(code, message).with_data(data)];
    }
}

/// [`retag`] as PACK-UNUSED-INPUT, `<file>: <reason>; not converted`.
fn retag_unused(jobs: &mut [ResolvedJob], path: &Path, reason: &str) {
    retag(
        jobs,
        path,
        Code::PackUnusedInput,
        format!("{}: {reason}; not converted", name(path)),
    );
}

/// A planned non-raw member that already provides `kind`'s output (spec §6.4 "Pre-packed conflict", DECISIONS
/// Note M9). A DDS counts once it is planned (re-encoded, S6; Note BV1); a skipped DDS has no job.
fn prepacked_member<'a>(
    ctx: &ResolveCtx,
    set: &TextureSet,
    files: &'a [InputFile],
    jobs: &[ResolvedJob],
    kind: RecipeKind,
) -> Option<&'a InputFile> {
    set.members.iter().map(|&i| &files[i]).find(|f| {
        let stem = f
            .rel
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        raw_role(f).is_none()
            && split_suffix_with(&stem, ctx.registry.alias_table())
                .suffix
                .as_deref()
                == Some(kind.suffix())
            && jobs.iter().any(|j| j.input == f.path)
    })
}

/// Note HC1: a specular beside a pre-packed `_rmaos`/`_rfaos` image whose alpha is flat (cs): the image joins `raw` as
/// `RawMap::Rmaos` and `recipes_for_with` takes A from the specular, the other channels as CO104. A generated AO that
/// `prepacked_b` kept fills B too. Any other loose source, or an alpha with content, keeps today's M9 result.
// shortcut: a pre-packed DDS is never joined (images only, as CO103), upgrade if users ask for a specular beside one.
fn prepacked_a<'a>(
    ctx: &ResolveCtx,
    set: &TextureSet,
    files: &'a [InputFile],
    flat_a: &BTreeSet<PathBuf>,
    raw: &mut Vec<RawInput>,
    members: &mut BTreeMap<PathBuf, &'a InputFile>,
) {
    let stem = |p: &Path| {
        p.file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    };
    let loose_ok = |r: &RawInput| match r.role {
        RawMap::Specular => true,
        RawMap::Ao => crate::group::is_generated(&stem(&r.path)),
        RawMap::Roughness | RawMap::Glossiness | RawMap::Metallic | RawMap::Fuzz | RawMap::Orm => {
            false
        }
        _ => true,
    };
    if !raw.iter().any(|r| r.role == RawMap::Specular)
        || raw.iter().any(|r| r.role == RawMap::Rmaos)
        || !raw.iter().all(loose_ok)
    {
        return;
    }
    let Some(img) = prepacked_image(ctx, set, files) else {
        return;
    };
    if flat_a.contains(&img.path) && !prepacked_aliased(ctx, img) {
        raw.push(RawInput {
            path: img.path.clone(),
            role: RawMap::Rmaos,
        });
        members.insert(img.path.clone(), img);
    }
}

/// The set's pre-packed `_rmaos`/`_rfaos` image, by name.
fn prepacked_image<'a>(
    ctx: &ResolveCtx,
    set: &TextureSet,
    files: &'a [InputFile],
) -> Option<&'a InputFile> {
    set.members.iter().map(|&i| &files[i]).find(|f| {
        f.kind == InputKind::Image
            && raw_role(f).is_none()
            && matches!(
                split_suffix_with(
                    &f.rel.file_stem().unwrap_or_default().to_string_lossy(),
                    ctx.registry.alias_table()
                )
                .suffix
                .as_deref(),
                Some("rmaos" | "rfaos")
            )
    })
}

/// CO107: the recipe kind and `virtual_stem` read the built-in suffix only, so an aliased image is never joined.
// shortcut: an aliased `_rmaos`/`_rfaos` image always keeps its own job; pass the alias table to the recipe and
// `virtual_stem` if users ask for AO on aliased pre-packed images.
fn prepacked_aliased(ctx: &ResolveCtx, img: &InputFile) -> bool {
    let stem = img.rel.file_stem().unwrap_or_default().to_string_lossy();
    crate::group::split_suffix(&stem).1.as_deref()
        != split_suffix_with(&stem, ctx.registry.alias_table())
            .suffix
            .as_deref()
}

/// Note CO103: a generated AO beside a pre-packed `_rmaos`/`_rfaos` image of the set (cs). When the image's B holds
/// AO (or was not analysed), the generated AO is skipped `GEN-REPLACED` and leaves `raw`. When it is empty and the
/// AO is the set's only `_rmaos`/`_rfaos` source, the image joins `raw` as `RawMap::Rmaos` and `recipes_for_with`
/// builds R, G and A from it, B from the AO. With other loose sources the M9 conflict of today stands.
#[allow(clippy::too_many_arguments)]
fn prepacked_b<'a>(
    ctx: &ResolveCtx,
    set: &TextureSet,
    files: &'a [InputFile],
    flat_b: &BTreeSet<PathBuf>,
    raw: &mut Vec<RawInput>,
    members: &mut BTreeMap<PathBuf, &'a InputFile>,
    jobs: &mut Vec<ResolvedJob>,
    skipped: &mut Vec<SkippedFile>,
    out: &mut PackPlan,
) {
    let stem = |p: &Path| {
        p.file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    };
    let Some(ai) = raw
        .iter()
        .position(|r| r.role == RawMap::Ao && crate::group::is_generated(&stem(&r.path)))
    else {
        return;
    };
    let Some(img) = prepacked_image(ctx, set, files) else {
        return;
    };
    let aliased = prepacked_aliased(ctx, img);
    if aliased || !flat_b.contains(&img.path) {
        let ao = raw.remove(ai).path;
        let f = members.remove(&ao).expect("a raw member");
        jobs.retain(|j| j.input != ao);
        skipped.push(SkippedFile {
            path: ao.clone(),
            output: f.output.clone(),
            set: Some(set.key.clone()),
            code: Some(Code::GenReplaced),
            reason: format!(
                "generated {} replaced by {}{}",
                name(&ao),
                name(&img.path),
                if aliased { "" } else { " (B)" }
            ),
        });
        out.gen_replaced.push(ao);
        return;
    }
    let ao = &raw[ai].path;
    let lone = !raw.iter().any(|r| {
        &r.path != ao
            && matches!(
                r.role,
                RawMap::Roughness
                    | RawMap::Glossiness
                    | RawMap::Metallic
                    | RawMap::Specular
                    | RawMap::Fuzz
                    | RawMap::Orm
                    | RawMap::Ao
            )
    });
    if lone {
        raw.push(RawInput {
            path: img.path.clone(),
            role: RawMap::Rmaos,
        });
        members.insert(img.path.clone(), img);
    }
}

/// PACK-SIZE-MISMATCH for `output`, whose sources have different aspect ratios (spec §6.4 "Dimensions"; derive TA22;
/// Note FM30).
pub(crate) fn size_mismatch_warning(output: &Path, found: &[(PathBuf, u32, u32)]) -> Warning {
    let data: Vec<_> = found
        .iter()
        .map(|(p, w, h)| json!({ "path": slash(p), "width": w, "height": h }))
        .collect();
    Warning::new(
        Code::PackSizeMismatch,
        format!(
            "sources of {} have different aspect ratios: {}",
            name(output),
            sizes(found)
        ),
    )
    .with_data(json!({ "sources": data }))
}

/// PACK-SOURCE-RESIZED for `output`: each source of `found` smaller than `to` is enlarged to it (Note FM30). `None`
/// when every source already has that size.
pub(crate) fn resized_warning(
    output: &Path,
    found: &[(PathBuf, u32, u32)],
    to: (u32, u32),
) -> Option<Warning> {
    let small: Vec<_> = found.iter().filter(|f| (f.1, f.2) != to).cloned().collect();
    if small.is_empty() {
        return None;
    }
    let data: Vec<_> = small
        .iter()
        .map(|(p, w, h)| json!({ "path": slash(p), "width": w, "height": h }))
        .collect();
    Some(
        Warning::new(
            Code::PackSourceResized,
            format!(
                "{}: {} resized to {}x{}",
                name(output),
                sizes(&small),
                to.0,
                to.1
            ),
        )
        .with_data(json!({
            "output": slash(output),
            "width": to.0,
            "height": to.1,
            "sources": data,
        })),
    )
}

/// A recipe (packed or derived) that cannot be built: an error job carrying `w`, so `run` fails it. Its sub-type
/// and category come from the recipe (`recipe:<id>`), as for a recipe job that runs; the caller adds
/// `provenance["target"]`, as for every job.
pub(crate) fn recipe_error_job(
    ctx: &ResolveCtx,
    vin: &InputFile,
    set_key: &str,
    subtype: crate::config::Subtype,
    recipe_id: &str,
    w: Warning,
) -> ResolvedJob {
    let recipe = Provenance::Recipe(recipe_id.into());
    ResolvedJob {
        input: vin.path.clone(),
        output: vin.output.clone(),
        game_path: vin.game_path.clone(),
        set_key: set_key.to_string(),
        profile: ctx.registry.base_profile(ctx.target, subtype),
        provenance: BTreeMap::from([
            ("subtype".to_string(), recipe.clone()),
            ("category".to_string(), recipe),
        ]),
        suggestions: vec![],
        status: JobStatus::Error {
            codes: vec![w.code],
        },
        warnings: vec![w],
        suffix_alias: None,
        source: None,
        pack: None,
        bake: None,
        derive: None,
        shader: None,
        cube: None,
        fill: None,
        unknown_suffix: false,
    }
}

/// The packed output's pass 1 profile (DECISIONS Note M6): its `packing.*` and `channels.grayscale_source`.
fn pass1_profile(
    ctx: &ResolveCtx,
    set: &TextureSet,
    vin: &InputFile,
    kind: RecipeKind,
    pa: &Analysis,
    pinfo: &SourceInfo,
) -> Profile {
    let facts = FileFacts {
        input: vin,
        suffix: Some(kind.suffix()),
        alias: None,
        set,
        analysis: pa,
        info: pinfo,
    };
    layered_profile_placeholder(ctx, &facts, kind.subtype())
        .map(|(p, _)| p)
        .unwrap_or_else(|_| ctx.registry.base_profile(ctx.target, kind.subtype()))
}

/// The set's `packing.recipe` (cs only; DECISIONS Note R-fills-1) and a notice for the packed job when the
/// setting could not be followed as written. The packed output's `[files]` key carries its suffix, which is what
/// the recipe decides, so both candidate keys (`<base>_rfaos`, `<base>_rmaos`) are read: an explicit non-auto
/// value on either wins; if they disagree the `_rfaos` key wins and the notice names both. A project, preset or
/// `--set` value applies to both keys alike. `rfaos` on a set with only a metallic map cannot build `_rfaos`
/// (no metallic channel): it falls back to `auto` with a notice.
fn chosen_recipe(
    ctx: &ResolveCtx,
    set: &TextureSet,
    member: &dyn Fn(&Path) -> InputFile,
    raw: &[RawInput],
    sel: &BTreeMap<PathBuf, crate::helper::HelperSel>,
) -> (PackRecipe, Option<(Code, String)>) {
    // CO109: a pre-packed image keeps its own layout (`_rmaos` or `_rfaos`), whatever `packing.recipe` says.
    if ctx.target != Target::Cs || raw.iter().any(|r| r.role == RawMap::Rmaos) {
        return (PackRecipe::Auto, None);
    }
    // Note DY: a hand-set fill may build the `_rfaos`/`_rmaos` only under its own recipe (a lone `_specular` filling
    // `rfaos.a`), so those are probed after `auto`. Without hand-set fills they add nothing: their triggers are a
    // subset of `auto`'s.
    let probes = [PackRecipe::Auto, PackRecipe::Rfaos, PackRecipe::Rmaos].map(|p| {
        recipes_for_sel(ctx.target, raw, p, false, sel)
            .0
            .into_iter()
            .find(|r| matches!(r.kind, RecipeKind::Rfaos | RecipeKind::Rmaos))
    });
    let Some(base) = probes.iter().flatten().next() else {
        return (PackRecipe::Auto, None);
    };
    let first = member(&base.sources()[0].0);
    let (on_rfaos, on_rmaos) = written_recipes(ctx, set, &first);
    // Note DV1: a set whose chosen shader is Advanced Skin packs `_rfaos` unless a recipe is written; the user wrote
    // none, so a set that cannot build it keeps `auto` without a notice. Final review M2: a hand-set `rfaos.*` fill
    // also builds it; the plain test runs first so a hand fill never takes an `_rfaos` away (DV1 unchanged).
    if chosen_skin(ctx, set) && on_rfaos == PackRecipe::Auto && on_rmaos == PackRecipe::Auto {
        let rfaos = |s: &BTreeMap<PathBuf, crate::helper::HelperSel>| {
            recipes_for_sel(ctx.target, raw, PackRecipe::Rfaos, false, s)
                .0
                .iter()
                .any(|r| r.kind == RecipeKind::Rfaos)
        };
        let built = rfaos(&BTreeMap::new()) || rfaos(sel);
        let pick = if built {
            PackRecipe::Rfaos
        } else {
            PackRecipe::Auto
        };
        return (pick, None);
    }
    let mut note = None;
    let pick = if on_rfaos != PackRecipe::Auto {
        if on_rmaos != PackRecipe::Auto && on_rmaos != on_rfaos {
            let key = |kind| slash(&virtual_input(&first, kind).rel.with_extension(""));
            let (rfaos_key, rmaos_key) = (key(RecipeKind::Rfaos), key(RecipeKind::Rmaos));
            note = Some((Code::PackRecipeConflict, format!(
                "packing.recipe disagrees: {rfaos_key} says {}, {rmaos_key} says {}; the _rfaos entry wins",
                on_rfaos.id(),
                on_rmaos.id()
            )));
        }
        on_rfaos
    } else {
        on_rmaos
    };
    if pick == PackRecipe::Rfaos
        && !recipes_for_sel(ctx.target, raw, pick, false, sel)
            .0
            .iter()
            .any(|r| r.kind == RecipeKind::Rfaos)
    {
        // Trace-debug #34 (Note BQ): when `auto` builds the `_rfaos` anyway (a fuzz source), nothing was refused.
        if base.kind == RecipeKind::Rfaos {
            return (PackRecipe::Auto, None);
        }
        return (
            PackRecipe::Auto,
            Some((Code::PackRecipeUnbuildable, format!(
                "packing.recipe = rfaos needs a roughness, glossiness or AO source; built _{} instead",
                base.kind.suffix()
            ))),
        );
    }
    (pick, note)
}

/// The set's written `packing.recipe` at its two packed keys, `(on <base>_rfaos, on <base>_rmaos)`, read from the
/// pass 1 profile of the packed output named after `first`, a raw member of the set (`chosen_recipe`).
pub(crate) fn written_recipes(
    ctx: &ResolveCtx,
    set: &TextureSet,
    first: &InputFile,
) -> (PackRecipe, PackRecipe) {
    let (pa, pinfo) = placeholder();
    let read = |kind| {
        let vin = virtual_input(first, kind);
        pass1_profile(ctx, set, &vin, kind, &pa, &pinfo)
            .packing
            .recipe
    };
    (read(RecipeKind::Rfaos), read(RecipeKind::Rmaos))
}

/// Note DV1: the set's explicit shader (`ShaderOrigin::Set`) is Advanced Skin, so its diffuse reads `skin_albedo`
/// and it packs `_rfaos`; Note DW1: not while its diffuse member is pinned in the GUI.
pub fn chosen_skin(ctx: &ResolveCtx, set: &TextureSet) -> bool {
    ctx.shaders.get(&set.key).is_some_and(|p| {
        p.shader == crate::shader::Shader::AdvancedSkin
            && p.provenance == crate::shader::ShaderOrigin::Set
    }) && !set
        .members
        .iter()
        .any(|i| ctx.files.skin_pinned.contains(i))
}

/// Fuzz-9: the set's explicit shader (`ShaderOrigin::Set`) is in the PBR family (`Pbr`, `PbrGrass`, `PbrLandscape`).
pub fn chosen_pbr(ctx: &ResolveCtx, set: &TextureSet) -> bool {
    ctx.shaders.get(&set.key).is_some_and(|p| {
        p.provenance == crate::shader::ShaderOrigin::Set && p.shader.is_pbr_family()
    })
}

/// Whether the set is Advanced skin (DECISIONS Note R-skin-1): shared skin (Note HG2); a fuzz map with other skin
/// evidence (Fuzz-5: a written `rfaos` on either packed key, or a member typed a skin sub-type); a hand-set
/// `helper.role = "fuzz"` (Fuzz-22); an `_rfaos` member; or a raw-map recipe other than a fuzz map's that resolves to
/// `_rfaos` (Note DV1's chosen Advanced Skin). A fuzz map alone is no evidence (Fuzz-1). Called at suggestion time,
/// before Pass 1, from the raw roles the set's suffixes name; `this` is any member of the set (it supplies the folder
/// and base the packed output's `[files]` key is read from).
///
/// `set.suffixes` also holds the suffixes `auto` predicts from the raw roles (DECISIONS Note M7). Without a fuzz map
/// `auto` never builds `_rfaos` (Fuzz-1), and `api::predict_suffixes` predicts an `rfaos` from a fuzz map only when the
/// set already reads it as the skin mask (this answer, on the set before prediction), so an `rfaos` in the suffixes is
/// either a real `_rfaos` member or the prediction of a set that is already skin.
pub fn set_is_advanced_skin(ctx: &ResolveCtx, set: &TextureSet, this: &InputFile) -> bool {
    if ctx.target != Target::Cs {
        return false;
    }
    if set.shared_skin {
        return true;
    }
    // The packed output's key is built from a raw map's name, so give it one: `this` may be `x_sk`, `x_wet` or `x`.
    // Built only where a recipe is read (this runs per file from `suggest`).
    let renamed = || {
        let stem = this
            .rel
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let base = member_split(&stem, set, ctx.registry.alias_table()).base;
        let ext = this
            .rel
            .extension()
            .map_or_else(|| "png".to_string(), |e| e.to_string_lossy().into_owned());
        let mut this = this.clone();
        this.rel = this.rel.with_file_name(format!("{base}_ao.{ext}"));
        this
    };
    // Fuzz-5: the evidence that keeps a fuzz map the skin mask. Only in a set with a fuzz map, so no other set's
    // reading changes (a metallic-only set with a written `rfaos` stays `_rmaos`, DQ2).
    if has_fuzz_map(set) {
        let (on_rfaos, on_rmaos) = written_recipes(ctx, set, &renamed());
        if on_rfaos == PackRecipe::Rfaos || on_rmaos == PackRecipe::Rfaos {
            return true;
        }
        if set.members.iter().any(|i| ctx.files.skin_typed.contains(i)) {
            return true;
        }
    }
    // Fuzz-22: a hand-set fuzz role is skin evidence; its file need not carry the `_fuzz` suffix (Note DY).
    if set
        .members
        .iter()
        .chain(&set.shares)
        .any(|i| ctx.files.hand_fuzz.contains(i))
    {
        return true;
    }
    // A real `_rfaos` member, with or without a fuzz map (JQ48); see the doc comment for a predicted one.
    // shortcut: an in-place skin project's own earlier `<base>_rfaos.dds` (a kept DDS) counts here too, so such a set
    // stays Advanced Skin until the file is deleted; upgrade when the planner can tell an output the manifest records
    // as Tessalith's from a hand-made file (follow-up F14), and leave that one out.
    if set.suffixes.contains("rfaos") {
        return true;
    }
    // Fuzz-1: a fuzz map takes no part in the recipe test: `auto` with a fuzz map alone builds `_rmaos`.
    let roles: BTreeSet<RawMap> = RawMap::INPUTS
        .into_iter()
        .filter(|&r| r != RawMap::Fuzz && set.suffixes.contains(&r.id()))
        .collect();
    let raw: Vec<RawInput> = roles
        .iter()
        .map(|&role| RawInput {
            path: PathBuf::from(format!("{}.{}", set.key, role.id())),
            role,
        })
        .collect();
    if raw.is_empty() {
        return false;
    }
    let this = renamed();
    let (choice, _) = chosen_recipe(ctx, set, &|_| this.clone(), &raw, &BTreeMap::new());
    recipes_for_with(ctx.target, &raw, choice, false)
        .0
        .iter()
        .any(|r| r.kind == RecipeKind::Rfaos)
}

/// What a set's fuzz map (`_fuzz`/`_sheen`) is (Fuzz-1, Fuzz-2, Fuzz-5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuzzReading {
    /// The Advanced Skin mask: a packing input of the `_rfaos` G channel.
    Skin,
    /// The PBR fuzz texture (`pbr_fuzz`), written `<base>_f.dds`.
    Pbr,
    /// Nothing reads it under this shader.
    Unused(crate::shader::Shader),
}

/// How the set reads its fuzz map `this` (Fuzz-2, Fuzz-3): not on cs it is never read. An explicit pick decides
/// first: the PBR family reads it as the PBR fuzz texture, Advanced Skin ([`chosen_skin`]) as the skin mask, any other
/// shader not at all. Then skin evidence ([`set_is_advanced_skin`]) makes it the skin mask, ahead of an automatic
/// pick; an automatic pick reads it as the explicit one would. With no pick (pass 1) it is the PBR fuzz texture.
/// An explicit Advanced Skin whose diffuse is pinned (Note DW1) has no arm of its own: it reads as if unpicked.
pub fn fuzz_reading(ctx: &ResolveCtx, set: &TextureSet, this: &InputFile) -> FuzzReading {
    use crate::shader::{Shader, ShaderOrigin};
    if ctx.target != Target::Cs {
        return FuzzReading::Unused(Shader::Default);
    }
    if chosen_pbr(ctx, set) {
        return FuzzReading::Pbr;
    }
    if chosen_skin(ctx, set) {
        return FuzzReading::Skin;
    }
    let pick = ctx.shaders.get(&set.key);
    if let Some(p) = pick.filter(|p| p.provenance == ShaderOrigin::Set) {
        if p.shader != Shader::AdvancedSkin {
            return FuzzReading::Unused(p.shader);
        }
    }
    if set_is_advanced_skin(ctx, set, this) {
        return FuzzReading::Skin;
    }
    match pick.filter(|p| p.provenance != ShaderOrigin::Set) {
        Some(p) if p.shader.is_pbr_family() => FuzzReading::Pbr,
        Some(p) if p.shader == Shader::AdvancedSkin => FuzzReading::Skin,
        Some(p) => FuzzReading::Unused(p.shader),
        None => FuzzReading::Pbr,
    }
}

/// Fuzz-4 (Note JQ53): the set's own `_f` members, excluding a fuzz map's earlier output.
pub(crate) fn pbr_f_members<'a>(
    ctx: &'a ResolveCtx,
    set: &'a TextureSet,
    files: &'a [InputFile],
) -> impl Iterator<Item = &'a InputFile> {
    let members: &[_] = if set.suffixes.contains("f") {
        &set.members
    } else {
        &[]
    };
    let aliases = ctx.registry.alias_table();
    members.iter().map(move |&i| &files[i]).filter(move |f| {
        split_suffix_with(
            &f.rel.file_stem().unwrap_or_default().to_string_lossy(),
            aliases,
        )
        .suffix
        .as_deref()
            == Some("f")
            && !crate::group::is_fuzz_own_f(set, files, f)
    })
}

/// Note DR: whether `f` is a `_f` that a set already Advanced skin packs into the `_rfaos` G channel (a `_f` whose
/// sub-type was requested is converted alone). `policy::resolve_file_with` applies the same test to a planned file.
pub fn is_skin_f(ctx: &ResolveCtx, set: &TextureSet, f: &InputFile) -> bool {
    skin_f_candidate(ctx, set, f) && !crate::policy::subtype_requested(ctx, f)
}

/// The part of the skin `_f` test that needs no provenance: an image with suffix `f` in an Advanced skin set that has
/// no `_fuzz`/`_sheen` raw map (that one fills G, Note DR7). Policy adds its provenance check to this.
pub fn skin_f_candidate(ctx: &ResolveCtx, set: &TextureSet, f: &InputFile) -> bool {
    let stem = f
        .rel
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    f.kind == InputKind::Image
        && split_suffix_with(&stem, ctx.registry.alias_table())
            .suffix
            .as_deref()
            == Some("f")
        && !has_fuzz_map(set)
        && set_is_advanced_skin(ctx, set, f)
}

/// The set lists a fuzz map, its own or one shared in (Note HH1): the gate of `set_is_advanced_skin`'s Fuzz-5
/// evidence, and of the fast re-plan's `FUZZ_SUBTYPE` check (`skeleton.rs`), so the two never disagree.
pub(crate) fn has_fuzz_map(set: &TextureSet) -> bool {
    set.suffixes.contains(&RawMap::Fuzz.id())
}

/// Pass 1 over every set. `jobs` holds every resolved job of the batch and is changed in place: consumed raw jobs
/// are removed, unused ones re-tagged `PACK-UNUSED-INPUT` (`IN-PACKING-INPUT` when their recipe is disabled), a
/// pre-packed job gets `PACK-CONFLICT` or (with `packing.prefer = raw`) moves to `skipped`, and a recipe whose
/// sources of different aspect ratios becomes an error job. `failed` holds the paths that failed planning and `cancelled` those
/// whose Load never started because the batch was cancelled: a recipe with a member in either is `Failed` (Note M11).
/// `packed_outputs` holds explicit GUI output paths of packed outputs by (set key, suffix), M3b. `derive_heights`
/// holds the raw heights of the landscape sets a Vanilla derive converts (Note CN26, O6): each gets the cs
/// `Parallax` recipe, which the vanilla target otherwise never plans. It is empty without `derive` (C6).
/// `flat_b` holds the pass-1 images whose B is empty (`!pack::b_has_content`, Note CO103), `flat_a` those whose alpha
/// is flat (Note HC1). `output_root` turns set keys into the set ids a fuzz-map message names.
#[allow(clippy::too_many_arguments)]
pub fn plan_packing(
    ctx: &ResolveCtx,
    sets: &[TextureSet],
    files: &[InputFile],
    jobs: &mut Vec<ResolvedJob>,
    failed: &[PathBuf],
    cancelled: &[PathBuf],
    skipped: &mut Vec<SkippedFile>,
    packed_outputs: &BTreeMap<(String, String), PathBuf>,
    derive_heights: &BTreeSet<PathBuf>,
    flat_b: &BTreeSet<PathBuf>,
    flat_a: &BTreeSet<PathBuf>,
    fills: &crate::helper::plan::FillPlan,
    output_root: &Path,
) -> PackPlan {
    let (pa, pinfo) = placeholder();
    let mut out = PackPlan {
        candidates: vec![],
        reports: BTreeMap::new(),
        gen_replaced: vec![],
        lost: vec![],
    };
    let mut consumed: BTreeSet<PathBuf> = BTreeSet::new();
    let mut heads = vec![];
    // Audit EP-5 (Note BJ): the waiting raw inputs, collected once; `jobs` does not change until the planning loop.
    // Note DR: the role comes from the job too, because a skin `_f` has no file-name role.
    let waiting: BTreeMap<PathBuf, RawMap> = jobs
        .iter()
        .filter_map(|j| match j.status {
            JobStatus::PackingInput { role } => Some((j.input.clone(), role)),
            _ => None,
        })
        .collect();
    // A kept _f.dds supplies fuzz by name. Converted inputs have a resolved subtype.
    let kept_dds = skipped
        .iter()
        .filter(|s| s.code == Some(Code::InDdsSkipped))
        .map(|s| (s.path.clone(), Subtype::PbrFuzz));
    let subtypes: BTreeMap<_, _> = jobs
        .iter()
        .map(|j| (j.input.clone(), j.profile.subtype))
        .chain(kept_dds)
        .collect();
    let renamed: BTreeMap<PathBuf, InputFile> = sets
        .iter()
        .flat_map(|set| joined_helper_names(ctx, set, files))
        .collect();
    // Note HH2: one stand-in per (set, shared file), indexed like `sets`.
    let stand_ins: Vec<BTreeMap<PathBuf, InputFile>> = sets
        .iter()
        .map(|set| {
            set.shares
                .iter()
                .map(|&i| {
                    (
                        files[i].path.clone(),
                        shared_stand_in(set, files, &files[i]),
                    )
                })
                .collect()
        })
        .collect();
    // Fuzz-2, Fuzz-3: per waiting fuzz map, each set that does not read it as the skin mask (set key, reading); per
    // set, why its mask has no shared fuzz source; and every file some set takes as a raw input.
    let mut unread: BTreeMap<PathBuf, Vec<(String, FuzzReading)>> = BTreeMap::new();
    let mut lent: BTreeMap<String, String> = BTreeMap::new();
    // Fuzz-4: a fuzz map a PBR set's own `_f` keeps out, with the reason (F15).
    let mut f_wins: BTreeMap<PathBuf, String> = BTreeMap::new();
    let mut in_raw: BTreeSet<PathBuf> = BTreeSet::new();
    for (set, stand_in) in sets.iter().zip(&stand_ins) {
        // Raw members still waiting for a recipe: a planned packing input, or one that failed to load or was
        // cancelled before its Load. A raw map whose sub-type a rule or `--set` named is an ordinary file
        // (spec §4.4) and takes no part. A shared file (Note HH1) takes part as a member does.
        let mut raw = vec![];
        let mut members: BTreeMap<PathBuf, &InputFile> = BTreeMap::new();
        let mut sel = BTreeMap::new();
        for &i in set.members.iter().chain(&set.shares) {
            let f = &files[i];
            // Images only: a re-encoded raw-map DDS keeps its old behaviour (no part in packing). The job's role comes
            // first: a hand-set `helper.role` (Note DY) overrides the name's.
            let image = f.kind == InputKind::Image;
            let by_name = || {
                raw_role(f).or_else(|| {
                    // A failed or cancelled file has no job, so a skin `_f` is recognised by the same test as policy.
                    // shortcut: a rule-named sub-type needs the analysis a failed file lacks, so it is not seen here.
                    ((failed.contains(&f.path) || cancelled.contains(&f.path))
                        && is_skin_f(ctx, set, f))
                    .then_some(RawMap::Fuzz)
                })
            };
            let role = match image.then(|| waiting.get(&f.path).copied()).flatten() {
                Some(r) => Some(r),
                // A failed or cancelled image has no job: its hand-set role decides as policy's would; `none` or a
                // layer role leaves it out of packing.
                None => match image
                    .then(|| crate::policy::helper_requested(ctx, f).role)
                    .flatten()
                {
                    Some(crate::helper::HelperRole::Raw(r)) => Some(r),
                    Some(_) => None,
                    None => by_name(),
                },
            };
            let Some(role) = role else {
                continue;
            };
            // Fuzz-2, Fuzz-3: a fuzz map the set does not read as the skin mask is no packing input, also when it
            // failed or was cancelled (its role then comes from its name). Under another shader it is listed unused.
            // A hand-set role is outside this (spec §3a).
            if role == RawMap::Fuzz
                && ctx.target == Target::Cs
                && crate::group::is_fuzz_map_image(f)
                && crate::policy::helper_requested(ctx, f).role.is_none()
            {
                let reading = fuzz_reading(ctx, set, f);
                // Fuzz-4: under PBR the set's own `_f` wins; the fuzz map is listed unused after the loop.
                if reading == FuzzReading::Pbr && waiting.contains_key(&f.path) {
                    if let Some(fm) = pbr_f_members(ctx, set, files)
                        .min_by_key(|m| (subtypes.get(&m.path) != Some(&Subtype::PbrFuzz), &m.path))
                    {
                        let n = name(&fm.path);
                        let reason = match subtypes.get(&fm.path) {
                            Some(Subtype::PbrFuzz) => {
                                format!("{n} is the set's PBR fuzz texture (_f)")
                            }
                            Some(st) => format!(
                                "{n} ({}) is written at the set's _f path",
                                json!(st).as_str().unwrap_or_default()
                            ),
                            None if failed.contains(&fm.path) => {
                                format!("{n}, the set's _f, failed to load")
                            }
                            None if cancelled.contains(&fm.path) => {
                                format!("{n}, the set's _f, was cancelled")
                            }
                            None => format!("{n}, the set's _f, is skipped"),
                        };
                        f_wins.entry(f.path.clone()).or_insert(reason);
                        continue;
                    }
                }
                if reading != FuzzReading::Skin {
                    // Listed unused after the loop, with every set that does not read it (Note HH1: one may lend it).
                    if waiting.contains_key(&f.path) {
                        unread
                            .entry(f.path.clone())
                            .or_default()
                            .push((set.key.clone(), reading));
                    }
                    continue;
                }
                // shortcut: a file has one job (a packed source has no job of its own: the run's terminal events,
                // `api.rs`), so a shared fuzz map its own set converts is no source of this set's mask; its G
                // PACK-DEFAULTED says why. The mirror is silent: a lender that packs it into its own `_rfaos` leaves
                // a PBR borrower no `<base>_f.dds` and no row (the file's job is consumed, so nothing carries one).
                // A set whose only fuzz map is shared in gets no Fuzz-1a pick (`api::pbr_read_fuzz`). Upgrade all
                // three when a packed source may also keep its own job (follow-up F10).
                let own_job = !waiting.contains_key(&f.path)
                    && !failed.contains(&f.path)
                    && !cancelled.contains(&f.path);
                if own_job && set.shares.contains(&i) {
                    if let Some(lender) = sets.iter().find(|s| s.members.contains(&i)) {
                        let shader = ctx
                            .shaders
                            .get(&lender.key)
                            .map(|p| format!(" ({} shader)", p.shader.id()))
                            .unwrap_or_default();
                        lent.insert(
                            set.key.clone(),
                            format!(
                                "the shared {} is converted by its own set {}{shader} and is not read here",
                                name(&f.path),
                                crate::sets::set_id(output_root, &lender.key)
                            ),
                        );
                    }
                }
            }
            if waiting.contains_key(&f.path)
                || failed.contains(&f.path)
                || cancelled.contains(&f.path)
            {
                raw.push(RawInput {
                    path: f.path.clone(),
                    role,
                });
                in_raw.insert(f.path.clone());
                let named = stand_in.get(&f.path).or_else(|| renamed.get(&f.path));
                members.insert(f.path.clone(), named.unwrap_or(f));
                // Note DY: its hand-set fills, read and invert.
                let s = crate::policy::helper_requested(ctx, f);
                if s != crate::helper::HelperSel::default() {
                    sel.insert(f.path.clone(), s);
                }
            }
        }
        if ctx.target == Target::Cs {
            prepacked_b(
                ctx,
                set,
                files,
                flat_b,
                &mut raw,
                &mut members,
                jobs,
                skipped,
                &mut out,
            );
            prepacked_a(ctx, set, files, flat_a, &mut raw, &mut members);
        }
        if raw.is_empty() {
            continue;
        }
        let (choice, recipe_note) = chosen_recipe(ctx, set, &|p| members[p].clone(), &raw, &sel);
        // Note AZ: a vanilla set whose shader pick is complex material packs the complex `_m`.
        let complex = ctx
            .shaders
            .get(&set.key)
            .is_some_and(|p| p.shader == crate::shader::Shader::ComplexMaterial);
        let (mut recipes, mut unused) = recipes_for_sel(ctx.target, &raw, choice, complex, &sel);
        if let Some(i) = unused
            .iter()
            .position(|u| u.role == RawMap::Height && derive_heights.contains(&u.path))
        {
            let h = unused.remove(i);
            recipes.push(super::Recipe {
                kind: RecipeKind::Parallax,
                channels: [
                    super::ChannelNeed::Source {
                        path: h.path,
                        role: RawMap::Height,
                        invert: false,
                        channel: None,
                    },
                    super::ChannelNeed::MirrorR,
                    super::ChannelNeed::MirrorR,
                    super::ChannelNeed::Opaque,
                ],
            });
        }
        heads.push((set, members, recipe_note, recipes, unused));
    }
    // Every recipe's output (explicit or default) before any is planned: an explicit packed path is compared with
    // recipes planned after it too (DECISIONS AF7). A recipe that writes nothing (a failed or cancelled source,
    // packing off, sources of different aspect ratios) holds no path. The planning loop below walks `heads` in this same
    // order, so a recipe's index here is its `me` there.
    let mut outputs: Vec<(PathBuf, PathBuf, bool)> = vec![];
    for (set, members, _, recipes, _) in &heads {
        for recipe in recipes {
            let sources: Vec<PathBuf> = recipe.sources().into_iter().map(|(p, _)| p).collect();
            let mut vin = virtual_input(members[&sources[0]], recipe.kind);
            let key = (set.key.clone(), recipe.kind.suffix().to_string());
            if let Some(p) = packed_outputs.get(&key) {
                vin.output = p.clone();
            }
            let writes = !sources
                .iter()
                .any(|p| failed.contains(p) || cancelled.contains(p))
                && pass1_profile(ctx, set, &vin, recipe.kind, &pa, &pinfo)
                    .packing
                    .enabled
                && !matches!(check_sizes(&sources), Err(PackError::SizeMismatch(_)));
            outputs.push((vin.output, vin.path, writes));
        }
    }
    let mut next = 0;
    for (set, members, recipe_note, recipes, unused) in heads {
        let mut unused: Vec<(PathBuf, String)> =
            unused.into_iter().map(|u| (u.path, u.reason)).collect();
        let mut reports = vec![];
        for recipe in recipes {
            let me = next;
            next += 1;
            let sources: Vec<PathBuf> = recipe.sources().into_iter().map(|(p, _)| p).collect();
            let mut vin = virtual_input(members[&sources[0]], recipe.kind);
            let mut explicit = false;
            if let Some(p) =
                packed_outputs.get(&(set.key.clone(), recipe.kind.suffix().to_string()))
            {
                vin.output = p.clone();
                explicit = true;
            }
            let st = recipe.kind.subtype();
            // Pass 1: the packed output's own profile decides `packing.*` (DECISIONS Note M6).
            let profile = pass1_profile(ctx, set, &vin, recipe.kind, &pa, &pinfo);
            let (packing, gray) = (profile.packing, profile.channels.grayscale_source);
            let (channels, defaulted) = resolve_channels(&recipe, &packing);
            let mut report = RecipeReport {
                output: slash(&vin.output),
                subtype: st,
                channels: channels
                    .iter()
                    .map(|c| {
                        c.provenance(gray)
                            .map(|p| p.to_string())
                            .unwrap_or_default()
                    })
                    .collect(),
                status: RecipeStatus::Run,
                reason: None,
            };
            // Audit 2026-10-14 finding 4: an explicit packed path (GUI `OutputKey::Packed`) is set here, after
            // step 5's duplicate check ran on the files, so it is checked against every other output now. Two
            // jobs on one path would both write it (the last rename wins) and append two manifest records.
            if explicit {
                // A pre-packed image this recipe reads (Note CO104) is a source, not another writer.
                let taken =
                    other_output(&vin.output, jobs, &outputs, me).filter(|o| !sources.contains(o));
                if let Some(other) = taken {
                    report.status = RecipeStatus::Failed;
                    report.reason = Some(format!(
                        "output path {} is also written by {}",
                        name(&vin.output),
                        name(&other)
                    ));
                    reports.push(report);
                    skipped.push(SkippedFile {
                        path: vin.path.clone(),
                        output: vin.output.clone(),
                        set: Some(set.key.clone()),
                        code: Some(Code::InDuplicate),
                        reason: format!(
                            "two outputs map to one path ({} and {}); change the packed output path",
                            name(&vin.output),
                            name(&other)
                        ),
                    });
                    consumed.extend(sources.iter().cloned());
                    continue;
                }
            }
            // DECISIONS Note M11: a source that failed to load fails the recipe. So does one whose Load was
            // cancelled: the report must not claim `default:` channels for sources that exist.
            if let Some(bad) = sources.iter().find(|p| failed.contains(p)) {
                report.status = RecipeStatus::Failed;
                report.reason = Some(format!("source {} could not be read", name(bad)));
                reports.push(report);
                out.lost.extend(sources.iter().map(|p| {
                    (
                        set.key.clone(),
                        recipe.kind,
                        p.clone(),
                        members[p].output.clone(),
                    )
                }));
                consumed.extend(sources.iter().cloned());
                continue;
            }
            if sources.iter().any(|p| cancelled.contains(p)) {
                report.status = RecipeStatus::Failed;
                report.reason = Some("cancelled".into());
                reports.push(report);
                out.lost.extend(sources.iter().map(|p| {
                    (
                        set.key.clone(),
                        recipe.kind,
                        p.clone(),
                        members[p].output.clone(),
                    )
                }));
                consumed.extend(sources.iter().cloned());
                continue;
            }
            if !packing.enabled {
                report.status = RecipeStatus::Disabled;
                report.reason = Some("packing.enabled = false".into());
                reports.push(report);
                for p in &sources {
                    retag(
                        jobs,
                        p,
                        Code::InPackingInput,
                        format!(
                            "{}: packing is off for {} (packing.enabled = false); a raw map is not converted on its own",
                            name(p),
                            name(&vin.output)
                        ),
                    );
                }
                continue;
            }
            // Headers only. Checked before the conflict below so that a recipe which cannot be packed never
            // replaces the pre-packed file the user had (DECISIONS Note M9: never write neither).
            let (mismatch, resized) = match check_file_sizes(&sources) {
                Err(PackError::SizeMismatch(found)) => (Some(found), None),
                Ok((to, found)) => (None, resized_warning(&vin.output, &found, to)),
                Err(_) => (None, None),
            };
            let mut kept = None;
            let pre = prepacked_member(ctx, set, files, jobs, recipe.kind);
            // Note CO104: a pre-packed image that is a channel source of this recipe is consumed like a raw map
            // (`PackedSource`), never a conflict; on a size mismatch its own job stays, as M9 keeps one.
            if let Some(own) = pre.filter(|f| sources.contains(&f.path)) {
                if mismatch.is_some() {
                    kept = Some(name(&own.path));
                } else {
                    jobs.retain(|j| j.input != own.path);
                }
            } else if let Some(pre) = pre {
                let pre_output = pre.output.clone();
                let pre = pre.path.clone();
                report.status = RecipeStatus::Conflict;
                if packing.prefer == PackPrefer::Prepacked {
                    report.reason = Some(format!(
                        "{} is used (packing.prefer = prepacked)",
                        name(&pre)
                    ));
                    reports.push(report);
                    if let Some(j) = jobs.iter_mut().find(|j| j.input == pre) {
                        j.warnings.push(Warning::new(
                            Code::PackConflict,
                            format!(
                                "{} and the raw maps {} both make {}; the pre-packed file is used (packing.prefer = prepacked)",
                                name(&pre),
                                list(&sources),
                                name(&vin.output)
                            ),
                        ));
                    }
                    for p in &sources {
                        unused.push((
                            p.clone(),
                            format!(
                                "the pre-packed {} is used (packing.prefer = prepacked)",
                                name(&pre)
                            ),
                        ));
                    }
                    continue;
                }
                if mismatch.is_some() {
                    // The recipe fails below; the pre-packed job stays: no `retain`, no `PACK-CONFLICT` skip.
                    kept = Some(name(&pre));
                } else {
                    report.reason =
                        Some(format!("{} is replaced (packing.prefer = raw)", name(&pre)));
                    jobs.retain(|j| j.input != pre);
                    skipped.push(SkippedFile {
                        path: pre.clone(),
                        output: pre_output,
                        set: Some(set.key.clone()),
                        code: Some(Code::PackConflict),
                        reason: format!(
                            "replaced by {} packed from {} (packing.prefer = raw)",
                            name(&vin.output),
                            list(&sources)
                        ),
                    });
                }
            }
            consumed.extend(sources.iter().cloned());
            if let Some(found) = mismatch {
                report.status = RecipeStatus::Failed;
                report.reason = Some(match kept {
                    Some(pre) => {
                        format!(
                            "sources have different aspect ratios; {pre} is kept (packing.prefer = raw)"
                        )
                    }
                    None => "sources have different aspect ratios".into(),
                });
                reports.push(report);
                jobs.push(recipe_error_job(
                    ctx,
                    &vin,
                    &set.key,
                    st,
                    recipe.kind.id(),
                    size_mismatch_warning(&vin.output, &found),
                ));
                continue;
            }
            reports.push(report);
            let job = PackJob {
                kind: recipe.kind,
                channels,
                gray,
                sources: recipe
                    .sources()
                    .into_iter()
                    .map(|(path, role)| PackSource {
                        stamp: SourceStamp::of(&path),
                        // Note DY: every source's explicit read, a pre-packed `_rmaos`/`_rfaos` image too
                        // (`prepacked_b`), whose channels it does not change.
                        read: files
                            .iter()
                            .find(|f| f.path == path && f.kind == InputKind::Image)
                            .and_then(|f| crate::policy::helper_requested(ctx, f).read),
                        path,
                        role,
                    })
                    .collect(),
            };
            out.candidates.push(Candidate {
                set_key: set.key.clone(),
                kind: recipe.kind,
                virtual_input: vin,
                job,
                defaulted,
                resized,
                notes: recipe_note
                    .iter()
                    .filter(|_| matches!(recipe.kind, RecipeKind::Rfaos | RecipeKind::Rmaos))
                    .cloned()
                    .collect(),
                fuzz_why: lent.get(&set.key).cloned(),
            });
        }
        for (p, reason) in unused {
            // Note DY: an alpha fill consumes it (below); otherwise its alpha reason says more than packing's.
            if fills.used.contains(&p) {
                continue;
            }
            let reason = fills.not_used.get(&p).cloned().unwrap_or(reason);
            if !failed.contains(&p) {
                retag_unused(jobs, &p, &reason);
            }
        }
        out.reports.insert(set.key.clone(), reports);
    }
    // Fuzz-3: a fuzz map no set takes as a raw input is listed unused when some set's shader reads no fuzz map. A
    // set that would read it as its PBR fuzz texture is named too (it is shared in: only its own set converts it).
    for (p, mut readers) in unread {
        if in_raw.contains(&p)
            || !readers
                .iter()
                .any(|(_, r)| matches!(r, FuzzReading::Unused(_)))
        {
            continue;
        }
        let own = |key: &str| {
            sets.iter()
                .any(|s| s.key == key && s.members.iter().any(|&m| files[m].path == p))
        };
        readers.sort_by_key(|(key, _)| !own(key));
        let reason = if let [(_, FuzzReading::Unused(sh))] = readers.as_slice() {
            format!("the {} shader reads no fuzz map", sh.id())
        } else {
            readers
                .iter()
                .map(|(key, r)| {
                    let id = crate::sets::set_id(output_root, key);
                    match r {
                        FuzzReading::Unused(sh) => {
                            format!("the {} shader of set {id} reads no fuzz map", sh.id())
                        }
                        _ => {
                            let sh = ctx.shaders.get(key).map_or("its", |p| p.shader.id());
                            format!(
                                "set {id} ({sh} shader) would read it as its fuzz texture, which only the file's own \
                                 set can convert"
                            )
                        }
                    }
                })
                .collect::<Vec<_>>()
                .join(", and ")
        };
        retag_unused(jobs, &p, &reason);
    }
    // Fuzz-4: a fuzz map beside the set's own `_f` under PBR is listed unused, unless another set takes it as a source.
    // Last on purpose: when a shared fuzz map is also in `unread`, its own set's `_f` is the reason that holds for the
    // file's own job, so this retag replaces the one above.
    for (p, reason) in f_wins {
        if !in_raw.contains(&p) {
            retag_unused(jobs, &p, &reason);
        }
    }
    // Note DY: a helper an alpha fill uses is consumed too, whatever packing did with it.
    consumed.extend(fills.used.iter().cloned());
    // DECISIONS Note M8: the recipe job stands for the raw jobs it consumed.
    jobs.retain(|j| {
        !(consumed.contains(&j.input) && matches!(j.status, JobStatus::PackingInput { .. }))
    });
    out
}

/// Pass 2 for one candidate: resolve the assembled image as the packed sub-type (provenance `recipe:<kind>`),
/// then record the channel provenance (`pack.r` …), one `PACK-DEFAULTED` per defaulted channel and one
/// `CHAN-NOT-GRAY` per non-grey source. The caller adds `provenance["target"]`, as for every job. Everything about
/// the packed channels uses `c.job.gray`, never the final profile's `channels.grayscale_source` (Note M6).
pub fn finish_recipe(
    ctx: &ResolveCtx,
    c: &Candidate,
    set: &TextureSet,
    analysis: &Analysis,
    info: &SourceInfo,
    non_gray: &[PathBuf],
) -> ResolvedJob {
    let facts = FileFacts {
        input: &c.virtual_input,
        suffix: Some(c.kind.suffix()),
        alias: None,
        set,
        analysis,
        info,
    };
    let decision = SubtypeDecision {
        subtype: c.kind.subtype(),
        provenance: Provenance::Recipe(c.kind.id().into()),
        decided: true,
        suggestions: vec![],
        waits: false,
    };
    let mut job = resolve_decided(ctx, &facts, decision);
    for (ch, src) in ["r", "g", "b", "a"].iter().zip(&c.job.channels) {
        if let Some(p) = src.provenance(c.job.gray) {
            job.provenance.insert(format!("pack.{ch}"), p);
        }
    }
    let out = name(&c.virtual_input.output);
    for &(i, slot) in &c.defaulted {
        if let ChannelSource::Default { value } = &c.job.channels[i] {
            let mut message = format!(
                "{out} {}: no {} source; neutral default {value}",
                CHANNELS[i],
                slot.id()
            );
            let mut data = json!({
                "output": slash(&c.virtual_input.output),
                "channel": CHANNELS[i],
                "slot": slot,
                "value": value,
            });
            if let (Slot::Fuzz, Some(why)) = (slot, &c.fuzz_why) {
                message = format!("{message}; {why}");
                data["why"] = json!(why);
            }
            job.warnings
                .push(Warning::new(Code::PackDefaulted, message).with_data(data));
        }
    }
    if let ChannelSource::Source {
        path,
        role: RawMap::Ao,
        ..
    } = &c.job.channels[2]
    {
        if crate::group::is_generated(&path.file_stem().unwrap_or_default().to_string_lossy()) {
            let file = name(path);
            job.warnings.push(
                Warning::new(
                    Code::GenAo,
                    format!(
                        "{out} B: AO generated from the normal map ({file}); a baked AO is better"
                    ),
                )
                .with_data(json!({
                    "output": slash(&c.virtual_input.output),
                    "channel": "B",
                    "source": file,
                })),
            );
        }
    }
    for (code, note) in &c.notes {
        job.warnings
            .push(Warning::new(*code, format!("{out}: {note}")));
    }
    job.warnings.extend(c.resized.clone());
    for p in non_gray {
        let pick = c.job.gray;
        let why = match pick {
            GraySource::R | GraySource::Luma => format!(
                "R/G/B differ by more than 4/255; packing its {}",
                pick.label()
            ),
            // An A pick on a file with no alpha channel lands here too: it reads 255 everywhere.
            _ => format!(
                "its {} holds one value everywhere{}; packing it as it is",
                pick.label(),
                if pick == GraySource::A {
                    " (or the file has no alpha channel)"
                } else {
                    ""
                }
            ),
        };
        job.warnings.push(Warning::new(
            Code::ChanNotGray,
            format!("{}: {why}", name(p)),
        ));
    }
    job.pack = Some(c.job.clone());
    job
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Subtype;

    fn job(input: &str, game_path: &str) -> ResolvedJob {
        ResolvedJob {
            input: input.into(),
            output: PathBuf::new(),
            game_path: game_path.into(),
            set_key: String::new(),
            profile: crate::presets::PresetRegistry::builtin()
                .unwrap()
                .base_profile(Target::Cs, Subtype::Diffuse),
            provenance: Default::default(),
            suggestions: vec![],
            warnings: vec![],
            status: JobStatus::Ready,
            suffix_alias: None,
            source: None,
            pack: None,
            bake: None,
            derive: None,
            shader: None,
            cube: None,
            fill: None,
            unknown_suffix: false,
        }
    }

    /// Note HS2: the own stem when the names agree but for letter case (ASCII or not), else the job's own name.
    #[test]
    fn files_stem_keeps_the_own_case_and_takes_a_set_name() {
        let stem =
            |input: &str, game_path: &str| files_stem(&job(input, game_path), RecipeKind::Rmaos);
        assert_eq!(
            stem("a/Rock_roughness.png", "textures/a/rock_rmaos.png"),
            "Rock_rmaos"
        );
        // `discover::game_path` folds ASCII only.
        assert_eq!(
            stem("a/ÄRMEL.png", "textures/a/Ärmel_rmaos.png"),
            "ÄRMEL_rmaos"
        );
        assert_eq!(
            stem("a/rock_roughness.png", "textures/c/slab_rmaos.png"),
            "slab_rmaos"
        );
        assert_eq!(stem("a/Rock_roughness.png", ""), "Rock_rmaos");
    }
}
