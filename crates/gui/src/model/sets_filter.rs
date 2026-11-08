//! The Sets column's search box, filter pills and shader filter (layout v2, owner addition R17). Filters only hide:
//! nothing here touches the selection, the multi-selection or a document (Review Focus 5). Ctrl+A and Shift-click
//! ranges on sets follow the visible sets, as they do on textures (K18).
use super::sets::{self, SetView};
use super::{bottom, cm_json, AppModel, ViewKey};
use engine::shader::Shader;
use std::sync::Arc;

/// Every part of the filter; they combine with AND, as the Textures filter.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SetsFilter {
    pub search: String,
    pub attention: bool,
    pub ready: bool,
    pub no_json: bool,
    pub shader: Option<Shader>,
}

/// One part of the filter, to switch on for a count (Note HP, SC5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    Attention,
    Ready,
    NoJson,
    Shader(Shader),
}

/// A copy of `f` with `p` switched on: the filter a click on that part would leave (the parts combine with AND).
pub fn with_part(f: &SetsFilter, p: Part) -> SetsFilter {
    let mut f = f.clone();
    match p {
        Part::Attention => f.attention = true,
        Part::Ready => f.ready = true,
        Part::NoJson => f.no_json = true,
        Part::Shader(s) => f.shader = Some(s),
    }
    f
}

/// What each pill shows: the keyed sets the list would show with that pill switched on (Note HP, SC5; K20: a
/// created set with no key counts toward none).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SetCounts {
    pub attention: usize,
    pub ready: usize,
    pub no_json: usize,
}

/// The cached list and counts, by what they were built from (K17: Ready reads `problems()`, which can change while
/// the views key does not).
#[derive(Default)]
pub(crate) struct SetsFilterCache {
    list: Option<(ListKey, Arc<Vec<usize>>)>,
    counts: Option<(CountsKey, SetsFilter, SetCounts)>,
}

#[derive(Clone)]
struct CountsKey {
    views: ViewKey,
    problems: Arc<Vec<bottom::Problem>>,
}

impl PartialEq for CountsKey {
    fn eq(&self, o: &Self) -> bool {
        self.views == o.views && Arc::ptr_eq(&self.problems, &o.problems)
    }
}

/// The inputs of a count, read once.
struct CountBase {
    views: Arc<Vec<SetView>>,
    flagged: Arc<bottom::Flagged>,
    entries: Arc<Vec<crate::sets_doc::SetEntry>>,
}

#[derive(PartialEq)]
struct ListKey {
    of: CountsKey,
    filter: SetsFilter,
    new_sets: Vec<String>,
}

/// Whether the set writes a JSON: a complex-material export (A8), or a PBR Landscape set's CS records or PGPatcher
/// entry, or a PGPatcher JSON set; never an Advanced skin set.
pub fn writes_json(m: &AppModel, v: &SetView) -> bool {
    let entries = m.sets_doc.as_ref().map(|d| d.entries()).unwrap_or_default();
    writes_json_in(m, &entries, v)
}

fn writes_json_in(m: &AppModel, entries: &[crate::sets_doc::SetEntry], v: &SetView) -> bool {
    cm_json::exports(entries, &v.id)
        || if sets::is_pbr_landscape(v) {
            m.cs_records_written(v) || m.pgp_written(v)
        } else if v.skin {
            false
        } else {
            sets::is_json_set(v, m.target)
        }
}

impl AppModel {
    fn sets_counts_key(&self) -> CountsKey {
        CountsKey {
            views: self.view_key(),
            problems: bottom::problems(self),
        }
    }

    /// The indices into `sets::views` the column shows, in display order. Built again only when the filter, the
    /// views or the problems changed.
    pub fn visible_sets(&self) -> Arc<Vec<usize>> {
        // Every input first: `problems` and `set_ready` borrow the view cache, never while this cache is borrowed.
        let views = sets::views(self);
        let key = ListKey {
            of: self.sets_counts_key(),
            filter: self.sets_filter.clone(),
            new_sets: self.new_sets_shown().to_vec(),
        };
        if let Some((k, l)) = &self.sets_filter_cache.borrow().list {
            if *k == key {
                return l.clone();
            }
        }
        let flagged = bottom::flagged(self);
        let f = &self.sets_filter;
        let q = engine::sets::fold_id(f.search.trim());
        let entries = self
            .sets_doc
            .as_ref()
            .map(|d| d.entries())
            .unwrap_or_default();
        let list: Arc<Vec<usize>> = Arc::new(if !self.sets_filter_active() {
            (0..views.len()).collect()
        } else {
            views
                .iter()
                .enumerate()
                .filter(|(_, v)| {
                    // A created set with no key passes only an empty filter, and a set created under this filter
                    // stays shown until the filter changes (Note FC13, CD16).
                    key.new_sets.contains(&v.id) || self.passes(f, v, &flagged, &entries, &q)
                })
                .map(|(n, _)| n)
                .collect()
        });
        self.sets_filter_cache.borrow_mut().list = Some((key, list.clone()));
        list
    }

