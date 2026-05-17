//! `guix pull` — per-user and root catalogs.

use std::path::PathBuf;

use crate::cmd::pkexec_guix_cmd;
use crate::error::GuixError;
use crate::operation::{spawn_operation_with, ExitClassifier, Operation};
use crate::repl;
use crate::system::{preflight_auth_agent, GUIX_PROFILES_ROOT};
use crate::Guix;

#[derive(Clone)]
pub struct PullOps {
    guix: Guix,
}

impl PullOps {
    pub(crate) fn new(guix: Guix) -> Self {
        Self { guix }
    }

    /// `guix pull` via a `guix repl -t machine` subprocess; structured
    /// fd-3 events instead of stderr scraping. See `crate::repl::op`.
    pub fn user(&self) -> Result<Operation, GuixError> {
        repl::op::spawn_repl_op(self.guix.binary_path(), repl::op::PULL_SCHEME)
    }

    /// `pkexec guix pull` — root catalog. See [`PullOps::user`] for per-user.
    pub fn as_root(&self, opts: SystemPullOptions) -> Result<Operation, GuixError> {
        preflight_auth_agent()?;

        let mut args: Vec<String> = vec!["pull".into()];
        if opts.dry_run {
            args.push("--dry-run".into());
        }

        let c = pkexec_guix_cmd(&args)?;
        spawn_operation_with(c, ExitClassifier::Pkexec)
    }

    /// mtime of this symlink is the canonical "last user-pull" signal —
    /// `~/.config/guix/current` is a user-owned symlink and can drift on
    /// foreign distros. Falls back to `"root"` if `$USER` is unset.
    pub fn user_path() -> PathBuf {
        let user = std::env::var("USER").unwrap_or_else(|_| "root".to_string());
        PathBuf::from(format!(
            "{}/per-user/{}/current-guix",
            GUIX_PROFILES_ROOT, user
        ))
    }

    pub fn root_path() -> PathBuf {
        PathBuf::from(format!("{}/per-user/root/current-guix", GUIX_PROFILES_ROOT))
    }
}

#[derive(Debug, Clone, Default)]
pub struct SystemPullOptions {
    pub dry_run: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_pull_options_are_quiet() {
        let o = SystemPullOptions::default();
        assert!(!o.dry_run);
    }

    #[test]
    fn pull_path_helpers_return_expected_layout() {
        let root = PullOps::root_path();
        assert_eq!(
            root,
            PathBuf::from("/var/guix/profiles/per-user/root/current-guix")
        );

        let user = PullOps::user_path();
        let s = user.to_string_lossy();
        assert!(
            s.starts_with("/var/guix/profiles/per-user/"),
            "user path under wrong root: {s}"
        );
        assert!(
            s.ends_with("/current-guix"),
            "user path doesn't end in /current-guix: {s}"
        );
    }
}
