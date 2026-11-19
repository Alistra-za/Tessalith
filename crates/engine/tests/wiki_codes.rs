//! The wiki's codes page and `Code::ALL` agree: every code has a table row that starts with the code in backticks
//! and gives its severity in the second column, and every such row names a code that exists.
use engine::codes::Code;
use std::collections::BTreeMap;
use std::path::Path;

/// The rows of the page: code -> severity word, in the order the page lists them. A row is a table line whose first
/// cell is one code in backticks.
fn page_rows(page: &str) -> Vec<(String, String)> {
    let mut rows = Vec::new();
    for line in page.lines() {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        // `| a | b | ...` splits into an empty first and last cell.
        if cells.len() < 3 || !cells[0].is_empty() {
            continue;
        }
        let Some(code) = cells[1].strip_prefix('`').and_then(|c| c.strip_suffix('`')) else {
            continue;
        };
        rows.push((code.to_string(), cells[2].to_string()));
    }
    rows
}

#[test]
fn every_code_has_its_row_on_the_codes_page_and_every_row_names_a_code() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/wiki/Diagnostic-Codes.md");
    let page = std::fs::read_to_string(&path).expect("docs/wiki/Diagnostic-Codes.md is readable");
    let rows = page_rows(&page);
    let mut by_code: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (code, sev) in &rows {
        by_code.entry(code).or_default().push(sev);
    }
    let mut faults = Vec::new();
    for code in Code::ALL {
        let name = code.as_str();
        match by_code.get(name).map(Vec::as_slice) {
            None => faults.push(format!(
                "{name}: no row on the codes page; add `| `{name}` | <severity> | ... |` to docs/wiki/Diagnostic-Codes.md"
            )),
            Some([sev]) => {
                let want = format!("{:?}", code.severity()).to_lowercase();
                if *sev != want {
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
        if Code::parse(code).is_none() {
            faults.push(format!(
                "{code}: a row names no code in Code::ALL; remove the row, or restore the code"
            ));
        }
    }
    assert!(faults.is_empty(), "{}", faults.join("\n"));
}
