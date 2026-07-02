//! User-level `~/.config/guix/channels.scm` editing — read, validate,
//! apply structured ops via the REPL actor + `channel_ops.scm` helper.
//!
//! Writes go through `(guix read-print) pretty-print-with-comments` —
//! no pure-Rust serializer. See TODO.md "Phase 1 — libguix: channels
//! module" for the design rationale.
//!
//! Phase 1b complete: read, `is_writable`, `validate`, `AddChannel`,
//! `RemoveChannelByName`, and atomic `.bak`-then-rename writes via
//! `write_atomic`.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::error::GuixError;
use crate::parsers::sexp::{parse_channels_list, ChannelsList};
use crate::repl::Repl;
use crate::types::Channel;

/// The error surface for channels ops — distinct from `GuixError` so the
/// GUI can render structured parse errors with line/column.
#[derive(Debug, thiserror::Error)]
pub enum ChannelsError {
    #[error("parse error{}: {message}", line_col_suffix(*line, *column))]
    ParseError {
        message: String,
        line: Option<u32>,
        column: Option<u32>,
    },

    #[error("eval error{}: {message}", line_col_suffix(*line, *column))]
    EvalError {
        message: String,
        line: Option<u32>,
        column: Option<u32>,
    },

    #[error("channel `{name}` already exists")]
    DuplicateName { name: String },

    #[error("channel `{name}` has no introduction — discovery-side guarantee")]
    MissingIntroduction { name: String },

    #[error("operation `{op}` not supported")]
    UnsupportedOp { op: String },

    #[error("channel `{name}` not found")]
    NotFound { name: String },

    #[error("channels.scm at {path} is store-managed (guix home / read-only). Set a writable source-path override.")]
    StoreManaged { path: PathBuf },

    #[error("channel name `{name}` contains characters that aren't valid in a Scheme symbol")]
    InvalidName { name: String },

    #[error("invalid {field}: {reason}")]
    Invalid { field: String, reason: String },

    #[error("channels.scm not found at {path}")]
    FileNotFound { path: PathBuf },

    #[error("internal error: {0}")]
    Internal(String),

