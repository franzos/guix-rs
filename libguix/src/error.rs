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
    /// Distinct from `NotAuthorized` (also 127) so we can surface the
    /// "binary missing" vs "not in trusted path" hint precisely.
    PkexecMissing,
}

/// Matched case-insensitively against guix's stderr to spot retryable
/// substitute/network hiccups.
const TRANSIENT_NEEDLES: &[&str] = &[
    "connection refused",
    "connection reset",
    "connection timed out",
    "connection closed",
    "timed out",
    "could not connect",
    "failed to connect",
    "name resolution",
    "temporary failure in name resolution",
    "network is unreachable",
    "no route to host",
    "tls handshake",
    "503 ",
    "502 ",
    "504 ",
    "bad gateway",
    "service unavailable",
    "gateway timeout",
    "download failed",
    "error downloading",
    "unexpected eof",
];

fn looks_transient(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    TRANSIENT_NEEDLES
        .iter()
        .any(|needle| lower.contains(needle))
}

impl GuixError {
    pub(crate) fn repl<S: Into<String>>(message: S) -> Self {
        GuixError::ReplProtocol {
            message: message.into(),
            stderr_tail: String::new(),
        }
    }

    /// Whether the failure looks like a retryable substitute/network hiccup.
    /// Conservative: only the variants carrying guix's own stderr/messages
    /// can be transient; everything else (spawn/io/polkit/cancel/…) is false.
    pub fn is_transient(&self) -> bool {
        match self {
            GuixError::OperationFailed { stderr_tail, .. } => looks_transient(stderr_tail),
            GuixError::NonZeroExit { stderr, .. } => looks_transient(stderr),
            GuixError::ReplProtocol {
                message,
                stderr_tail,
            } => looks_transient(message) || looks_transient(stderr_tail),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transient_operation_failed() {
        let e = GuixError::OperationFailed {
            code: 1,
            stderr_tail: "guix substitute: error: Connection timed out".into(),
        };
        assert!(e.is_transient());
    }

    #[test]
    fn ordinary_build_failure_is_not_transient() {
        let e = GuixError::NonZeroExit {
            code: 1,
            stderr: "guix build: error: build failed".into(),
        };
        assert!(!e.is_transient());
    }

    #[test]
    fn cancelled_is_not_transient() {
        assert!(!GuixError::Cancelled.is_transient());
    }

    #[test]
    fn polkit_is_not_transient() {
        let e = GuixError::Polkit {
            kind: PolkitFailure::AuthFailed,
            code: 126,
            stderr_tail: "connection refused".into(),
        };
        assert!(!e.is_transient());
    }

    #[test]
    fn match_is_case_insensitive() {
        let e = GuixError::OperationFailed {
            code: 1,
            stderr_tail: "CONNECTION REFUSED by substitute server".into(),
        };
        assert!(e.is_transient());
    }
}
