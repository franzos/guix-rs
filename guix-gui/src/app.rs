use std::collections::HashMap;
use std::path::{Path, PathBuf};

use iced::theme::Palette;
use iced::widget::{button, column, container, row, scrollable, svg, text, tooltip, Column, Space};
use iced::{Alignment, Element, Font, Length, Subscription, Task, Theme};
use libguix::{
    CancelHandle, Channel, ChannelOp, ChannelsFile, Guix, GuixError, InstalledPackage, KnownBug,
    Operation, PackageSummary, ProgressEvent, ProgressStream, PullOps, ReconfigureOptions,
    SearchFastResult, SystemPullOptions, DEFAULT_SEARCH_LIMIT,
};

use crate::app_metadata::{AppMetadata, MetadataClient};
use crate::carrier::Carrier;
use crate::channels::{
    describe_channels_error, load_channels_file, outcome_to_result, ChannelsFileLoad,
    ChannelsFileLoadOutcome, ChannelsState, ChannelsSubMode, RollbackOffer,
};
use crate::operation_subscription::{operation_subscription, OpEvent, OpId, OpKind, SharedOp};
use crate::progress_summary::ProgressSummary;
use crate::recommended::RECOMMENDED;
use crate::settings::{probe_first_run_config, Settings, Tab};
use crate::styles::{self, BG, PRIMARY, TEXT};
use crate::terminal_buffer::TerminalBuffer;
use crate::views::{about, channels as channels_view, home, installed, search, system, updates};
use guix_gui::discovery::{DiscoveredChannel, DiscoveredPackage, Discovery, DiscoveryError};

pub const BOOTSTRAP_HINT_PATTERN: &str = "no code for module";

#[must_use]
pub fn bootstrap_help_message(
    auto_load_path: Option<&Path>,
    source_config_path: Option<&Path>,
) -> String {
    let load = auto_load_path
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "<parent of config>".into());
    let cfg = source_config_path
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "<set source config>".into());
    crate::t!("app-bootstrap-help", load = load, cfg = cfg)
}

pub struct App {
    pub guix: Option<Guix>,
    pub discovery_error: Option<String>,
    pub active_tab: Tab,
    pub active_op: Option<ActiveOp>,
    pub terminal: TerminalBuffer,
    pub show_log: bool,
    pub debug_events: bool,
    pub settings: Settings,
    pub search: SearchState,
    pub installed: InstalledState,
    pub updates: UpdatesState,
    pub system: SystemState,
    pub channels: ChannelsState,
    /// Gates `repl.interrupt()` — SIGINT during initial module
    /// expansion can half-load modules. See NOTES.md "SIGINT cancellation".
    pub warmup_done: bool,
    /// Cached custom theme; cloned per frame by `App::theme`.
    theme: Theme,
    pub metadata_client: MetadataClient,
    /// In-memory only — re-fetched each session. Keyed by guix package
    /// name; `None` value means "in-flight", `Some` is the result
    /// (which may itself be empty if both sources missed).
    pub metadata_cache: HashMap<String, Option<AppMetadata>>,
    /// Active screenshot lightbox. Bytes are cloned from the metadata
    /// cache when opened so the overlay survives cache eviction
    /// (e.g. toggling sources mid-view).
    pub lightbox: Option<Vec<u8>>,
    /// Icon-only cache for the Home tab — separate from `metadata_cache`
    /// so the heavyweight screenshot fetch isn't triggered just to
    /// populate tile thumbnails.
    pub home_icons: HashMap<String, IconCacheEntry>,
}

#[derive(Debug, Clone)]
pub enum IconCacheEntry {
    Loading,
    Done(Option<Vec<u8>>),
}

pub struct ActiveOp {
    pub id: OpId,
    pub kind: OpKind,
    pub cancel: Option<CancelHandle>,
    pub op_slot: SharedOp,
    pub final_code: Option<i32>,
    pub finished: bool,
    pub bootstrap_likely: bool,
    pub progress: ProgressSummary,
    /// Sticky flag — set the first time we observe
    /// `ProgressEvent::KnownBug(ChannelShadow74396)` in the operation's
    /// progress stream. Consumed by the channels-tab rollback offer to
    /// attach bug context to the CTA when a Channels-tab-triggered pull
    /// fails with #74396 in flight.
    pub channel_shadow_seen: bool,
}

#[derive(Default)]
pub struct SearchState {
    pub query: String,
    pub query_seq: u64,
    pub results: Vec<PackageSummary>,
    pub selected: Option<usize>,
    pub searching: bool,
    pub error: Option<SearchError>,
    pub truncated: bool,
    pub last_limit: usize,
    /// When set, the next successful `SearchCompleted` will auto-select
    /// the result whose `name` matches exactly. Consumed on completion
    /// regardless of whether the match is found — a stray name from a
    /// stale Home click shouldn't latch onto an unrelated future query.
    pub pending_select: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SearchError {
    pub summary: String,
    pub details: String,
}

#[derive(Default)]
pub struct InstalledState {
    pub packages: Vec<InstalledPackage>,
    pub refreshing: bool,
    pub error: Option<String>,
}

#[derive(Default)]
pub struct UpdatesState {
    pub channels: Vec<Channel>,
    pub loading_channels: bool,
    pub error: Option<String>,
    pub mtimes: PullMtimes,
}

/// Refreshed alongside the channel list — never `stat(2)` from `view()`.
#[derive(Default, Debug, Clone)]
pub struct PullMtimes {
    pub user_pull: Option<std::time::SystemTime>,
    pub root_pull: Option<std::time::SystemTime>,
    pub system_profile: Option<std::time::SystemTime>,
}

#[derive(Default)]
pub struct SystemState {
    pub current_config_display: Option<String>,
    pub current_config_error: Option<String>,
    pub source_input: String,
    pub validation_message: Option<String>,
    pub load_path_input: String,
    /// Transient feedback for the "clear cache" action. Set when the
    /// click is dispatched, replaced when the async result arrives.
    pub cache_action_message: Option<String>,
    /// Buffered input for the channels source-path override; mirrors
    /// `source_input` for the `config.scm` override.
    pub channels_source_input: String,
    /// Snapshot captured on the first "Update system" click and shown
    /// in the confirm card, so a settings change between click and
    /// confirm can't mutate what `pkexec` actually runs.
    pub pending_reconfigure: Option<PendingReconfigure>,
}

#[derive(Debug, Clone)]
pub struct PendingReconfigure {
    pub config_path: PathBuf,
    pub load_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub enum Message {
    DiscoveryComplete(Result<Carrier<Guix>, String>),
    ReplWarmedUp(Result<(), String>),

    TabSelected(Tab),

    SearchInputChanged(String),
    SearchDebounceTick(u64),
    SearchCompleted {
        seq: u64,
        result: Result<Carrier<SearchFastResult>, String>,
    },
    SearchResultSelected(usize),
    SearchErrorCopy,

    InstalledRefresh,
    InstalledLoaded(Result<Vec<InstalledPackage>, String>),
    RemoveRequested(String),

    ChannelsLoaded(Result<Vec<Channel>, String>),
    PullMtimesLoaded(PullMtimes),
    FetchUserCatalogClicked,
    FetchSystemCatalogClicked,
    UpgradeClicked,
    ReconfigureClicked,
    ReconfigureConfirmed,
    ReconfigureCancelled,

    SystemConfigChecked(Result<String, String>),
    SourceConfigChanged(String),
    SourceConfigValidate,
    ChannelsSourcePathChanged(String),
    ChannelsSourcePathUseDefault,
    LoadPathInputChanged(String),
    LoadPathAdd,
    LoadPathRemove(usize),

    ChannelsRefresh,
    ChannelsFileLoaded(ChannelsFileLoadOutcome),
    ChannelsRestoreClicked,
    ChannelsRestoreCancelled,
    ChannelsRestoreConfirmed,
    ChannelsRestoreCompleted(Result<ChannelsFileLoad, String>),
    ChannelsRemoveClicked(String),
    ChannelsRemoveCancelled,
    ChannelsRemoveConfirmed(String),
    ChannelsAddSubmitted,
    ChannelsAddNameChanged(String),
    ChannelsAddUrlChanged(String),
    ChannelsAddBranchChanged(String),
    ChannelsAddCommitChanged(String),
    ChannelsAddIntroCommitChanged(String),
    ChannelsAddIntroFprChanged(String),
    ChannelsApplyCompleted(Result<ChannelsFileLoad, String>),
    ChannelsToastDismissed,
    /// Best-effort introspection that maps installed packages to their
    /// source channel — backs the Remove warning dialog. On failure
    /// we store an empty map (don't retry) and don't surface an error.
    ChannelsInstalledByChannelLoaded(Result<HashMap<String, Vec<InstalledPackage>>, String>),

    // -- Discover sub-mode (Channels tab) ------------------------------
    DiscoveryEnabledToggled(bool),
    ChannelsSubModeSelected(ChannelsSubMode),
    DiscoverChannelsLoaded(Result<Carrier<Vec<DiscoveredChannel>>, String>),
    DiscoverQueryChanged(String),
    DiscoverSearchDebounceTick(u64),
    DiscoverPackagesLoaded {
        seq: u64,
        result: Result<Carrier<Vec<DiscoveredPackage>>, String>,
    },
    DiscoverAddClicked(Carrier<Channel>),
    /// Sibling of `DiscoverAddClicked` carrying the *package* name that
    /// the user clicked from. Stashed so that after the channel write
    /// succeeds we can offer "Pull, then install <pkg>" in one tap.
    DiscoverAddAndInstallClicked(Carrier<Channel>, String),
    DiscoverAddCancelled,
    DiscoverAddConfirmed,

    /// "Pull now" from the post-write Channels-tab toast. Wraps
    /// `FetchUserCatalogClicked` plus the side effect of flagging the
    /// next pull completion as belonging to a channels-tab edit, so a
    /// non-zero exit surfaces the rollback offer.
    ChannelsToastPullClicked,
    /// "Pull, then install <pkg>" from the combined Discover-add toast.
    /// Stashes the package name in `pending_install` so the next pull
    /// completion auto-fires `InstallRequested(name)` on success.
    ChannelsToastPullAndInstallClicked(String),
    ChannelsRollbackConfirmed,
    ChannelsRollbackDismissed,

    InstallRequested(String),

    AppMetadataLoaded {
        name: String,
        metadata: Carrier<AppMetadata>,
    },
    HomeAppClicked(String),
    HomeIconLoaded {
        name: String,
        bytes: Carrier<Option<Vec<u8>>>,
    },
    ClearMetadataCacheClicked,
    MetadataCacheCleared(Result<(), String>),
    AppMetadataEnabledToggled(bool),
    AppMetadataFlathubToggled(bool),
    AppMetadataDebianToggled(bool),
    /// `None` follows the system locale; `Some(tag)` is a BCP-47 override.
    LanguageSelected(Option<String>),
    LightboxOpened(Carrier<Vec<u8>>),
    LightboxClosed,
    OpenUrl(String),

    OpStarted {
        id: OpId,
        kind: OpKind,
        slot: SharedOp,
        cancel: Carrier<CancelHandle>,
    },
    OpStartFailed(String),
    Progress(OpEvent),
    CancelClicked,
    DismissOverlay,
    CopyTerminalClicked,
    ToggleLog,
    /// 1Hz wakeup so the overlay's elapsed-time readout keeps ticking
    /// between event batches. Handler is a no-op; the dispatch itself
    /// triggers a redraw.
    Tick,
}

impl App {
    pub fn new() -> (Self, Task<Message>) {
        let mut settings = Settings::load();
        if settings.source_config_path.is_none() {
            if let Some(p) = probe_first_run_config(Path::new("/")) {
                settings.source_config_path = Some(p);
                let _ = settings.save();
            }
        }
        // Every launch lands on Home — the discover surface is the
        // intended entry point, not whichever tab the user happened to
        // close on. `Tab` is no longer persisted in `Settings`.
        let active_tab = Tab::Home;
        let show_log = settings.show_log_by_default;
        let debug_events = std::env::var("GUIX_GUI_DEBUG_EVENTS").is_ok();
        let source_input = settings
            .source_config_path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let channels_source_input = settings
            .channels_source_path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let theme = Theme::custom(
            "GuixGold".to_string(),
            Palette {
                background: BG,
                text: TEXT,
                primary: PRIMARY,
                success: styles::SUCCESS,
                warning: styles::WARNING,
                danger: styles::DANGER,
            },
        );
        let state = Self {
            guix: None,
            discovery_error: None,
            active_tab,
            active_op: None,
            terminal: TerminalBuffer::default(),
            show_log,
            debug_events,
            settings,
            search: SearchState::default(),
            installed: InstalledState::default(),
            updates: UpdatesState::default(),
            system: SystemState {
                source_input,
                channels_source_input,
                ..Default::default()
            },
            channels: ChannelsState::default(),
            warmup_done: false,
            theme,
            metadata_client: MetadataClient::new().unwrap_or_else(|e| {
                tracing::warn!(
                    target: "guix_gui",
                    "metadata client build failed ({e}); falling back to degraded client"
                );
                MetadataClient::degraded()
            }),
            metadata_cache: HashMap::new(),
            lightbox: None,
            home_icons: HashMap::new(),
        };
        let boot = Task::perform(
            async {
                Guix::discover()
                    .await
                    .map(Carrier::new)
                    .map_err(|e| format!("{e}"))
            },
            Message::DiscoveryComplete,
        );
        (state, boot)
    }

