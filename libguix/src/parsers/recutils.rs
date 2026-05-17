//! Minimal recutils parser for `guix package -s` / `--show`. `+ ` continues
//! the previous field; blank line separates records.

use crate::error::GuixError;

#[derive(Debug, Default, Clone)]
pub(crate) struct Record {
    pub fields: Vec<(String, String)>,
}

impl Record {
    pub(crate) fn get(&self, key: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}

pub(crate) fn parse(input: &str) -> Result<Vec<Record>, GuixError> {
    let mut records = Vec::new();
    let mut current = Record::default();
    let mut last_key: Option<String> = None;

    for (lineno, raw_line) in input.split('\n').enumerate() {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);

        if line.is_empty() {
            if !current.fields.is_empty() {
                records.push(std::mem::take(&mut current));
                last_key = None;
            }
            continue;
        }

        if let Some(rest) = line.strip_prefix("+ ") {
            let key = last_key.as_deref().ok_or_else(|| {
                GuixError::Parse(format!(
                    "recutils line {}: continuation `+ …` with no preceding field",
                    lineno + 1
                ))
            })?;
            let slot = current
                .fields
                .iter_mut()
                .rev()
                .find(|(k, _)| k == key)
                .ok_or_else(|| {
                    GuixError::Parse(format!(
                        "recutils line {}: continuation `+ …` references field {key:?} that is not in the current record",
                        lineno + 1
                    ))
                })?;
            slot.1.push('\n');
            slot.1.push_str(rest);
            continue;
        }

        let Some(colon) = line.find(':') else {
            return Err(GuixError::Parse(format!(
                "recutils line {}: no `:` separator in {:?}",
                lineno + 1,
                line
            )));
        };
        let key = line[..colon].to_string();
        let value = if line.len() == colon + 1 {
            String::new()
        } else if line.as_bytes().get(colon + 1) == Some(&b' ') {
            line[colon + 2..].to_string()
        } else {
            line[colon + 1..].to_string()
        };
        last_key = Some(key.clone());
        current.fields.push((key, value));
    }

    if !current.fields.is_empty() {
        records.push(current);
    }
    Ok(records)
}

pub(crate) fn split_ws(s: &str) -> Vec<String> {
    s.split_whitespace().map(str::to_owned).collect()
}

pub(crate) fn parse_outputs(value: &str) -> Vec<String> {
    value
        .lines()
        .filter_map(|l| {
            let l = l.trim_start();
            if l.is_empty() {
                return None;
            }
            let name = l.split_once(':').map(|(n, _)| n).unwrap_or(l);
            Some(name.trim().to_owned())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_search_hello_fixture() {
        let s = include_str!("../../tests/fixtures/search-hello.recutils");
        let recs = parse(s).unwrap();
        assert_eq!(recs.len(), 1);
        let r = &recs[0];
        assert_eq!(r.get("name"), Some("hello"));
        assert_eq!(r.get("version"), Some("2.12.3"));
        assert_eq!(r.get("relevance"), Some("30"));
        let outs = r.get("outputs").unwrap();
        assert_eq!(parse_outputs(outs), vec!["out".to_string()]);
        let desc = r.get("description").unwrap();
        assert!(desc.starts_with("GNU Hello prints"));
        assert_eq!(desc.lines().count(), 3);
        assert_eq!(
            split_ws(r.get("systems").unwrap()),
            vec!["x86_64-linux", "i686-linux"]
        );
    }

    #[test]
    fn parses_show_hello_fixture() {
        let s = include_str!("../../tests/fixtures/show-hello.recutils");
        let recs = parse(s).unwrap();
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].get("name"), Some("hello"));
        assert!(recs[0].get("relevance").is_none());
    }

    #[test]
    fn rejects_orphan_continuation() {
        let s = "+ orphan\n";
        assert!(parse(s).is_err());
    }

    #[test]
    fn handles_empty_value() {
        let s = "name: hello\ndependencies: \n";
        let recs = parse(s).unwrap();
        assert_eq!(recs[0].get("dependencies"), Some(""));
    }

    #[test]
    fn parses_two_records_back_to_back() {
        let s = "\
name: alpha
version: 1.0
synopsis: first

name: beta
version: 2.0
synopsis: second
";
        let recs = parse(s).unwrap();
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0].get("name"), Some("alpha"));
        assert_eq!(recs[0].get("version"), Some("1.0"));
        assert_eq!(recs[1].get("name"), Some("beta"));
        assert_eq!(recs[1].get("synopsis"), Some("second"));
    }

    #[test]
    fn parses_record_without_trailing_blank_line() {
        let s = "name: solo\nversion: 0.1";
        let recs = parse(s).unwrap();
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].get("name"), Some("solo"));
        assert_eq!(recs[0].get("version"), Some("0.1"));
    }
}
