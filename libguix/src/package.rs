//! `guix package` operations — read and write.

use crate::cmd::{guix_cmd, run_guix};
use crate::error::GuixError;
use crate::operation::{spawn_operation, Operation};
use crate::parsers::recutils::{parse, parse_outputs, split_ws, Record};
use crate::parsers::tsv;
use crate::types::{Generation, InstalledPackage, PackageDetail, PackageSummary};
use crate::Guix;

/// Capped server-side via `call/cc` — dropping a 5k-cell `Value::Cons`
/// chain overflows the tokio worker stack. See NOTES.md.
pub const DEFAULT_SEARCH_LIMIT: usize = 200;

#[derive(Debug, Clone)]
pub struct SearchFastResult {
    pub results: Vec<PackageSummary>,
    pub truncated: bool,
    pub limit: usize,
}

#[derive(Clone)]
pub struct PackageOps {
    guix: Guix,
}

impl PackageOps {
    pub(crate) fn new(guix: Guix) -> Self {
        Self { guix }
    }

    fn binary(&self) -> &std::path::Path {
        self.guix.binary_path()
    }

    /// Crate-internal access for sibling modules (`installed.rs`) that
    /// extend `PackageOps` and need to reach the underlying `Guix`.
    pub(crate) fn guix_inner(&self) -> &crate::Guix {
        &self.guix
    }

    fn profile(&self) -> Option<&std::path::Path> {
        self.guix.profile_path()
    }

    async fn run(&self, args: &[&str]) -> Result<Vec<u8>, GuixError> {
        run_guix(self.binary(), self.profile(), args.iter().copied()).await
    }

    /// `guix package -s <query>`.
    pub async fn search(&self, query: &str) -> Result<Vec<PackageSummary>, GuixError> {
        let out = self.run(&["package", "-s", query]).await?;
        let s = String::from_utf8_lossy(&out);
        let records = parse(&s)?;
        Ok(records.into_iter().map(record_to_summary).collect())
    }

    /// `guix package --show=<name>`.
    pub async fn show(&self, name: &str) -> Result<PackageDetail, GuixError> {
        let arg = format!("--show={name}");
        let out = self.run(&["package", &arg]).await?;
        let s = String::from_utf8_lossy(&out);
        let mut records = parse(&s)?;
        let r = records.pop().ok_or_else(|| {
            GuixError::Parse(format!(
                "no record returned for `guix package --show={name}`"
            ))
        })?;
        Ok(record_to_detail(r))
    }

    /// `guix package -I`.
    pub async fn list_installed(&self) -> Result<Vec<InstalledPackage>, GuixError> {
        let out = self.run(&["package", "-I"]).await?;
        let s = String::from_utf8_lossy(&out);
        tsv::parse_installed(&s)
    }

    /// `guix package -l`.
    pub async fn list_generations(&self) -> Result<Vec<Generation>, GuixError> {
        let out = self.run(&["package", "-l"]).await?;
        let s = String::from_utf8_lossy(&out);
        tsv::parse_generations(&s)
    }

    /// Capped at [`DEFAULT_SEARCH_LIMIT`].
    pub async fn search_fast(&self, query: &str) -> Result<Vec<PackageSummary>, GuixError> {
        Ok(self
            .search_fast_limited(query, DEFAULT_SEARCH_LIMIT)
            .await?
            .results)
    }

