//! Mechanical checks run on every frame of the UI driver (Note HM5). They read a `Snapshot` only: clipping comes
//! from egui's interact_rect, hover text is not probed (a hover would change the replay).

use crate::frame::{Node, Snapshot, Text};
use eframe::egui::{Rect, Vec2};
use serde::Serialize;

#[derive(Serialize, Debug, Clone, Default)]
pub struct Subject {
    pub role: String,
    pub label: String,
    /// x, y, w, h
    pub rect: [f32; 4],
}

#[derive(Serialize, Debug, Clone)]
pub struct Finding {
    pub check: &'static str,
    pub rule: String,
    pub subject: Subject,
    pub other: Option<Subject>,
    pub detail: String,
}

fn r4(r: Rect) -> [f32; 4] {
    [r.min.x, r.min.y, r.width(), r.height()]
}

fn subject(n: &Node) -> Subject {
    let label = if n.label.is_empty() {
        n.value
            .clone()
            .or_else(|| n.placeholder.clone())
            .unwrap_or_default()
    } else {
        n.label.clone()
    };
    Subject {
        role: n.role.clone(),
        label,
        rect: r4(n.rect),
    }
}

fn text_subject(t: &Text) -> Subject {
    Subject {
        role: "Text".into(),
        label: t.text.clone(),
        rect: r4(t.rect),
    }
}

const R_NAMES: &str =
    "egui-ui-rules 8.2: every painted or icon widget gets widget_info with a role and a name";
const R_CLIP: &str = "egui-ui-rules 3.4: every truncation keeps the full value reachable, on hover and in the AccessKit name";
const R_OFF: &str = "egui-ui-rules 3.1/3.3: a widget never draws under its neighbour; a row's buttons stay on screen";
const R_OVERLAP: &str = "egui-ui-rules 3.1: a widget never draws under its neighbour";
const R_ALIGN: &str = "owner 2026-11-07 (design, Why): labels of one kind sit in one place; a tile never lacks the label its siblings have";
const R_WRAP: &str = "owner 2026-11-07 (design, Why): one tile wraps onto a second row by itself";
const R_PAINTED: &str =
    "egui-ui-rules 8.2 and design §2: text the user reads gets a node with its rect";

fn positive(r: Rect) -> bool {
    r.is_positive() && r.min.is_finite() && r.max.is_finite()
}

/// Nodes of one parent with the same size (within 1 px, at least 8x8), at least two of them.
fn sibling_groups(s: &Snapshot) -> Vec<Vec<usize>> {
    let mut by_parent: std::collections::BTreeMap<usize, Vec<usize>> = Default::default();
    for (i, n) in s.nodes.iter().enumerate() {
        if let Some(p) = n.parent {
            if positive(n.rect) && n.rect.width() >= 8.0 && n.rect.height() >= 8.0 {
                by_parent.entry(p).or_default().push(i);
            }
        }
    }
    let mut out = vec![];
    for (_, kids) in by_parent {
        let mut clusters: Vec<Vec<usize>> = vec![];
        for i in kids {
            let sz = s.nodes[i].rect.size();
            match clusters
                .iter_mut()
                .find(|c| (s.nodes[c[0]].rect.size() - sz).abs().max_elem() <= 1.0)
            {
                Some(c) => c.push(i),
                None => clusters.push(vec![i]),
            }
        }
        out.extend(clusters.into_iter().filter(|c| c.len() >= 2));
    }
    out
}

/// Members of `g` in rows: centres within half a height of the row's first member, rows top to bottom.
fn rows(s: &Snapshot, g: &[usize]) -> Vec<Vec<usize>> {
    let mut sorted = g.to_vec();
    sorted.sort_by(|&a, &b| {
        let (a, b) = (s.nodes[a].rect, s.nodes[b].rect);
        a.center()
            .y
            .total_cmp(&b.center().y)
            .then(a.min.x.total_cmp(&b.min.x))
    });
    let mut rows: Vec<Vec<usize>> = vec![];
    for i in sorted {
        let r = s.nodes[i].rect;
        match rows.last_mut() {
            Some(row)
                if (s.nodes[row[0]].rect.center().y - r.center().y).abs() <= r.height() / 2.0 =>
            {
                row.push(i)
            }
            _ => rows.push(vec![i]),
        }
    }
    rows
}

/// A text whose centre is inside `r`: the first painted.
fn text_in(s: &Snapshot, r: Rect) -> Option<&Text> {
    s.texts.iter().find(|t| r.contains(t.rect.center()))
}

fn word_in(hay: &str, needle: &str) -> bool {
    hay.match_indices(needle).any(|(i, m)| {
        let before = hay[..i]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric());
        let after = hay[i + m.len()..]
            .chars()
            .next()
            .is_none_or(|c| !c.is_alphanumeric());
        before && after
    })
}

