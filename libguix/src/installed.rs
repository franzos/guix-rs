//! Profile introspection: walks the user's manifest and buckets the
//! installed packages by their source channel.
//!
//! Attribution comes from `(guix describe) package-channels` — the same
//! routine `guix describe` uses internally. It matches the package's
//! `(package-location)` source-file against the per-channel checkout
//! store paths recorded in the pull-profile manifest (via the `source`
//! property written by `channel-instances->manifest`).
//!
//! `package-channels` returns at least the `guix` channel for any
//! package coming from `(gnu packages …)`, so the typical result is a
//! single-element list. An empty list means attribution failed
//! entirely; those packages land in the `(unknown)` bucket. The bucket
//! is preserved so callers see the full count; the GUI's Remove dialog
//! suppresses it deliberately because "(unknown)" isn't a channel name
//! the user typed.
//!
//! Performance note: `package-channels` walks the pull-profile manifest
//! on every invocation (no caching across calls). On a profile with a
//! few hundred packages this is sub-second after the lightweight warmup,
//! but it's not free — callers should cache the result.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::error::GuixError;
use crate::types::InstalledPackage;
use crate::Guix;

/// Bucket label assigned to entries whose channel attribution failed.
/// Public so callers can filter it out of user-facing surfaces.
pub const UNKNOWN_BUCKET: &str = "(unknown)";

/// Installed-package introspection via the REPL actor.
#[derive(Clone)]
pub struct InstalledOps {
    guix: Guix,
}

impl InstalledOps {
    pub(crate) fn new(guix: Guix) -> Self {
        Self { guix }
    }

    /// Returns installed packages grouped by their source channel.
    ///
    /// Channel attribution comes from `(guix describe) package-channels`.
    /// Packages with no resolvable channel (or whose Scheme-side lookup
    /// failed) bucket to `(unknown)`. When `package-channels` returns
    /// multiple names, the first wins — same behavior as `guix describe`.
    ///
    /// Walks the user's current profile manifest via the REPL actor's
    /// persistent namespace. Slow on big profiles (~hundreds of ms) —
    /// callers should cache.
    pub async fn by_channel(&self) -> Result<HashMap<String, Vec<InstalledPackage>>, GuixError> {
        let repl = self.guix.repl().await?;
        let profile = resolve_profile_path();
        let profile_str = profile.to_string_lossy().into_owned();

        let escaped = scheme_string(&profile_str);
        let form = format!("(libguix-rs:installed-with-locations {escaped})");
        let value = repl.eval_persistent(&form).await?;

        let entries = interpret_response(value)?;
        Ok(bucket_entries(entries))
    }
}

/// Unwraps the `(ok …)` / `(error …)` response shape from
/// `installed_ops.scm`. The error variant is propagated so callers can
/// log it instead of silently rendering an empty profile.
fn interpret_response(value: lexpr::Value) -> Result<Vec<InstalledEntry>, GuixError> {
    let mut it = value
        .list_iter()
        .ok_or_else(|| GuixError::Parse("installed-with-locations: not a list".into()))?;
    let head = it.next().and_then(lexpr::Value::as_symbol).ok_or_else(|| {
        GuixError::Parse("installed-with-locations: missing response head".into())
    })?;
    match head {
        "ok" => {
            let payload = it.next().ok_or_else(|| {
                GuixError::Parse("installed-with-locations: ok response missing payload".into())
            })?;
            Ok(parse_entries(payload.clone()))
        }
        "error" => {
            let msg = it
                .next()
                .and_then(scheme_string_value)
                .unwrap_or_else(|| "<no message>".into());
            Err(GuixError::Parse(format!(
                "installed-with-locations failed: {msg}"
            )))
        }
        other => Err(GuixError::Parse(format!(
            "installed-with-locations: unexpected head `{other}`"
        ))),
    }
}