    #[error(transparent)]
    Guix(#[from] GuixError),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

fn line_col_suffix(line: Option<u32>, column: Option<u32>) -> String {
    match (line, column) {
        (Some(l), Some(c)) => format!(" at {l}:{c}"),
        (Some(l), None) => format!(" at line {l}"),
        _ => String::new(),
    }
}

/// In-memory view of `channels.scm` plus the data we need to decide
/// whether the file is safe to overwrite.
#[derive(Debug, Clone)]
pub struct ChannelsFile {
    pub path: PathBuf,
    pub list: ChannelsList,
    pub raw: String,
    /// True when `path` resolves into `/gnu/store/...` — typical of
    /// `guix home`-managed configs. Writes must use a writable
    /// source-path override in that case.
    pub is_store_managed: bool,
}

impl ChannelsFile {
    /// Reads `~/.config/guix/channels.scm` (or the override) into a
    /// parsed `ChannelsFile`. Missing files surface as `io::ErrorKind::NotFound`.
    pub async fn read(path_override: Option<&Path>) -> Result<Self, ChannelsError> {
        let path = match path_override {
            Some(p) => p.to_path_buf(),
            None => default_path()?,
        };

        // `channels.scm` is at most a few KB — a blocking read is fine
        // and avoids pulling in the `fs` feature of tokio across the crate.
        let read_path = path.clone();
        let raw = match tokio::task::spawn_blocking(move || std::fs::read_to_string(&read_path))
            .await
            .map_err(|e| ChannelsError::Internal(format!("read task panicked: {e}")))?
        {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(ChannelsError::FileNotFound { path });
            }
            Err(e) => return Err(ChannelsError::Io(e)),
        };
        let list = parse_channels_list(&raw).map_err(|e| match e {
            GuixError::Parse(msg) => ChannelsError::ParseError {
                message: msg,
                line: None,
                column: None,
            },
            other => ChannelsError::Guix(other),
        })?;

        let is_store_managed = resolves_into_store(&path);

        Ok(ChannelsFile {
            path,
            list,
            raw,
            is_store_managed,
        })
    }

    /// Fresh in-memory `channels.scm` seeded with `(cons* %default-channels)`
    /// for the first-channel create case — nothing is written to disk yet.
    pub fn seed(path_override: Option<&Path>) -> Result<Self, ChannelsError> {
        let path = match path_override {
            Some(p) => p.to_path_buf(),
            None => default_path()?,
        };
        let raw = "(cons* %default-channels)".to_string();
        let list = parse_channels_list(&raw).map_err(|e| match e {
            GuixError::Parse(msg) => ChannelsError::ParseError {
                message: msg,
                line: None,
                column: None,
            },
            other => ChannelsError::Guix(other),
        })?;
        let is_store_managed = resolves_into_store(&path);
        Ok(ChannelsFile {
            path,
            list,
            raw,
            is_store_managed,
        })
    }

    /// `false` when the file resolves into `/gnu/store/...` — those are
    /// immutable. Other failures (permission etc.) are reported on
    /// actual write rather than here.
    pub fn is_writable(&self) -> bool {
        !self.is_store_managed
    }

    /// The `.bak` sibling that `write_atomic` produces on each edit.
    /// Always `self.path.with_extension("scm.bak")` regardless of the
    /// source extension — so `/tmp/foo` and `/tmp/foo.txt` both yield
    /// `/tmp/foo.scm.bak`. Use this when probing whether a previous
    /// edit exists (e.g. to enable a "Restore last backup" affordance).
    pub fn backup_path(&self) -> PathBuf {
        self.path.with_extension("scm.bak")
    }

    /// Sandboxed validation of an arbitrary source string via the REPL
    /// actor. Parses with `read-with-comments` to surface line/column
    /// when available. **Doesn't execute** the channels form — we only
    /// need to know whether Guile can read it.
    pub async fn validate(repl: &Repl, source: &str) -> Result<(), ChannelsError> {
        let escaped = scheme_quote_string(source);
        let form = format!(
            "(catch #t \
              (lambda () \
                (call-with-input-string {escaped} \
                  (lambda (port) \
                    (let loop () \
                      (let ((v (read port))) \
                        (if (eof-object? v) (list 'ok \"\") (loop)))))) ) \
              (lambda (key . args) \
                (list 'error 'parse-error \
                      (format #f \"~a: ~a\" key args) #f #f)))"
        );
        let v = repl.eval_persistent(&form).await?;
        match interpret_response(&v) {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// Applies an op via the Scheme helper, returning the new source
    /// string. Pre-flight validation (duplicate names, missing
    /// introduction) happens **before** we touch the actor.
    pub async fn apply(&self, repl: &Repl, op: ChannelOp) -> Result<String, ChannelsError> {
        self.preflight(&op)?;

        let op_sexp = op.to_scheme_sexp();
        let source_lit = scheme_quote_string(&self.raw);
        let form = format!("(libguix-rs:apply-channel-op {source_lit} '{op_sexp})");

        let v = repl.eval_persistent(&form).await?;
        let s = interpret_response(&v)?;
        Ok(s)
    }

    fn preflight(&self, op: &ChannelOp) -> Result<(), ChannelsError> {
        match op {
            ChannelOp::AddChannel(ch) => {
                if !is_valid_channel_name(&ch.name) {
                    return Err(ChannelsError::InvalidName {
                        name: ch.name.clone(),
                    });
                }
                validate_channel_fields(ch)?;
                if ch.introduction_commit.is_none() || ch.introduction_fingerprint.is_none() {
                    return Err(ChannelsError::MissingIntroduction {
                        name: ch.name.clone(),
                    });
                }
                if self.list.channels().iter().any(|c| c.name == ch.name) {
                    return Err(ChannelsError::DuplicateName {
                        name: ch.name.clone(),
                    });
                }
                Ok(())
            }
            ChannelOp::RemoveChannelByName(name) => {
                // `guix` comes from `%default-channels` in WithDefaults
                // form and isn't enumerated in the file; in Explicit
                // form it's a regular entry but removing it would break
                // the user's setup. Refuse both shapes.
                if name == "guix" {
                    return Err(ChannelsError::UnsupportedOp {
                        op: "remove `guix` channel".into(),
                    });
                }
                if !self.list.channels().iter().any(|c| c.name == *name) {
                    return Err(ChannelsError::NotFound { name: name.clone() });
                }
                Ok(())
            }
        }
    }

    /// Atomic write — refuses store-managed paths and symlinks. Writes a
    /// random-named tempfile in the same dir, fsyncs it, copies the
    /// current file to `.bak`, renames the tempfile over the target,
    /// then fsyncs the parent.
    ///
    /// Symlinks at `path` are rejected so dotfile setups that pin
    /// `channels.scm` from a config repo aren't silently broken.
    ///
    /// Each successful write overwrites the previous `.bak` — callers
    /// who need a session-pristine snapshot must capture it themselves.
    pub async fn write_atomic(&self, content: &str) -> Result<(), ChannelsError> {
        if self.is_store_managed {
            return Err(ChannelsError::StoreManaged {
                path: self.path.clone(),
            });
        }

        let path = self.path.clone();
        let bak_path = self.backup_path();
        let content = content.to_owned();
        tokio::task::spawn_blocking(move || -> Result<(), ChannelsError> {
            use std::fs;
            use std::io::Write as _;

            // `symlink_metadata` doesn't follow the link; replacing a
            // dotfiles-pinned `channels.scm` would silently break it.
            match fs::symlink_metadata(&path) {
                Ok(md) if md.file_type().is_symlink() => {
                    return Err(ChannelsError::Invalid {
                        field: "path".into(),
                        reason: format!(
                            "{} is a symlink; refusing to replace it. Resolve or remove the link first.",
                            path.display()
                        ),
                    });
                }
                Ok(_) | Err(_) => {} // regular file, missing → proceed
            }

            let parent = path.parent().unwrap_or_else(|| Path::new("."));

            // Random suffix avoids fixed-name `EEXIST` from a crashed
            // prior run and defeats same-uid pre-creation DoS.
            let mut named =
                tempfile::Builder::new()
                    .prefix(".channels.")
                    .suffix(".scm.tmp")
                    .tempfile_in(parent)
                    .map_err(ChannelsError::Io)?;
            named.write_all(content.as_bytes()).map_err(ChannelsError::Io)?;
            named.as_file().sync_all().map_err(ChannelsError::Io)?;

            // Copy (not rename) so the canonical path is never empty
            // between steps. Missing source means first-time write.
            match fs::copy(&path, &bak_path) {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(ChannelsError::Io(e)),
            }

            named.persist(&path).map_err(|e| ChannelsError::Io(e.error))?;

            // Best-effort: on filesystems without directory fsync this
            // is a no-op, not an error.
            if let Ok(dir) = fs::File::open(parent) {
                let _ = dir.sync_all();
            }

            Ok(())
        })
        .await
        .map_err(|e| ChannelsError::Internal(format!("write task panicked: {e}")))??;

        Ok(())
    }
}

/// The narrow op vocabulary the Scheme dispatcher pattern-matches on.
/// See `channel_ops.scm` and TODO.md "Wire protocol".
#[derive(Debug, Clone)]
pub enum ChannelOp {
    AddChannel(Channel),
    RemoveChannelByName(String),
}

impl ChannelOp {
    /// Serialises this op to its Scheme s-expression representation
    /// (un-quoted — the caller quotes it once with a leading `'`).
    fn to_scheme_sexp(&self) -> String {
        match self {
            ChannelOp::AddChannel(ch) => {
                let ch_sexp = channel_to_sexp(ch);
                format!("(add-channel {ch_sexp})")
            }
            ChannelOp::RemoveChannelByName(name) => {
                // Bare symbol — the outer op-sexp is quoted at the call
                // site, so the symbol arrives un-evaluated to the helper.
                format!("(remove-channel-by-name {})", scheme_symbol(name))
            }
        }
    }
}

/// Mirrors the shape `(guix channels)` reads.
fn channel_to_sexp(ch: &Channel) -> String {
    let mut s = String::from("(channel");
    let _ = write!(s, " (name '{})", scheme_symbol(&ch.name));
    let _ = write!(s, " (url {})", scheme_quote_string(&ch.url));
    if let Some(b) = &ch.branch {
        let _ = write!(s, " (branch {})", scheme_quote_string(b));
    }
    if let Some(c) = &ch.commit {
        let _ = write!(s, " (commit {})", scheme_quote_string(c));
    }
    if let (Some(ic), Some(fpr)) = (&ch.introduction_commit, &ch.introduction_fingerprint) {
        let _ = write!(
            s,
            " (introduction (make-channel-introduction {} (openpgp-fingerprint {})))",
            scheme_quote_string(ic),
            scheme_quote_string(fpr),
        );
    }
    s.push(')');
    s
}

/// Beyond the Scheme-symbol shape check on `name`, blocks ASCII controls
/// and deceptive Unicode (bidi, zero-width, separators) so a malicious
/// discovery feed can't smuggle spoofed URLs into `channels.scm`.
fn validate_channel_fields(ch: &Channel) -> Result<(), ChannelsError> {
    validate_url(&ch.url)?;
    if let Some(b) = &ch.branch {
        validate_branch(b)?;
    }
    if let Some(c) = &ch.introduction_commit {
        validate_introduction_commit(c)?;
    }
    if let Some(f) = &ch.introduction_fingerprint {
        validate_introduction_fingerprint(f)?;
    }
    if matches!(ch.url.split("://").next(), Some("http" | "git")) {
        // Warn-only by policy; MITM on cleartext swaps the Guile module
        // set, so it must at least be observable in logs.
        #[cfg(feature = "tracing")]
        tracing::warn!(
            target: "libguix::channels",
            url = %ch.url,
            "insecure channel scheme (http:// or git://) — vulnerable to MITM"
        );
    }
    Ok(())
}

/// Invisible or direction-altering codepoints. The URL is shown to the
/// user verbatim before confirmation, so what they see must match what
/// `guix pull` resolves.
fn is_deceptive_unicode(c: char) -> bool {
    matches!(c,
        // Bidi controls (RTL override, embedding, isolates)
        '\u{200E}' | '\u{200F}'
        | '\u{202A}'..='\u{202E}'
        | '\u{2066}'..='\u{2069}'
        // Zero-width chars
        | '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{FEFF}'
        // Line/paragraph separators
        | '\u{2028}' | '\u{2029}'
        // Soft hyphen, word joiner, invisible math operators
        | '\u{00AD}' | '\u{2060}' | '\u{2061}'..='\u{2064}'
    )
}

fn validate_url(url: &str) -> Result<(), ChannelsError> {
    const SCHEMES: &[&str] = &["https://", "http://", "git://", "ssh://", "file://"];
    if !SCHEMES.iter().any(|s| url.starts_with(s)) {
        return Err(ChannelsError::Invalid {
            field: "url".into(),
            reason: "must start with https://, http://, git://, ssh:// or file://".into(),
        });
    }
    if url.len() > 2048 {
        return Err(ChannelsError::Invalid {
            field: "url".into(),
            reason: format!("length {} exceeds 2048", url.len()),
        });
    }
    for c in url.chars() {
        if c.is_control() {
            return Err(ChannelsError::Invalid {
                field: "url".into(),
                reason: format!("contains control char (U+{:04X})", c as u32),
            });
        }
        if is_deceptive_unicode(c) {
            return Err(ChannelsError::Invalid {
                field: "url".into(),
                reason: format!(
                    "contains deceptive Unicode codepoint (U+{:04X}) — bidi override, zero-width, or similar",
                    c as u32
                ),
            });
        }
    }
    Ok(())
}

fn validate_branch(branch: &str) -> Result<(), ChannelsError> {
    if branch.is_empty() || branch.len() > 200 {
        return Err(ChannelsError::Invalid {
            field: "branch".into(),
            reason: format!("length {} out of range 1..=200", branch.len()),
        });
    }
    for c in branch.chars() {
        let ok = c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '/' | '-' | '+');
        if !ok {
            return Err(ChannelsError::Invalid {
                field: "branch".into(),
                reason: format!("contains disallowed character `{c}`"),
            });
        }
    }
    Ok(())
}

fn validate_introduction_commit(commit: &str) -> Result<(), ChannelsError> {
    if commit.len() < 7 || commit.len() > 64 {
        return Err(ChannelsError::Invalid {
            field: "introduction.commit".into(),
            reason: format!("length {} out of range 7..=64", commit.len()),
        });
    }
    if !commit.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(ChannelsError::Invalid {
            field: "introduction.commit".into(),
            reason: "must be hex digits only".into(),
        });
    }
    Ok(())
}

fn validate_introduction_fingerprint(fpr: &str) -> Result<(), ChannelsError> {
    if fpr.len() < 8 || fpr.len() > 128 {
        return Err(ChannelsError::Invalid {
            field: "introduction.fingerprint".into(),
            reason: format!("length {} out of range 8..=128", fpr.len()),
        });
    }
    for c in fpr.chars() {
        if !(c.is_ascii_hexdigit() || c == ' ') {
            return Err(ChannelsError::Invalid {
                field: "introduction.fingerprint".into(),
                reason: format!("contains disallowed character `{c}`"),
            });
        }
    }
    Ok(())
}

/// Channel names must shape into a Scheme symbol so the embedded
/// `(name 'foo)` form parses. Validated up-front by `preflight` —
/// anything that fails this check is rejected with `InvalidName`
/// rather than silently rewritten on disk.
pub(crate) fn is_valid_channel_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '+' | '.'))
}

