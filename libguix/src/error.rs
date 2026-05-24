use std::io;
use thiserror::Error;

use crate::types::KnownBug;

#[derive(Debug, Error)]
pub enum GuixError {
    #[error("failed to spawn guix subprocess: {0}")]
    Spawn(#[source] io::Error),

    #[error("io error: {0}")]
    Io(#[source] io::Error),

    #[error("guix exited with code {code}: {stderr}")]
    NonZeroExit { code: i32, stderr: String },

    #[error("parse error: {0}")]
    Parse(String),

    /// `stderr_tail`: last ~64 KB of repl stderr for triage.
    #[error("repl protocol error: {message}{}",
        if stderr_tail.is_empty() {
            String::new()
        } else {
            format!("\n--- recent repl stderr ---\n{stderr_tail}")
        }
    )]
    ReplProtocol {
        message: String,
        stderr_tail: String,
    },

    #[error("guix version {found} is below the supported minimum {min}")]
    VersionUnsupported { found: String, min: String },

    #[error("operation cancelled")]
    Cancelled,

    #[error("operation failed with code {code}{}",
        if stderr_tail.is_empty() {
            String::new()
        } else {
            format!("\n--- recent stderr ---\n{stderr_tail}")
        }
    )]
    OperationFailed { code: i32, stderr_tail: String },

    #[error("polkit failure: {kind:?} (code {code}){}",
        if stderr_tail.is_empty() {
            String::new()
        } else {
            format!("\n--- recent stderr ---\n{stderr_tail}")
        }
    )]
    Polkit {
        kind: PolkitFailure,
        code: i32,
        stderr_tail: String,
    },

    #[error("hit known guix bug: {0:?} — see {url}", url = .0.url())]
    KnownBug(KnownBug),

    #[error("not running on a Guix System (no /run/current-system/configuration.scm)")]
    NotOnGuixSystem,

    #[error("internal: {0}")]
    Internal(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolkitFailure {
    AuthFailed,
    NotAuthorized,
    KilledBySignal(i32),
    NoAuthAgent,
    /// Distinct from `NotAuthorized` (also 127) so we can surface the
    /// "binary missing" vs "not in trusted path" hint precisely.
    PkexecMissing,
}

impl GuixError {
    pub(crate) fn repl<S: Into<String>>(message: S) -> Self {
        GuixError::ReplProtocol {
            message: message.into(),
            stderr_tail: String::new(),
        }
    }
}
