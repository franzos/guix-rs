//! `guix system` operations, including the pkexec'd `reconfigure` and
//! root-catalog `pull`. See NOTES.md for the two-catalog distinction.

use std::path::{Path, PathBuf};

use crate::error::GuixError;
use crate::operation::{spawn_operation_with, Operation};
use crate::options::{privileged_guix_cmd, BuildOptions, Privilege};

/// Display-only — reconfiguring requires the source `.scm`, not this snapshot.
pub const CURRENT_SYSTEM_CONFIG: &str = "/run/current-system/configuration.scm";

pub(crate) const GUIX_PROFILES_ROOT: &str = "/var/guix/profiles";

/// `binary` is only consulted under [`Privilege::AlreadyRoot`]; the `pkexec`
/// path always targets the trusted-path guix (see `cmd::POLKIT_GUIX_PATH`).
#[derive(Clone)]
pub struct SystemOps {
    binary: PathBuf,
}

impl SystemOps {
    pub(crate) fn new(binary: PathBuf) -> Self {
        Self { binary }
    }

    pub(crate) fn new_for_tests() -> Self {
        Self {
            binary: PathBuf::from("/run/current-system/profile/bin/guix"),
        }
    }

    /// Distinguishes `NotFound` (→ `NotOnGuixSystem`, expected on foreign
    /// distros) from permission errors (→ `Spawn`). `Path::exists()`
    /// collapses both.
    pub fn current_configuration_path(&self) -> Result<PathBuf, GuixError> {
        let p = resolve_config_path();
        current_configuration_path_with(&p)
    }

    /// `guix system reconfigure …`. Under [`Privilege::Pkexec`] (default)
    /// runs via `pkexec`; under [`Privilege::AlreadyRoot`] spawns guix
    /// directly (installer path). `-L` flag positioning is load-bearing
    /// for polkit — see NOTES.md.
    pub fn reconfigure(
        &self,
        config: &Path,
        opts: ReconfigureOptions,
    ) -> Result<Operation, GuixError> {
        let args = build_reconfigure_args(config, &opts);
        self.spawn_system_op(&args, opts.privilege)
    }

    /// `guix system init <config> <target>` — populates a freshly-mounted
    /// root (the installer uses `/mnt`). Identical progress output to
    /// `reconfigure`, so the same stderr parser covers it.
    pub fn init(
        &self,
        config: &Path,
        target: &Path,
        opts: InitOptions,
    ) -> Result<Operation, GuixError> {
        let args = build_init_args(config, target, &opts);
        self.spawn_system_op(&args, opts.privilege)
    }

    fn spawn_system_op(
        &self,
        args: &[String],
        privilege: Privilege,
    ) -> Result<Operation, GuixError> {
        let (cmd, classifier) = privileged_guix_cmd(privilege, &self.binary, args)?;
        spawn_operation_with(cmd, classifier)
    }
}

/// Subcommand pair MUST stay in argv positions 1/2 — polkit binds those.
fn build_reconfigure_args(config: &Path, opts: &ReconfigureOptions) -> Vec<String> {
    let mut args: Vec<String> = vec!["system".into(), "reconfigure".into()];
    for p in &opts.load_paths {
        args.push("-L".into());
        args.push(p.to_string_lossy().into_owned());
    }
    opts.build.append_args(&mut args);
    if opts.dry_run {
        args.push("--dry-run".into());
    }
    if opts.allow_downgrades {
        args.push("--allow-downgrades".into());
    }
    args.push(config.to_string_lossy().into_owned());
    args
}

/// `system init` takes two positionals: `<config> <target>`, in that order.
fn build_init_args(config: &Path, target: &Path, opts: &InitOptions) -> Vec<String> {
    let mut args: Vec<String> = vec!["system".into(), "init".into()];
    for p in &opts.load_paths {
        args.push("-L".into());
        args.push(p.to_string_lossy().into_owned());
    }
    opts.build.append_args(&mut args);
    args.push(config.to_string_lossy().into_owned());
    args.push(target.to_string_lossy().into_owned());
    args
}

