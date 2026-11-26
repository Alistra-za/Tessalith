//! The return scan (Note LH3): every early `return` in a test says why. It reads `src` and `tests` of the six
//! main-team crates with the walker in `main.rs`.

use super::rs_files;

/// `line` with the bodies of string and char literals and any `//` comment removed, so a brace or a keyword in a
/// literal does not count. A string or char keeps its quotes.
fn strip_literals(line: &str) -> String {
    let c: Vec<char> = line.chars().collect();
    let (mut out, mut i) = (String::new(), 0);
    while i < c.len() {
        let ident_before = i > 0 && (c[i - 1].is_alphanumeric() || c[i - 1] == '_');
        if c[i] == '/' && c.get(i + 1) == Some(&'/') {
            break;
        }
        if c[i] == 'r' && !ident_before {
            let mut j = i + 1;
            while c.get(j) == Some(&'#') {
                j += 1;
            }
            if c.get(j) == Some(&'"') {
                let hashes = j - i - 1;
                let close = (j + 1..c.len())
                    .find(|&k| c[k] == '"' && (1..=hashes).all(|h| c.get(k + h) == Some(&'#')));
                out.push_str("\"\"");
                let Some(k) = close else { break };
                i = k + 1 + hashes;
                continue;
            }
        }
        if c[i] == '"' {
            i += 1;
            while i < c.len() && c[i] != '"' {
                i += if c[i] == '\\' { 2 } else { 1 };
            }
            i += 1;
            out.push_str("\"\"");
        } else if c[i] == '\'' && c.get(i + 1) == Some(&'\\') {
            i += 3;
            while i < c.len() && c[i] != '\'' {
                i += 1;
            }
            i += 1;
            out.push_str("''");
        } else if c[i] == '\'' && c.get(i + 2) == Some(&'\'') {
            i += 3;
            out.push_str("''");
        } else {
            out.push(c[i]);
            i += 1;
        }
    }
    out
}

/// One source line: its code (see `strip_literals`) and the brace depth at its start, its lowest depth inside it and
/// its depth at its end.
struct CodeLine {
    code: String,
    start: i32,
    low: i32,
    end: i32,
}

fn code_lines(text: &str) -> Vec<CodeLine> {
    let mut depth = 0;
    text.lines()
        .map(|l| {
            let code = strip_literals(l);
            let (start, mut low) = (depth, depth);
            for ch in code.chars() {
                match ch {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        low = low.min(depth);
                    }
                    _ => {}
                }
            }
            CodeLine {
                code,
                start,
                low,
                end: depth,
            }
        })
        .collect()
}

fn is_fn_line(line: &str) -> bool {
    let mut t = line.trim_start();
    if let Some(r) = t.strip_prefix("pub") {
        t = match r.strip_prefix('(') {
            Some(inner) => inner.split_once(')').map_or(r, |(_, after)| after),
            None => r,
        }
        .trim_start();
    }
    while let Some(r) = ["const ", "async ", "unsafe "]
        .iter()
        .find_map(|q| t.strip_prefix(q))
    {
        t = r.trim_start();
    }
    t.starts_with("fn ")
}

/// True when `code` calls `helper` as itself: not as the tail of a longer identifier, and not under a `!`, where the
/// helper's printed reason belongs to the other branch.
fn calls_helper(code: &str, helper: &str) -> bool {
    code.match_indices(helper).any(|(at, _)| {
        let before = &code[..at];
        let ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
        let path = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == ':';
        !before.bytes().next_back().is_some_and(ident)
            && !before.trim_end_matches(path).trim_end().ends_with('!')
    })
}

/// True when `code` holds `return;`, `return }` or `return Ok(());`.
fn has_return(code: &str) -> bool {
    code.match_indices("return").any(|(at, _)| {
        let ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
        let before = code[..at].bytes().next_back().is_some_and(ident);
        let rest = &code[at + 6..];
        let after = rest.bytes().next().is_some_and(ident);
        let rest = rest.trim_start();
        !before
            && !after
            && (rest.starts_with(';') || rest.starts_with('}') || rest.starts_with("Ok(())"))
    })
}

/// Skip helpers that print their own reason before a test returns (Note LH3).
const SKIP_HELPERS: [&str; 2] = ["render_state()", "skip_byte_pins()"];

