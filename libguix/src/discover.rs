//! Resolve `guix` binary once and cache. `guix pull` rewrites
//! `~/.config/guix/current` — re-resolving mid-session yanks the binary.

use std::env;
use std::path::{Path, PathBuf};

use crate::cmd::cmd;
use crate::error::GuixError;
#[allow(unused_imports)]
use crate::trace_warn;

pub const MIN_GUIX_VERSION_DATE: &str = "2025-05-01";

#[derive(Debug, Clone)]
pub struct Discovered {
    pub binary: PathBuf,
    pub version: String,
}

pub fn resolve_binary() -> Result<PathBuf, GuixError> {
    let candidates = candidate_paths();
    for c in &candidates {
        if c.is_file() && is_executable(c) {
            return Ok(c.clone());
        }
    }
    Err(GuixError::Spawn(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!(
            "could not find a `guix` binary in any of: {}",
            candidates
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    )))
}

fn candidate_paths() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(p) = env::var_os("GUIX_PROFILE") {
        v.push(PathBuf::from(p).join("bin/guix"));
    }
    if let Some(home) = dirs_home() {
        v.push(home.join(".config/guix/current/bin/guix"));
    }
    v.push(PathBuf::from("/run/current-system/profile/bin/guix"));
    if let Some(path) = env::var_os("PATH") {
        for entry in env::split_paths(&path) {
            v.push(entry.join("guix"));
        }
    }
    v
}

fn dirs_home() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from)
}

#[cfg(unix)]
fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata()
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(p: &Path) -> bool {
    p.is_file()
}

pub async fn discover() -> Result<Discovered, GuixError> {
    let binary = resolve_binary()?;
    let out = cmd(&binary)
        .arg("--version")
        .output()
        .await
        .map_err(GuixError::Spawn)?;
    if !out.status.success() {
        return Err(GuixError::NonZeroExit {
            code: out.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        });
    }
    let first_line = String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()
        .unwrap_or("")
        .to_owned();

    // Release strings get a 1.4.0 floor; commit-hash builds (development
    // Guix) pass — we can't date them without git. Malformed warns + passes.
    if let Some(version_token) = first_line.split_whitespace().last() {
        if looks_like_release_version(version_token) {
            match release_version_at_least(version_token, "1.4.0") {
                Some(true) => {}
                Some(false) => {
                    return Err(GuixError::VersionUnsupported {
                        found: version_token.to_owned(),
                        min: format!("1.4.0 or commit build (anchor date {MIN_GUIX_VERSION_DATE})"),
                    });
                }
                None => {
                    trace_warn!(
                        target: "libguix::discover",
                        "could not parse guix version {:?}; assuming compatible",
                        version_token
                    );
                }
            }
        }
    }

    Ok(Discovered {
        binary,
        version: first_line,
    })
}

fn looks_like_release_version(s: &str) -> bool {
    s.chars()
        .next()
        .map(|c| c.is_ascii_digit())
        .unwrap_or(false)
        && s.contains('.')
}

/// Returns `None` if `found` has a non-numeric component — caller warns + passes.
fn release_version_at_least(found: &str, min: &str) -> Option<bool> {
    fn parse_strict(v: &str) -> Option<Vec<u32>> {
        v.split('.').map(|p| p.parse::<u32>().ok()).collect()
    }
    fn parse_lenient(v: &str) -> Vec<u32> {
        v.split('.').filter_map(|p| p.parse::<u32>().ok()).collect()
    }
    let a = parse_strict(found)?;
    let b = parse_lenient(min);
    Some(a >= b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_compare() {
        assert_eq!(release_version_at_least("1.4.0", "1.4.0"), Some(true));
        assert_eq!(release_version_at_least("1.4.1", "1.4.0"), Some(true));
        assert_eq!(release_version_at_least("2.0.0", "1.4.0"), Some(true));
        assert_eq!(release_version_at_least("1.3.0", "1.4.0"), Some(false));
    }

    #[test]
    fn release_compare_malformed_returns_none() {
        assert_eq!(release_version_at_least("1.foo", "1.4.0"), None);
        assert_eq!(release_version_at_least("foo.bar", "1.4.0"), None);
    }

    #[test]
    fn looks_like_release() {
        assert!(looks_like_release_version("1.4.0"));
        assert!(!looks_like_release_version("fc27102e8acb19"));
    }
}