#[derive(Debug, Clone, Default)]
pub struct ReconfigureOptions {
    pub dry_run: bool,
    pub allow_downgrades: bool,
    /// Forwarded as `-L <path>` per entry between `reconfigure` and the
    /// config — required for configs importing local modules.
    pub load_paths: Vec<PathBuf>,
    /// Substitute/scheduler flags forwarded to `guix`.
    pub build: BuildOptions,
    /// How to acquire root. Defaults to `pkexec`.
    pub privilege: Privilege,
}

#[derive(Debug, Clone, Default)]
pub struct InitOptions {
    /// Forwarded as `-L <path>` per entry — required for configs importing
    /// local modules.
    pub load_paths: Vec<PathBuf>,
    /// Substitute/scheduler flags forwarded to `guix`.
    pub build: BuildOptions,
    /// How to acquire root. The installer runs already-root.
    pub privilege: Privilege,
}

/// Best-effort check for a running polkit auth agent. Honors
/// `LIBGUIX_SKIP_AGENT_CHECK` (force true) and `LIBGUIX_FORCE_NO_AGENT`
/// (force false) for testing/overrides; otherwise scans `/proc`.
pub fn auth_agent_present() -> bool {
    if std::env::var_os("LIBGUIX_SKIP_AGENT_CHECK").is_some() {
        return true;
    }
    if std::env::var_os("LIBGUIX_FORCE_NO_AGENT").is_some() {
        return false;
    }
    auth_agent_present_scan()
}