    pub fn title(&self) -> String {
        crate::t!("app-title")
    }

    pub fn theme(&self) -> Theme {
        self.theme.clone()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subs: Vec<Subscription<Message>> = Vec::new();
        if let Some(op) = &self.active_op {
            if !op.finished {
                subs.push(
                    operation_subscription(op.kind, op.id, op.op_slot.clone())
                        .map(Message::Progress),
                );
                subs.push(
                    iced::time::every(std::time::Duration::from_secs(1)).map(|_| Message::Tick),
                );
            }
        }
        if self.lightbox.is_some() {
            // ESC closes the lightbox — matches the convention every
            // image viewer uses. Non-Escape keypresses map to `Tick`
            // (a no-op) so the subscription stays cheap.
            subs.push(iced::keyboard::listen().map(|evt| match evt {
                iced::keyboard::Event::KeyPressed {
                    key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                    ..
                } => Message::LightboxClosed,
                _ => Message::Tick,
            }));
        }
        Subscription::batch(subs)
    }

    pub fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::DiscoveryComplete(Ok(c)) => {
                if let Some(g) = c.take() {
                    self.guix = Some(g);
                }
                let warmup_task = self
                    .guix
                    .clone()
                    .map(|g| {
                        Task::perform(
                            async move {
                                match g.repl().await {
                                    Ok(repl) => repl.warmup().await.map_err(|e| format!("{e}")),
                                    Err(e) => Err(format!("{e}")),
                                }
                            },
                            Message::ReplWarmedUp,
                        )
                    })
                    .unwrap_or_else(Task::none);
                let tab_task = match self.active_tab {
                    Tab::Home => self.spawn_home_icons_prefetch(),
                    Tab::Search => Task::none(),
                    Tab::Installed => Task::none(),
                    Tab::Updates => self.spawn_pull_mtimes_refresh(),
                    Tab::Channels => self.spawn_channels_file_load(),
                    Tab::System => self.spawn_system_load(),
                    Tab::About => Task::none(),
                };
                // Always load the installed list at startup — the Search
                // detail pane needs it to flip Install/Remove correctly.
                let installed_task = self.spawn_installed_refresh();
                // Always load channels at startup — the Home tab uses the
                // channel list to decide which channel-gated tiles are
                // eligible. Cheap enough to run unconditionally.
                let channels_task = self.spawn_channels_refresh();
                Task::batch([warmup_task, tab_task, installed_task, channels_task])
            }
            Message::DiscoveryComplete(Err(e)) => {
                self.discovery_error = Some(e);
                Task::none()
            }
            Message::ReplWarmedUp(result) => {
                match result {
                    Ok(()) => {
                        self.warmup_done = true;
                        tracing::debug!(target: "guix_gui", "repl warmup complete");
                    }
                    Err(e) => {
                        // Leave warmup_done false — modules may be mid-load.
                        tracing::warn!(target: "guix_gui", "repl warmup failed: {e}");
                    }
                }
                Task::none()
            }
            Message::TabSelected(t) => {
                self.active_tab = t;
                match t {
                    Tab::Home => self.spawn_home_icons_prefetch(),
                    Tab::Installed if self.installed.packages.is_empty() => {
                        self.spawn_installed_refresh()
                    }
                    Tab::Updates => Task::batch([
                        self.spawn_channels_refresh(),
                        self.spawn_pull_mtimes_refresh(),
                    ]),
                    Tab::Channels => {
                        let mut tasks: Vec<Task<Message>> = Vec::new();
                        if self.channels.file.is_none() && !self.channels.loading {
                            tasks.push(self.spawn_channels_file_load());
                        }
                        if self.channels.installed_by_channel.is_none()
                            && !self.channels.installed_by_channel_loading
                        {
                            tasks.push(self.spawn_installed_by_channel_fetch());
                        }
                        Task::batch(tasks)
                    }
                    Tab::System if self.system.current_config_display.is_none() => {
                        self.spawn_system_load()
                    }
                    _ => Task::none(),
                }
            }

            Message::SearchInputChanged(q) => {
                self.search.query = q;
                self.search.query_seq = self.search.query_seq.wrapping_add(1);
                let seq = self.search.query_seq;
                // Both flags required — see NOTES.md "SIGINT cancellation".
                if self.warmup_done && self.search.searching {
                    if let Some(repl) = self.guix.as_ref().and_then(|g| g.repl_if_ready()) {
                        let _ = repl.interrupt();
                    }
                }
                Task::perform(
                    async move {
                        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                        seq
                    },
                    Message::SearchDebounceTick,
                )
            }
            Message::SearchDebounceTick(seq) => {
                if seq != self.search.query_seq || self.search.query.trim().is_empty() {
                    return Task::none();
                }
                self.search.error = None;
                self.search.searching = true;
                let q = self.search.query.clone();
                let Some(g) = self.guix.clone() else {
                    return Task::none();
                };
                Task::perform(
                    async move {
                        g.package()
                            .search_fast_limited(&q, DEFAULT_SEARCH_LIMIT)
                            .await
                            .map(Carrier::new)
                            .map_err(|e| format!("{e}"))
                    },
                    move |r| Message::SearchCompleted { seq, result: r },
                )
            }
            Message::SearchCompleted { seq, result } => {
                // Must clear `searching` BEFORE the staleness check —
                // otherwise the next keystroke fires SIGINT at an idle REPL.
                self.search.searching = false;
                if seq != self.search.query_seq {
                    return Task::none();
                }
                match result {
                    Ok(carrier) => {
                        if let Some(res) = carrier.take() {
                            self.search.results = res.results;
                            self.search.truncated = res.truncated;
                            self.search.last_limit = res.limit;
                            self.search.selected = None;
                            self.search.error = None;
                            if let Some(name) = self.search.pending_select.take() {
                                if let Some(i) =
                                    self.search.results.iter().position(|p| p.name == name)
                                {
                                    self.search.selected = Some(i);
                                    return self.spawn_app_metadata_fetch(i);
                                }
                            }
                        } else {
                            self.search.error = None;
                        }
                    }
                    Err(e) => self.search.error = Some(build_search_error(e)),
                }
                Task::none()
            }
            Message::SearchResultSelected(i) => {
                self.search.selected = Some(i);
                self.spawn_app_metadata_fetch(i)
            }
            Message::SearchErrorCopy => {
                if let Some(err) = self.search.error.as_ref() {
                    iced::clipboard::write::<Message>(err.details.clone())
                } else {
                    Task::none()
                }
            }

            Message::InstalledRefresh => self.spawn_installed_refresh(),
            Message::InstalledLoaded(Ok(ps)) => {
                self.installed.refreshing = false;
                self.installed.packages = ps;
                self.installed.error = None;
                Task::none()
            }
            Message::InstalledLoaded(Err(e)) => {
                self.installed.refreshing = false;
                self.installed.error = Some(e);
                Task::none()
            }
            Message::RemoveRequested(name) => self.start_op(OpKind::Remove, move |g| {
                g.package().remove(&[name.as_str()])
            }),

            Message::ChannelsLoaded(Ok(cs)) => {
                self.updates.loading_channels = false;
                self.updates.channels = cs;
                self.updates.error = None;
                Task::none()
            }
            Message::ChannelsLoaded(Err(e)) => {
                self.updates.loading_channels = false;
                self.updates.error = Some(e);
                Task::none()
            }
            Message::PullMtimesLoaded(m) => {
                self.updates.mtimes = m;
                Task::none()
            }
            Message::FetchUserCatalogClicked => self.start_op(OpKind::Pull, |g| g.pull().user()),
            Message::FetchSystemCatalogClicked => self.start_op(OpKind::SystemPull, |g| {
                g.pull().as_root(SystemPullOptions::default())
            }),
            Message::UpgradeClicked => {
                self.start_op(OpKind::Upgrade, |g| g.package().upgrade(None))
            }
            Message::ReconfigureClicked => {
                let Some(path) = self.settings.source_config_path.clone() else {
                    self.system.validation_message = Some(crate::t!("app-set-source-config-first"));
                    self.active_tab = Tab::System;
                    return Task::none();
                };
                self.system.pending_reconfigure = Some(PendingReconfigure {
                    config_path: path,
                    load_paths: self.settings.effective_load_paths(),
                });
                Task::none()
            }
            Message::ReconfigureCancelled => {
                self.system.pending_reconfigure = None;
                Task::none()
            }
            Message::ReconfigureConfirmed => {
                let Some(pending) = self.system.pending_reconfigure.take() else {
                    return Task::none();
                };
                let PendingReconfigure {
                    config_path,
                    load_paths,
                } = pending;
                self.start_op(OpKind::Reconfigure, move |g| {
                    g.system().reconfigure(
                        &config_path,
                        ReconfigureOptions {
                            load_paths,
                            ..Default::default()
                        },
                    )
                })
            }

