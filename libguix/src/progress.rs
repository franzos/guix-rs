//! Monotonic stage state machine over [`ProgressEvent`]s — stages only
//! advance unless a failure flips us to `Stage::Failed`. No i18n here;
//! consumers map [`Stage`] / [`Failure`] to display strings themselves.

use std::time::{Duration, Instant};

use indexmap::IndexMap;

use crate::types::ProgressEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Starting,
    ChannelUpdate,
    ComputingDeriv,
    Downloading,
    Building,
    Profile,
    Done,
    Failed,
}

impl Stage {
    fn order(self) -> u8 {
        match self {
            Stage::Starting => 0,
            Stage::ChannelUpdate => 1,
            Stage::ComputingDeriv => 2,
            Stage::Downloading => 3,
            Stage::Building => 4,
            Stage::Profile => 5,
            Stage::Done | Stage::Failed => 6,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildStatus {
    Running,
    Done,
    Failed,
}

#[derive(Debug, Clone)]
pub struct BuildEntry {
    pub drv: String,
    pub pretty_name: String,
    pub status: BuildStatus,
}

#[derive(Debug, Clone)]
pub struct DownloadEntry {
    pub item: String,
    pub pretty_name: String,
    pub bytes_done: u64,
    pub bytes_total: Option<u64>,
    /// Stderr-parsed ops never emit `SubstituteDownloadDone` — see NOTES.md.
    pub done: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// Non-zero process exit with no more specific cause.
    Exit { code: i32 },
    /// A derivation build failed.
    Build {
        name: String,
        log_path: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub struct Summary {
    pub stage: Stage,
    pub builds: IndexMap<String, BuildEntry>,
    pub downloads: IndexMap<String, DownloadEntry>,
    pub build_count_started: usize,
    pub build_count_done: usize,
    pub build_count_failed: usize,
    pub download_count_started: usize,
    pub download_count_done: usize,
    pub bytes_downloaded: u64,
    pub last_status_line: Option<String>,
    pub started_at: Option<Instant>,
    pub finished_at: Option<Instant>,
    pub failure: Option<Failure>,
    pub would_build_count: Option<usize>,
    pub would_download_count: Option<usize>,
    pub would_download_bytes: Option<u64>,
}

impl Default for Summary {
    fn default() -> Self {
        Self {
            stage: Stage::Starting,
            builds: IndexMap::new(),
            downloads: IndexMap::new(),
            build_count_started: 0,
            build_count_done: 0,
            build_count_failed: 0,
            download_count_started: 0,
            download_count_done: 0,
            bytes_downloaded: 0,
            last_status_line: None,
            started_at: None,
            finished_at: None,
            failure: None,
            would_build_count: None,
            would_download_count: None,
            would_download_bytes: None,
        }
    }
}

impl Summary {
    #[must_use]
    pub fn new() -> Self {
        Self {
            started_at: Some(Instant::now()),
            ..Self::default()
        }
    }

    fn advance_to(&mut self, next: Stage) {
        if self.stage == Stage::Failed {
            return;
        }
        if next == Stage::Failed {
            self.stage = Stage::Failed;
            return;
        }
        if next.order() > self.stage.order() {
            self.stage = next;
        }
    }

    pub fn ingest(&mut self, evt: &ProgressEvent) {
        match evt {
            ProgressEvent::ExitSummary { code, .. } => {
                self.finished_at = Some(Instant::now());
                if *code == 0 {
                    self.advance_to(Stage::Done);
                } else {
                    if self.failure.is_none() {
                        self.failure = Some(Failure::Exit { code: *code });
                    }
                    self.advance_to(Stage::Failed);
                }
            }
            ProgressEvent::BuildStart { drv } => {
                if !self.builds.contains_key(drv) {
                    self.build_count_started += 1;
                    self.builds.insert(
                        drv.clone(),
                        BuildEntry {
                            drv: drv.clone(),
                            pretty_name: pretty_store_name(drv),
                            status: BuildStatus::Running,
                        },
                    );
                }
                self.advance_to(Stage::Building);
            }
            ProgressEvent::BuildDone { drv } => {
                if let Some(entry) = self.builds.get_mut(drv) {
                    if entry.status == BuildStatus::Running {
                        self.build_count_done += 1;
                    }
                    entry.status = BuildStatus::Done;
                } else {
                    self.build_count_started += 1;
                    self.build_count_done += 1;
                    self.builds.insert(
                        drv.clone(),
                        BuildEntry {
                            drv: drv.clone(),
                            pretty_name: pretty_store_name(drv),
                            status: BuildStatus::Done,
                        },
                    );
                }
            }
            ProgressEvent::BuildFailed { drv, log_path } => {
                if let Some(entry) = self.builds.get_mut(drv) {
                    // Guard against a duplicate BuildFailed double-counting.
                    if entry.status == BuildStatus::Running {
                        self.build_count_failed += 1;
                    }
                    entry.status = BuildStatus::Failed;
                } else {
                    self.build_count_started += 1;
                    self.build_count_failed += 1;
                    self.builds.insert(
                        drv.clone(),
                        BuildEntry {
                            drv: drv.clone(),
                            pretty_name: pretty_store_name(drv),
                            status: BuildStatus::Failed,
                        },
                    );
                }
                self.failure.get_or_insert(Failure::Build {
                    name: pretty_store_name(drv),
                    log_path: log_path.clone(),
                });
                self.advance_to(Stage::Failed);
            }
            ProgressEvent::SubstituteDownload {
                item,
                bytes_done,
                bytes_total,
            } => {
                if let Some(existing) = self.downloads.get_mut(item) {
                    let delta = bytes_done.saturating_sub(existing.bytes_done);
                    self.bytes_downloaded = self.bytes_downloaded.saturating_add(delta);
                    existing.bytes_done = *bytes_done;
                    if bytes_total.is_some() {
                        existing.bytes_total = *bytes_total;
                    }
                } else {
                    self.download_count_started += 1;
                    self.bytes_downloaded = self.bytes_downloaded.saturating_add(*bytes_done);
                    self.downloads.insert(
                        item.clone(),
                        DownloadEntry {
                            item: item.clone(),
                            pretty_name: pretty_store_name(item),
                            bytes_done: *bytes_done,
                            bytes_total: *bytes_total,
                            done: false,
                        },
                    );
                }
                // Mid-build downloads shouldn't regress the header.
                if matches!(
                    self.stage,
                    Stage::Starting | Stage::ChannelUpdate | Stage::ComputingDeriv
                ) {
                    self.advance_to(Stage::Downloading);
                }
            }
            ProgressEvent::SubstituteDownloadDone { item, bytes_total } => {
                if let Some(entry) = self.downloads.get_mut(item) {
                    let was_already_done = entry.done;
                    entry.done = true;
                    if let Some(total) = bytes_total {
                        entry.bytes_done = (*total).max(entry.bytes_done);
                        if entry.bytes_total.is_none() {
                            entry.bytes_total = Some(*total);
                        }
                    }
                    if !was_already_done {
                        self.download_count_done += 1;
                    }
                } else {
                    self.download_count_started += 1;
                    self.download_count_done += 1;
                    self.downloads.insert(
                        item.clone(),
                        DownloadEntry {
                            item: item.clone(),
                            pretty_name: pretty_store_name(item),
                            bytes_done: bytes_total.unwrap_or(0),
                            bytes_total: *bytes_total,
                            done: true,
                        },
                    );
                }
            }
            ProgressEvent::PullComputingDerivation { .. } => {
                self.advance_to(Stage::ComputingDeriv);
            }
            ProgressEvent::Line { text, .. } => {
                let trimmed = text.trim_end_matches(['\r', '\n']);
                if !trimmed.is_empty() {
                    self.last_status_line = Some(trimmed.to_string());
                }
                if text.contains("Updating channel")
                    || text.contains("Building from these channels")
                {
                    self.advance_to(Stage::ChannelUpdate);
                }
                if text.contains("Computing Guix derivation") {
                    self.advance_to(Stage::ComputingDeriv);
                }
                if text.contains("running profile hook") {
                    self.advance_to(Stage::Profile);
                }
                if text.contains("nothing to be done") {
                    self.finished_at = Some(Instant::now());
                    self.advance_to(Stage::Done);
                }
            }
            ProgressEvent::WouldBuild { items, .. } => {
                self.would_build_count = Some(items.len());
            }
            ProgressEvent::WouldDownload { bytes, items } => {
                self.would_download_count = Some(items.len());
                self.would_download_bytes = Some(*bytes);
            }
            ProgressEvent::SubstituteLookup { .. }
            | ProgressEvent::BuildPhase { .. }
            | ProgressEvent::StorePathListed { .. }
            | ProgressEvent::DryRunHeader { .. }
            | ProgressEvent::KnownBug(_) => {}
        }
    }

    #[must_use]
    pub fn elapsed(&self) -> Option<Duration> {
        let start = self.started_at?;
        let end = self.finished_at.unwrap_or_else(Instant::now);
        Some(end.duration_since(start))
    }

    /// `None` until a total is known; prefers the up-front `Would*` totals.
    #[must_use]
    pub fn percent_complete(&self) -> Option<f32> {
        if matches!(self.stage, Stage::Done) {
            return Some(1.0);
        }
        let build_total = self.would_build_count.unwrap_or(self.build_count_started);
        let dl_total = self
            .would_download_count
            .unwrap_or(self.download_count_started);
        let total = build_total + dl_total;
        if total == 0 {
            return None;
        }
        let done = self.build_count_done + self.download_count_done;
        Some((done as f32 / total as f32).clamp(0.0, 1.0))
    }
}

/// `/gnu/store/abc123-foo-1.2.3.drv` → `foo-1.2.3`.
#[must_use]
pub fn pretty_store_name(path: &str) -> String {
    let after_store = match path.rfind("/gnu/store/") {
        Some(idx) => &path[idx + "/gnu/store/".len()..],
        None => path,
    };
    let after_hash = match after_store.find('-') {
        Some(idx) => &after_store[idx + 1..],
        None => return path.to_string(),
    };
    let trimmed = after_hash.strip_suffix(".drv").unwrap_or(after_hash);
    if trimmed.is_empty() {
        path.to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ProgressStream;

    #[test]
    fn pretty_store_name_drv() {
        assert_eq!(
            pretty_store_name("/gnu/store/abc123def456-foo-1.2.3.drv"),
            "foo-1.2.3"
        );
    }

    #[test]
    fn pretty_store_name_store_item() {
        assert_eq!(
            pretty_store_name("/gnu/store/abc123-hello-2.12.1-bin"),
            "hello-2.12.1-bin"
        );
    }

    #[test]
    fn pretty_store_name_non_matching() {
        assert_eq!(pretty_store_name("just-a-name"), "a-name");
        assert_eq!(pretty_store_name("noseparator"), "noseparator");
    }

    fn line(text: &str) -> ProgressEvent {
        ProgressEvent::Line {
            stream: ProgressStream::Stderr,
            text: text.to_string(),
            redraw: false,
        }
    }

    #[test]
    fn stage_transitions_user_pull_happy_path() {
        let mut s = Summary::new();
        assert_eq!(s.stage, Stage::Starting);

        s.ingest(&line("Updating channel 'guix' from Git repository at ..."));
        assert_eq!(s.stage, Stage::ChannelUpdate);

        s.ingest(&ProgressEvent::PullComputingDerivation {
            system: "x86_64-linux".into(),
        });
        assert_eq!(s.stage, Stage::ComputingDeriv);

        s.ingest(&ProgressEvent::BuildStart {
            drv: "/gnu/store/abc-guix-cache.drv".into(),
        });
        assert_eq!(s.stage, Stage::Building);

        s.ingest(&ProgressEvent::BuildDone {
            drv: "/gnu/store/abc-guix-cache.drv".into(),
        });
        assert_eq!(s.stage, Stage::Building);

        s.ingest(&ProgressEvent::ExitSummary {
            code: 0,
            duration_secs: 1.2,
        });
        assert_eq!(s.stage, Stage::Done);
    }

    #[test]
    fn downloading_doesnt_regress_from_building() {
        let mut s = Summary::new();
        s.ingest(&ProgressEvent::BuildStart {
            drv: "/gnu/store/x-foo.drv".into(),
        });
        assert_eq!(s.stage, Stage::Building);
        s.ingest(&ProgressEvent::SubstituteDownload {
            item: "/gnu/store/y-bar".into(),
            bytes_done: 0,
            bytes_total: Some(1000),
        });
        assert_eq!(s.stage, Stage::Building);
    }

    #[test]
    fn build_counters() {
        let mut s = Summary::new();
        s.ingest(&ProgressEvent::BuildStart {
            drv: "/gnu/store/a-foo.drv".into(),
        });
        s.ingest(&ProgressEvent::BuildStart {
            drv: "/gnu/store/b-bar.drv".into(),
        });
        s.ingest(&ProgressEvent::BuildDone {
            drv: "/gnu/store/a-foo.drv".into(),
        });
        s.ingest(&ProgressEvent::BuildFailed {
            drv: "/gnu/store/b-bar.drv".into(),
            log_path: None,
        });
        assert_eq!(s.build_count_started, 2);
        assert_eq!(s.build_count_done, 1);
        assert_eq!(s.build_count_failed, 1);
        assert_eq!(s.stage, Stage::Failed);
        assert!(matches!(s.failure, Some(Failure::Build { .. })));
    }

    #[test]
    fn double_build_failed_does_not_double_count() {
        let mut s = Summary::new();
        s.ingest(&ProgressEvent::BuildStart {
            drv: "/gnu/store/b-bar.drv".into(),
        });
        s.ingest(&ProgressEvent::BuildFailed {
            drv: "/gnu/store/b-bar.drv".into(),
            log_path: None,
        });
        s.ingest(&ProgressEvent::BuildFailed {
            drv: "/gnu/store/b-bar.drv".into(),
            log_path: None,
        });
        assert_eq!(s.build_count_failed, 1);
    }

    #[test]
    fn download_upsert_updates_bytes() {
        let mut s = Summary::new();
        s.ingest(&ProgressEvent::SubstituteDownload {
            item: "/gnu/store/x-foo".into(),
            bytes_done: 500,
            bytes_total: Some(1000),
        });
        s.ingest(&ProgressEvent::SubstituteDownload {
            item: "/gnu/store/x-foo".into(),
            bytes_done: 900,
            bytes_total: Some(1000),
        });
        s.ingest(&ProgressEvent::SubstituteDownload {
            item: "/gnu/store/y-bar".into(),
            bytes_done: 200,
            bytes_total: Some(400),
        });
        assert_eq!(s.download_count_started, 2);
        assert_eq!(s.bytes_downloaded, 1100);
        assert_eq!(s.downloads.get("/gnu/store/x-foo").unwrap().bytes_done, 900);
    }

    #[test]
    fn failed_exit_captures_failure() {
        let mut s = Summary::new();
        s.ingest(&ProgressEvent::ExitSummary {
            code: 1,
            duration_secs: 2.0,
        });
        assert_eq!(s.stage, Stage::Failed);
        assert_eq!(s.failure, Some(Failure::Exit { code: 1 }));
    }

    #[test]
    fn nothing_to_be_done_moves_to_done_early() {
        let mut s = Summary::new();
        s.ingest(&line("nothing to be done"));
        assert_eq!(s.stage, Stage::Done);
        assert!(s.finished_at.is_some());
    }

    #[test]
    fn download_done_marks_entry_and_increments_counter() {
        let mut s = Summary::new();
        s.ingest(&ProgressEvent::SubstituteDownload {
            item: "/gnu/store/x-foo".into(),
            bytes_done: 0,
            bytes_total: Some(1000),
        });
        assert_eq!(s.download_count_done, 0);
        assert!(!s.downloads.get("/gnu/store/x-foo").unwrap().done);

        s.ingest(&ProgressEvent::SubstituteDownloadDone {
            item: "/gnu/store/x-foo".into(),
            bytes_total: Some(1000),
        });
        assert_eq!(s.download_count_done, 1);
        let entry = s.downloads.get("/gnu/store/x-foo").unwrap();
        assert!(entry.done);
        assert_eq!(entry.bytes_done, 1000);
    }

    #[test]
    fn double_done_does_not_double_count() {
        let mut s = Summary::new();
        s.ingest(&ProgressEvent::SubstituteDownload {
            item: "/gnu/store/x-foo".into(),
            bytes_done: 0,
            bytes_total: Some(1000),
        });
        s.ingest(&ProgressEvent::SubstituteDownloadDone {
            item: "/gnu/store/x-foo".into(),
            bytes_total: Some(1000),
        });
        s.ingest(&ProgressEvent::SubstituteDownloadDone {
            item: "/gnu/store/x-foo".into(),
            bytes_total: Some(1000),
        });
        assert_eq!(s.download_count_done, 1);
    }

    #[test]
    fn download_done_without_start_records_defensively() {
        let mut s = Summary::new();
        s.ingest(&ProgressEvent::SubstituteDownloadDone {
            item: "/gnu/store/orphan-foo".into(),
            bytes_total: Some(2048),
        });
        assert_eq!(s.download_count_started, 1);
        assert_eq!(s.download_count_done, 1);
        let entry = s.downloads.get("/gnu/store/orphan-foo").unwrap();
        assert!(entry.done);
        assert_eq!(entry.bytes_total, Some(2048));
    }

    #[test]
    fn last_status_line_records_recent() {
        let mut s = Summary::new();
        s.ingest(&line("Updating channel 'guix' from ..."));
        s.ingest(&line("Computing Guix derivation for 'x86_64-linux'..."));
        assert_eq!(
            s.last_status_line.as_deref(),
            Some("Computing Guix derivation for 'x86_64-linux'...")
        );
    }

    #[test]
    fn would_build_total_drives_percent_denominator() {
        let mut s = Summary::new();
        // Up-front plan: three derivations to build, no downloads.
        s.ingest(&ProgressEvent::WouldBuild {
            bytes: 0,
            items: vec![
                "/gnu/store/a-foo.drv".into(),
                "/gnu/store/b-bar.drv".into(),
                "/gnu/store/c-baz.drv".into(),
            ],
        });
        // Only the first build has actually started and finished.
        s.ingest(&ProgressEvent::BuildStart {
            drv: "/gnu/store/a-foo.drv".into(),
        });
        s.ingest(&ProgressEvent::BuildDone {
            drv: "/gnu/store/a-foo.drv".into(),
        });
        assert_eq!(s.build_count_started, 1);
        assert_eq!(s.would_build_count, Some(3));
        // Denominator must be the would-build total of 3, not the started 1.
        assert_eq!(s.percent_complete(), Some(1.0 / 3.0));
    }
}