#[cfg(test)]
fn resolve_config_path() -> PathBuf {
    std::env::var_os("LIBGUIX_TEST_CONFIG_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(CURRENT_SYSTEM_CONFIG))
}

#[cfg(not(test))]
fn resolve_config_path() -> PathBuf {
    PathBuf::from(CURRENT_SYSTEM_CONFIG)
}

fn current_configuration_path_with(p: &Path) -> Result<PathBuf, GuixError> {
    match std::fs::metadata(p) {
        Ok(_) => Ok(p.to_path_buf()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(GuixError::NotOnGuixSystem),
        Err(e) => Err(GuixError::Spawn(e)),
    }
}

/// Matched as truncated-comm equality (NOT prefix) against `/proc/<pid>/comm`.
#[cfg(target_os = "linux")]
const AGENT_NAMES: &[&str] = &[
    "lxqt-policykit-agent",
    "polkit-gnome-authentication-agent-1",
    "polkit-mate-authentication-agent-1",
    "polkit-kde-authentication-agent-1",
    "mate-polkit-bin",
    "mate-polkit",
    "hyprpolkitagent",
    "xfce-polkit",
    "polkit-efl-auth",
    "polkit-1-auth-a",
    "polkit-dumb-agent",
];

/// `TASK_COMM_LEN - 1` — kernel caps `/proc/<pid>/comm` at 15 visible chars.
#[cfg(target_os = "linux")]
const COMM_MAX: usize = 15;

#[cfg(target_os = "linux")]
const fn truncate_comm(s: &str) -> &str {
    let bytes = s.as_bytes();
    let len = if bytes.len() < COMM_MAX {
        bytes.len()
    } else {
        COMM_MAX
    };
    let (head, _) = bytes.split_at(len);
    match std::str::from_utf8(head) {
        Ok(s) => s,
        Err(_) => "",
    }
}

/// Best-effort `/proc/*/comm` scan against [`AGENT_NAMES`]. Non-Linux
/// returns `true` (skip).
#[cfg(target_os = "linux")]
fn auth_agent_present_scan() -> bool {
    auth_agent_present_in(Path::new("/proc"))
}

#[cfg(not(target_os = "linux"))]
fn auth_agent_present_scan() -> bool {
    true
}

#[cfg(target_os = "linux")]
pub(crate) fn auth_agent_present_in(proc_root: &Path) -> bool {
    let mut needles = [""; AGENT_NAMES.len()];
    let mut i = 0;
    while i < AGENT_NAMES.len() {
        needles[i] = truncate_comm(AGENT_NAMES[i]);
        i += 1;
    }

    let Ok(entries) = std::fs::read_dir(proc_root) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.bytes().all(|b| b.is_ascii_digit()))
        {
            continue;
        }
        let comm_path = path.join("comm");
        let Ok(comm) = std::fs::read_to_string(&comm_path) else {
            continue;
        };
        let comm = comm.trim();
        // Equality, NOT prefix — `xfce-polkit-*` must not match `xfce-polkit`.
        for needle in &needles {
            if comm == *needle {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_options_are_quiet() {
        let o = ReconfigureOptions::default();
        assert!(!o.dry_run);
        assert!(!o.allow_downgrades);
        assert!(o.load_paths.is_empty());
    }

    #[test]
    fn reconfigure_args_bare() {
        let cfg = PathBuf::from("/etc/config.scm");
        let args = build_reconfigure_args(&cfg, &ReconfigureOptions::default());
        assert_eq!(args, vec!["system", "reconfigure", "/etc/config.scm"]);
    }

    /// Pins polkit `argv1=system argv2=reconfigure` invariant under -L flags.
    #[test]
    fn reconfigure_args_include_load_paths() {
        let cfg = PathBuf::from("/home/me/dotfiles/system/framework.scm");
        let opts = ReconfigureOptions {
            load_paths: vec![
                PathBuf::from("/home/me/dotfiles/system"),
                PathBuf::from("/home/me/dotfiles/extra"),
            ],
            ..Default::default()
        };
        let args = build_reconfigure_args(&cfg, &opts);
        assert_eq!(
            args,
            vec![
                "system",
                "reconfigure",
                "-L",
                "/home/me/dotfiles/system",
                "-L",
                "/home/me/dotfiles/extra",
                "/home/me/dotfiles/system/framework.scm",
            ],
        );
        assert_eq!(args[0], "system");
        assert_eq!(args[1], "reconfigure");
    }

    #[test]
    fn reconfigure_args_include_build_options() {
        let cfg = PathBuf::from("/etc/config.scm");
        let opts = ReconfigureOptions {
            build: BuildOptions {
                substitute_urls: vec!["https://ci.example".into()],
                cores: Some(4),
                ..Default::default()
            },
            ..Default::default()
        };
        let args = build_reconfigure_args(&cfg, &opts);
        assert_eq!(args[0], "system");
        assert_eq!(args[1], "reconfigure");
        assert!(args.contains(&"--substitute-urls=https://ci.example".to_string()));
        assert!(args.contains(&"--cores=4".to_string()));
        assert_eq!(args.last().unwrap(), "/etc/config.scm");
    }

    #[test]
    fn init_args_bare() {
        let cfg = PathBuf::from("/mnt/etc/config.scm");
        let target = PathBuf::from("/mnt");
        let args = build_init_args(&cfg, &target, &InitOptions::default());
        assert_eq!(args, vec!["system", "init", "/mnt/etc/config.scm", "/mnt"]);
    }

    /// Config precedes target; build options sit between the subcommand and
    /// the positionals.
    #[test]
    fn init_args_with_build_options_and_load_paths() {
        let cfg = PathBuf::from("/mnt/etc/config.scm");
        let target = PathBuf::from("/mnt");
        let opts = InitOptions {
            load_paths: vec![PathBuf::from("/mnt/modules")],
            build: BuildOptions {
                substitute_urls: vec!["https://ci.example".into()],
                no_substitutes: false,
                max_jobs: Some(2),
                ..Default::default()
            },
            ..Default::default()
        };
        let args = build_init_args(&cfg, &target, &opts);
        assert_eq!(
            args,
            vec![
                "system",
                "init",
                "-L",
                "/mnt/modules",
                "--substitute-urls=https://ci.example",
                "--max-jobs=2",
                "/mnt/etc/config.scm",
                "/mnt",
            ]
        );
    }

    #[test]
    fn reconfigure_args_load_paths_with_flags() {
        let cfg = PathBuf::from("/etc/config.scm");
        let opts = ReconfigureOptions {
            dry_run: true,
            allow_downgrades: true,
            load_paths: vec![PathBuf::from("/srv/cfg")],
            ..Default::default()
        };
        let args = build_reconfigure_args(&cfg, &opts);
        assert_eq!(
            args,
            vec![
                "system",
                "reconfigure",
                "-L",
                "/srv/cfg",
                "--dry-run",
                "--allow-downgrades",
                "/etc/config.scm",
            ],
        );
    }

    #[test]
    fn agent_check_does_not_panic() {
        let _ = auth_agent_present();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn truncate_comm_matches_kernel_limit() {
        assert_eq!(
            truncate_comm("polkit-gnome-authentication-agent-1"),
            "polkit-gnome-au"
        );
        assert_eq!(truncate_comm("lxqt-policykit-agent"), "lxqt-policykit-");
        assert_eq!(truncate_comm("mate-polkit"), "mate-polkit");
        assert_eq!(truncate_comm("hyprpolkitagent"), "hyprpolkitagent");
    }

    #[test]
    fn current_config_path_missing_returns_not_on_guix() {
        let p = PathBuf::from("/tmp/libguix-definitely-does-not-exist-xyz/config.scm");
        let err = current_configuration_path_with(&p).expect_err("expected error");
        assert!(matches!(err, GuixError::NotOnGuixSystem), "got {err:?}");
    }

    #[test]
    fn current_config_path_existing_returns_ok() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let p = tmp.path().join("configuration.scm");
        std::fs::write(&p, "(operating-system ...)").expect("write");
        let got = current_configuration_path_with(&p).expect("ok");
        assert_eq!(got, p);
    }

    /// EACCES must NOT collapse to `NotOnGuixSystem` — lying about Guix
    /// System presence on a permission error masks real bugs.
    #[cfg(unix)]
    #[test]
    fn current_config_path_permission_denied_is_spawn() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().expect("tempdir");
        let unreadable_dir = tmp.path().join("locked");
        std::fs::create_dir(&unreadable_dir).expect("mkdir");
        let inside = unreadable_dir.join("configuration.scm");
        std::fs::write(&inside, "x").expect("write");
        // Root bypasses dir-mode checks — `Ok` is tolerated in that case.
        std::fs::set_permissions(&unreadable_dir, std::fs::Permissions::from_mode(0o000))
            .expect("chmod");
        let result = current_configuration_path_with(&inside);
        let _ = std::fs::set_permissions(&unreadable_dir, std::fs::Permissions::from_mode(0o755));
        match result {
            Ok(_) | Err(GuixError::Spawn(_)) => {}
            Err(GuixError::NotOnGuixSystem) => {
                panic!("permission-denied collapsed to NotOnGuixSystem");
            }
            Err(other) => panic!("unexpected error: {other:?}"),
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn auth_agent_present_in_fake_proc_detects_lxqt() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let pid_dir = tmp.path().join("123");
        std::fs::create_dir(&pid_dir).expect("mkdir");
        std::fs::write(pid_dir.join("comm"), "lxqt-policykit-\n").expect("write comm");
        assert!(auth_agent_present_in(tmp.path()));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn auth_agent_present_in_fake_proc_returns_false_when_absent() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let pid_dir = tmp.path().join("456");
        std::fs::create_dir(&pid_dir).expect("mkdir");
        std::fs::write(pid_dir.join("comm"), "bash\n").expect("write comm");
        assert!(!auth_agent_present_in(tmp.path()));

        let pid_dir2 = tmp.path().join("789");
        std::fs::create_dir(&pid_dir2).expect("mkdir");
        assert!(!auth_agent_present_in(tmp.path()));
    }

    /// Regression for equality-vs-`starts_with` agent matching.
    #[cfg(target_os = "linux")]
    #[test]
    fn auth_agent_present_in_rejects_prefix_only_match() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let pid_dir = tmp.path().join("321");
        std::fs::create_dir(&pid_dir).expect("mkdir");
        std::fs::write(pid_dir.join("comm"), "xfce-polkit-imp\n").expect("write");
        assert!(
            !auth_agent_present_in(tmp.path()),
            "prefix-only match must not trigger"
        );
    }
}