            Message::SystemConfigChecked(Ok(p)) => {
                self.system.current_config_display = Some(p);
                self.system.current_config_error = None;
                Task::none()
            }
            Message::SystemConfigChecked(Err(e)) => {
                self.system.current_config_error = Some(e);
                Task::none()
            }
            Message::SourceConfigChanged(s) => {
                self.system.source_input = s.clone();
                self.settings.source_config_path = if s.trim().is_empty() {
                    None
                } else {
                    Some(PathBuf::from(s))
                };
                let _ = self.settings.save();
                Task::none()
            }
            Message::SourceConfigValidate => {
                let p = PathBuf::from(self.system.source_input.trim());
                self.system.validation_message = Some(if p.as_os_str().is_empty() {
                    crate::t!("system-validation-empty")
                } else if !p.exists() {
                    crate::t!("system-validation-missing", path = p.display().to_string())
                } else if !p.is_file() {
                    crate::t!("system-validation-not-file", path = p.display().to_string())
                } else {
                    crate::t!("system-validation-ok", path = p.display().to_string())
                });
                Task::none()
            }
            Message::LoadPathInputChanged(s) => {
                self.system.load_path_input = s;
                Task::none()
            }
            Message::LoadPathAdd => {
                let trimmed = self.system.load_path_input.trim();
                if !trimmed.is_empty() {
                    self.settings.custom_load_paths.push(PathBuf::from(trimmed));
                    self.system.load_path_input.clear();
                    let _ = self.settings.save();
                }
                Task::none()
            }
            Message::LoadPathRemove(i) => {
                if i < self.settings.custom_load_paths.len() {
                    self.settings.custom_load_paths.remove(i);
                    let _ = self.settings.save();
                }
                Task::none()
            }
            Message::ChannelsSourcePathChanged(s) => {
                self.system.channels_source_input = s.clone();
                self.settings.channels_source_path = if s.trim().is_empty() {
                    None
                } else {
                    Some(PathBuf::from(s))
                };
                let _ = self.settings.save();
                // Force a reload on the next Channels tab visit / explicit
                // refresh — the override changed under it.
                self.channels.file = None;
                self.channels.backup_path = None;
                self.channels.error = None;
                Task::none()
            }
            Message::ChannelsSourcePathUseDefault => {
                self.system.channels_source_input.clear();
                self.settings.channels_source_path = None;
                let _ = self.settings.save();
                self.channels.file = None;
                self.channels.backup_path = None;
                self.channels.error = None;
                Task::none()
            }

            Message::ChannelsRefresh => self.spawn_channels_file_load(),
            Message::ChannelsFileLoaded(outcome) => {
                self.channels.loading = false;
                match outcome {
                    ChannelsFileLoadOutcome::Loaded(load) => {
                        self.channels.file = Some(load.file);
                        self.channels.backup_path = load.backup_path;
                        self.channels.error = None;
                    }
                    // Missing file is the empty state — let the user
                    // create one by adding their first channel.
                    ChannelsFileLoadOutcome::Missing => {
                        self.channels.file = None;
                        self.channels.backup_path = None;
                        self.channels.error = None;
                    }
                    ChannelsFileLoadOutcome::Failed(e) => {
                        self.channels.file = None;
                        self.channels.backup_path = None;
                        self.channels.error = Some(e);
                    }
                }
                Task::none()
            }
            Message::ChannelsRemoveClicked(name) => {
                self.channels.pending_remove = Some(name);
                self.channels.last_message = None;
                // Kick off the introspection fetch only when we don't
                // already have one (the cache survives across opens
                // until an install/remove/upgrade invalidates it). The
                // dialog renders the minimal Confirm/Cancel while the
                // fetch is in flight — never blocks Remove.
                if self.channels.installed_by_channel.is_none()
                    && !self.channels.installed_by_channel_loading
                {
                    return self.spawn_installed_by_channel_fetch();
                }
                Task::none()
            }
            Message::ChannelsRemoveCancelled => {
                self.channels.pending_remove = None;
                Task::none()
            }
            Message::ChannelsRemoveConfirmed(name) => {
                self.channels.pending_remove = None;
                self.spawn_channels_apply(ChannelOp::RemoveChannelByName(name))
            }
            Message::ChannelsRestoreClicked => {
                self.channels.pending_restore = true;
                self.channels.last_message = None;
                Task::none()
            }
            Message::ChannelsRestoreCancelled => {
                self.channels.pending_restore = false;
                Task::none()
            }
            Message::ChannelsRestoreConfirmed => {
                self.channels.pending_restore = false;
                self.spawn_channels_restore()
            }
            Message::ChannelsRestoreCompleted(Ok(load)) => {
                self.channels.saving = false;
                self.channels.file = Some(load.file);
                self.channels.backup_path = load.backup_path;
                self.channels.error = None;
                self.channels.last_message = Some(crate::t!("channels-restored"));
                Task::none()
            }
            Message::ChannelsRestoreCompleted(Err(e)) => {
                self.channels.saving = false;
                self.channels.error = Some(e);
                Task::none()
            }
            Message::ChannelsAddSubmitted => {
                let channel_result = self.channels.add_form.to_channel();
                match channel_result {
                    Ok(ch) => {
                        self.channels.validation_message = None;
                        self.spawn_channels_apply(ChannelOp::AddChannel(ch))
                    }
                    Err(msg) => {
                        self.channels.validation_message = Some(msg);
                        Task::none()
                    }
                }
            }
            Message::ChannelsAddNameChanged(s) => {
                self.channels.add_form.name = s;
                Task::none()
            }
            Message::ChannelsAddUrlChanged(s) => {
                self.channels.add_form.url = s;
                Task::none()
            }
            Message::ChannelsAddBranchChanged(s) => {
                self.channels.add_form.branch = s;
                Task::none()
            }
            Message::ChannelsAddCommitChanged(s) => {
                self.channels.add_form.commit = s;
                Task::none()
            }
            Message::ChannelsAddIntroCommitChanged(s) => {
                self.channels.add_form.intro_commit = s;
                Task::none()
            }
            Message::ChannelsAddIntroFprChanged(s) => {
                self.channels.add_form.intro_fpr = s;
                Task::none()
            }
            Message::ChannelsApplyCompleted(Ok(load)) => {
                self.channels.saving = false;
                self.channels.file = Some(load.file);
                self.channels.backup_path = load.backup_path;
                self.channels.error = None;
                self.channels.add_form.clear();
                // Pop the install pairing so a later add (without
                // install) doesn't accidentally inherit it.
                let pkg = self.channels.discover_pending_install.take();
                if let Some(name) = pkg {
                    self.channels.post_apply_install_prompt = Some(name.clone());
                    self.channels.last_message =
                        Some(crate::t!("channels-added-install-prompt", pkg = name));
                } else {
                    self.channels.post_apply_install_prompt = None;
                    self.channels.last_message = Some(crate::t!("channels-updated"));
                }
                Task::none()
            }
            Message::ChannelsApplyCompleted(Err(e)) => {
                self.channels.saving = false;
                self.channels.error = Some(e);
                Task::none()
            }
            Message::ChannelsToastDismissed => {
                self.channels.last_message = None;
                self.channels.post_apply_install_prompt = None;
                Task::none()
            }
            Message::ChannelsInstalledByChannelLoaded(result) => {
                self.channels.installed_by_channel_loading = false;
                // Failure is silent — store an empty map so the next
                // open doesn't trigger another doomed fetch. The Remove
                // dialog falls through to its minimal Confirm/Cancel
                // branch when the cache is empty.
                self.channels.installed_by_channel = Some(match result {
                    Ok(m) => m,
                    Err(e) => {
                        tracing::warn!(
                            target: "guix_gui",
                            "installed_by_channel introspection failed: {e}",
                        );
                        HashMap::new()
                    }
                });
                Task::none()
            }

            Message::LanguageSelected(tag) => {
                self.settings.language = tag;
                let _ = self.settings.save();
                crate::i18n::select_language(&crate::i18n::requested_languages(
                    self.settings.language.as_deref(),
                ));
                Task::none()
            }
            Message::DiscoveryEnabledToggled(v) => {
                self.settings.discovery_enabled = v;
                let _ = self.settings.save();
                if !v {
                    // Tear down everything discovery-related so no
                    // stale data or in-flight task can re-surface.
                    self.channels.sub_mode = ChannelsSubMode::Installed;
                    self.channels.discovery = None;
                    self.channels.discover_channels.clear();
                    self.channels.discover_packages.clear();
                    self.channels.discover_query.clear();
                    self.channels.discover_error = None;
                    self.channels.discover_pending_add = None;
                    self.channels.discover_pending_install = None;
                    self.channels.discover_channels_loading = false;
                    self.channels.discover_packages_loading = false;
                }
                Task::none()
            }
            Message::ChannelsSubModeSelected(mode) => {
                if !self.settings.discovery_enabled && mode == ChannelsSubMode::Discover {
                    // Hard gate — even if a stale message reaches us
                    // while the toggle is off, refuse the transition.
                    return Task::none();
                }
                self.channels.sub_mode = mode;
                if mode == ChannelsSubMode::Discover
                    && self.channels.discover_channels.is_empty()
                    && !self.channels.discover_channels_loading
                {
                    return self.spawn_discover_channels_fetch();
                }
                Task::none()
            }
            Message::DiscoverChannelsLoaded(Ok(c)) => {
                self.channels.discover_channels_loading = false;
                if let Some(list) = c.take() {
                    self.channels.discover_channels = list;
                    self.channels.discover_error = None;
                }
                Task::none()
            }
            Message::DiscoverChannelsLoaded(Err(e)) => {
                self.channels.discover_channels_loading = false;
                self.channels.discover_error = Some(e);
                Task::none()
            }
            Message::DiscoverQueryChanged(q) => {
                self.channels.discover_query = q;
                self.channels.discover_query_seq = self.channels.discover_query_seq.wrapping_add(1);
                let seq = self.channels.discover_query_seq;
                Task::perform(
                    async move {
                        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                        seq
                    },
                    Message::DiscoverSearchDebounceTick,
                )
            }
            Message::DiscoverSearchDebounceTick(seq) => {
                if seq != self.channels.discover_query_seq {
                    return Task::none();
                }
                let q = self.channels.discover_query.trim().to_string();
                if q.is_empty() {
                    self.channels.discover_packages.clear();
                    self.channels.discover_packages_loading = false;
                    return Task::none();
                }
                self.channels.discover_packages_loading = true;
                let Some(client) = self.ensure_discovery() else {
                    self.channels.discover_packages_loading = false;
                    return Task::none();
                };
                Task::perform(
                    async move {
                        client
                            .search_packages(&q, 1, 50)
                            .await
                            .map(Carrier::new)
                            .map_err(|e: DiscoveryError| e.to_string())
                    },
                    move |result| Message::DiscoverPackagesLoaded { seq, result },
                )
            }
            Message::DiscoverPackagesLoaded { seq, result } => {
                if seq != self.channels.discover_query_seq {
                    return Task::none();
                }
                self.channels.discover_packages_loading = false;
                match result {
                    Ok(c) => {
                        if let Some(list) = c.take() {
                            self.channels.discover_packages = list;
                            self.channels.discover_error = None;
                        }
                    }
                    Err(e) => self.channels.discover_error = Some(e),
                }
                Task::none()
            }
            Message::DiscoverAddClicked(c) => {
                if let Some(ch) = c.take() {
                    self.channels.discover_pending_add = Some(ch);
                    // Plain "Add" — no install follow-up. Clear any
                    // stale install pairing from a previous
                    // package-row click that was never confirmed.
                    self.channels.discover_pending_install = None;
                }
                Task::none()
            }
            Message::DiscoverAddAndInstallClicked(c, pkg_name) => {
                if let Some(ch) = c.take() {
                    self.channels.discover_pending_add = Some(ch);
                    self.channels.discover_pending_install = Some(pkg_name);
                }
                Task::none()
            }
            Message::DiscoverAddCancelled => {
                self.channels.discover_pending_add = None;
                self.channels.discover_pending_install = None;
                Task::none()
            }
            Message::DiscoverAddConfirmed => {
                let Some(ch) = self.channels.discover_pending_add.take() else {
                    return Task::none();
                };
                // Carry the install pairing into the apply flow — read
                // on `ChannelsApplyCompleted` to decide whether to
                // render the combined CTA.
                self.spawn_channels_apply(ChannelOp::AddChannel(ch))
            }
            Message::ChannelsToastPullClicked => {
                self.channels.pending_pull_after_write = true;
                self.channels.pending_install = None;
                self.channels.last_message = None;
                self.channels.post_apply_install_prompt = None;
                self.update(Message::FetchUserCatalogClicked)
            }
            Message::ChannelsToastPullAndInstallClicked(name) => {
                self.channels.pending_pull_after_write = true;
                self.channels.pending_install = Some(name);
                self.channels.last_message = None;
                self.channels.post_apply_install_prompt = None;
                self.update(Message::FetchUserCatalogClicked)
            }
            Message::ChannelsRollbackConfirmed => {
                // Reuse the existing restore async path — same
                // semantics, just triggered from the rollback offer
                // rather than the "Restore last backup" button.
                self.channels.rollback_offer = None;
                self.spawn_channels_restore()
            }
            Message::ChannelsRollbackDismissed => {
                self.channels.rollback_offer = None;
                Task::none()
            }