    /// The one per-set test: a keyed set that matches the search, the three pills and the shader of `f`. `q` is the
    /// folded, trimmed search. The list and every count use it, so a count is what its click shows (Note HP, SC5).
    fn passes(
        &self,
        f: &SetsFilter,
        v: &SetView,
        flagged: &bottom::Flagged,
        entries: &[crate::sets_doc::SetEntry],
        q: &str,
    ) -> bool {
        !v.key.is_empty()
            && (q.is_empty() || engine::sets::fold_id(&v.id).contains(q))
            && (!f.attention || !bottom::set_ready_in(flagged, v))
            && (!f.ready || bottom::set_ready_in(flagged, v))
            && (!f.no_json || !writes_json_in(self, entries, v))
            && f.shader
                .is_none_or(|s| v.shader.as_ref().map(|p| p.shader) == Some(s))
    }

    /// What every count reads, built once per call that makes several (Note HP).
    fn count_base(&self) -> CountBase {
        CountBase {
            views: sets::views(self),
            flagged: bottom::flagged(self),
            entries: self
                .sets_doc
                .as_ref()
                .map(|d| d.entries())
                .unwrap_or_default(),
        }
    }

    /// How many sets the list would show with `p` switched on.
    fn count_with(&self, base: &CountBase, p: Part) -> usize {
        let f = with_part(&self.sets_filter, p);
        let q = engine::sets::fold_id(f.search.trim());
        base.views
            .iter()
            .filter(|v| self.passes(&f, v, &base.flagged, &base.entries, &q))
            .count()
    }

    /// The pill counts, cached by the views, the problems and the whole filter.
    pub fn set_counts(&self) -> SetCounts {
        let key = self.sets_counts_key();
        if let Some((k, f, c)) = &self.sets_filter_cache.borrow().counts {
            if *k == key && *f == self.sets_filter {
                return *c;
            }
        }
        let base = self.count_base();
        let c = SetCounts {
            attention: self.count_with(&base, Part::Attention),
            ready: self.count_with(&base, Part::Ready),
            no_json: self.count_with(&base, Part::NoJson),
        };
        self.sets_filter_cache.borrow_mut().counts = Some((key, self.sets_filter.clone(), c));
        c
    }

    /// How many sets the list shows when shader `s` is picked. Uncached: only the open shader popup asks.
    pub fn sets_with_shader(&self, s: Shader) -> usize {
        self.count_with(&self.count_base(), Part::Shader(s))
    }

    /// `sets_with_shader` for each of `shaders`, reading the views, problems and entries once.
    // shortcut: one pass over the views per shader each popup frame; cache by revision when set counts reach the thousands.
    pub fn sets_with_shaders(&self, shaders: &[Shader]) -> Vec<usize> {
        let base = self.count_base();
        shaders
            .iter()
            .map(|s| self.count_with(&base, Part::Shader(*s)))
            .collect()
    }

    /// Any pill, the shader or search text is on.
    pub fn sets_filter_active(&self) -> bool {
        let f = &self.sets_filter;
        f.attention || f.ready || f.no_json || f.shader.is_some() || !f.search.trim().is_empty()
    }

    /// Note FC13 (CD16): remember a set created while the filter is on; it stays shown until the filter changes.
    pub(super) fn note_new_set(&mut self, id: &str) {
        if !self.sets_filter_active() {
            return;
        }
        match &mut self.new_sets {
            Some((f, ids)) if *f == self.sets_filter => ids.push(id.to_string()),
            n => *n = Some((self.sets_filter.clone(), vec![id.to_string()])),
        }
    }

    /// The ids of sets created under the filter now on (empty once the filter has changed).
    pub fn new_sets_shown(&self) -> &[String] {
        match &self.new_sets {
            Some((f, ids)) if *f == self.sets_filter && self.sets_filter_active() => ids,
            _ => &[],
        }
    }

    /// Note FC13 (CD13): the id of the one selected set when the filter hides it ("Go to set" leaves the filter on).
    pub fn selected_set_hidden_by_filter(&self) -> Option<String> {
        let [key] = self.selected_sets().try_into().ok()?;
        let views = sets::views(self);
        let i = views.iter().position(|v| v.key == key)?;
        (!self.visible_sets().contains(&i)).then(|| views[i].id.clone())
    }

    /// "Clear filter": the pills, the shader and the search.
    pub fn clear_sets_filter(&mut self) {
        self.sets_filter = SetsFilter::default();
    }
}

/// Task 5: how many set cards have been laid out, for the virtualisation pin.
#[cfg(feature = "test-hooks")]
pub fn set_cards_drawn(m: &AppModel) -> u64 {
    m.set_cards_drawn.get()
}