    /// Each record is a 7-tuple `(name version synopsis description
    /// homepage license (outputs ...))` — homepage/license normalized in
    /// Guile so the Rust parser only sees strings.
    pub async fn search_fast_limited(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<SearchFastResult, GuixError> {
        let limit = limit.max(1);
        let repl = self.guix.repl().await?;

        let escaped = scheme_str_escape(query);
        // Result shape: `(truncated? . records)`. `call/cc` bails out of
        // fold-packages once we have `limit` matches.
        let form = format!(
            "(let* ((limit {limit}) \
                    (count 0) \
                    (truncated? #f) \
                    (homepage->str (lambda (h) (if (string? h) h \"\"))) \
                    (str-or-empty (lambda (x) (cond ((string? x) x) \
                                                    ((not x) \"\") \
                                                    (else (format #f \"~a\" x))))) \
                    (one-license->str \
                      (lambda (l) \
                        (cond ((not l) \"\") \
                              ((and (record? l) \
                                    (false-if-exception (license-name l))) \
                               => (lambda (n) (if (string? n) n (format #f \"~a\" n)))) \
                              (else (format #f \"~a\" l))))) \
                    (license->str \
                      (lambda (l) \
                        (cond ((not l) \"\") \
                              ((list? l) \
                               (string-join (map one-license->str l) \", \")) \
                              (else (one-license->str l))))) \
                    (acc (call-with-current-continuation \
                           (lambda (return) \
                             (fold-packages \
                               (lambda (p a) \
                                 (if (string-contains (package-name p) {q}) \
                                     (let ((a* (cons (list (package-name p) \
                                                           (package-version p) \
                                                           (str-or-empty (package-synopsis p)) \
                                                           (str-or-empty (package-description p)) \
                                                           (homepage->str (package-home-page p)) \
                                                           (license->str (package-license p)) \
                                                           (package-outputs p)) \
                                                     a))) \
                                       (set! count (+ count 1)) \
                                       (if (>= count limit) \
                                           (begin (set! truncated? #t) (return a*)) \
                                           a*)) \
                                     a)) \
                               '()))))) \
                (cons truncated? acc))",
            limit = limit,
            q = escaped,
        );

        let value = repl
            .eval_with_modules(
                &["(gnu packages)", "(guix packages)", "(guix licenses)"],
                &form,
            )
            .await?;

        // Destructure via `into_pair` so the cdr spine drops iteratively.
        let (truncated, results_value) = match value {
            lexpr::Value::Cons(cell) => {
                let (car, cdr) = cell.into_pair();
                let truncated = car.as_bool().unwrap_or(false);
                (truncated, cdr)
            }
            _ => (false, lexpr::Value::Null),
        };

        let results = parse_records(results_value);

        Ok(SearchFastResult {
            results,
            truncated,
            limit,
        })
    }

    /// REPL-native install via fd-3 events. Rejects newlines/null bytes in names.
    pub fn install(&self, packages: &[&str]) -> Result<Operation, GuixError> {
        let mut argv: Vec<&str> = vec!["-i"];
        argv.extend(packages.iter().copied());
        let payload = crate::repl::op::build_package_payload(self.profile(), &argv)?;
        crate::repl::op::spawn_repl_op(self.binary(), &payload)
    }

    pub fn remove(&self, packages: &[&str]) -> Result<Operation, GuixError> {
        let mut argv: Vec<&str> = vec!["-r"];
        argv.extend(packages.iter().copied());
        let payload = crate::repl::op::build_package_payload(self.profile(), &argv)?;
        crate::repl::op::spawn_repl_op(self.binary(), &payload)
    }

    /// `regex = None` upgrades everything; `Some(re)` filters by name.
    pub fn upgrade(&self, regex: Option<&str>) -> Result<Operation, GuixError> {
        let argv: Vec<&str> = match regex {
            None => vec!["-u"],
            Some(r) => vec!["-u", r],
        };
        let payload = crate::repl::op::build_package_payload(self.profile(), &argv)?;
        crate::repl::op::spawn_repl_op(self.binary(), &payload)
    }

    pub fn rollback(&self) -> Result<Operation, GuixError> {
        let c = guix_cmd(
            self.binary(),
            self.profile(),
            true,
            ["package", "--roll-back"],
        );
        spawn_operation(c)
    }

    /// `guix package -S <n>` — accepts relative offsets like `-1`.
    pub fn switch_generation(&self, n: i64) -> Result<Operation, GuixError> {
        let n_str = n.to_string();
        let c = guix_cmd(
            self.binary(),
            self.profile(),
            true,
            ["package", "-S", &n_str],
        );
        spawn_operation(c)
    }

    pub fn delete_generations(&self, spec: &str) -> Result<Operation, GuixError> {
        let c = guix_cmd(self.binary(), self.profile(), true, ["package", "-d", spec]);
        spawn_operation(c)
    }
}

fn scheme_str_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn value_to_string(v: lexpr::Value) -> String {
    match v {
        lexpr::Value::String(s) | lexpr::Value::Symbol(s) => s.to_string(),
        other => other.as_str().unwrap_or("").to_owned(),
    }
}

fn value_to_string_list(v: lexpr::Value) -> Vec<String> {
    let mut out = Vec::new();
    let cell = match v {
        lexpr::Value::Cons(c) => c,
        _ => return out,
    };
    for (item, _) in cell {
        let s = value_to_string(item);
        if !s.is_empty() {
            out.push(s);
        }
    }
    out
}

/// Iterative spine dismantle via `Cons::into_iter` — naive `drop` of a
/// long cons-list overflows the stack. See NOTES.md + the regression
/// test `drains_long_cons_list_without_overflow`.
fn parse_records(value: lexpr::Value) -> Vec<PackageSummary> {
    let mut out = Vec::new();
    let cell = match value {
        lexpr::Value::Cons(c) => c,
        _ => return out,
    };

    for (record, _improper_tail) in cell {
        let record_cell = match record {
            lexpr::Value::Cons(c) => c,
            _ => continue,
        };
        let mut name = String::new();
        let mut version = String::new();
        let mut synopsis = String::new();
        let mut description = String::new();
        let mut homepage = String::new();
        let mut license = String::new();
        let mut outputs: Vec<String> = Vec::new();
        for (i, (field, _)) in record_cell.into_iter().enumerate() {
            match i {
                0 => name = value_to_string(field),
                1 => version = value_to_string(field),
                2 => synopsis = value_to_string(field),
                3 => description = value_to_string(field),
                4 => homepage = value_to_string(field),
                5 => license = value_to_string(field),
                6 => outputs = value_to_string_list(field),
                _ => {}
            }
        }
        if outputs.is_empty() {
            outputs.push("out".to_owned());
        }
        out.push(PackageSummary {
            name,
            version,
            synopsis,
            description,
            homepage,
            license,
            outputs,
        });
    }
    out
}

fn record_to_summary(r: Record) -> PackageSummary {
    PackageSummary {
        name: r.get("name").unwrap_or_default().to_owned(),
        version: r.get("version").unwrap_or_default().to_owned(),
        synopsis: r.get("synopsis").unwrap_or_default().trim().to_owned(),
        description: r.get("description").map(str::to_owned).unwrap_or_default(),
        homepage: r.get("homepage").map(str::to_owned).unwrap_or_default(),
        license: r.get("license").map(str::to_owned).unwrap_or_default(),
        outputs: r.get("outputs").map(parse_outputs).unwrap_or_default(),
    }
}

fn record_to_detail(r: Record) -> PackageDetail {
    PackageDetail {
        name: r.get("name").unwrap_or_default().to_owned(),
        version: r.get("version").unwrap_or_default().to_owned(),
        synopsis: r.get("synopsis").unwrap_or_default().trim().to_owned(),
        description: r.get("description").unwrap_or_default().to_owned(),
        homepage: r
            .get("homepage")
            .map(str::to_owned)
            .filter(|s| !s.is_empty()),
        license: r
            .get("license")
            .map(str::to_owned)
            .filter(|s| !s.is_empty()),
        location: r
            .get("location")
            .map(str::to_owned)
            .filter(|s| !s.is_empty()),
        outputs: r.get("outputs").map(parse_outputs).unwrap_or_default(),
        systems: r.get("systems").map(split_ws).unwrap_or_default(),
        dependencies: r.get("dependencies").map(split_ws).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins iterative spine dismantle — naive Drop overflows the stack.
    #[test]
    fn drains_long_cons_list_without_overflow() {
        const N: usize = 10_000;

        let mut spine = lexpr::Value::Null;
        for i in 0..N {
            let name = format!("pkg-{i}");
            let record = lexpr::Value::list(vec![
                lexpr::Value::String(name.into()),
                lexpr::Value::String("1.0".into()),
                lexpr::Value::String("synopsis".into()),
                lexpr::Value::String("description".into()),
                lexpr::Value::String("https://example.org".into()),
                lexpr::Value::String("GPL3+".into()),
                lexpr::Value::list(vec![lexpr::Value::String("out".into())]),
            ]);
            spine = lexpr::Value::Cons(lexpr::Cons::new(record, spine));
        }

        let results = parse_records(spine);
        assert_eq!(results.len(), N);
        assert_eq!(results[0].name, format!("pkg-{}", N - 1));
        assert_eq!(results[N - 1].name, "pkg-0");
        assert_eq!(results[0].synopsis, "synopsis");
        assert_eq!(results[0].description, "description");
        assert_eq!(results[0].homepage, "https://example.org");
        assert_eq!(results[0].license, "GPL3+");
        assert_eq!(results[0].outputs, vec!["out".to_owned()]);
    }

    /// Iterative drain even when no records are produced.
    #[test]
    fn drains_long_non_record_cons_list_without_overflow() {
        const N: usize = 10_000;
        let mut spine = lexpr::Value::Null;
        for i in 0..N {
            spine = lexpr::Value::Cons(lexpr::Cons::new(lexpr::Value::from(i as i64), spine));
        }
        let results = parse_records(spine);
        assert!(results.is_empty());
    }

    #[test]
    fn outputs_falls_back_to_out_when_missing() {
        let record = lexpr::Value::list(vec![
            lexpr::Value::String("hello".into()),
            lexpr::Value::String("2.12".into()),
            lexpr::Value::String("Greeter".into()),
            lexpr::Value::String("Long desc.".into()),
            lexpr::Value::String("https://example.org".into()),
            lexpr::Value::String("GPL3+".into()),
            lexpr::Value::Null,
        ]);
        let spine = lexpr::Value::Cons(lexpr::Cons::new(record, lexpr::Value::Null));
        let results = parse_records(spine);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].outputs, vec!["out".to_owned()]);
    }

    #[test]
    fn default_search_limit_is_reasonable() {
        assert_eq!(DEFAULT_SEARCH_LIMIT, 200);
    }

    #[test]
    fn scheme_str_escape_basic() {
        assert_eq!(scheme_str_escape("hello"), "\"hello\"");
        assert_eq!(scheme_str_escape("a\"b"), "\"a\\\"b\"");
        assert_eq!(scheme_str_escape("a\\b"), "\"a\\\\b\"");
    }
}