            Message::InstallRequested(name) => self.start_op(OpKind::Install, move |g| {
                g.package().install(&[name.as_str()])
            }),

            Message::AppMetadataLoaded { name, metadata } => {
                if let Some(m) = metadata.take() {
                    self.metadata_cache.insert(name, Some(m));
                } else {
                    // Carrier already consumed (shouldn't happen) — clear
                    // the in-flight marker so we can retry on re-select.
                    self.metadata_cache.remove(&name);
                }
                Task::none()
            }
            Message::HomeAppClicked(name) => {
                // Switch to Search, queue an exact-name auto-select for
                // the next completion, and feed the query into the
                // existing debounce flow. The user lands on the detail
                // pane for the exact variant they clicked, while still
                // seeing related results in the list.
                self.active_tab = Tab::Search;
                self.search.pending_select = Some(name.clone());
                self.update(Message::SearchInputChanged(name))
            }
            Message::HomeIconLoaded { name, bytes } => {
                let result = bytes.take().unwrap_or(None);
                self.home_icons.insert(name, IconCacheEntry::Done(result));
                Task::none()
            }
            Message::ClearMetadataCacheClicked => {
                // Wipe both in-memory caches alongside the disk wipe so
                // the UI immediately reflects the cleared state rather
                // than continuing to show whatever was loaded this session.
                self.home_icons.clear();
                self.metadata_cache.clear();
                self.system.cache_action_message = Some(crate::t!("system-clearing-cache"));
                // Replacing the client also drops its in-memory Flathub
                // index — otherwise the next fetch would skip the
                // network and miss the chance to pick up a refreshed ID
                // list. New() is cheap; just rebuilds the reqwest pool.
                let new_client = MetadataClient::new().unwrap_or_else(|e| {
                    tracing::warn!(
                        target: "guix_gui",
                        "metadata client rebuild failed ({e}); keeping degraded client"
                    );
                    MetadataClient::degraded()
                });
                let to_clear = self.metadata_client.clone();
                self.metadata_client = new_client;
                Task::perform(
                    async move { to_clear.clear_disk_cache().await },
                    Message::MetadataCacheCleared,
                )
            }
            Message::MetadataCacheCleared(result) => {
                self.system.cache_action_message = Some(match result {
                    Ok(()) => crate::t!("system-cache-cleared"),
                    Err(e) => crate::t!("system-cache-clear-failed", error = e),
                });
                // If we're on Home with metadata enabled, repopulate the
                // tile icons so the cleared state isn't visible as
                // missing thumbnails.
                if self.active_tab == Tab::Home {
                    self.spawn_home_icons_prefetch()
                } else {
                    Task::none()
                }
            }
            Message::AppMetadataEnabledToggled(v) => {
                self.settings.app_metadata.enabled = v;
                let _ = self.settings.save();
                let mut tasks: Vec<Task<Message>> = Vec::new();
                // Re-fetch for the currently-selected result so the
                // panel populates immediately when the user enables it.
                if let Some(i) = self.search.selected {
                    tasks.push(self.spawn_app_metadata_fetch(i));
                }
                if v && self.active_tab == Tab::Home {
                    tasks.push(self.spawn_home_icons_prefetch());
                }
                Task::batch(tasks)
            }
            Message::AppMetadataFlathubToggled(v) => {
                self.settings.app_metadata.use_flathub = v;
                let _ = self.settings.save();
                // Drop cached results so the toggle takes effect on
                // the next selection without a manual refresh.
                self.metadata_cache.clear();
                self.home_icons.clear();
                if v && self.active_tab == Tab::Home {
                    return self.spawn_home_icons_prefetch();
                }
                Task::none()
            }
            Message::AppMetadataDebianToggled(v) => {
                self.settings.app_metadata.use_debian_screenshots = v;
                let _ = self.settings.save();
                self.metadata_cache.clear();
                Task::none()
            }
            Message::LightboxOpened(c) => {
                if let Some(bytes) = c.take() {
                    self.lightbox = Some(bytes);
                }
                Task::none()
            }
            Message::LightboxClosed => {
                self.lightbox = None;
                Task::none()
            }
            Message::OpenUrl(url) => {
                // Parse via `url::Url` so we hand xdg-open a canonical
                // string. Reject non-http(s) schemes and any control
                // bytes that slipped through the package homepage field.
                let parsed = match ::url::Url::parse(&url) {
                    Ok(u) => u,
                    Err(e) => {
                        tracing::warn!(target: "guix_gui", "refusing unparseable url ({e}): {url}");
                        return Task::none();
                    }
                };
                if !matches!(parsed.scheme(), "http" | "https") {
                    tracing::warn!(target: "guix_gui", "refusing non-http url: {url}");
                    return Task::none();
                }
                let canonical = parsed.as_str();
                if canonical.bytes().any(|b| b < 0x20 || b == 0x7f) {
                    tracing::warn!(target: "guix_gui", "refusing url with control bytes: {url}");
                    return Task::none();
                }
                if let Err(e) = std::process::Command::new("xdg-open")
                    .arg(canonical)
                    .spawn()
                {
                    tracing::warn!(target: "guix_gui", "xdg-open failed: {e}");
                }
                Task::none()
            }

            Message::OpStarted {
                id,
                kind,
                slot,
                cancel,
            } => {
                self.terminal.clear();
                self.show_log = self.settings.show_log_by_default;
                self.active_op = Some(ActiveOp {
                    id,
                    kind,
                    cancel: cancel.take(),
                    op_slot: slot,
                    final_code: None,
                    finished: false,
                    bootstrap_likely: false,
                    progress: ProgressSummary::new(kind),
                    channel_shadow_seen: false,
                });
                Task::none()
            }
            Message::OpStartFailed(e) => {
                self.system.validation_message = Some(crate::t!("app-op-start-failed", error = e));
                Task::none()
            }
            Message::Progress(OpEvent::Progress(batch)) => {
                if let Some(op) = self.active_op.as_mut() {
                    for evt in &batch {
                        if let ProgressEvent::ExitSummary { code, .. } = evt {
                            op.final_code = Some(*code);
                        }
                        if let ProgressEvent::KnownBug(KnownBug::ChannelShadow74396) = evt {
                            op.channel_shadow_seen = true;
                        }
                        if !op.bootstrap_likely {
                            if let ProgressEvent::Line {
                                stream: ProgressStream::Stderr,
                                text,
                                ..
                            } = evt
                            {
                                if text.contains(BOOTSTRAP_HINT_PATTERN) {
                                    op.bootstrap_likely = true;
                                }
                            }
                        }
                    }
                    for evt in &batch {
                        if !self.debug_events {
                            if let ProgressEvent::Line { text, .. } = evt {
                                if text.starts_with("[repl-op]") {
                                    continue;
                                }
                            }
                        }
                        self.terminal.feed_event(evt);
                    }
                    for evt in &batch {
                        op.progress.ingest(evt);
                    }
                }
                Task::none()
            }
            Message::ToggleLog => {
                self.show_log = !self.show_log;
                Task::none()
            }
            Message::Tick => Task::none(),
            Message::CopyTerminalClicked => {
                // Capture scrollback + visible rows — rows() alone is just the visible 40.
                let mut payload = self.terminal.scrollback().join("\n");
                if !payload.is_empty() {
                    payload.push('\n');
                }
                payload.push_str(&self.terminal.rows().join("\n"));
                if self.show_bootstrap_help() {
                    payload.push_str("\n\n");
                    payload.push_str(&bootstrap_help_message(
                        self.auto_load_path().as_deref(),
                        self.settings.source_config_path.as_deref(),
                    ));
                }
                iced::clipboard::write::<Message>(payload)
            }
            Message::Progress(OpEvent::Finished) => {
                let refresh_after_pull = matches!(
                    self.active_op.as_ref(),
                    Some(op)
                        if (op.kind == OpKind::Pull || op.kind == OpKind::SystemPull)
                            && op.final_code == Some(0)
                );
                let refresh_after_profile_change = matches!(
                    self.active_op.as_ref(),
                    Some(op)
                        if matches!(op.kind, OpKind::Install | OpKind::Remove | OpKind::Upgrade)
                            && op.final_code == Some(0)
                );
                // Profile-mutating ops invalidate the
                // installed-by-channel cache. We invalidate on any
                // completion (success or failure) of these kinds
                // because a partial run can still leave the profile
                // changed; the next dialog open re-fetches.
                if matches!(
                    self.active_op.as_ref(),
                    Some(op)
                        if matches!(op.kind, OpKind::Install | OpKind::Remove | OpKind::Upgrade)
                ) {
                    self.channels.installed_by_channel = None;
                }
                // Channels-tab continuation: did the pull we just
                // finished originate from a channels-tab edit? Decide
                // BEFORE flipping `op.finished` so we can read the
                // active op's state cleanly.
                let channels_pull_outcome: Option<(bool, bool)> = self
                    .active_op
                    .as_ref()
                    .filter(|op| op.kind == OpKind::Pull && self.channels.pending_pull_after_write)
                    .map(|op| (op.final_code == Some(0), op.channel_shadow_seen));

                if let Some(op) = self.active_op.as_mut() {
                    op.finished = true;
                }

                // Branching policy:
                // 1. Channels-tab pull SUCCESS — clear pending flags, fire
                //    the deferred install if requested, and run the
                //    standard post-pull refresh tasks.
                // 2. Channels-tab pull FAILURE — drop any deferred install
                //    (rollback takes precedence) and stage the rollback
                //    offer with optional bug context. No refresh.
                // 3. Anything else — preserve the pre-existing behaviour.
                if let Some((success, shadow_seen)) = channels_pull_outcome {
                    let pending_install = self.channels.pending_install.take();
                    self.channels.pending_pull_after_write = false;
                    // Clear the post-write toast — it's stale at this
                    // point regardless of outcome.
                    self.channels.last_message = None;
                    self.channels.post_apply_install_prompt = None;
                    if success {
                        let mut tasks: Vec<Task<Message>> = vec![
                            self.spawn_channels_refresh(),
                            self.spawn_pull_mtimes_refresh(),
                        ];
                        if let Some(name) = pending_install {
                            tasks.push(Task::done(Message::InstallRequested(name)));
                        }
                        return Task::batch(tasks);
                    }
                    // Failure path — drop any pending install (handled
                    // by the early `take()`) and stage the rollback CTA.
                    let bug = if shadow_seen {
                        Some(KnownBug::ChannelShadow74396)
                    } else {
                        None
                    };
                    self.channels.rollback_offer = Some(RollbackOffer {
                        backup_path: self.channels.backup_path.clone(),
                        bug,
                    });
                    return Task::none();
                }

                if refresh_after_pull {
                    Task::batch([
                        self.spawn_channels_refresh(),
                        self.spawn_pull_mtimes_refresh(),
                    ])
                } else if refresh_after_profile_change {
                    self.spawn_installed_refresh()
                } else {
                    Task::none()
                }
            }
            Message::CancelClicked => {
                if let Some(op) = self.active_op.as_mut() {
                    if op_supports_cancel(op.kind) {
                        if let Some(cancel) = op.cancel.take() {
                            return Task::perform(
                                async move {
                                    let _ = cancel.cancel().await;
                                },
                                |()| Message::Progress(OpEvent::Finished),
                            );
                        }
                    }
                }
                Task::none()
            }
            Message::DismissOverlay => {
                self.active_op = None;
                self.terminal.clear();
                Task::none()
            }
        }
    }

