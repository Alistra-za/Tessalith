//! Note DY17: the helper panel's edits. Each writes one `helper.*` selector of the helper file's own `[files]` entry in
//! `sdc.toml` as one undo step, then plans again; a read-only `sdc.toml` takes none (`AppModel::edit`).
use super::sets::Tails;
use super::AppModel;
use engine::helper::report::HelperReport;
use engine::helper::{FillTarget, HelperRead, HelperRole};
use engine::helper::{FILLS_FIELD, INVERT_FIELD, READ_FIELD, ROLE_FIELD};
use serde_json::{json, Value};
use std::path::Path;

/// The set's base name that fill targets are labelled with ("x" in "x_rmaos R").
pub fn set_base(h: &HelperReport) -> String {
    Path::new(&h.set_key)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

/// A fill target's name as the user sees it: the stem of the planned output that ends in the target's suffix, minus
/// the suffix (so it matches the `Show …` button), else the set's base name when the target did not land.
pub fn target_label(h: &HelperReport, t: FillTarget, tails: Tails) -> String {
    use engine::helper::AlphaTarget as A;
    let suffix = match t {
        FillTarget::Alpha(A::Diffuse) => None,
        FillTarget::Alpha(A::N) => Some("n"),
        FillTarget::Alpha(A::F) => Some("f"),
        FillTarget::Alpha(A::S) => Some("s"),
        FillTarget::Alpha(A::Cnr) => Some("cnr"),
        FillTarget::Packed { suffix, .. } => Some(suffix),
    };
    let stems = || h.outputs.iter().filter_map(|o| o.file_stem()?.to_str());
    let named = suffix
        .map(|s| format!("_{s}"))
        .and_then(|tail| stems().find_map(|s| s.strip_suffix(tail.as_str())));
    // A target with no output (or the colour one, which has no suffix) takes any output's base name, so a renamed
    // set reads `steel` beside the rows that read `steel_rmaos R`.
    let any = || {
        stems().next().map(|s| {
            let tail = super::sets::member_tail(s, tails).len();
            &s[..s.len() - tail]
        })
    };
    let base = set_base(h);
    t.label(named.or_else(any).unwrap_or(&base))
}

impl AppModel {
    /// The tails of the set a helper report belongs to (`SetView::tails`), `Plain` when the set is not listed.
    pub fn helper_tails(&self, h: &HelperReport) -> Tails {
        super::sets::views(self)
            .iter()
            .find(|s| s.key == h.set_key)
            .map_or(Tails::Plain, |s| s.tails())
    }

    /// Note DY18b: a helper's current list. A hand-set list is its `sdc.toml` `helper.fills`, so a target that did not
    /// land is not lost; otherwise the engine's reported fills.
    pub fn current_fills(&self, key: &str, h: &HelperReport) -> Vec<FillTarget> {
        let hand = self
            .project
            .as_ref()
            .and_then(|p| p.file_value(key, FILLS_FIELD))
            .and_then(|v| engine::helper::fills_value(&v));
        match hand {
            Some(v) if h.fills_hand_set => v,
            _ => h.fills.clone(),
        }
    }

    fn helper_edit(&mut self, label: &str, key: &str, field: &'static str, v: Option<Value>) {
        let k = key.to_string();
        self.edit(label, move |p| match v {
            Some(v) => p.set_file_value(&k, field, &v),
            None => p.clear_file_value(&k, field),
        });
    }

    /// `None` clears `helper.role` (the role guessed from the name again).
    pub fn set_helper_role(&mut self, key: &str, role: Option<HelperRole>) {
        let v = role.map(|r| json!(r.id()));
        self.helper_edit("Helper role", key, ROLE_FIELD, v);
    }

    /// `None` clears `helper.fills` (the suggestion again); `Some(vec![])` writes `[]` (not used).
    pub fn set_helper_fills(&mut self, key: &str, fills: Option<Vec<FillTarget>>) {
        let v = fills.map(|f| json!(f.iter().map(FillTarget::id).collect::<Vec<_>>()));
        self.helper_edit("Helper fills", key, FILLS_FIELD, v);
    }

    /// `None` clears `helper.read` (R, or the recipe's luma).
    pub fn set_helper_read(&mut self, key: &str, read: Option<HelperRead>) {
        let v = read.map(|r| json!(r.id()));
        self.helper_edit("Helper read from", key, READ_FIELD, v);
    }

    /// `false` clears `helper.invert`.
    pub fn set_helper_invert(&mut self, key: &str, on: bool) {
        self.helper_edit(
            "Helper invert",
            key,
            INVERT_FIELD,
            on.then_some(json!(true)),
        );
    }
}