/// The early returns in the `#[test]` functions of `text` that give no reason (Note LH3), as `label:line`, and the
/// `helpers` entries seen at a return line.
// shortcut: line by line, so a string (raw or ordinary) spanning lines and a `/* */` block comment can miscount braces,
// a `return` inside a closure counts as the test's own, and a match arm `=> return,` is not seen as a return at all;
// upgrade when a test does any of these.
// shortcut: the checks are positional, so a print whose block closes on its own line before the return
// (`if a() { println!("a"); }` above `if b() { return; }`) and a helper behind `||` on the condition line
// (`if other() || skip_byte_pins() {`) both pass, as do a helper on a `{ } else {` line above the return (the
// return sits in the branch where the helper did not fire) and `helper() == false`; upgrade to tracking the block at
// the print token and matching the exact guard shapes (`if helper() {`, `let ... = helper() else { return };`) when a
// test writes any of these shapes.
fn unexplained_returns(
    label: &str,
    text: &str,
    helpers: &[&str],
) -> (Vec<String>, std::collections::BTreeSet<String>) {
    const PRINTS: [&str; 4] = ["eprintln!(", "println!(", "eprint!(", "print!("];
    let ends = |l: &CodeLine, set: &[char]| {
        l.code
            .trim_end()
            .chars()
            .last()
            .is_some_and(|c| set.contains(&c))
    };
    let raw: Vec<&str> = text.lines().collect();
    let ln = code_lines(text);
    let (mut hits, mut seen) = (Vec::new(), std::collections::BTreeSet::new());
    let mut i = 0;
    while i < raw.len() {
        if !is_fn_line(raw[i]) {
            i += 1;
            continue;
        }
        // The attribute block: the run of lines above the `fn`, up to a blank line or a statement or block end.
        let mut a = i;
        while a > 0 && !raw[a - 1].trim().is_empty() && !ends(&ln[a - 1], &[';', '{', '}']) {
            a -= 1;
        }
        let attrs = &raw[a..i];
        if !attrs.iter().any(|l| l.trim() == "#[test]") {
            i += 1;
            continue;
        }
        let ignored = attrs.iter().any(|l| l.trim().starts_with("#[ignore = \""));
        let d0 = ln[i].start;
        let (mut opened, mut nested) = (false, None::<(i32, bool)>);
        let mut j = i;
        while j < raw.len() {
            let l = &ln[j];
            let skip = if let Some((nd, op)) = nested.as_mut() {
                *op |= l.code.contains('{');
                if *op && l.end <= *nd {
                    nested = None;
                }
                true
            } else if j > i && is_fn_line(raw[j]) {
                let op = l.code.contains('{');
                if !(op && l.end <= l.start) {
                    nested = Some((l.start, op));
                }
                true
            } else {
                false
            };
            if !skip && has_return(&l.code) {
                let mut excused = ignored;
                for h in helpers {
                    let above =
                        j > i && ends(&ln[j - 1], &['{']) && calls_helper(&ln[j - 1].code, h);
                    if calls_helper(&l.code, h) || above {
                        seen.insert((*h).to_owned());
                        excused = true;
                    }
                }
                for p in j.saturating_sub(3).max(i)..=j {
                    if PRINTS.iter().any(|m| ln[p].code.contains(m))
                        && ln[p + 1..=j].iter().all(|x| x.low >= ln[p].end)
                    {
                        excused = true;
                    }
                }
                if j > i && ends(&ln[j - 1], &[';']) {
                    let mut k = j - 1;
                    while k > i && !ends(&ln[k - 1], &[';', '{', '}']) {
                        k -= 1;
                    }
                    excused |= raw[k].trim_start().starts_with("assert");
                }
                if !excused {
                    hits.push(format!("{label}:{}", j + 1));
                }
            }
            opened |= l.code.contains('{');
            if opened && l.end <= d0 {
                break;
            }
            j += 1;
        }
        i = j + 1;
    }
    (hits, seen)
}

/// A test that returns early prints why, is `#[ignore = "..."]`, ends a checked exit, or goes through a skip helper
/// (Note LH3).
#[test]
fn every_early_return_in_a_test_says_why() {
    let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    for krate in ["engine", "texcore", "gui", "cli", "bcenc", "encoders-sys"] {
        for (sub, required) in [("src", true), ("tests", false)] {
            let root = crates.join(krate).join(sub);
            rs_files(&root, required, &mut files)
                .unwrap_or_else(|e| panic!("{}: {e}", root.display()));
        }
    }
    assert!(
        files.len() > 500,
        "the scan read only {} files",
        files.len()
    );
    let (mut hits, mut seen) = (Vec::new(), std::collections::BTreeSet::new());
    for f in &files {
        let text = std::fs::read_to_string(f).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        let label = f.strip_prefix(&crates).unwrap_or(f).display().to_string();
        let (h, s) = unexplained_returns(&label, &text, &SKIP_HELPERS);
        hits.extend(h);
        seen.extend(s);
    }
    for h in SKIP_HELPERS {
        assert!(
            seen.contains(h),
            "{h} is a skip helper no return uses; drop it from SKIP_HELPERS"
        );
    }
    assert!(
        hits.is_empty(),
        "{} files read; a test returns early without saying why: print the reason (eprintln!(\"skipped: ...\")) on or within three lines above the return, or give the test #[ignore = \"...\"], or end it after an assertion: {hits:#?}",
        files.len()
    );
}