/// Emits the bare symbol form for a name that has already passed
/// [`is_valid_channel_name`]. Callers must validate first — this is a
/// formatter, not a sanitiser.
fn scheme_symbol(name: &str) -> &str {
    name
}

/// Scheme string literal — escapes `\` and `"`.
fn scheme_quote_string(s: &str) -> String {
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

/// Interprets the helper's `(ok …)` / `(error <sym> <msg> <line> <col>)`
/// response shape. See `channel_ops.scm` and TODO.md "Wire protocol".
fn interpret_response(v: &lexpr::Value) -> Result<String, ChannelsError> {
    let mut it = v.list_iter().ok_or_else(|| ChannelsError::ParseError {
        message: format!("response is not a list: {v:?}"),
        line: None,
        column: None,
    })?;
    let head =
        it.next()
            .and_then(lexpr::Value::as_symbol)
            .ok_or_else(|| ChannelsError::ParseError {
                message: format!("response missing head: {v:?}"),
                line: None,
                column: None,
            })?;

    match head {
        "ok" => {
            let payload = it.next().ok_or_else(|| ChannelsError::ParseError {
                message: "ok response missing payload".into(),
                line: None,
                column: None,
            })?;
            let s = payload.as_str().ok_or_else(|| ChannelsError::ParseError {
                message: format!("ok payload not a string: {payload:?}"),
                line: None,
                column: None,
            })?;
            Ok(s.to_owned())
        }
        "error" => {
            let kind = it
                .next()
                .and_then(lexpr::Value::as_symbol)
                .unwrap_or("unknown")
                .to_owned();
            let msg = it
                .next()
                .and_then(lexpr::Value::as_str)
                .unwrap_or("<no message>")
                .to_owned();
            let line = it.next().and_then(lexpr::Value::as_u64).map(|n| n as u32);
            let column = it.next().and_then(lexpr::Value::as_u64).map(|n| n as u32);
            Err(match kind.as_str() {
                "parse-error" => ChannelsError::ParseError {
                    message: msg,
                    line,
                    column,
                },
                "duplicate-name" => ChannelsError::DuplicateName { name: msg },
                "not-found" => ChannelsError::NotFound { name: msg },
                "unsupported-op" => ChannelsError::UnsupportedOp { op: msg },
                // Defensive: pre-flight already refuses removing `guix`
                // and unknown names, so these symbols mostly cover races
                // / future ops. Surface them with explicit context.
                "guix-locked" => ChannelsError::EvalError {
                    message: format!("guix-locked: {msg}"),
                    line,
                    column,
                },
                "wrapper-around-target" => ChannelsError::EvalError {
                    message: format!("wrapper-around-target: {msg}"),
                    line,
                    column,
                },
                "eval-error" => ChannelsError::EvalError {
                    message: msg,
                    line,
                    column,
                },
                _ => ChannelsError::EvalError {
                    message: format!("{kind}: {msg}"),
                    line,
                    column,
                },
            })
        }
        other => Err(ChannelsError::ParseError {
            message: format!("unexpected response head `{other}`: {v:?}"),
            line: None,
            column: None,
        }),
    }
}

fn default_path() -> Result<PathBuf, ChannelsError> {
    let home = std::env::var_os("HOME")
        .ok_or_else(|| ChannelsError::Internal("HOME not set; pass an explicit path".into()))?;
    let mut p = PathBuf::from(home);
    p.push(".config/guix/channels.scm");
    Ok(p)
}

/// `true` if `path` (after resolving symlinks) starts with `/gnu/store/`.
/// Tolerant: a path that doesn't exist at all is treated as writable
/// (callers create the file on first write).
fn resolves_into_store(path: &Path) -> bool {
    match std::fs::read_link(path) {
        Ok(target) => {
            // For relative symlink targets, anchor at the link's parent.
            let resolved = if target.is_absolute() {
                target
            } else {
                path.parent()
                    .map_or(target.clone(), |parent| parent.join(&target))
            };
            // Walk further in case the link points at another link.
            // `canonicalize` would do this but fails on dangling links;
            // we want a best-effort answer on offline tests too.
            let stringy = resolved.to_string_lossy().to_string();
            stringy.starts_with("/gnu/store/")
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_yields_empty_writable_non_store_defaults() {
        // Override into a tempdir so store-managed HOME setups don't skew
        // the writability assertion.
        let dir = tempfile::tempdir().expect("tempdir");
        let p = dir.path().join("channels.scm");
        let cf = ChannelsFile::seed(Some(&p)).expect("seed");
        assert_eq!(cf.raw, "(cons* %default-channels)");
        assert!(cf.list.channels().is_empty(), "seed has no custom channels");
        assert!(
            !cf.is_store_managed,
            "tempdir seed must not be store-managed"
        );
        assert!(cf.is_writable());
        assert_eq!(cf.path, p);
    }

    #[test]
    fn seed_none_resolves_default_path() {
        let cf = ChannelsFile::seed(None).expect("seed");
        assert!(cf.path.ends_with(".config/guix/channels.scm"));
        assert!(cf.list.channels().is_empty());
    }

    #[test]
    fn quote_string_escapes_backslash_and_quote() {
        assert_eq!(scheme_quote_string("a\\b\"c"), r#""a\\b\"c""#);
    }

    #[test]
    fn channel_to_sexp_includes_optional_fields() {
        let ch = Channel {
            name: "foo".into(),
            url: "https://example/foo.git".into(),
            branch: Some("master".into()),
            commit: Some("deadbeef".into()),
            introduction_commit: Some("intro-commit".into()),
            introduction_fingerprint: Some("AA BB".into()),
        };
        let s = channel_to_sexp(&ch);
        assert!(s.contains("(name 'foo)"));
        assert!(s.contains("(url \"https://example/foo.git\")"));
        assert!(s.contains("(branch \"master\")"));
        assert!(s.contains("(commit \"deadbeef\")"));
        assert!(s.contains("(introduction"));
        assert!(s.contains("\"AA BB\""));
    }

    #[test]
    fn channel_to_sexp_omits_missing_optionals() {
        let ch = Channel {
            name: "foo".into(),
            url: "https://example/foo.git".into(),
            branch: None,
            commit: None,
            introduction_commit: None,
            introduction_fingerprint: None,
        };
        let s = channel_to_sexp(&ch);
        assert!(!s.contains("branch"));
        assert!(!s.contains("commit"));
        assert!(!s.contains("introduction"));
    }

    #[test]
    fn is_valid_channel_name_accepts_scheme_safe() {
        assert!(is_valid_channel_name("good-name_1.2"));
        assert!(is_valid_channel_name("guix"));
        assert!(is_valid_channel_name("non+guix"));
    }

    #[test]
    fn is_valid_channel_name_rejects_pathological() {
        assert!(!is_valid_channel_name(""));
        assert!(!is_valid_channel_name("bad name"));
        assert!(!is_valid_channel_name("nope;(drop)"));
        assert!(!is_valid_channel_name("with/slash"));
    }

    #[test]
    fn validate_url_accepts_known_schemes() {
        assert!(validate_url("https://example.org/foo.git").is_ok());
        assert!(validate_url("http://example.org/foo.git").is_ok());
        assert!(validate_url("git://example.org/foo.git").is_ok());
        assert!(validate_url("ssh://git@example.org/foo.git").is_ok());
        assert!(validate_url("file:///srv/repos/foo.git").is_ok());
    }

    #[test]
    fn validate_url_rejects_unknown_scheme() {
        let err = validate_url("javascript:alert(1)").unwrap_err();
        assert!(matches!(err, ChannelsError::Invalid { field, .. } if field == "url"));
    }

    #[test]
    fn validate_url_rejects_control_chars() {
        // Embedded newline, NUL, DEL, zero-width — anything < 0x20 or 0x7f.
        assert!(validate_url("https://example.org/foo\nbar").is_err());
        assert!(validate_url("https://example.org/foo\0bar").is_err());
        assert!(validate_url("https://example.org/foo\u{7f}bar").is_err());
    }

    #[test]
    fn validate_url_rejects_deceptive_unicode() {
        // Bidi override, zero-width space, line separator, soft hyphen.
        assert!(validate_url("https://example.org/foo\u{202E}bar").is_err());
        assert!(validate_url("https://example.org/\u{200B}example.com").is_err());
        assert!(validate_url("https://example.org/\u{2028}").is_err());
        assert!(validate_url("https://example.org/foo\u{00AD}bar").is_err());
    }

    #[test]
    fn validate_url_rejects_over_length() {
        let mut s = String::from("https://example.org/");
        s.push_str(&"a".repeat(2048));
        assert!(validate_url(&s).is_err());
    }

    #[test]
    fn validate_branch_accepts_typical_names() {
        assert!(validate_branch("master").is_ok());
        assert!(validate_branch("release/1.2.3").is_ok());
        assert!(validate_branch("feature/foo-bar_baz+x").is_ok());
    }

    #[test]
    fn validate_branch_rejects_bad_chars() {
        assert!(validate_branch("").is_err());
        assert!(validate_branch("bad name").is_err());
        assert!(validate_branch("bad;name").is_err());
        assert!(validate_branch("bad\nname").is_err());
        assert!(validate_branch(&"x".repeat(201)).is_err());
    }

    #[test]
    fn validate_introduction_commit_accepts_hex() {
        assert!(validate_introduction_commit("abcdef0").is_ok());
        assert!(validate_introduction_commit(&"a".repeat(64)).is_ok());
        assert!(validate_introduction_commit("DEADBEEF").is_ok());
    }

    #[test]
    fn validate_introduction_commit_rejects_non_hex_or_bad_length() {
        assert!(validate_introduction_commit("xyz1234").is_err());
        assert!(validate_introduction_commit("abc").is_err());
        assert!(validate_introduction_commit(&"a".repeat(65)).is_err());
        assert!(validate_introduction_commit("dead beef").is_err());
    }

    #[test]
    fn validate_introduction_fingerprint_accepts_hex_with_spaces() {
        assert!(validate_introduction_fingerprint("AABBCCDD").is_ok());
        assert!(validate_introduction_fingerprint("AA BB CC DD EE FF").is_ok());
        assert!(validate_introduction_fingerprint(
            "BBB0 2EA2 96C4 B96B 8BA8  5DEA 16CD AC4F 0386 5722"
        )
        .is_ok());
    }

    #[test]
    fn validate_introduction_fingerprint_rejects_non_hex_or_bad_length() {
        assert!(validate_introduction_fingerprint("nothex!").is_err());
        assert!(validate_introduction_fingerprint("AA").is_err());
        assert!(validate_introduction_fingerprint(&"A".repeat(129)).is_err());
        assert!(validate_introduction_fingerprint("AA\nBB CC DD").is_err());
    }

    /// `/gnu/store/...` symlink targets must be flagged as
    /// non-writable. We can't realistically create one outside the
    /// real store, so test the detector through a dangling symlink
    /// whose target string is store-prefixed.
    #[cfg(unix)]
    #[test]
    fn resolves_into_store_detects_dangling_store_link() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().expect("tempdir");
        let link = dir.path().join("channels.scm");
        symlink(
            "/gnu/store/00000000000000000000000000000000-channels/channels.scm",
            &link,
        )
        .expect("symlink");
        assert!(
            resolves_into_store(&link),
            "expected store-shaped symlink target to trip is_store_managed"
        );
    }

    #[test]
    fn resolves_into_store_false_for_regular_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let p = dir.path().join("channels.scm");
        std::fs::write(&p, "(list)").expect("write");
        assert!(!resolves_into_store(&p));
    }

    /// Pins the `.bak` naming contract — must always yield `scm.bak`
    /// regardless of the source extension, because `write_atomic` uses
    /// the same `with_extension("scm.bak")` call. Mirror this in any
    /// caller that probes for the backup file.
    #[test]
    fn backup_path_is_always_scm_bak() {
        let mk = |path: &str| ChannelsFile {
            path: PathBuf::from(path),
            list: crate::parsers::sexp::ChannelsList::Explicit(Vec::new()),
            raw: String::new(),
            is_store_managed: false,
        };
        assert_eq!(
            mk("/tmp/channels.scm").backup_path(),
            PathBuf::from("/tmp/channels.scm.bak"),
        );
        assert_eq!(
            mk("/tmp/channels").backup_path(),
            PathBuf::from("/tmp/channels.scm.bak"),
        );
        assert_eq!(
            mk("/tmp/foo.txt").backup_path(),
            PathBuf::from("/tmp/foo.scm.bak"),
        );
    }
}