    fn spawn_installed_refresh(&mut self) -> Task<Message> {
        let Some(g) = self.guix.clone() else {
            return Task::none();
        };
        self.installed.refreshing = true;
        self.installed.error = None;
        Task::perform(
            async move {
                g.package()
                    .list_installed()
                    .await
                    .map_err(|e| format!("{e}"))
            },
            Message::InstalledLoaded,
        )
    }

    fn spawn_channels_refresh(&mut self) -> Task<Message> {
        let Some(g) = self.guix.clone() else {
            return Task::none();
        };
        self.updates.loading_channels = true;
        self.updates.error = None;
        Task::perform(
            async move { g.describe().channels().await.map_err(|e| format!("{e}")) },
            Message::ChannelsLoaded,
        )
    }

    fn spawn_pull_mtimes_refresh(&mut self) -> Task<Message> {
        Task::perform(
            async {
                async fn stat(p: std::path::PathBuf) -> Option<std::time::SystemTime> {
                    tokio::fs::symlink_metadata(&p).await.ok()?.modified().ok()
                }
                let (user_pull, root_pull, system_profile) = tokio::join!(
                    stat(PullOps::user_path()),
                    stat(PullOps::root_path()),
                    stat(libguix::Guix::system_profile_path()),
                );
                PullMtimes {
                    user_pull,
                    root_pull,
                    system_profile,
                }
            },
            Message::PullMtimesLoaded,
        )
    }

    /// Kick off an icon+screenshot fetch for the selected search result,
    /// unless the feature is off, the cache already has it, or the index
    /// is out of range. In-flight requests are marked with `None` so
    /// rapid selection changes don't queue duplicate fetches.
    fn spawn_app_metadata_fetch(&mut self, index: usize) -> Task<Message> {
        if !self.settings.app_metadata.enabled {
            return Task::none();
        }
        let Some(p) = self.search.results.get(index) else {
            return Task::none();
        };
        let name = p.name.clone();
        if self.metadata_cache.contains_key(&name) {
            return Task::none();
        }
        self.metadata_cache.insert(name.clone(), None);
        let client = self.metadata_client.clone();
        let cfg = self.settings.app_metadata.clone();
        let fetch_name = name.clone();
        Task::perform(
            async move {
                let m = client.fetch(&fetch_name, cfg).await;
                Carrier::new(m)
            },
            move |metadata| Message::AppMetadataLoaded {
                name: name.clone(),
                metadata,
            },
        )
    }

    /// Fan out icon fetches for the curated Home list. No-op when app
    /// metadata is disabled, Flathub is the icon source disabled, or
    /// `guix` hasn't been discovered yet (icons don't need guix, but the
    /// Home tab doesn't render before discovery completes either).
    fn spawn_home_icons_prefetch(&mut self) -> Task<Message> {
        if !self.settings.app_metadata.enabled || !self.settings.app_metadata.use_flathub {
            return Task::none();
        }
        let mut tasks: Vec<Task<Message>> = Vec::new();
        for ra in RECOMMENDED {
            if self.home_icons.contains_key(ra.name) {
                continue;
            }
            self.home_icons
                .insert(ra.name.to_string(), IconCacheEntry::Loading);
            let client = self.metadata_client.clone();
            let name_for_fetch = ra.name.to_string();
            let name_for_msg = ra.name.to_string();
            tasks.push(Task::perform(
                async move {
                    let bytes = client.fetch_icon(&name_for_fetch).await;
                    Carrier::new(bytes)
                },
                move |bytes| Message::HomeIconLoaded {
                    name: name_for_msg.clone(),
                    bytes,
                },
            ));
        }
        Task::batch(tasks)
    }

    fn spawn_channels_file_load(&mut self) -> Task<Message> {
        self.channels.loading = true;
        self.channels.error = None;
        let path_override = self.settings.channels_source_path.clone();
        Task::perform(
            async move { load_channels_file(path_override).await },
            Message::ChannelsFileLoaded,
        )
    }

    /// Best-effort introspection over the user's profile manifest,
    /// bucketing installed packages by source channel. Cache is held
    /// in `ChannelsState::installed_by_channel`; failures resolve to
    /// an empty map so the next open doesn't retry on every paint.
    fn spawn_installed_by_channel_fetch(&mut self) -> Task<Message> {
        let Some(g) = self.guix.clone() else {
            return Task::none();
        };
        if self.channels.installed_by_channel_loading {
            return Task::none();
        }
        self.channels.installed_by_channel_loading = true;
        Task::perform(
            async move { g.installed().by_channel().await.map_err(|e| format!("{e}")) },
            Message::ChannelsInstalledByChannelLoaded,
        )
    }

    /// Pipeline: pre-flight via `ChannelsFile::apply` (which calls the
    /// REPL helper), `validate` the resulting source, then `write_atomic`.
    /// On success, re-read the file so the parsed view stays authoritative.
    fn spawn_channels_apply(&mut self, op: ChannelOp) -> Task<Message> {
        let Some(g) = self.guix.clone() else {
            return Task::none();
        };
        let Some(file) = self.channels.file.clone() else {
            self.channels.error = Some(crate::t!("channels-no-file-loaded"));
            return Task::none();
        };
        if !file.is_writable() {
            self.channels.error = Some(crate::t!(
                "channels-store-managed-error",
                path = file.path.display().to_string()
            ));
            return Task::none();
        }
        self.channels.saving = true;
        self.channels.error = None;
        self.channels.last_message = None;
        let path_override = self.settings.channels_source_path.clone();
        Task::perform(
            async move {
                let repl = g.repl().await.map_err(|e| format!("{e}"))?;
                let new_source = file
                    .apply(&repl, op)
                    .await
                    .map_err(|e| describe_channels_error(&e))?;
                ChannelsFile::validate(&repl, &new_source)
                    .await
                    .map_err(|e| describe_channels_error(&e))?;
                file.write_atomic(&new_source)
                    .await
                    .map_err(|e| describe_channels_error(&e))?;
                // Re-read so we get the canonical parsed view (the new
                // source is what we just wrote, but rereading also picks
                // up any normalisation the pretty-printer applied).
                outcome_to_result(load_channels_file(path_override).await)
            },
            Message::ChannelsApplyCompleted,
        )
    }

