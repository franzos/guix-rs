//! Desktop-environment menu refresh. Guix repoints `~/.guix-profile` to a
//! new store path on every install, orphaning the inode that menu caches
//! watch; the watcher never fires, so the menu stays stale until relogin.
//! This module rebuilds the relevant cache / reloads the panel explicitly.

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopEnv {
    KdePlasma,
    Xfce,
    Mate,
    Lxqt,
    Gnome,
    Cinnamon,
    Unknown,
}

struct Cmd {
    prog: &'static str,
    args: &'static [&'static str],
}

enum Mode {
    /// Run to completion, await exit with a timeout guard (KDE, XFCE).
    Complete,
    /// Spawn detached and drop the Child; the binary daemonizes (MATE).
    Daemon,
    /// Kill the existing panel and relaunch it (LXQt).
    Restart,
}

struct Refresh {
    candidates: &'static [Cmd],
    mode: Mode,
}

/// Outcome of a refresh attempt. Best-effort: every variant is logged, never
/// surfaced to the user.
#[derive(Debug, Clone)]
pub enum MenuRefresh {
    Refreshed(DesktopEnv),
    NotNeeded(DesktopEnv),
    CommandMissing(DesktopEnv),
    Failed(DesktopEnv, String),
    Disabled,
}

/// Colon-split, case-insensitive token match. Tries `XDG_CURRENT_DESKTOP`
/// first; if that yields `Unknown`/empty, falls back to `DESKTOP_SESSION`.
pub fn detect_from(xdg_current: &str, desktop_session: &str) -> DesktopEnv {
    let primary = classify(xdg_current);
    if primary != DesktopEnv::Unknown {
        return primary;
    }
    classify(desktop_session)
}

fn classify(value: &str) -> DesktopEnv {
    for token in value.split(':') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        let de = match token.to_ascii_uppercase().as_str() {
            "KDE" => DesktopEnv::KdePlasma,
            "XFCE" => DesktopEnv::Xfce,
            "MATE" => DesktopEnv::Mate,
            "LXQT" => DesktopEnv::Lxqt,
            "GNOME" => DesktopEnv::Gnome,
            "X-CINNAMON" | "CINNAMON" => DesktopEnv::Cinnamon,
            _ => DesktopEnv::Unknown,
        };
        if de != DesktopEnv::Unknown {
            return de;
        }
    }
    DesktopEnv::Unknown
}

pub fn detect() -> DesktopEnv {
    let xdg = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let session = std::env::var("DESKTOP_SESSION").unwrap_or_default();
    detect_from(&xdg, &session)
}

fn plan_for(de: DesktopEnv) -> Option<Refresh> {
    match de {
        DesktopEnv::KdePlasma => Some(Refresh {
            candidates: &[
                Cmd {
                    prog: "kbuildsycoca6",
                    args: &["--noincremental"],
                },
                Cmd {
                    prog: "kbuildsycoca5",
                    args: &["--noincremental"],
                },
            ],
            mode: Mode::Complete,
        }),
        DesktopEnv::Xfce => Some(Refresh {
            candidates: &[Cmd {
                prog: "xfce4-panel",
                args: &["-r"],
            }],
            mode: Mode::Complete,
        }),
        DesktopEnv::Mate => Some(Refresh {
            candidates: &[Cmd {
                prog: "mate-panel",
                args: &["--replace"],
            }],
            mode: Mode::Daemon,
        }),
        DesktopEnv::Lxqt => Some(Refresh {
            candidates: &[Cmd {
                prog: "lxqt-panel",
                args: &[],
            }],
            mode: Mode::Restart,
        }),
        DesktopEnv::Gnome | DesktopEnv::Cinnamon | DesktopEnv::Unknown => None,
    }
}

