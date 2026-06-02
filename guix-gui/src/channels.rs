//! Channels tab state and helpers.

use std::collections::HashMap;
use std::path::PathBuf;

use libguix::{Channel, ChannelsError, ChannelsFile, InstalledPackage, KnownBug};

use guix_gui::discovery::{DiscoveredChannel, DiscoveredPackage, Discovery};

/// Bundles the parsed `ChannelsFile` with the `.bak` probe so the view
/// never has to `stat(2)` from inside `view()`.
#[derive(Debug, Clone)]
pub struct ChannelsFileLoad {
    pub file: ChannelsFile,
    pub backup_path: Option<PathBuf>,
}

/// Carrying `Missing` separately from `Failed` lets the view render
/// the empty-state without substring-matching the error text.
#[derive(Debug, Clone)]
pub enum ChannelsFileLoadOutcome {
    Loaded(ChannelsFileLoad),
    Missing,
    Failed(String),
}

#[derive(Default)]
pub struct ChannelsState {
    pub file: Option<ChannelsFile>,
    pub loading: bool,
    pub saving: bool,
    pub error: Option<String>,
    pub add_form: AddChannelForm,
    pub last_message: Option<String>,
    pub validation_message: Option<String>,
    pub pending_remove: Option<String>,
    /// Probed during the same async load that produces `file`, so the
    /// view never has to `stat(2)`.
    pub backup_path: Option<PathBuf>,
    pub pending_restore: bool,
    /// Only rendered when `Settings::discovery_enabled` — strictly
    /// opt-in. Not persisted across restarts.
    pub sub_mode: ChannelsSubMode,
    /// `None` while the discovery toggle is off (no HTTP client, no
    /// allocations). Built on first transition into Discover.
    pub discovery: Option<Discovery>,
    pub discover_channels: Vec<DiscoveredChannel>,
    pub discover_channels_loading: bool,
    pub discover_query: String,
    pub discover_query_seq: u64,
    pub discover_packages: Vec<DiscoveredPackage>,
    pub discover_packages_loading: bool,
    pub discover_error: Option<String>,
    pub discover_pending_add: Option<Channel>,
    /// Paired with `discover_pending_add` — when set, the post-apply
    /// toast offers the combined "Pull, then install <pkg>" CTA.
    pub discover_pending_install: Option<String>,
    pub post_apply_install_prompt: Option<String>,
    /// Package to install after a channels-tab user pull succeeds.
    /// Cleared on pull completion regardless of outcome — failure
    /// surfaces the rollback offer and never auto-installs.
    pub pending_install: Option<String>,
    /// Marks the next `OpKind::Pull` completion as belonging to a
    /// channels-tab edit so a non-zero exit can surface the rollback.
    pub pending_pull_after_write: bool,
    pub rollback_offer: Option<RollbackOffer>,
    /// `Some(empty)` means the lookup failed — we keep the empty map so
    /// repaints don't re-fetch. Invalidated on Install/Remove/Upgrade.
    pub installed_by_channel: Option<HashMap<String, Vec<InstalledPackage>>>,
    /// Race guard — true between dispatch and reply so a second
    /// concurrent trigger doesn't queue.
    pub installed_by_channel_loading: bool,
}

/// The `.bak` snapshot is captured at offer-creation time so a
/// concurrent edit can't shift it under the user.
#[derive(Debug, Clone)]
pub struct RollbackOffer {
    pub backup_path: Option<PathBuf>,
    pub bug: Option<KnownBug>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChannelsSubMode {
    #[default]
    Installed,
    Discover,
}

#[derive(Default)]
pub struct AddChannelForm {
    pub name: String,
    pub url: String,
    pub branch: String,
    pub commit: String,
    pub intro_commit: String,
    pub intro_fpr: String,
}

impl AddChannelForm {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn to_channel(&self) -> Result<Channel, String> {
        let name = self.name.trim();
        let url = self.url.trim();
        let intro_commit = self.intro_commit.trim();
        let intro_fpr = self.intro_fpr.trim();
        if name.is_empty() {
            return Err(crate::t!("channels-form-name-required"));
        }
        if url.is_empty() {
            return Err(crate::t!("channels-form-url-required"));
        }
        if intro_commit.is_empty() || intro_fpr.is_empty() {
            return Err(crate::t!("channels-form-intro-required"));
        }
        let opt = |s: &String| {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_owned())
            }
        };
        Ok(Channel {
            name: name.to_owned(),
            url: url.to_owned(),
            branch: opt(&self.branch),
            commit: opt(&self.commit),
            introduction_commit: Some(intro_commit.to_owned()),
            introduction_fingerprint: Some(intro_fpr.to_owned()),
        })
    }
}

pub fn describe_channels_error(e: &ChannelsError) -> String {
    e.to_string()
}

/// `Missing` after a write is itself an error — the file we just wrote
/// shouldn't be absent.
pub fn outcome_to_result(o: ChannelsFileLoadOutcome) -> Result<ChannelsFileLoad, String> {
    match o {
        ChannelsFileLoadOutcome::Loaded(load) => Ok(load),
        ChannelsFileLoadOutcome::Missing => Err(crate::t!("channels-vanished-after-write")),
        ChannelsFileLoadOutcome::Failed(e) => Err(e),
    }
}

/// Reads the file and probes the `.bak` sibling in the same async task
/// so the view never `stat`s from `view()`.
pub async fn load_channels_file(path_override: Option<PathBuf>) -> ChannelsFileLoadOutcome {
    let file = match ChannelsFile::read(path_override.as_deref()).await {
        Ok(f) => f,
        Err(ChannelsError::FileNotFound { .. }) => return ChannelsFileLoadOutcome::Missing,
        Err(e) => return ChannelsFileLoadOutcome::Failed(describe_channels_error(&e)),
    };
    let bak = file.backup_path();
    let backup_path = match tokio::fs::metadata(&bak).await {
        Ok(_) => Some(bak),
        Err(_) => None,
    };
    ChannelsFileLoadOutcome::Loaded(ChannelsFileLoad { file, backup_path })
}
