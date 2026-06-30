//! Persisted GUI settings at `$XDG_CONFIG_HOME/guix-gui/config.json`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tab {
    #[default]
    Home,
    Search,
    Installed,
    Updates,
    Channels,
    System,
    About,
}

impl Tab {
    pub fn label(self) -> String {
        match self {
            Tab::Home => crate::t!("tab-home"),
            Tab::Search => crate::t!("tab-search"),
            Tab::Installed => crate::t!("tab-installed"),
            Tab::Updates => crate::t!("tab-updates"),
            Tab::Channels => crate::t!("tab-channels"),
            Tab::System => crate::t!("tab-system"),
            Tab::About => crate::t!("tab-about"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub source_config_path: Option<PathBuf>,
    /// Override for `~/.config/guix/channels.scm`. Used when the default
    /// path resolves into `/gnu/store/...` (e.g. `guix home`-managed
    /// setups) so the user can keep their channels in a writable source.
    #[serde(default)]
    pub channels_source_path: Option<PathBuf>,
    #[serde(default)]
    pub custom_load_paths: Vec<PathBuf>,
    #[serde(default)]
    pub show_log_by_default: bool,
    #[serde(default)]
    pub app_metadata: AppMetadataSettings,
    /// Opt-in for the channel/package discovery surface backed by
    /// `toys.whereis.social`. When `false`, nothing about discovery renders
    /// anywhere in the app — no sub-mode toggle, no buttons, no network.
    #[serde(default)]
    pub discovery_enabled: bool,
    /// BCP-47 language tag overriding the system locale. `None` follows
    /// the system default.
    #[serde(default)]
    pub language: Option<String>,
    /// Refresh the running desktop's application menu after a profile
    /// change so newly installed apps appear without a relogin. The
    /// `serde(default)` only fires on deserialization; `impl Default`
    /// below covers the first-run path.
    #[serde(default = "default_true")]
    pub desktop_menu_refresh: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            source_config_path: None,
            channels_source_path: None,
            custom_load_paths: Vec::new(),
            show_log_by_default: false,
            app_metadata: AppMetadataSettings::default(),
            discovery_enabled: false,
            language: None,
            desktop_menu_refresh: true,
        }
    }
}

/// Opt-in fetch of icons + screenshots from third-party catalogs. Off by
/// default — enabling it makes network requests to flathub.org /
/// screenshots.debian.net when the user selects a search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppMetadataSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub use_flathub: bool,
    #[serde(default = "default_true")]
    pub use_debian_screenshots: bool,
}

fn default_true() -> bool {
    true
}

impl Default for AppMetadataSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            use_flathub: true,
            use_debian_screenshots: true,
        }
    }
}

impl Settings {
    pub fn default_path() -> Option<PathBuf> {
        ProjectDirs::from("", "", "guix-gui").map(|d| d.config_dir().join("config.json"))
    }

    /// Corrupt JSON degrades to defaults so a bad config can't wedge
    /// the GUI. The bad file is copied to a `.bak` sibling for
    /// inspection; an existing `.bak` is never overwritten.
    pub fn load_from(path: &Path) -> Self {
        match fs::read_to_string(path) {
            Ok(s) if s.trim().is_empty() => Self::default(),
            Ok(s) => match serde_json::from_str(&s) {
                Ok(parsed) => parsed,
                Err(_) => {
                    tracing::warn!(
                        target: "guix_gui",
                        "settings parse failed for {}; using defaults",
                        path.display()
                    );
                    save_corrupt_backup(path);
                    Self::default()
                }
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => Self::default(),
            Err(_) => {
                tracing::warn!(
                    target: "guix_gui",
                    "settings parse failed for {}; using defaults",
                    path.display()
                );
                save_corrupt_backup(path);
                Self::default()
            }
        }
    }

    pub fn load() -> Self {
        Self::default_path()
            .map(|p| Self::load_from(&p))
            .unwrap_or_default()
    }

    pub fn save_to(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let s = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        fs::write(path, s)
    }

    pub fn save(&self) -> io::Result<()> {
        if let Some(p) = Self::default_path() {
            self.save_to(&p)?;
        }
        Ok(())
    }

    /// Always prepends `parent(source_config_path)` so sibling-module
    /// imports work without manual configuration. Dedup'd.
    #[must_use]
    pub fn effective_load_paths(&self) -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = Vec::new();
        if let Some(cfg) = &self.source_config_path {
            if let Some(dir) = cfg.parent() {
                if !dir.as_os_str().is_empty() {
                    out.push(dir.to_path_buf());
                }
            }
        }
        for p in &self.custom_load_paths {
            if !out.contains(p) {
                out.push(p.clone());
            }
        }
        out
    }
}

/// Copy `path` to its `.bak` sibling, but never clobber an existing
/// `.bak` — the first preserved corruption wins.
fn save_corrupt_backup(path: &Path) {
    let bak = path.with_extension("bak");
    if bak.exists() {
        tracing::warn!(
            target: "guix_gui",
            "settings backup already exists at {}; not overwriting",
            bak.display()
        );
        return;
    }
    let _ = fs::copy(path, &bak);
}

pub const FIRST_RUN_CONFIG_CANDIDATES: &[&str] = &["/etc/config.scm", "/etc/system.scm"];