/// `$GUIX_PROFILE` if set, else `~/.guix-profile`. Mirrors the
/// resolution `(guix profiles)` itself uses for the user's active
/// profile. We don't validate existence here — the Scheme helper
/// returns an empty list on any failure, which is the right shape for
/// "not introspectable".
fn resolve_profile_path() -> PathBuf {
    if let Some(p) = std::env::var_os("GUIX_PROFILE") {
        return PathBuf::from(p);
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"));
    home.join(".guix-profile")
}

#[derive(Debug, PartialEq, Eq)]
struct InstalledEntry {
    name: String,
    version: String,
    source_file: String,
    channels: Vec<String>,
}

/// Parses the Scheme response shape:
/// `((name version source-file (channel-name ...)) ...)`.
///
/// Symbols and strings are both accepted for the leaf fields so the
/// parser tolerates either reader output. Malformed entries are
/// dropped rather than erroring — the Scheme helper already swallows
/// exceptions and returns "" / `'()` for partial data.
fn parse_entries(value: lexpr::Value) -> Vec<InstalledEntry> {
    let mut out = Vec::new();
    let Some(iter) = value.list_iter() else {
        return out;
    };
    for entry in iter {
        let Some(fields) = entry.list_iter() else {
            continue;
        };
        let mut name = String::new();
        let mut version = String::new();
        let mut source_file = String::new();
        let mut channels: Vec<String> = Vec::new();
        for (i, field) in fields.enumerate() {
            match i {
                0 => name = scheme_string_value(field).unwrap_or_default(),
                1 => version = scheme_string_value(field).unwrap_or_default(),
                2 => source_file = scheme_string_value(field).unwrap_or_default(),
                3 => {
                    if let Some(list) = field.list_iter() {
                        for ch in list {
                            if let Some(s) = scheme_string_value(ch) {
                                if !s.is_empty() {
                                    channels.push(s);
                                }
                            }
                        }
                    }
                }
                _ => break,
            }
        }
        if !name.is_empty() {
            out.push(InstalledEntry {
                name,
                version,
                source_file,
                channels,
            });
        }
    }
    out
}

fn scheme_string_value(v: &lexpr::Value) -> Option<String> {
    match v {
        lexpr::Value::String(s) | lexpr::Value::Symbol(s) => Some(s.to_string()),
        _ => None,
    }
}

/// Buckets parsed entries by channel. Multi-channel results take the
/// first name (matches `guix describe`'s behavior). Empty channel
/// lists fall into `UNKNOWN_BUCKET`.
fn bucket_entries(entries: Vec<InstalledEntry>) -> HashMap<String, Vec<InstalledPackage>> {
    let mut buckets: HashMap<String, Vec<InstalledPackage>> = HashMap::new();
    for e in entries {
        let bucket = e
            .channels
            .first()
            .cloned()
            .unwrap_or_else(|| UNKNOWN_BUCKET.to_owned());
        // The Rust-side `InstalledPackage` carries a store_path; we
        // don't have one for these manifest-derived rows, so we store
        // the source-file path as a best-effort. The GUI only displays
        // name + version from this struct.
        buckets.entry(bucket).or_default().push(InstalledPackage {
            name: e.name,
            version: e.version,
            output: "out".into(),
            store_path: PathBuf::from(e.source_file),
        });
    }
    buckets
}

fn scheme_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use lexpr::from_str;

    /// Parses the canonical four-field shape and buckets each entry
    /// under its first channel name.
    #[test]
    fn parses_and_buckets_typical_response() {
        let v = from_str(
            r#"(("hello" "2.12" "/path/to/foo.scm" ("guix"))
                ("panther-foo" "0.4" "/path/to/bar.scm" ("pantherx")))"#,
        )
        .unwrap();
        let entries = parse_entries(v);
        assert_eq!(entries.len(), 2);
        let buckets = bucket_entries(entries);
        assert_eq!(buckets.len(), 2);
        let guix = buckets.get("guix").expect("guix bucket present");
        assert_eq!(guix.len(), 1);
        assert_eq!(guix[0].name, "hello");
        assert_eq!(guix[0].version, "2.12");
        let px = buckets.get("pantherx").expect("pantherx bucket present");
        assert_eq!(px.len(), 1);
        assert_eq!(px[0].name, "panther-foo");
    }

    /// An empty channel list signals attribution failure — bucket to
    /// `(unknown)` rather than dropping the package.
    #[test]
    fn empty_channel_list_buckets_to_unknown() {
        let v = from_str(r#"(("orphan" "1.0" "" ()))"#).unwrap();
        let entries = parse_entries(v);
        assert_eq!(entries.len(), 1);
        assert!(entries[0].channels.is_empty());
        let buckets = bucket_entries(entries);
        let unknown = buckets.get(UNKNOWN_BUCKET).expect("unknown bucket present");
        assert_eq!(unknown.len(), 1);
        assert_eq!(unknown[0].name, "orphan");
    }

    /// Multi-channel attribution picks the first listed — same as
    /// `guix describe`'s display behavior.
    #[test]
    fn multi_channel_picks_first() {
        let v = from_str(r#"(("shared" "1.0" "/x.scm" ("nonguix" "guix")))"#).unwrap();
        let buckets = bucket_entries(parse_entries(v));
        assert!(buckets.contains_key("nonguix"));
        assert!(!buckets.contains_key("guix"));
    }

    /// Symbols at the leaf (Guile sometimes reads bare identifiers as
    /// symbols, e.g. unquoted channel names) coerce to strings.
    #[test]
    fn accepts_symbol_channel_names() {
        let v = from_str(r#"(("hello" "2.12" "/x.scm" (guix)))"#).unwrap();
        let buckets = bucket_entries(parse_entries(v));
        assert!(buckets.contains_key("guix"));
    }

    /// An entirely empty response (Scheme helper returned `'()` on
    /// catch) yields an empty map — no error, no `(unknown)` bucket.
    #[test]
    fn empty_response_yields_empty_map() {
        let v = from_str("()").unwrap();
        let buckets = bucket_entries(parse_entries(v));
        assert!(buckets.is_empty());
    }
}
