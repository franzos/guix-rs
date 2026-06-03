//! Shared knobs for the long-running write ops (`pull`, `system
//! reconfigure`, `system init`): how to elevate, and which build flags to
//! forward to `guix`.

use std::path::Path;

use crate::cmd::{cmd, pkexec_guix_cmd};
use crate::error::GuixError;
use crate::operation::ExitClassifier;
use tokio::process::Command;

/// How a privileged `guix` op acquires root.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Privilege {
    /// Elevate via `pkexec` — needs polkit and a running auth agent. The
    /// desktop default. Cancellation can't signal the root child (see
    /// [`crate::CancelHandle`]).
    #[default]
    Pkexec,
    /// The caller is already root: spawn `guix` directly, no `pkexec`.
    /// Required in the installer (bare TTY, no polkit/dbus). Cancellation
    /// works because the child is ours.
    AlreadyRoot,
}

/// Build-server and scheduler flags forwarded verbatim to `guix`. Spliced
/// after the subcommand so polkit's `argv1`/`argv2` binding holds (see
/// NOTES.md). `None`/`false` fields emit nothing — `guix` keeps its default.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildOptions {
    /// `--substitute-urls=<space-joined>`. Empty list emits nothing.
    pub substitute_urls: Vec<String>,
    /// `--no-substitutes` — build everything locally.
    pub no_substitutes: bool,
    /// `--fallback` — build from source if a substitute download fails.
    pub fallback: bool,
    /// `--cores=N` — build parallelism per derivation.
    pub cores: Option<u32>,
    /// `--max-jobs=N` — concurrent derivations.
    pub max_jobs: Option<u32>,
    /// `--system=<arch>` e.g. `x86_64-linux`.
    pub system: Option<String>,
}

impl BuildOptions {
    pub(crate) fn append_args(&self, args: &mut Vec<String>) {
        if !self.substitute_urls.is_empty() {
            args.push(format!(
                "--substitute-urls={}",
                self.substitute_urls.join(" ")
            ));
        }
        if self.no_substitutes {
            args.push("--no-substitutes".into());
        }
        if self.fallback {
            args.push("--fallback".into());
        }
        if let Some(c) = self.cores {
            args.push(format!("--cores={c}"));
        }
        if let Some(j) = self.max_jobs {
            args.push(format!("--max-jobs={j}"));
        }
        if let Some(s) = &self.system {
            args.push(format!("--system={s}"));
        }
    }
}

/// Builds the `guix` [`Command`] + matching [`ExitClassifier`] for the given
/// privilege. `Pkexec` keeps the trusted-path guix + pkexec pre-flight;
/// `AlreadyRoot` spawns `binary` directly with guix's own exit codes.
pub(crate) fn privileged_guix_cmd(
    privilege: Privilege,
    binary: &Path,
    args: &[String],
) -> Result<(Command, ExitClassifier), GuixError> {
    match privilege {
        Privilege::Pkexec => Ok((pkexec_guix_cmd(args)?, ExitClassifier::Pkexec)),
        Privilege::AlreadyRoot => {
            let mut c = cmd(binary);
            c.kill_on_drop(true);
            for a in args {
                c.arg(a);
            }
            Ok((c, ExitClassifier::Standard))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_build_options_emit_nothing() {
        let mut args = Vec::new();
        BuildOptions::default().append_args(&mut args);
        assert!(args.is_empty());
    }

    #[test]
    fn build_options_emit_all_flags_in_order() {
        let opts = BuildOptions {
            substitute_urls: vec![
                "https://ci.guix.gnu.org".into(),
                "https://bordeaux.guix.gnu.org".into(),
            ],
            no_substitutes: true,
            fallback: true,
            cores: Some(4),
            max_jobs: Some(2),
            system: Some("x86_64-linux".into()),
        };
        let mut args = Vec::new();
        opts.append_args(&mut args);
        assert_eq!(
            args,
            vec![
                "--substitute-urls=https://ci.guix.gnu.org https://bordeaux.guix.gnu.org",
                "--no-substitutes",
                "--fallback",
                "--cores=4",
                "--max-jobs=2",
                "--system=x86_64-linux",
            ]
        );
    }

    #[test]
    fn empty_substitute_urls_emit_nothing() {
        let opts = BuildOptions {
            substitute_urls: vec![],
            cores: Some(8),
            ..Default::default()
        };
        let mut args = Vec::new();
        opts.append_args(&mut args);
        assert_eq!(args, vec!["--cores=8"]);
    }

    #[test]
    fn privilege_defaults_to_pkexec() {
        assert_eq!(Privilege::default(), Privilege::Pkexec);
    }

    #[test]
    fn already_root_spawns_binary_directly_with_standard_classifier() {
        let (cmd, classifier) = privileged_guix_cmd(
            Privilege::AlreadyRoot,
            Path::new("/run/current-system/profile/bin/guix"),
            &["system".into(), "init".into()],
        )
        .expect("already-root cmd");
        let std = cmd.as_std();
        assert_eq!(
            std.get_program(),
            std::ffi::OsStr::new("/run/current-system/profile/bin/guix")
        );
        let argv: Vec<_> = std.get_args().collect();
        assert_eq!(argv, vec!["system", "init"]);
        assert!(matches!(classifier, ExitClassifier::Standard));
    }
}
