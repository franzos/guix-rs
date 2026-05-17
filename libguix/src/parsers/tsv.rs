//! Parsers for `guix package -I` (installed) and `-l` (generations).

use std::path::PathBuf;

use crate::error::GuixError;
use crate::types::{Generation, InstalledPackage};

pub(crate) fn parse_installed(input: &str) -> Result<Vec<InstalledPackage>, GuixError> {
    let mut out = Vec::new();
    for (lineno, raw) in input.lines().enumerate() {
        if raw.trim().is_empty() {
            continue;
        }
        out.push(parse_pkg_row(raw, lineno + 1)?);
    }
    Ok(out)
}

pub(crate) fn parse_generations(input: &str) -> Result<Vec<Generation>, GuixError> {
    let mut gens: Vec<Generation> = Vec::new();
    let mut current: Option<Generation> = None;

    for (lineno, raw) in input.lines().enumerate() {
        if raw.trim().is_empty() {
            if let Some(g) = current.take() {
                gens.push(g);
            }
            continue;
        }

        if let Some(rest) = raw.strip_prefix("Generation ") {
            if let Some(g) = current.take() {
                gens.push(g);
            }
            let cols: Vec<&str> = rest.split('\t').collect();
            if cols.is_empty() {
                return Err(GuixError::Parse(format!(
                    "generations line {}: empty header",
                    lineno + 1
                )));
            }
            let number = cols[0].trim().parse::<u32>().map_err(|e| {
                GuixError::Parse(format!(
                    "generations line {}: bad generation number {:?}: {}",
                    lineno + 1,
                    cols[0],
                    e
                ))
            })?;
            let date = cols.get(1).map(|s| s.trim().to_owned()).unwrap_or_default();
            let current_flag = cols.iter().any(|c| c.trim() == "(current)");
            current = Some(Generation {
                number,
                date,
                current: current_flag,
                packages: Vec::new(),
            });
            continue;
        }

        let line = raw.trim_start_matches(' ');
        let pkg = parse_pkg_row(line, lineno + 1)?;
        match current.as_mut() {
            Some(g) => g.packages.push(pkg),
            None => {
                return Err(GuixError::Parse(format!(
                    "generations line {}: package row before any `Generation N` header",
                    lineno + 1
                )))
            }
        }
    }

    if let Some(g) = current {
        gens.push(g);
    }
    Ok(gens)
}

fn parse_pkg_row(line: &str, lineno: usize) -> Result<InstalledPackage, GuixError> {
    let cols: Vec<&str> = line.split('\t').collect();
    if cols.len() < 4 {
        return Err(GuixError::Parse(format!(
            "tsv line {}: expected 4 tab-separated cols, got {} in {:?}",
            lineno,
            cols.len(),
            line
        )));
    }
    Ok(InstalledPackage {
        name: cols[0].trim().to_owned(),
        version: cols[1].trim().to_owned(),
        output: cols[2].trim().to_owned(),
        store_path: PathBuf::from(cols[3].trim()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_installed_fixture() {
        let s = include_str!("../../tests/fixtures/installed.tsv");
        let v = parse_installed(s).unwrap();
        assert_eq!(v.len(), 4);
        assert_eq!(v[0].name, "wpa-supplicant");
        assert_eq!(v[0].version, "2.10");
        assert_eq!(v[0].output, "out");
        assert!(v[0]
            .store_path
            .to_string_lossy()
            .ends_with("wpa-supplicant-2.10"));
        assert_eq!(v[3].name, "wakatime-cli");
        assert_eq!(v[3].version, "1.132.1");
    }

    #[test]
    fn parses_installed_empty_input() {
        assert_eq!(parse_installed("").unwrap().len(), 0);
        assert_eq!(parse_installed("   \n\n").unwrap().len(), 0);
    }

    #[test]
    fn parses_generations_fixture() {
        let s = include_str!("../../tests/fixtures/generations.tsv");
        let g = parse_generations(s).unwrap();
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].number, 28);
        assert!(g[0].current);
        assert_eq!(g[0].date, "Apr 09 2026 09:56:57");
        assert_eq!(g[0].packages.len(), 4);
        assert_eq!(g[0].packages[0].name, "wpa-supplicant");
        assert_eq!(g[0].packages[3].name, "wakatime-cli");
    }

    /// Both generations missing `(current)` — happens post-rollback.
    #[test]
    fn parses_generations_without_current_marker() {
        let s = "\
Generation 1\tJan 01 2026 00:00:00
  hello\t1.0\tout\t/gnu/store/aaa-hello-1.0

Generation 2\tJan 02 2026 00:00:00
  hello\t2.0\tout\t/gnu/store/bbb-hello-2.0
";
        let g = parse_generations(s).unwrap();
        assert_eq!(g.len(), 2);
        assert!(!g[0].current);
        assert!(!g[1].current);
        assert_eq!(g[0].number, 1);
        assert_eq!(g[1].number, 2);
        assert_eq!(g[1].packages.len(), 1);
        assert_eq!(g[1].packages[0].version, "2.0");
    }

    #[test]
    fn rejects_orphan_package_row() {
        let s = "  hello\t1.0\tout\t/gnu/store/x\n";
        assert!(parse_generations(s).is_err());
    }
}