/// The matcher tells an unexplained return from a skip or a checked exit. Each case is one source line, so the scans
/// that read this file do not see a fixture as a test.
#[test]
fn the_return_rule_tells_a_skip_from_a_checked_exit() {
    let hits = |src: &str| unexplained_returns("x.rs", src, &SKIP_HELPERS);
    // 1: a bare return is a hit; 2: a print above it explains it.
    assert_eq!(
        hits("#[test]\nfn t() {\n    if !cfg!(windows) {\n        return;\n    }\n}").0,
        ["x.rs:4"]
    );
    assert!(hits("#[test]\nfn t() {\n    if !cfg!(windows) {\n        eprintln!(\"skipped: Windows only\");\n        return;\n    }\n}").0.is_empty());
    // 3: #[ignore = "..."] explains it, a bare #[ignore] does not.
    assert!(hits("#[test]\n#[ignore = \"needs a GPU\"]\nfn t() {\n    let Ok(o) = std::env::var(\"X\") else {\n        return;\n    };\n}").0.is_empty());
    assert_eq!(hits("#[test]\n#[ignore]\nfn t() {\n    let Ok(o) = std::env::var(\"X\") else {\n        return;\n    };\n}").0, ["x.rs:5"]);
    // 4: a return after an assertion (rule (d)) is a checked exit.
    assert!(hits("#[test]\nfn t() {\n    for _ in 0..3 {\n        if let Some(v) = f() {\n            assert_eq!(\n                v,\n                1\n            );\n            return;\n        }\n    }\n    panic!();\n}").0.is_empty());
    // 5: a skip helper on the return line, and it is reported as seen.
    let (h, seen) =
        hits("#[test]\nfn t() {\n    let Some(rs) = render_state() else { return };\n}");
    assert!(
        h.is_empty() && seen.contains("render_state()"),
        "{h:?} {seen:?}"
    );
    // 6: the render.rs shape: the helper excuses its own line only.
    assert_eq!(hits("#[test]\nfn t() {\n    let Some(rs) = render_state() else { return };\n    let r = R::new(rs);\n    if !r.supports_bc() {\n        return;\n    }\n}").0, ["x.rs:6"]);
    // 7: a return in a nested fn is not the test's; 8: nor is one outside a test.
    assert!(hits(
        "#[test]\nfn t() {\n    fn walk(a: u8) {\n        return;\n    }\n    walk(1);\n}"
    )
    .0
    .is_empty());
    assert!(hits("fn t() {\n    return;\n}").0.is_empty());
    // 9: `return Ok(());` counts.
    assert_eq!(
        hits(
            "#[test]\nfn t() -> R {\n    if skip() {\n        return Ok(());\n    }\n    Ok(())\n}"
        )
        .0,
        ["x.rs:4"]
    );
    // 10: a print in a sibling branch does not explain the return.
    assert_eq!(hits("#[test]\nfn t() {\n    if a() {\n        eprintln!(\"a\");\n    } else if b() {\n        return;\n    }\n}").0, ["x.rs:6"]);
    // 11: a helper on an earlier statement excuses nothing; one on the line that opens the block does.
    assert_eq!(
        hits("#[test]\nfn t() {\n    let rs = render_state();\n    if other() { return; }\n}").0,
        ["x.rs:4"]
    );
    assert!(hits(
        "#[test]\nfn t() {\n    if encoders_sys::skip_byte_pins() {\n        return;\n    }\n}"
    )
    .0
    .is_empty());
    // 12: a wrapped attribute between #[test] and fn still marks a test.
    assert_eq!(hits("#[test]\n#[cfg_attr(\n    not(windows),\n    allow(unused)\n)]\nfn t() {\n    return;\n}").0, ["x.rs:7"]);
    // 13: a print three lines above explains it, four lines above does not.
    assert!(hits(
        "#[test]\nfn t() {\n    eprintln!(\"a\");\n    let a = 1;\n    let b = 2;\n    return;\n}"
    )
    .0
    .is_empty());
    assert_eq!(hits("#[test]\nfn t() {\n    eprintln!(\"a\");\n    let a = 1;\n    let b = 2;\n    let c = 3;\n    return;\n}").0, ["x.rs:7"]);
    // 14: `return }` counts.
    assert_eq!(
        hits("#[test]\nfn t() {\n    let Some(a) = f() else { return };\n}").0,
        ["x.rs:3"]
    );
    // 15: a helper is matched as itself, and not under a `!`.
    assert_eq!(
        hits("#[test]\nfn t() {\n    let Some(rs) = my_render_state() else { return };\n}").0,
        ["x.rs:3"]
    );
    assert_eq!(hits("#[test]\nfn t() {\n    if !encoders_sys::skip_byte_pins() {\n        return;\n    }\n}").0, ["x.rs:4"]);
    // 16: a nested fn in any form keeps its returns out of the test's.
    assert!(hits("#[test]\nfn t() {\n    pub(super) fn w() {\n        return;\n    }\n    const fn k() {\n        return;\n    }\n    unsafe fn u() {\n        return;\n    }\n}").0.is_empty());
}