/// Resolve `bin` against `$PATH`. Never hardcode `/gnu/store` paths: they
/// rotate on every DE upgrade.
fn on_path(bin: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(bin);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

// Rejected: `xdg-desktop-menu forceupdate` only runs update-desktop-database
// on ~/.local/share/applications; it never calls kbuildsycoca, never touches
// the Guix profile, and never signals a running panel. Useless here.
pub async fn refresh_application_menu(enabled: bool) -> MenuRefresh {
    if !enabled {
        return MenuRefresh::Disabled;
    }
    let de = detect();
    let Some(refresh) = plan_for(de) else {
        return MenuRefresh::NotNeeded(de);
    };

    let Some(chosen) = refresh
        .candidates
        .iter()
        .find(|c| on_path(c.prog).is_some())
    else {
        return MenuRefresh::CommandMissing(de);
    };

    match refresh.mode {
        Mode::Complete => run_complete(de, chosen).await,
        Mode::Daemon => spawn_daemon(de, chosen),
        Mode::Restart => restart_panel(de, chosen.prog),
    }
}

async fn run_complete(de: DesktopEnv, cmd: &Cmd) -> MenuRefresh {
    let mut c = Command::new(cmd.prog);
    c.args(cmd.args)
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    match tokio::time::timeout(Duration::from_secs(10), c.output()).await {
        Ok(Ok(out)) if out.status.success() => MenuRefresh::Refreshed(de),
        Ok(Ok(out)) => {
            let code = out.status.code().unwrap_or(-1);
            let stderr = String::from_utf8_lossy(&out.stderr);
            MenuRefresh::Failed(de, format!("{} exited {code}: {}", cmd.prog, stderr.trim()))
        }
        Ok(Err(e)) => MenuRefresh::Failed(de, format!("spawn {} failed: {e}", cmd.prog)),
        Err(_) => MenuRefresh::Failed(de, format!("{} timed out", cmd.prog)),
    }
}

fn spawn_daemon(de: DesktopEnv, cmd: &Cmd) -> MenuRefresh {
    let mut c = Command::new(cmd.prog);
    c.args(cmd.args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    match c.spawn() {
        Ok(_child) => MenuRefresh::Refreshed(de),
        Err(e) => MenuRefresh::Failed(de, format!("spawn {} failed: {e}", cmd.prog)),
    }
}

fn restart_panel(de: DesktopEnv, bin: &str) -> MenuRefresh {
    let mut c = Command::new("sh");
    c.arg("-c")
        .arg(format!("killall -q {bin} 2>/dev/null; exec {bin}"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    match c.spawn() {
        Ok(_child) => MenuRefresh::Refreshed(de),
        Err(e) => MenuRefresh::Failed(de, format!("restart {bin} failed: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_from_known_tokens() {
        assert_eq!(detect_from("XFCE", ""), DesktopEnv::Xfce);
        assert_eq!(detect_from("KDE", ""), DesktopEnv::KdePlasma);
        assert_eq!(detect_from("MATE", ""), DesktopEnv::Mate);
        assert_eq!(detect_from("LXQt", ""), DesktopEnv::Lxqt);
        assert_eq!(detect_from("ubuntu:GNOME", ""), DesktopEnv::Gnome);
        assert_eq!(detect_from("X-Cinnamon", ""), DesktopEnv::Cinnamon);
    }

    #[test]
    fn detect_from_unknown_and_empty() {
        assert_eq!(detect_from("", ""), DesktopEnv::Unknown);
        assert_eq!(detect_from("Frobnicator", ""), DesktopEnv::Unknown);
    }

    #[test]
    fn detect_from_falls_back_to_session() {
        assert_eq!(detect_from("", "xfce"), DesktopEnv::Xfce);
        assert_eq!(
            detect_from("Unknown", "kde-plasma:KDE"),
            DesktopEnv::KdePlasma
        );
    }

    #[test]
    fn plan_for_known_first_candidate() {
        assert_eq!(
            plan_for(DesktopEnv::KdePlasma).unwrap().candidates[0].prog,
            "kbuildsycoca6"
        );
        assert_eq!(
            plan_for(DesktopEnv::Xfce).unwrap().candidates[0].prog,
            "xfce4-panel"
        );
        assert_eq!(
            plan_for(DesktopEnv::Mate).unwrap().candidates[0].prog,
            "mate-panel"
        );
        assert_eq!(
            plan_for(DesktopEnv::Lxqt).unwrap().candidates[0].prog,
            "lxqt-panel"
        );
    }

    #[test]
    fn plan_for_noop_desktops() {
        assert!(plan_for(DesktopEnv::Gnome).is_none());
        assert!(plan_for(DesktopEnv::Cinnamon).is_none());
        assert!(plan_for(DesktopEnv::Unknown).is_none());
    }
}
