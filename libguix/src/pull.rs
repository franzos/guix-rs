//! `guix pull` — per-user and root catalogs.

use std::path::PathBuf;

use crate::error::GuixError;
use crate::operation::{spawn_operation_with, Operation};
use crate::options::{privileged_guix_cmd, BuildOptions, Privilege};
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

    /// `guix pull` for the root catalog. Under [`Privilege::Pkexec`]
    /// (default) runs auth-agent pre-flight then `pkexec`; under
    /// [`Privilege::AlreadyRoot`] spawns guix directly (installer path,
    /// stderr-parsed). See [`PullOps::user`] for the per-user REPL path.
    pub fn as_root(&self, opts: SystemPullOptions) -> Result<Operation, GuixError> {
        let args = build_pull_args(&opts);
        if opts.privilege == Privilege::Pkexec {
            preflight_auth_agent()?;
        }
        let (cmd, classifier) =
            privileged_guix_cmd(opts.privilege, self.guix.binary_path(), &args)?;
        spawn_operation_with(cmd, classifier)
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

fn build_pull_args(opts: &SystemPullOptions) -> Vec<String> {
    let mut args: Vec<String> = vec!["pull".into()];
    if let Some(channels) = &opts.channels {
        args.push(format!("--channels={}", channels.to_string_lossy()));
    }
    opts.build.append_args(&mut args);
    if opts.dry_run {
        args.push("--dry-run".into());
    }
    args
}

#[derive(Debug, Clone, Default)]
pub struct SystemPullOptions {
    pub dry_run: bool,
    /// `--channels=<file>` — the installer points this at the generated
    /// `channels.scm`. `None` lets `guix` use its default channels file.
    pub channels: Option<PathBuf>,
    /// Substitute/scheduler flags forwarded to `guix`.
    pub build: BuildOptions,
    /// How to acquire root. Defaults to `pkexec`.
    pub privilege: Privilege,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_pull_options_are_quiet() {
        let o = SystemPullOptions::default();
        assert!(!o.dry_run);
        assert_eq!(o.privilege, Privilege::Pkexec);
        assert!(o.build.substitute_urls.is_empty());
    }

    #[test]
    fn pull_args_bare() {
        assert_eq!(build_pull_args(&SystemPullOptions::default()), vec!["pull"]);
    }

    #[test]
    fn pull_args_with_build_options_and_dry_run() {
        let opts = SystemPullOptions {
            dry_run: true,
            build: BuildOptions {
                substitute_urls: vec!["https://ci.example".into()],
                no_substitutes: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let args = build_pull_args(&opts);
        assert_eq!(
            args,
            vec![
                "pull",
                "--substitute-urls=https://ci.example",
                "--no-substitutes",
                "--dry-run",
            ]
        );
    }

    #[test]
    fn pull_args_with_channels_file() {
        let opts = SystemPullOptions {
            channels: Some(PathBuf::from("/mnt/etc/guix/channels.scm")),
            ..Default::default()
        };
        let args = build_pull_args(&opts);
        assert_eq!(args, vec!["pull", "--channels=/mnt/etc/guix/channels.scm"]);
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