    /// Copies the cached `.bak` over the active file, then re-reads. The
    /// `.bak` is intentionally left in place — keeping the
    /// previous-previous version means a second "Restore" press won't
    /// silently no-op. See TODO.md "channels UX polish".
    fn spawn_channels_restore(&mut self) -> Task<Message> {
        let Some(file) = self.channels.file.clone() else {
            self.channels.error = Some(crate::t!("channels-no-file-loaded"));
            return Task::none();
        };
        let Some(bak) = self.channels.backup_path.clone() else {
            self.channels.error = Some(crate::t!("channels-no-backup"));
            return Task::none();
        };
        if !file.is_writable() {
            self.channels.error = Some(crate::t!(
                "channels-store-managed-error",
                path = file.path.display().to_string()
            ));
            return Task::none();
        }
        self.channels.saving = true;
        self.channels.error = None;
        self.channels.last_message = None;
        let path = file.path.clone();
        let path_override = self.settings.channels_source_path.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || -> std::io::Result<()> {
                    let content = std::fs::read(&bak)?;
                    let tmp = path.with_extension("scm.restore-tmp");
                    std::fs::write(&tmp, &content)?;
                    std::fs::rename(&tmp, &path)?;
                    Ok(())
                })
                .await
                .map_err(|e| crate::t!("app-restore-panicked", detail = format!("{e}")))?
                .map_err(|e| crate::t!("app-restore-failed", detail = format!("{e}")))?;
                outcome_to_result(load_channels_file(path_override).await)
            },
            Message::ChannelsRestoreCompleted,
        )
    }

    /// Lazily construct the discovery client. Only ever called from
    /// inside a Discover-sub-mode code path (guarded upstream by
    /// `settings.discovery_enabled`), so when the toggle is off this
    /// method is unreachable and no `Discovery` allocation happens.
    ///
    /// Returns `None` when `reqwest::Client::builder()` fails — the
    /// caller stashes the error in `discover_error` and the sub-mode
    /// renders the failure inline. Previous behaviour silently fell
    /// back to a default client without UA/timeout, which is the worst
    /// possible outcome in a constrained sandbox.
    fn ensure_discovery(&mut self) -> Option<Discovery> {
        if let Some(d) = self.channels.discovery.clone() {
            return Some(d);
        }
        match Discovery::new() {
            Ok(d) => {
                self.channels.discovery = Some(d.clone());
                Some(d)
            }
            Err(e) => {
                self.channels.discover_error = Some(crate::t!(
                    "app-discovery-client-failed",
                    detail = format!("{e}")
                ));
                None
            }
        }
    }

    fn spawn_discover_channels_fetch(&mut self) -> Task<Message> {
        self.channels.discover_channels_loading = true;
        self.channels.discover_error = None;
        let Some(client) = self.ensure_discovery() else {
            self.channels.discover_channels_loading = false;
            return Task::none();
        };
        Task::perform(
            async move {
                client
                    .channels()
                    .await
                    .map(Carrier::new)
                    .map_err(|e: DiscoveryError| e.to_string())
            },
            Message::DiscoverChannelsLoaded,
        )
    }

    fn spawn_system_load(&self) -> Task<Message> {
        let Some(g) = self.guix.clone() else {
            return Task::none();
        };
        Task::perform(
            async move {
                let sys = g.system();
                sys.current_configuration_path()
                    .map(|p| p.display().to_string())
                    .map_err(|e| format!("{e}"))
            },
            Message::SystemConfigChecked,
        )
    }

    fn start_op<F>(&mut self, kind: OpKind, build: F) -> Task<Message>
    where
        F: FnOnce(&Guix) -> Result<Operation, GuixError> + Send + 'static,
    {
        if self.active_op.is_some() {
            return Task::none();
        }
        let Some(g) = self.guix.clone() else {
            return Task::none();
        };
        let id = OpId::next();
        Task::perform(
            async move {
                match build(&g) {
                    Ok(mut op) => {
                        let cancel = op.take_cancel();
                        let slot = SharedOp::new(op);
                        let cancel = cancel.map_or_else(Carrier::empty, Carrier::new);
                        Ok((id, kind, slot, cancel))
                    }
                    Err(e) => Err(format!("{e}")),
                }
            },
            move |r| match r {
                Ok((id, kind, slot, cancel)) => Message::OpStarted {
                    id,
                    kind,
                    slot,
                    cancel,
                },
                Err(e) => Message::OpStartFailed(e),
            },
        )
    }

    pub(crate) fn auto_load_path(&self) -> Option<PathBuf> {
        let cfg = self.settings.source_config_path.as_ref()?;
        let parent = cfg.parent()?;
        if parent.as_os_str().is_empty() {
            None
        } else {
            Some(parent.to_path_buf())
        }
    }

    pub(crate) fn show_bootstrap_help(&self) -> bool {
        match &self.active_op {
            Some(op) => {
                op.finished
                    && op.kind == OpKind::Reconfigure
                    && op.bootstrap_likely
                    && op.final_code != Some(0)
            }
            None => false,
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        if let Some(err) = &self.discovery_error {
            return container(
                column![
                    text(crate::t!("app-discover-failed")).size(24),
                    text(err.clone()).size(14),
                ]
                .spacing(12)
                .padding(24)
                .max_width(720),
            )
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into();
        }
        if self.guix.is_none() {
            return container(text(crate::t!("app-discovering")).size(20))
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into();
        }

        let sidebar = self.view_sidebar();
        let body: Element<'_, Message> = match self.active_tab {
            Tab::Home => home::view(self),
            Tab::Search => search::view(self),
            Tab::Installed => installed::view(self),
            Tab::Updates => updates::view(self),
            Tab::Channels => channels_view::view(self),
            Tab::System => system::view(self),
            Tab::About => about::view(self),
        };
        let body = container(body)
            .padding(24)
            .width(Length::Fill)
            .height(Length::Fill);

        let main: Element<'_, Message> = row![sidebar, body]
            .width(Length::Fill)
            .height(Length::Fill)
            .into();

        if self.active_op.is_some() {
            self.view_overlay()
        } else if let Some(bytes) = &self.lightbox {
            self.view_lightbox(bytes)
        } else {
            main
        }
    }

    fn view_lightbox<'a>(&'a self, bytes: &'a [u8]) -> Element<'a, Message> {
        use iced::widget::image as iced_image;
        if !crate::app_metadata::is_supported_image(bytes) {
            let close_btn = button(text(crate::t!("app-lightbox-close")).size(13))
                .padding([8, 16])
                .style(styles::btn_secondary)
                .on_press(Message::LightboxClosed);
            let header = row![Space::new().width(Length::Fill), close_btn].padding(12);
            let msg = container(text(crate::t!("app-lightbox-no-image")).size(14))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill);
            return container(column![header, msg].spacing(0))
                .width(Length::Fill)
                .height(Length::Fill)
                .style(styles::card)
                .into();
        }
        let handle = iced_image::Handle::from_bytes(bytes.to_vec());
        let img = iced_image(handle)
            .width(Length::Fill)
            .height(Length::Fill)
            .content_fit(iced::ContentFit::Contain);

        let close_btn = button(text(crate::t!("app-lightbox-close")).size(13))
            .padding([8, 16])
            .style(styles::btn_secondary)
            .on_press(Message::LightboxClosed);

        let header = row![Space::new().width(Length::Fill), close_btn,].padding(12);

        // Click-anywhere-on-background to dismiss: wrap the image in a
        // button styled as a ghost so it doesn't draw a frame, then a
        // click outside the image margins fires LightboxClosed.
        let dismiss_layer = button(container(img).width(Length::Fill).height(Length::Fill))
            .padding(24)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(styles::btn_ghost)
            .on_press(Message::LightboxClosed);

        container(column![header, dismiss_layer].spacing(0))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(styles::card)
            .into()
    }

    fn view_sidebar(&self) -> Element<'_, Message> {
        let nav_btn = |icon: &'static str, tab: Tab| -> Element<'_, Message> {
            let active = tab == self.active_tab;
            button(
                row![
                    text(icon).size(16).width(Length::Fixed(24.0)),
                    text(tab.label()).size(14),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .width(Length::Fill)
            .padding([10, 14])
            .style(styles::nav_btn(active))
            .on_press(Message::TabSelected(tab))
            .into()
        };

        // Primary nav — top-aligned.
        let primary = column![
            nav_btn("\u{1F3E0}", Tab::Home),
            nav_btn("\u{1F50D}", Tab::Search),
            nav_btn("\u{1F4E6}", Tab::Installed),
            nav_btn("\u{2191}", Tab::Updates),
            nav_btn("\u{1F4E1}", Tab::Channels),
        ]
        .spacing(2);

        // Brand mark — same isometric package icon as the README logo.
        let icon = svg(svg::Handle::from_memory(
            include_bytes!("../../assets/icon.svg").as_slice(),
        ))
        .width(Length::Fixed(32.0))
        .height(Length::Fixed(32.0));
        let brand = container(
            row![
                icon,
                text(crate::t!("app-brand"))
                    .size(18)
                    .font(styles::BOLD)
                    .color(TEXT),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        )
        .padding(iced::Padding {
            top: 8.0,
            right: 14.0,
            bottom: 16.0,
            left: 6.0,
        });

        let settings = nav_btn("\u{2699}", Tab::System);
        let about = nav_btn("\u{2139}", Tab::About);

        let col = column![
            brand,
            primary,
            Space::new().height(Length::Fill),
            styles::separator(),
            Space::new().height(Length::Fixed(8.0)),
            settings,
            about,
        ]
        .spacing(2)
        .padding(12)
        .width(Length::Fixed(210.0));

        container(col)
            .height(Length::Fill)
            .style(styles::sidebar)
            .into()
    }

    /// Reusable page header — title on the left, optional action(s) on the
    /// right. Views call this to keep the header treatment consistent.
    pub(crate) fn view_header<'a>(
        title: impl Into<String>,
        action: Option<Element<'a, Message>>,
    ) -> Element<'a, Message> {
        let title_widget = text(title.into()).size(24).color(TEXT);
        let mut header_row = row![title_widget, Space::new().width(Length::Fill)]
            .align_y(Alignment::Center)
            .spacing(8);
        if let Some(a) = action {
            header_row = header_row.push(a);
        }
        column![header_row, Space::new().height(Length::Fixed(8.0))]
            .width(Length::Fill)
            .into()
    }

    fn view_overlay(&self) -> Element<'_, Message> {
        let op = self.active_op.as_ref().expect("active_op present");
        if self.show_log {
            self.view_overlay_log(op)
        } else {
            crate::views::progress::view(self, op)
        }
    }

    fn view_overlay_log<'a>(&'a self, op: &'a ActiveOp) -> Element<'a, Message> {
        let title = crate::t!("app-op-title", label = op.kind.label(), id = op.id.0);

        let mut log_lines: Column<'_, Message> = Column::new().spacing(0);
        for row in self.terminal.rows() {
            let line: Element<'_, Message> = text(row).size(12).font(Font::MONOSPACE).into();
            log_lines = log_lines.push(line);
        }
        let log_scroll = scrollable(log_lines).height(Length::Fill);

        let mut footer = row![].spacing(8);
        if !op.finished {
            let supports_cancel = op_supports_cancel(op.kind);
            let on_press: Option<Message> = if supports_cancel && op.cancel.is_some() {
                Some(Message::CancelClicked)
            } else {
                None
            };
            let cancel_btn = button(text(crate::t!("common-cancel"))).on_press_maybe(on_press);
            let cancel_element: Element<'_, Message> = if supports_cancel {
                cancel_btn.into()
            } else {
                tooltip(
                    cancel_btn,
                    container(text(crate::t!("app-cancel-pkexec-tooltip")))
                        .padding(6)
                        .style(container::rounded_box),
                    tooltip::Position::Top,
                )
                .into()
            };
            footer = footer
                .push(cancel_element)
                .push(text(crate::t!("app-running")));
        } else if self.show_bootstrap_help() {
            // Iced's default text shaper swallows `\n` — render per-line via Column.
            let help = bootstrap_help_message(
                self.auto_load_path().as_deref(),
                self.settings.source_config_path.as_deref(),
            );
            let mut help_col: Column<'_, Message> = Column::new().spacing(2);
            for ln in help.lines() {
                let line: Element<'_, Message> =
                    text(ln.to_string()).size(12).font(Font::MONOSPACE).into();
                help_col = help_col.push(line);
            }
            footer = footer
                .push(button(text(crate::t!("common-close"))).on_press(Message::DismissOverlay))
                .push(help_col);
        } else {
            let summary = match op.final_code {
                Some(0) => crate::t!("app-done"),
                Some(code) => crate::t!("app-failed-exit", code = code),
                None => crate::t!("app-ended-no-summary"),
            };
            footer = footer
                .push(button(text(crate::t!("common-close"))).on_press(Message::DismissOverlay))
                .push(text(summary));
        }
        let log_label = if self.show_log {
            crate::t!("app-hide-log")
        } else {
            crate::t!("app-show-log")
        };
        footer = footer
            .push(Space::new().width(Length::Fill))
            .push(button(text(crate::t!("app-copy"))).on_press(Message::CopyTerminalClicked))
            .push(button(text(log_label)).on_press(Message::ToggleLog));

        container(
            column![text(title).size(20), log_scroll, footer]
                .spacing(10)
                .padding(16),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
}

pub(crate) fn op_supports_cancel(kind: OpKind) -> bool {
    !matches!(kind, OpKind::SystemPull | OpKind::Reconfigure)
}

/// Returns `true` when the Channels tab should expand the row's
/// confirmation into the warning dialog (channel name + affected
/// package list + "Remove channel anyway" / Cancel). The dialog
/// surfaces ONLY when:
///
/// - the user has armed Remove on this exact channel
///   (`pending_remove == Some(ch_name)`), AND
/// - the introspection cache has loaded (`Some`), AND
/// - the cache reports at least one installed package sourced from
///   this channel.
///
/// All other states — different channel armed, cache not loaded
/// (`None`), or cache loaded but empty for this channel — return
/// `false`, and the existing minimal inline Confirm/Cancel renders
/// instead. The dialog never blocks Remove; the cache miss path is a
/// deliberate fallthrough so a slow introspection fetch can't lock
/// the user out of removing a channel.
#[must_use]
pub fn should_render_remove_warning(
    pending_remove: Option<&str>,
    installed_by_channel: Option<&HashMap<String, Vec<InstalledPackage>>>,
    channel_name: &str,
) -> bool {
    if pending_remove != Some(channel_name) {
        return false;
    }
    let Some(map) = installed_by_channel else {
        return false;
    };
    map.get(channel_name).is_some_and(|v| !v.is_empty())
}

const SEARCH_ERROR_SUMMARY_CAP: usize = 200;

