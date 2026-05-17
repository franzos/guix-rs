//! Subprocess spawn helpers. Every spawn goes through here so `LC_ALL=C`
//! is never forgotten. See NOTES.md.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::process::Command;

use crate::error::{GuixError, PolkitFailure};

/// pkexec rejects binaries outside trusted paths — polkit action is keyed here.
pub(crate) const POLKIT_GUIX_PATH: &str = "/run/current-system/profile/bin/guix";

pub(crate) const PKEXEC_PATH: &str = "/run/privileged/bin/pkexec";

/// `Stdio::null()` on stdin — substitute/GPG prompts would otherwise hang.
pub(crate) fn cmd(program: impl AsRef<OsStr>) -> Command {
    let mut c = Command::new(program);
    c.env("LC_ALL", "C")
        .env("LANG", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    c
}

/// `-p` is a subcommand flag — spliced after the subcommand. Pass
/// `with_profile=false` for `gc` / top-level `pull` which don't accept it.
/// `kill_on_drop` is a panic-safety backstop only; normal cancel goes via
/// [`crate::process::graceful_kill`].
pub(crate) fn guix_cmd<I, S>(
    binary: &Path,
    profile: Option<&Path>,
    with_profile: bool,
    args: I,
) -> Command
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut c = cmd(binary);
    c.kill_on_drop(true);
    let mut iter = args.into_iter();
    if let Some(subcmd) = iter.next() {
        c.arg(subcmd);
        if with_profile {
            if let Some(p) = profile {
                c.arg("-p").arg(p);
            }
        }
        for a in iter {
            c.arg(a);
        }
    }
    c
}

pub(crate) async fn run_guix<I, S>(
    binary: &Path,
    profile: Option<&Path>,
    args: I,
) -> Result<Vec<u8>, GuixError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut c = guix_cmd(binary, profile, true, args);
    let out = c.output().await.map_err(GuixError::Spawn)?;
    if !out.status.success() {
        let code = out.status.code().unwrap_or(-1);
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        return Err(GuixError::NonZeroExit { code, stderr });
    }
    Ok(out.stdout)
}

/// Precedence: `LIBGUIX_PKEXEC` → [`PKEXEC_PATH`] → `$PATH`. Env
/// override is NOT stat-checked so tests get verbatim spawn errors.
fn resolve_pkexec() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("LIBGUIX_PKEXEC") {
        return Some(PathBuf::from(p));
    }
    let preferred = PathBuf::from(PKEXEC_PATH);
    if preferred.exists() {
        return Some(preferred);
    }
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join("pkexec");
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

/// Pre-flights pkexec + system-guix existence so the GUI surfaces precise
/// errors instead of a generic 127. Test override: `LIBGUIX_POLKIT_GUIX`.
pub(crate) fn pkexec_guix_cmd<I, S>(args: I) -> Result<Command, GuixError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let pkexec = resolve_pkexec().ok_or_else(|| GuixError::Polkit {
        kind: PolkitFailure::PkexecMissing,
        code: -1,
        stderr_tail: "pkexec not found".into(),
    })?;

    let guix = if let Some(p) = std::env::var_os("LIBGUIX_POLKIT_GUIX") {
        PathBuf::from(p)
    } else {
        let p = PathBuf::from(POLKIT_GUIX_PATH);
        if !p.exists() {
            return Err(GuixError::NotOnGuixSystem);
        }
        p
    };

    let mut c = cmd(pkexec);
    c.kill_on_drop(true);
    c.arg(guix);
    for a in args {
        c.arg(a);
    }
    Ok(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_pkexec_env_override_wins() {
        let saved = std::env::var_os("LIBGUIX_PKEXEC");
        std::env::set_var("LIBGUIX_PKEXEC", "/tmp/libguix-fake-pkexec-xyz");
        let r = resolve_pkexec();
        assert_eq!(r, Some(PathBuf::from("/tmp/libguix-fake-pkexec-xyz")));
        if let Some(v) = saved {
            std::env::set_var("LIBGUIX_PKEXEC", v);
        } else {
            std::env::remove_var("LIBGUIX_PKEXEC");
        }
    }
}