#[must_use]
pub fn probe_first_run_config(root: &Path) -> Option<PathBuf> {
    for rel in FIRST_RUN_CONFIG_CANDIDATES {
        // `Path::join` with an absolute rhs discards lhs — must strip leading `/`.
        let stripped = rel.trim_start_matches('/');
        let candidate = root.join(stripped);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_none() {
        let s = Settings::default();
        assert!(s.source_config_path.is_none());
        assert!(s.custom_load_paths.is_empty());
        assert!(!s.show_log_by_default);
    }

    #[test]
    fn roundtrip_via_file() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("config.json");
        let original = Settings {
            source_config_path: Some(PathBuf::from("/home/me/dotfiles/config.scm")),
            channels_source_path: Some(PathBuf::from("/home/me/dotfiles/channels.scm")),
            custom_load_paths: vec![PathBuf::from("/home/me/extra-modules")],
            show_log_by_default: true,
            app_metadata: AppMetadataSettings::default(),
            discovery_enabled: true,
            language: Some("de-DE".to_string()),
            desktop_menu_refresh: true,
        };
        original.save_to(&path).expect("save");
        let loaded = Settings::load_from(&path);
        assert_eq!(
            loaded.source_config_path.as_deref(),
            Some(Path::new("/home/me/dotfiles/config.scm"))
        );
        assert_eq!(
            loaded.channels_source_path.as_deref(),
            Some(Path::new("/home/me/dotfiles/channels.scm"))
        );
        assert_eq!(
            loaded.custom_load_paths,
            vec![PathBuf::from("/home/me/extra-modules")]
        );
        assert!(loaded.show_log_by_default);
        assert!(loaded.discovery_enabled);
        assert_eq!(loaded.language.as_deref(), Some("de-DE"));
    }

    #[test]
    fn discovery_defaults_to_off() {
        let s = Settings::default();
        assert!(!s.discovery_enabled);
    }

    #[test]
    fn desktop_menu_refresh_defaults_to_on() {
        assert!(Settings::default().desktop_menu_refresh);
    }

    #[test]
    fn missing_file_yields_defaults() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("absent.json");
        let _s = Settings::load_from(&path);
    }

    #[test]
    fn corrupt_file_degrades_to_defaults() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("config.json");
        fs::write(&path, "{ this is not valid json").unwrap();
        let s = Settings::load_from(&path);
        assert!(s.source_config_path.is_none());
    }

    #[test]
    fn corrupt_file_creates_backup() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("config.json");
        fs::write(&path, "{ broken").unwrap();
        let _ = Settings::load_from(&path);
        let bak = path.with_extension("bak");
        assert!(bak.exists(), ".bak should exist after corrupt-JSON load");
        let bak_contents = fs::read_to_string(&bak).unwrap();
        assert_eq!(bak_contents, "{ broken");
    }

    #[test]
    fn corrupt_file_preserves_existing_backup() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("config.json");
        let bak = path.with_extension("bak");
        fs::write(&bak, "good prior backup").unwrap();
        fs::write(&path, "{ freshly broken").unwrap();
        let _ = Settings::load_from(&path);
        let bak_contents = fs::read_to_string(&bak).unwrap();
        assert_eq!(
            bak_contents, "good prior backup",
            "existing .bak must not be overwritten by a fresh corruption"
        );
    }

    #[test]
    fn save_creates_parent_dirs() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let nested = tmp.path().join("a/b/c/config.json");
        Settings::default().save_to(&nested).expect("save");
        assert!(nested.exists());
    }

    #[test]
    fn first_run_probes_etc_config() {
        let tmp = tempfile::tempdir().expect("tempdir");
        fs::create_dir_all(tmp.path().join("etc")).unwrap();
        fs::write(tmp.path().join("etc/config.scm"), "(operating-system)").unwrap();
        let got = probe_first_run_config(tmp.path()).expect("found");
        assert_eq!(got, tmp.path().join("etc/config.scm"));
    }

    #[test]
    fn first_run_falls_through_to_system_scm() {
        let tmp = tempfile::tempdir().expect("tempdir");
        fs::create_dir_all(tmp.path().join("etc")).unwrap();
        fs::write(tmp.path().join("etc/system.scm"), "(operating-system)").unwrap();
        let got = probe_first_run_config(tmp.path()).expect("found");
        assert_eq!(got, tmp.path().join("etc/system.scm"));
    }

    #[test]
    fn first_run_returns_none_when_neither_exists() {
        let tmp = tempfile::tempdir().expect("tempdir");
        assert!(probe_first_run_config(tmp.path()).is_none());
    }

    #[test]
    fn effective_load_paths_auto_derives_parent() {
        let s = Settings {
            source_config_path: Some(PathBuf::from("/home/me/dotfiles/system/config.scm")),
            ..Default::default()
        };
        assert_eq!(
            s.effective_load_paths(),
            vec![PathBuf::from("/home/me/dotfiles/system")]
        );
    }

    #[test]
    fn load_paths_dedup() {
        let s = Settings {
            source_config_path: Some(PathBuf::from("/home/me/dotfiles/system/config.scm")),
            custom_load_paths: vec![
                PathBuf::from("/home/me/dotfiles/system"),
                PathBuf::from("/home/me/dotfiles/extra"),
            ],
            ..Default::default()
        };
        assert_eq!(
            s.effective_load_paths(),
            vec![
                PathBuf::from("/home/me/dotfiles/system"),
                PathBuf::from("/home/me/dotfiles/extra"),
            ]
        );
    }

    #[test]
    fn effective_load_paths_without_config() {
        let s = Settings {
            custom_load_paths: vec![PathBuf::from("/srv/cfg")],
            ..Default::default()
        };
        assert_eq!(s.effective_load_paths(), vec![PathBuf::from("/srv/cfg")]);
    }
}