pub(crate) fn build_search_error(details: String) -> SearchError {
    let summary = details
        .lines()
        .find(|l| !l.trim().is_empty())
        .map(|l| {
            l.strip_prefix("repl protocol error: ")
                .unwrap_or(l)
                .to_string()
        })
        .unwrap_or_else(|| crate::t!("search-failed"));
    let summary = if summary.chars().count() > SEARCH_ERROR_SUMMARY_CAP {
        let mut s: String = summary.chars().take(SEARCH_ERROR_SUMMARY_CAP - 1).collect();
        s.push('\u{2026}');
        s
    } else {
        summary
    };
    SearchError { summary, details }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_ops_support_cancel() {
        for k in [
            OpKind::Install,
            OpKind::Remove,
            OpKind::Upgrade,
            OpKind::Pull,
        ] {
            assert!(op_supports_cancel(k), "{k:?} must support cancel");
        }
    }

    #[test]
    fn pkexec_ops_do_not_support_cancel() {
        assert!(!op_supports_cancel(OpKind::SystemPull));
        assert!(!op_supports_cancel(OpKind::Reconfigure));
    }

    /// Bootstrap detection: a stderr line containing the
    /// `no code for module` substring trips the flag, and the rendered
    /// help message contains the auto-derived load path and the source
    /// config path on the suggested `sudo guix system reconfigure` line.
    #[test]
    fn bootstrap_detection_matches_unbound_module_error() {
        let sample_stderr = "guix system: error: no code for module (px packages libguix)";
        assert!(sample_stderr.contains(BOOTSTRAP_HINT_PATTERN));

        let auto = PathBuf::from("/home/franz/dotfiles/system");
        let cfg = PathBuf::from("/home/franz/dotfiles/system/framework.scm");
        let help = bootstrap_help_message(Some(&auto), Some(&cfg));

        assert!(help.contains("Reconfigure failed"));
        assert!(help.contains("doesn't recognise"));
        assert!(help.contains("sudo guix system reconfigure -L"));
        assert!(help.contains("/home/franz/dotfiles/system"));
        assert!(help.contains("/home/franz/dotfiles/system/framework.scm"));
    }

    /// Without an auto load path or source config, the help message
    /// still renders with placeholder tokens so the command shape stays
    /// readable.
    #[test]
    fn bootstrap_help_renders_placeholders_without_settings() {
        let help = bootstrap_help_message(None, None);
        assert!(help.contains("<parent of config>"));
        assert!(help.contains("<set source config>"));
    }

    /// While an op is running and unfinished, `subscription()` must
    /// return a non-empty (`!= Subscription::none()`) value so the 1Hz
    /// Tick is wired alongside the operation_subscription. We can't
    /// peek inside iced's `Subscription`; the best we can do is assert
    /// that it isn't the no-op variant, which iced exposes via its
    /// `Recipe`-collection internals — instead, we just exercise the
    /// code path and assert the function returns without panic.
    #[test]
    fn tick_subscription_active_during_running_op() {
        let (mut app, _) = App::new();
        app.active_op = Some(ActiveOp {
            id: crate::operation_subscription::OpId(1),
            kind: crate::operation_subscription::OpKind::Install,
            cancel: None,
            op_slot: crate::operation_subscription::SharedOp::new_empty_for_tests(),
            final_code: None,
            finished: false,
            bootstrap_likely: false,
            progress: ProgressSummary::new(crate::operation_subscription::OpKind::Install),
            channel_shadow_seen: false,
        });
        let _sub = app.subscription();
    }

    /// With debug_events off, an unmapped `[repl-op]` Line gets filtered
    /// before reaching the terminal buffer while typed events (here,
    /// the format_event path for SubstituteDownload) still come through.
    #[test]
    fn repl_op_line_filtered_when_debug_off() {
        use crate::operation_subscription::{OpEvent, OpId, OpKind, SharedOp};
        let (mut app, _) = App::new();
        app.debug_events = false;
        app.active_op = Some(ActiveOp {
            id: OpId(1),
            kind: OpKind::Pull,
            cancel: None,
            op_slot: SharedOp::new_empty_for_tests(),
            final_code: None,
            finished: false,
            bootstrap_likely: false,
            progress: ProgressSummary::new(OpKind::Pull),
            channel_shadow_seen: false,
        });

        let batch = vec![
            ProgressEvent::Line {
                stream: ProgressStream::Stderr,
                text: "[repl-op] (some-unknown-tag x)".into(),
                redraw: false,
            },
            ProgressEvent::BuildStart {
                drv: "/gnu/store/abc-foo.drv".into(),
            },
        ];
        let _ = app.update(Message::Progress(OpEvent::Progress(batch)));

        let rows = app.terminal.rows();
        let joined = rows.join("\n");
        assert!(
            !joined.contains("[repl-op]"),
            "repl-op marker leaked through with debug off: {joined}"
        );
        assert!(
            joined.contains("building /gnu/store/abc-foo.drv"),
            "BuildStart didn't reach the terminal buffer: {joined}"
        );
    }

    /// Regression for the idle-SIGINT-kills-repl bug: a stale
    /// `SearchCompleted` (one whose `seq` doesn't match the current
    /// `query_seq`) must still clear `searching`, otherwise the next
    /// `SearchInputChanged` will call `repl.interrupt()` against an
    /// already-idle repl. Before the in-eval guard landed in
    /// `libguix::repl::actor`, that killed the `guix repl`
    /// subprocess.
    #[test]
    fn stale_search_reply_clears_searching_flag() {
        use crate::carrier::Carrier;
        use libguix::SearchFastResult;

        let (mut app, _) = App::new();
        app.search.searching = true;
        app.search.query_seq = 5;

        let stale = Message::SearchCompleted {
            seq: 3,
            result: Ok(Carrier::new(SearchFastResult {
                results: Vec::new(),
                truncated: false,
                limit: 0,
            })),
        };
        let _ = app.update(stale);

        assert!(
            !app.search.searching,
            "stale SearchCompleted must clear `searching` so a \
             subsequent SearchInputChanged doesn't fire interrupt() \
             against an idle repl"
        );
        assert!(app.search.results.is_empty());
    }

    /// Pre-warmup keystrokes must not fire `repl.interrupt()`: SIGINT
    /// during the initial `(gnu packages …)` module expansion can leave
    /// modules half-loaded, producing cascading unbound-variable errors
    /// on subsequent searches. With `warmup_done == false`, the
    /// interrupt branch is skipped and we just return the debounce
    /// task. No `Guix` is needed — `repl_if_ready()` would return
    /// `None` anyway in tests; this asserts the gate short-circuits
    /// before reaching that check.
    #[test]
    fn interrupt_skipped_when_warmup_incomplete() {
        let (mut app, _) = App::new();
        app.warmup_done = false;
        app.search.searching = true;

        let prev_seq = app.search.query_seq;
        let _task = app.update(Message::SearchInputChanged("foo".into()));

        assert!(app.search.searching, "searching flag must persist");
        assert_eq!(app.search.query, "foo");
        assert_eq!(
            app.search.query_seq,
            prev_seq.wrapping_add(1),
            "seq must advance regardless of the interrupt gate"
        );
    }

    /// Summary picks the first non-empty line and strips the literal
    /// `repl protocol error: ` prefix; details retain the full string.
    #[test]
    fn search_error_summary_picks_first_line_strips_prefix() {
        let raw = "\n\nrepl protocol error: unbound variable: foo\n\
                   stderr tail:\n  line a\n  line b\n";
        let err = build_search_error(raw.to_string());
        assert_eq!(err.summary, "unbound variable: foo");
        assert_eq!(err.details, raw);
    }

    /// Single-line monstrosities get capped at SEARCH_ERROR_SUMMARY_CAP
    /// chars with an ellipsis appended.
    #[test]
    fn search_error_summary_truncates_long_text() {
        let long_line: String = "x".repeat(SEARCH_ERROR_SUMMARY_CAP + 50);
        let err = build_search_error(long_line.clone());
        assert_eq!(err.summary.chars().count(), SEARCH_ERROR_SUMMARY_CAP);
        assert!(err.summary.ends_with('\u{2026}'));
        assert_eq!(err.details, long_line);
    }

    /// And with debug_events on, the `[repl-op]` line is preserved.
    #[test]
    fn repl_op_line_visible_when_debug_on() {
        use crate::operation_subscription::{OpEvent, OpId, OpKind, SharedOp};
        let (mut app, _) = App::new();
        app.debug_events = true;
        app.active_op = Some(ActiveOp {
            id: OpId(1),
            kind: OpKind::Pull,
            cancel: None,
            op_slot: SharedOp::new_empty_for_tests(),
            final_code: None,
            finished: false,
            bootstrap_likely: false,
            progress: ProgressSummary::new(OpKind::Pull),
            channel_shadow_seen: false,
        });
        let batch = vec![ProgressEvent::Line {
            stream: ProgressStream::Stderr,
            text: "[repl-op] (some-unknown-tag x)".into(),
            redraw: false,
        }];
        let _ = app.update(Message::Progress(OpEvent::Progress(batch)));
        let joined = app.terminal.rows().join("\n");
        assert!(joined.contains("[repl-op]"), "rows: {joined}");
    }

    // --- Phase 3a — pull integration polish --------------------------
    //
    // The async pull path itself can't run offline (no `guix` actor),
    // so these tests exercise only the in-memory state machine the
    // Channels tab depends on: the pending-install pairing, the
    // pending-pull-after-write flag, and the rollback-offer assembly.
    // They don't touch `start_op` — instead they pre-populate
    // `active_op` to mimic the moment the pull subscription finishes.

    use crate::operation_subscription::{OpEvent, OpId, OpKind, SharedOp};

    /// Helper: build a finished-pull `ActiveOp` with a specific exit
    /// code and bug flag. Mirrors what `Message::Progress` would have
    /// produced by the time `Finished` arrives.
    fn finished_pull_op(final_code: Option<i32>, channel_shadow_seen: bool) -> ActiveOp {
        ActiveOp {
            id: OpId(1),
            kind: OpKind::Pull,
            cancel: None,
            op_slot: SharedOp::new_empty_for_tests(),
            final_code,
            finished: false,
            bootstrap_likely: false,
            progress: ProgressSummary::new(OpKind::Pull),
            channel_shadow_seen,
        }
    }

    /// Progress(KnownBug) flips the sticky `channel_shadow_seen` flag.
    /// Subsequent unrelated events don't clear it.
    #[test]
    fn channel_shadow_event_sets_sticky_flag() {
        let (mut app, _) = App::new();
        app.active_op = Some(finished_pull_op(None, false));
        let batch = vec![
            ProgressEvent::KnownBug(KnownBug::ChannelShadow74396),
            ProgressEvent::Line {
                stream: ProgressStream::Stderr,
                text: "ordinary line".into(),
                redraw: false,
            },
        ];
        let _ = app.update(Message::Progress(OpEvent::Progress(batch)));
        assert!(app.active_op.as_ref().unwrap().channel_shadow_seen);
    }

    /// Channels-tab pull succeeds with no pending install → pending
    /// flags clear, no rollback offer is raised.
    #[test]
    fn channels_pull_success_clears_pending_and_no_rollback() {
        let (mut app, _) = App::new();
        app.channels.pending_pull_after_write = true;
        app.channels.last_message = Some("toast".into());
        app.active_op = Some(finished_pull_op(Some(0), false));
        let _ = app.update(Message::Progress(OpEvent::Finished));
        assert!(!app.channels.pending_pull_after_write);
        assert!(app.channels.pending_install.is_none());
        assert!(app.channels.rollback_offer.is_none());
        assert!(app.channels.last_message.is_none());
    }

    /// Channels-tab pull succeeds with a pending install → an
    /// `InstallRequested` is queued and the pending state clears.
    #[test]
    fn channels_pull_success_fires_install_when_pending() {
        let (mut app, _) = App::new();
        app.channels.pending_pull_after_write = true;
        app.channels.pending_install = Some("nyxt".into());
        app.active_op = Some(finished_pull_op(Some(0), false));
        // We can't introspect the returned `Task` directly, but we can
        // assert the state mutations the handler is responsible for.
        let _ = app.update(Message::Progress(OpEvent::Finished));
        assert!(!app.channels.pending_pull_after_write);
        // `pending_install` is consumed by the handler — the install
        // message is queued via `Task::done`.
        assert!(app.channels.pending_install.is_none());
        assert!(app.channels.rollback_offer.is_none());
    }

    /// Channels-tab pull fails → the rollback offer is staged with the
    /// captured backup path and no install fires even if one was
    /// pending. The offer carries no bug context when the channel-
    /// shadow detector didn't trip.
    #[test]
    fn channels_pull_failure_stages_rollback_without_bug() {
        let (mut app, _) = App::new();
        app.channels.pending_pull_after_write = true;
        app.channels.pending_install = Some("emacs".into());
        app.channels.backup_path = Some(PathBuf::from("/tmp/channels.scm.bak"));
        app.active_op = Some(finished_pull_op(Some(1), false));
        let _ = app.update(Message::Progress(OpEvent::Finished));
        assert!(!app.channels.pending_pull_after_write);
        assert!(app.channels.pending_install.is_none());
        let offer = app.channels.rollback_offer.as_ref().expect("rollback");
        assert_eq!(
            offer.backup_path.as_deref(),
            Some(Path::new("/tmp/channels.scm.bak"))
        );
        assert!(offer.bug.is_none());
    }

    /// Channels-tab pull fails AND a channel-shadow bug was observed →
    /// the rollback offer carries the bug for the CTA to surface.
    #[test]
    fn channels_pull_failure_surfaces_channel_shadow_bug() {
        let (mut app, _) = App::new();
        app.channels.pending_pull_after_write = true;
        app.channels.backup_path = Some(PathBuf::from("/tmp/channels.scm.bak"));
        app.active_op = Some(finished_pull_op(Some(1), true));
        let _ = app.update(Message::Progress(OpEvent::Finished));
        let offer = app.channels.rollback_offer.as_ref().expect("rollback");
        assert_eq!(offer.bug, Some(KnownBug::ChannelShadow74396));
    }

    /// Pull completions NOT originating from a channels-tab edit must
    /// not raise the rollback offer, even on failure.
    #[test]
    fn non_channels_pull_failure_does_not_stage_rollback() {
        let (mut app, _) = App::new();
        app.channels.pending_pull_after_write = false;
        app.active_op = Some(finished_pull_op(Some(1), true));
        let _ = app.update(Message::Progress(OpEvent::Finished));
        assert!(app.channels.rollback_offer.is_none());
    }

    /// Clicking "Add channel & install" stashes both the channel
    /// pending-add AND the package-name pairing. Plain "Add" clears
    /// the pairing.
    #[test]
    fn discover_add_and_install_records_package_name() {
        let (mut app, _) = App::new();
        let ch = Channel {
            name: "nonguix".into(),
            url: "https://gitlab.com/nonguix/nonguix".into(),
            branch: None,
            commit: None,
            introduction_commit: Some("abc".into()),
            introduction_fingerprint: Some("DEAD BEEF".into()),
        };
        let _ = app.update(Message::DiscoverAddAndInstallClicked(
            Carrier::new(ch.clone()),
            "steam".into(),
        ));
        assert_eq!(
            app.channels.discover_pending_add.as_ref().map(|c| &c.name),
            Some(&"nonguix".to_string())
        );
        assert_eq!(
            app.channels.discover_pending_install.as_deref(),
            Some("steam")
        );

        // Plain Add (e.g. from the channel-row CTA) clears the pairing.
        let _ = app.update(Message::DiscoverAddClicked(Carrier::new(ch)));
        assert!(app.channels.discover_pending_install.is_none());
    }

    /// "Pull, then install <pkg>" toast click flips both flags and
    /// fires the existing user-pull message.
    #[test]
    fn toast_pull_and_install_sets_state() {
        let (mut app, _) = App::new();
        // Without `guix`, FetchUserCatalogClicked is a no-op for the
        // op start, but the channels state mutation still runs.
        let _ = app.update(Message::ChannelsToastPullAndInstallClicked("emacs".into()));
        assert!(app.channels.pending_pull_after_write);
        assert_eq!(app.channels.pending_install.as_deref(), Some("emacs"));
    }

    /// "Pull only" toast click sets the after-write flag but leaves
    /// `pending_install` empty.
    #[test]
    fn toast_pull_only_does_not_set_install() {
        let (mut app, _) = App::new();
        app.channels.pending_install = Some("stale".into());
        let _ = app.update(Message::ChannelsToastPullClicked);
        assert!(app.channels.pending_pull_after_write);
        assert!(app.channels.pending_install.is_none());
    }

    // --- Phase 3b — Remove warning dialog branch -----------------------

    fn one_pkg(name: &str) -> InstalledPackage {
        InstalledPackage {
            name: name.into(),
            version: "1.0".into(),
            output: "out".into(),
            store_path: PathBuf::from("/gnu/store/x"),
        }
    }

    /// Cache `None` (loading or never fetched) → no warning dialog.
    /// The minimal Confirm/Cancel renders and Remove proceeds normally.
    #[test]
    fn remove_warning_skipped_when_cache_not_loaded() {
        assert!(!should_render_remove_warning(
            Some("pantherx"),
            None,
            "pantherx",
        ));
    }

    /// Pending matches and the channel has affected packages → warning
    /// dialog renders.
    #[test]
    fn remove_warning_renders_when_pending_and_has_affected() {
        let mut map: HashMap<String, Vec<InstalledPackage>> = HashMap::new();
        map.insert("pantherx".into(), vec![one_pkg("panther-foo")]);
        assert!(should_render_remove_warning(
            Some("pantherx"),
            Some(&map),
            "pantherx",
        ));
    }

    /// Cache loaded but the channel has zero entries → no warning
    /// dialog. The minimal Confirm/Cancel handles the Remove.
    #[test]
    fn remove_warning_skipped_when_no_affected_packages() {
        let map: HashMap<String, Vec<InstalledPackage>> = HashMap::new();
        assert!(!should_render_remove_warning(
            Some("pantherx"),
            Some(&map),
            "pantherx",
        ));
    }

    /// A different channel is armed for removal → this row doesn't
    /// render the warning, even if the queried channel has affected
    /// packages.
    #[test]
    fn remove_warning_skipped_when_other_channel_pending() {
        let mut map: HashMap<String, Vec<InstalledPackage>> = HashMap::new();
        map.insert("pantherx".into(), vec![one_pkg("panther-foo")]);
        assert!(!should_render_remove_warning(
            Some("nonguix"),
            Some(&map),
            "pantherx",
        ));
    }

    /// No row is armed → no dialog, regardless of cache contents.
    #[test]
    fn remove_warning_skipped_when_nothing_pending() {
        let mut map: HashMap<String, Vec<InstalledPackage>> = HashMap::new();
        map.insert("pantherx".into(), vec![one_pkg("panther-foo")]);
        assert!(!should_render_remove_warning(None, Some(&map), "pantherx"));
    }

    /// Clicking Remove for a channel arms `pending_remove` AND kicks
    /// off the introspection fetch when the cache is empty. The fetch
    /// is best-effort — Remove still works even if it never completes.
    #[test]
    fn remove_clicked_arms_pending_and_triggers_introspection_fetch() {
        let (mut app, _) = App::new();
        assert!(app.channels.installed_by_channel.is_none());
        assert!(!app.channels.installed_by_channel_loading);
        let _ = app.update(Message::ChannelsRemoveClicked("pantherx".into()));
        assert_eq!(app.channels.pending_remove.as_deref(), Some("pantherx"));
        // Without `guix`, the spawn helper early-returns without
        // flipping the loading flag — that's also fine for the dialog
        // contract, which only requires `installed_by_channel` to
        // remain `None` (the fallthrough → minimal Confirm/Cancel).
        assert!(app.channels.installed_by_channel.is_none());
    }

    /// Profile-mutating op completion invalidates the cache so the
    /// next Remove dialog re-fetches. Adding a channel doesn't.
    #[test]
    fn install_finished_invalidates_installed_by_channel_cache() {
        let (mut app, _) = App::new();
        let mut prefilled: HashMap<String, Vec<InstalledPackage>> = HashMap::new();
        prefilled.insert("pantherx".into(), vec![one_pkg("panther-foo")]);
        app.channels.installed_by_channel = Some(prefilled);
        app.active_op = Some(ActiveOp {
            id: OpId(1),
            kind: OpKind::Install,
            cancel: None,
            op_slot: SharedOp::new_empty_for_tests(),
            final_code: Some(0),
            finished: false,
            bootstrap_likely: false,
            progress: ProgressSummary::new(OpKind::Install),
            channel_shadow_seen: false,
        });
        let _ = app.update(Message::Progress(OpEvent::Finished));
        assert!(app.channels.installed_by_channel.is_none());
    }

    /// Pull completion does NOT invalidate the cache — pull refreshes
    /// the catalog but doesn't touch the user's installed profile.
    #[test]
    fn pull_finished_does_not_invalidate_installed_by_channel_cache() {
        let (mut app, _) = App::new();
        let mut prefilled: HashMap<String, Vec<InstalledPackage>> = HashMap::new();
        prefilled.insert("pantherx".into(), vec![one_pkg("panther-foo")]);
        app.channels.installed_by_channel = Some(prefilled);
        app.active_op = Some(finished_pull_op(Some(0), false));
        let _ = app.update(Message::Progress(OpEvent::Finished));
        assert!(app.channels.installed_by_channel.is_some());
    }

    /// Introspection failure stores an empty map so the next dialog
    /// open doesn't retry on every paint. The user-facing surface is
    /// identical to a real empty result.
    #[test]
    fn installed_by_channel_failure_stores_empty_map() {
        let (mut app, _) = App::new();
        let _ = app.update(Message::ChannelsInstalledByChannelLoaded(Err(
            "no guix found".into(),
        )));
        let m = app
            .channels
            .installed_by_channel
            .as_ref()
            .expect("Some after load");
        assert!(m.is_empty());
        assert!(!app.channels.installed_by_channel_loading);
    }

    /// Dismissing the rollback offer clears it without touching the
    /// channels file state.
    #[test]
    fn rollback_dismissed_clears_offer() {
        let (mut app, _) = App::new();
        app.channels.rollback_offer = Some(RollbackOffer {
            backup_path: Some(PathBuf::from("/tmp/x.bak")),
            bug: None,
        });
        let _ = app.update(Message::ChannelsRollbackDismissed);
        assert!(app.channels.rollback_offer.is_none());
    }
}
