//! Public data types.

use std::path::PathBuf;

/// Empty strings rather than `Option` — these are display fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageSummary {
    pub name: String,
    pub version: String,
    pub synopsis: String,
    pub description: String,
    pub homepage: String,
    pub license: String,
    pub outputs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageDetail {
    pub name: String,
    pub version: String,
    pub synopsis: String,
    pub description: String,
    pub homepage: Option<String>,
    pub license: Option<String>,
    pub location: Option<String>,
    pub outputs: Vec<String>,
    pub systems: Vec<String>,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledPackage {
    pub name: String,
    pub version: String,
    pub output: String,
    pub store_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Generation {
    pub number: u32,
    pub date: String,
    pub current: bool,
    pub packages: Vec<InstalledPackage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Channel {
    pub name: String,
    pub url: String,
    pub branch: Option<String>,
    pub commit: Option<String>,
    pub introduction_commit: Option<String>,
    pub introduction_fingerprint: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KnownBug {
    /// <https://issues.guix.gnu.org/74396>.
    ChannelShadow74396,
}

impl KnownBug {
    pub fn url(self) -> &'static str {
        match self {
            KnownBug::ChannelShadow74396 => "https://issues.guix.gnu.org/74396",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProgressEvent {
    /// `redraw: true` ↔ `\r`-terminated upstream — replace, don't append.
    Line {
        stream: ProgressStream,
        text: String,
        redraw: bool,
    },

    SubstituteLookup {
        url: String,
        percent: f32,
    },

    SubstituteDownload {
        item: String,
        bytes_done: u64,
        bytes_total: Option<u64>,
    },

    /// Only emitted from the REPL fd-3 stream — the stderr path has no equivalent.
    SubstituteDownloadDone {
        item: String,
        bytes_total: Option<u64>,
    },

    BuildStart {
        drv: String,
    },

    BuildPhase {
        drv: Option<String>,
        phase: String,
    },

    BuildDone {
        drv: String,
    },

    BuildFailed {
        drv: String,
        log_path: Option<String>,
    },

    WouldDownload {
        bytes: u64,
        items: Vec<String>,
    },

    WouldBuild {
        bytes: u64,
        items: Vec<String>,
    },

    StorePathListed {
        path: String,
    },

    PullComputingDerivation {
        system: String,
    },

    DryRunHeader {
        text: String,
    },

    KnownBug(KnownBug),

    /// Synthesised at end-of-stream. Always the final event.
    ExitSummary {
        code: i32,
        duration_secs: f64,
    },
}