pub fn run_checks(s: &Snapshot) -> Vec<Finding> {
    let mut out = vec![];
    let win = Rect::from_min_size(Default::default(), s.size);
    let (mut clip_vertical, mut clip_unknown, mut off_other) = (0, 0, 0);

    for n in &s.nodes {
        // names
        // a floating scroll bar has no name to give
        if n.interactive
            && n.role != "ScrollBar"
            && n.label.is_empty()
            && n.value.as_deref().is_none_or(str::is_empty)
            && n.placeholder.as_deref().is_none_or(str::is_empty)
            && positive(n.rect)
            // a panel's resize handle: drag-only, 8 px or thinner, no role of its own
            && !(n.rect.size().min_elem() <= 8.0 && matches!(n.role.as_str(), "Unknown" | "GenericContainer"))
        {
            out.push(Finding {
                check: "names",
                rule: R_NAMES.into(),
                subject: subject(n),
                other: None,
                detail: format!("interactive {} has no name", n.role),
            });
        }
        // clipped_text
        if n.role == "Label" && positive(n.rect) {
            if n.clip.is_none() {
                clip_unknown += 1;
            }
            let bound = n.clip.map_or(win, |c| c.intersect(win));
            if positive(bound) && positive(n.rect.intersect(bound)) {
                let (l, r) = (bound.min.x - n.rect.min.x, n.rect.max.x - bound.max.x);
                if l > 0.5 || r > 0.5 {
                    out.push(Finding {
                        check: "clipped_text",
                        rule: R_CLIP.into(),
                        subject: subject(n),
                        other: None,
                        detail: format!(
                            "text sticks out of its clip by {:.1} px on the left, {:.1} px on the right; hover text was not probed",
                            l.max(0.0),
                            r.max(0.0)
                        ),
                    });
                } else if n.rect.min.y < bound.min.y - 0.5 || n.rect.max.y > bound.max.y + 0.5 {
                    clip_vertical += 1;
                }
            }
        }
        // off_window
        if n.interactive && positive(n.rect) {
            let hidden = n.clip.is_some_and(|c| !positive(c.intersect(n.rect)));
            if hidden || !positive(n.rect.intersect(win)) {
                off_other += 1;
            } else if n.rect.min.x < -0.5 || n.rect.max.x > win.max.x + 0.5 {
                out.push(Finding {
                    check: "off_window",
                    rule: R_OFF.into(),
                    subject: subject(n),
                    other: None,
                    detail: format!(
                        "cut by the window edge: x {:.1}..{:.1}, window width {:.1}",
                        n.rect.min.x, n.rect.max.x, win.max.x
                    ),
                });
            } else if n.rect.min.y < -0.5 || n.rect.max.y > win.max.y + 0.5 {
                off_other += 1;
            }
        }
    }

    // overlap
    let cands: Vec<(usize, Rect)> = s
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| {
            n.interactive && n.role != "ScrollBar" && n.layer.is_some() && positive(n.rect)
        })
        .map(|(i, n)| (i, n.clip.map_or(n.rect, |c| c.intersect(n.rect))))
        .filter(|(_, r)| positive(*r))
        .collect();
    // Note HP (Decision 3): a helper badge is centred on its chip's corner by design. A pair is spared only when
    // `a` is at most 24 px on each side and its centre is within 1.5 px of a corner of `b` on each axis.
    let on_corner = |a: Rect, b: Rect| {
        a.width() <= 24.0
            && a.height() <= 24.0
            && [
                b.left_top(),
                b.right_top(),
                b.left_bottom(),
                b.right_bottom(),
            ]
            .iter()
            .any(|p| (a.center() - *p).abs().max_elem() <= 1.5)
    };
    for (a, &(i, ri)) in cands.iter().enumerate() {
        for &(j, rj) in &cands[a + 1..] {
            if s.nodes[i].layer != s.nodes[j].layer {
                continue;
            }
            let o = ri.intersect(rj);
            if o.width() > 1.0
                && o.height() > 1.0
                && !ri.expand(0.5).contains_rect(rj)
                && !rj.expand(0.5).contains_rect(ri)
                && !on_corner(ri, rj)
                && !on_corner(rj, ri)
            {
                out.push(Finding {
                    check: "overlap",
                    rule: R_OVERLAP.into(),
                    subject: subject(&s.nodes[i]),
                    other: Some(subject(&s.nodes[j])),
                    detail: format!("overlap {:.1}x{:.1} px", o.width(), o.height()),
                });
            }
        }
    }

    // alignment and lonely_wrap
    for g in sibling_groups(s) {
        let place: Vec<Option<(Vec2, Vec2)>> = g
            .iter()
            .map(|&i| {
                let r = s.nodes[i].rect;
                text_in(s, r).map(|t| (t.rect.min - r.min, t.rect.center() - r.center()))
            })
            .collect();
        let agree = |a: (Vec2, Vec2), b: (Vec2, Vec2)| {
            (a.0 - b.0).abs().max_elem() <= 2.0 || (a.1 - b.1).abs().max_elem() <= 2.0
        };
        let with: Vec<(Vec2, Vec2)> = place.iter().flatten().copied().collect();
        if !with.is_empty() {
            let mut mode = with[0];
            let mut best = 0;
            for &c in &with {
                let votes = with.iter().filter(|&&o| agree(c, o)).count();
                if votes > best {
                    (best, mode) = (votes, c);
                }
            }
            let without = place.len() - with.len();
            for (&i, p) in g.iter().zip(&place) {
                match p {
                    Some(p) if !agree(*p, mode) => out.push(Finding {
                        check: "alignment",
                        rule: R_ALIGN.into(),
                        subject: subject(&s.nodes[i]),
                        other: None,
                        detail: format!(
                            "text offset from top-left ({:.0},{:.0}), from centre ({:.0},{:.0}); most of {} siblings: top-left ({:.0},{:.0}), centre ({:.0},{:.0})",
                            p.0.x, p.0.y, p.1.x, p.1.y, g.len(), mode.0.x, mode.0.y, mode.1.x, mode.1.y
                        ),
                    }),
                    // shortcut: only a minority without text is flagged, so one lettered tile among bare ones is not
                    None if with.len() > without => out.push(Finding {
                        check: "alignment",
                        rule: R_ALIGN.into(),
                        subject: subject(&s.nodes[i]),
                        other: None,
                        detail: format!(
                            "no painted text; {} of {} same-size siblings have some",
                            with.len(),
                            g.len()
                        ),
                    }),
                    _ => {}
                }
            }
        }
        let rs = rows(s, &g);
        for row in rs.iter().filter(|r| r.len() >= 2) {
            let tops: Vec<f32> = row.iter().map(|&i| s.nodes[i].rect.min.y).collect();
            let mode = *tops
                .iter()
                .max_by_key(|&&t| tops.iter().filter(|&&o| (o - t).abs() <= 1.0).count())
                .unwrap();
            for (&i, &t) in row.iter().zip(&tops) {
                if (t - mode).abs() > 1.0 {
                    out.push(Finding {
                        check: "alignment",
                        rule: R_ALIGN.into(),
                        subject: subject(&s.nodes[i]),
                        other: None,
                        detail: format!("top at {t:.1}, its row's tops are at {mode:.1}"),
                    });
                }
            }
        }
        if rs.len() >= 2 {
            let (last, above) = (&rs[rs.len() - 1], &rs[rs.len() - 2]);
            if last.len() == 1 && above.len() >= 2 {
                out.push(Finding {
                    check: "lonely_wrap",
                    rule: R_WRAP.into(),
                    subject: subject(&s.nodes[last[0]]),
                    other: None,
                    detail: format!("alone on the last row; the row above holds {}", above.len()),
                });
            }
        }
    }

    // painted_only
    for t in &s.texts {
        // each piece of a middle-elided text ("C:\…\export") is named on its own; a trailing "▾" or a leading "⚠" is
        // not part of the name
        let pieces: Vec<&str> = t
            .text
            .split('…')
            .map(|p| p.trim_matches(|c: char| !c.is_alphanumeric()))
            .filter(|p| !p.is_empty())
            .collect();
        let named = pieces.iter().all(|txt| {
            s.nodes.iter().any(|n| {
                positive(n.rect)
                    // shortcut: the node only has to touch the text's rect, not hold its centre; the rect of rotated
                    // or right-aligned text is wider than the glyphs. Upgrade when a stray neighbour hides a miss.
                    && positive(n.rect.intersect(t.rect))
                    && [Some(&n.label), n.value.as_ref(), n.placeholder.as_ref()]
                        .into_iter()
                        .flatten()
                        .any(|h| word_in(h, txt))
            })
        });
        if !named {
            out.push(Finding {
                check: "painted_only",
                rule: R_PAINTED.into(),
                subject: text_subject(t),
                other: None,
                detail: "no node under this text names it".into(),
            });
        }
    }

    let mut counted = vec![];
    if clip_vertical > 0 {
        counted.push(format!(
            "clipped_text: {clip_vertical} cut at the top or bottom"
        ));
    }
    if clip_unknown > 0 {
        counted.push(format!(
            "clipped_text: {clip_unknown} labels had no usable clip (checked against the window only)"
        ));
    }
    if off_other > 0 {
        counted.push(format!(
            "off_window: {off_other} interactive nodes cut at the top or bottom, scrolled away or wholly outside"
        ));
    }
    if !counted.is_empty() {
        out.push(Finding {
            check: "summary",
            rule: "counted, not listed".into(),
            subject: Subject::default(),
            other: None,
            detail: counted.join("; "),
        });
    }
    out
}
