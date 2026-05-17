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
    Search,
    Installed,
    Updates,
    System,
}

impl Tab {
    pub fn label(self) -> &'static str {
        match self {
            Tab::Search => "Search",
            Tab::Installed => "Installed",
            Tab::Updates => "Updates",
            Tab::System => "Settings",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub active_tab: Tab,
    #[serde(default)]
    pub source_config_path: Option<PathBuf>,
    #[serde(default)]
    pub custom_load_paths: Vec<PathBuf>,
    #[serde(default)]
    pub show_log_by_default: bool,
}

impl Settings {
    pub fn default_path() -> Option<PathBuf> {
        ProjectDirs::from("", "", "guix-gui").map(|d| d.config_dir().join("config.json"))
    }

    /// Corrupt JSON degrades silently to defaults so a bad config can't wedge the GUI.
    pub fn load_from(path: &Path) -> Self {
        match fs::read_to_string(path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Self::default(),
            Err(_) => Self::default(),
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
    fn defaults_are_search_and_none() {
        let s = Settings::default();
        assert_eq!(s.active_tab, Tab::Search);
        assert!(s.source_config_path.is_none());
        assert!(s.custom_load_paths.is_empty());
        assert!(!s.show_log_by_default);
    }

    #[test]
    fn roundtrip_via_file() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("config.json");
        let original = Settings {
            active_tab: Tab::System,
            source_config_path: Some(PathBuf::from("/home/me/dotfiles/config.scm")),
            custom_load_paths: vec![PathBuf::from("/home/me/extra-modules")],
            show_log_by_default: true,
        };
        original.save_to(&path).expect("save");
        let loaded = Settings::load_from(&path);
        assert_eq!(loaded.active_tab, Tab::System);
        assert_eq!(
            loaded.source_config_path.as_deref(),
            Some(Path::new("/home/me/dotfiles/config.scm"))
        );
        assert_eq!(
            loaded.custom_load_paths,
            vec![PathBuf::from("/home/me/extra-modules")]
        );
        assert!(loaded.show_log_by_default);
    }

    #[test]
    fn missing_file_yields_defaults() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("absent.json");
        let s = Settings::load_from(&path);
        assert_eq!(s.active_tab, Tab::default());
    }

    #[test]
    fn corrupt_file_degrades_to_defaults() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("config.json");
        fs::write(&path, "{ this is not valid json").unwrap();
        let s = Settings::load_from(&path);
        assert_eq!(s.active_tab, Tab::default());
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
