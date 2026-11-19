//! The wiki's codes page and `Code::ALL` agree: every code has a row in a rendered table of the page (a table whose
//! first column is headed `Code`), the row gives its severity in the second column, and every row of such a table
//! names a code that exists.
use engine::codes::Code;
use std::collections::BTreeMap;
use std::path::Path;

/// The cells of a table line, with or without outer pipes.
fn cells(line: &str) -> Vec<&str> {
    let t = line.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').unwrap_or(t);
    t.split('|').map(str::trim).collect()
}

/// A table's delimiter row: every cell is dashes with optional colons, and the line has a pipe.
fn is_delimiter(line: &str) -> bool {
    line.contains('|')
        && cells(line).iter().all(|c| {
            let c = c.trim_matches(':');
            !c.is_empty() && c.chars().all(|ch| ch == '-')
        })
}

/// The rows of the page's diagnostic tables: (code, severity word), in the order the page lists them, and the
/// faults of data rows whose first cell is not exactly one code in single backticks. A table is rendered where
/// a header line is followed by a delimiter row, outside a fenced block; it is a diagnostic table when its first
/// cell is `Code`. The test fails when the page has none.
fn page_rows(page: &str) -> (Vec<(String, String)>, Vec<String>) {
    let lines: Vec<&str> = page.lines().collect();
    let (mut rows, mut faults) = (Vec::new(), Vec::new());
    let (mut fence, mut tables, mut i) = (false, 0, 0);
    while i < lines.len() {
        let t = lines[i].trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            fence = !fence;
            i += 1;
            continue;
        }
        let header = !fence
            && lines[i].contains('|')
            && i + 1 < lines.len()
            && is_delimiter(lines[i + 1])
            && cells(lines[i]).len() == cells(lines[i + 1]).len();
        if !header {
            i += 1;
            continue;
        }
        let diagnostic = cells(lines[i])[0] == "Code";
        i += 2;
        if diagnostic {
            tables += 1;
        }
        // The body runs to the first blank line or a line without a pipe (a fence ends it too).
        while i < lines.len() && lines[i].contains('|') && !lines[i].trim_start().starts_with("```")
        {
            if diagnostic {
                let c = cells(lines[i]);
                let code = c[0]
                    .strip_prefix('`')
                    .and_then(|x| x.strip_suffix('`'))
                    .filter(|x| !x.is_empty() && !x.contains('`'));
                match (code, c.get(1)) {
                    (Some(code), Some(sev)) => rows.push((code.to_string(), sev.to_string())),
                    _ => faults.push(format!(
                        "line {}: a row of a `Code` table must start with one code in single backticks and have a severity cell: {}",
                        i + 1,
                        lines[i].trim()
                    )),
                }
            }
            i += 1;
        }
    }
    if tables == 0 {
        faults.push("no table headed `Code` found on the codes page".to_string());
    }
    (rows, faults)
}

/// Every disagreement between `page` and `codes` (name, severity word), one line each; empty when they agree.
fn check(page: &str, codes: &[(String, String)]) -> Vec<String> {
    let (rows, mut faults) = page_rows(page);
    let mut by_code: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (code, sev) in &rows {
        by_code.entry(code).or_default().push(sev);
    }
    for (name, want) in codes {
        match by_code.get(name.as_str()).map(Vec::as_slice) {
            None => faults.push(format!(
                "{name}: no row in a table of the codes page; add `| `{name}` | {want} | <meaning> | <fix> |` to docs/wiki/Diagnostic-Codes.md"
            )),
            Some([sev]) => {
                if sev != want {
                    faults.push(format!(
                        "{name}: the page says severity `{sev}`, the engine says `{want}`; fix the page"
                    ));
                }
            }
            Some(many) => faults.push(format!(
                "{name}: {} rows on the codes page; keep one",
                many.len()
            )),
        }
    }
    for (code, _) in &rows {
        if !codes.iter().any(|(name, _)| name == code) {
            faults.push(format!(
                "{code}: a row names no code in Code::ALL; remove the row, or restore the code"
            ));
        }
    }
    faults
}

fn engine_codes() -> Vec<(String, String)> {
    Code::ALL
        .iter()
        .map(|c| {
            (
                c.as_str().to_string(),
                format!("{:?}", c.severity()).to_lowercase(),
            )
        })
        .collect()
}

#[test]
fn every_code_has_its_row_on_the_codes_page_and_every_row_names_a_code() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/wiki/Diagnostic-Codes.md");
    let page = std::fs::read_to_string(&path).expect("docs/wiki/Diagnostic-Codes.md is readable");
    let faults = check(&page, &engine_codes());
    assert!(faults.is_empty(), "{}", faults.join("\n"));
}

fn two() -> Vec<(String, String)> {
    vec![
        ("AAA-ONE".to_string(), "info".to_string()),
        ("BBB-TWO".to_string(), "error".to_string()),
    ]
}

const TABLE: &str = "| Code | Severity | Meaning |\n|---|---|---|\n| `AAA-ONE` | info | a |\n| `BBB-TWO` | error | b |\n";

#[test]
fn the_checker_accepts_a_page_that_agrees() {
    assert_eq!(check(TABLE, &two()), Vec::<String>::new());
}

#[test]
fn a_row_inside_a_fence_is_not_a_row() {
    let page = "| Code | Severity | Meaning |\n|---|---|---|\n| `AAA-ONE` | info | a |\n\n```\n| `BBB-TWO` | error | b |\n```\n";
    let faults = check(page, &two());
    assert_eq!(faults.len(), 1, "{faults:?}");
    assert!(faults[0].starts_with("BBB-TWO: no row"), "{faults:?}");
    assert!(
        faults[0].contains("| error |"),
        "the real severity is printed: {faults:?}"
    );
}

#[test]
fn an_unknown_code_in_a_table_without_outer_pipes_is_found() {
    let page = "Code | Severity | Meaning\n---|---|---\n`AAA-ONE` | info | a\n`BBB-TWO` | error | b\n`CCC-THREE` | info | c\n";
    let faults = check(page, &two());
    assert_eq!(faults.len(), 1, "{faults:?}");
    assert!(
        faults[0].starts_with("CCC-THREE: a row names no code"),
        "{faults:?}"
    );
}

#[test]
fn a_whole_table_inside_a_fence_is_not_read() {
    let page = format!(
        "{TABLE}\n```\n| Code | Severity | Meaning |\n|---|---|---|\n| `CCC-THREE` | info | c |\n```\n"
    );
    assert_eq!(check(&page, &two()), Vec::<String>::new());
}
