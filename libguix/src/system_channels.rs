//! Reads the system channel baseline that `guix pull` falls back to when
//! no user `channels.scm` exists. Priority: `/etc/guix/channels.scm`
//! (branch-only, what pull actually uses) -> `/run/current-system/channels.scm`
//! (system provenance, commit-pinned -> commits stripped) -> `%default-channels`
//! (foreign distro / upstream Guix). See brain plan
//! 2026-07-03-channels-system-baseline.

use std::path::Path;

use crate::parsers::sexp::parse_channels_list;
use crate::types::Channel;

pub const DEFAULT_ETC_CHANNELS: &str = "/etc/guix/channels.scm";
pub const DEFAULT_PROVENANCE_CHANNELS: &str = "/run/current-system/channels.scm";

/// Which source supplied the baseline. `Etc` means deleting the user file
/// self-heals to `/etc`; `Provenance`/`Defaults` mean it does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaselineSource {
    Etc,
    Provenance,
    Defaults,
}

/// The system baseline channel set. `channels` is empty for `Defaults`.
/// Provenance channels have their commit pins stripped so they track
/// their branch once materialized into a user file.
#[derive(Debug, Clone)]
pub struct SystemBaseline {
    pub channels: Vec<Channel>,
    pub source: BaselineSource,
}

impl SystemBaseline {
    pub async fn read(etc_override: Option<&Path>, provenance_override: Option<&Path>) -> Self {
        let etc = etc_override
            .map(Path::to_path_buf)
            .unwrap_or_else(|| DEFAULT_ETC_CHANNELS.into());
        let prov = provenance_override
            .map(Path::to_path_buf)
            .unwrap_or_else(|| DEFAULT_PROVENANCE_CHANNELS.into());

        // Small files; a blocking read on the runtime's blocking pool
        // mirrors ChannelsFile::read and avoids tokio's fs feature.
        tokio::task::spawn_blocking(move || {
            // 1. /etc: already branch-only, introductions intact. Verbatim.
            if let Some(chans) = read_channels_file(&etc) {
                return SystemBaseline {
                    channels: chans,
                    source: BaselineSource::Etc,
                };
            }
            // 2. Provenance: commit-pinned, strip commits to track branches.
            if let Some(chans) = read_channels_file(&prov) {
                return SystemBaseline {
                    channels: chans.into_iter().map(Channel::without_commit).collect(),
                    source: BaselineSource::Provenance,
                };
            }
            // 3. Neither present: foreign distro / upstream Guix.
            SystemBaseline {
                channels: Vec::new(),
                source: BaselineSource::Defaults,
            }
        })
        .await
        .unwrap_or(SystemBaseline {
            channels: Vec::new(),
            source: BaselineSource::Defaults,
        })
    }

    /// True when `/etc` supplies the baseline, so deleting the user file
    /// lets `guix pull` self-heal to it.
    pub fn etc_present(&self) -> bool {
        self.source == BaselineSource::Etc
    }

    pub fn contains(&self, name: &str) -> bool {
        self.channels.iter().any(|c| c.name == name)
    }
}

/// Reads and parses a channels file, returning its channels, or `None`
/// when the file is missing or unparseable (caller falls through).
fn read_channels_file(path: &Path) -> Option<Vec<Channel>> {
    let raw = std::fs::read_to_string(path).ok()?;
    parse_channels_list(&raw)
        .ok()
        .map(crate::parsers::sexp::ChannelsList::into_channels)
}
