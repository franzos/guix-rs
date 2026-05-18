use std::collections::HashMap;
use std::path::{Path, PathBuf};

use iced::theme::Palette;
use iced::widget::{button, column, container, row, scrollable, svg, text, tooltip, Column, Space};
use iced::{Alignment, Element, Font, Length, Subscription, Task, Theme};
use libguix::{
    CancelHandle, Channel, Guix, GuixError, InstalledPackage, Operation, PackageSummary,
    ProgressEvent, ProgressStream, PullOps, ReconfigureOptions, SearchFastResult,
    SystemPullOptions, DEFAULT_SEARCH_LIMIT,
};

use crate::app_metadata::{AppMetadata, MetadataClient};
use crate::carrier::Carrier;
use crate::operation_subscription::{operation_subscription, OpEvent, OpId, OpKind, SharedOp};
use crate::progress_summary::ProgressSummary;
use crate::recommended::RECOMMENDED;
use crate::settings::{probe_first_run_config, Settings, Tab};
use crate::styles::{self, BG, PRIMARY, TEXT};
use crate::terminal_buffer::TerminalBuffer;
use crate::views::{home, installed, search, system, updates};

pub const CANCEL_PKEXEC_TOOLTIP: &str =
    "Cannot cancel privileged operations — the kernel doesn't allow signaling root-owned processes. Wait for it to complete.";

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
    let mut s = String::new();
    s.push_str("Reconfigure failed: the running system Guix doesn't recognise a module your\n");
    s.push_str("config imports. This usually means a channel updated after your last\n");
    s.push_str("reconfigure and the new module isn't baked into the system Guix yet.\n");
    s.push('\n');
    s.push_str("Bootstrap once manually:\n");
    s.push('\n');
    s.push_str(&format!(
        "    sudo guix system reconfigure -L {load} {cfg}\n"
    ));
    s.push('\n');
    s.push_str("After that, this button will work for subsequent updates.");
    s
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

    SystemConfigChecked(Result<String, String>),
    SourceConfigChanged(String),
    SourceConfigValidate,
    LoadPathInputChanged(String),
    LoadPathAdd,
    LoadPathRemove(usize),

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
                ..Default::default()
            },
            warmup_done: false,
            theme,
            metadata_client: MetadataClient::new(),
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
        "Guix GUI".into()
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
                    Tab::Updates => Task::batch([
                        self.spawn_channels_refresh(),
                        self.spawn_pull_mtimes_refresh(),
                    ]),
                    Tab::System => self.spawn_system_load(),
                };
                // Always load the installed list at startup — the Search
                // detail pane needs it to flip Install/Remove correctly.
                let installed_task = self.spawn_installed_refresh();
                Task::batch([warmup_task, tab_task, installed_task])
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
                let path: Option<PathBuf> = self.settings.source_config_path.clone();
                let Some(path) = path else {
                    self.system.validation_message =
                        Some("Set the source config path on the System tab first.".into());
                    self.active_tab = Tab::System;
                    return Task::none();
                };
                let load_paths = self.settings.effective_load_paths();
                self.start_op(OpKind::Reconfigure, move |g| {
                    g.system().reconfigure(
                        &path,
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
                    "Path is empty.".into()
                } else if !p.exists() {
                    format!("Path does not exist: {}", p.display())
                } else if !p.is_file() {
                    format!("Path is not a regular file: {}", p.display())
                } else {
                    format!("OK: {}", p.display())
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
                self.system.cache_action_message = Some("Clearing cache...".into());
                // Replacing the client also drops its in-memory Flathub
                // index — otherwise the next fetch would skip the
                // network and miss the chance to pick up a refreshed ID
                // list. New() is cheap; just rebuilds the reqwest pool.
                let new_client = MetadataClient::new();
                let to_clear = self.metadata_client.clone();
                self.metadata_client = new_client;
                Task::perform(
                    async move { to_clear.clear_disk_cache().await },
                    Message::MetadataCacheCleared,
                )
            }
            Message::MetadataCacheCleared(result) => {
                self.system.cache_action_message = Some(match result {
                    Ok(()) => "Cache cleared.".into(),
                    Err(e) => format!("Failed to clear cache: {e}"),
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
                // Reject anything that isn't http(s) — defence in depth
                // so a malicious package homepage can't smuggle a shell
                // command past xdg-open's URL handler.
                if !(url.starts_with("http://") || url.starts_with("https://")) {
                    tracing::warn!(target: "guix_gui", "refusing to open non-http url: {url}");
                    return Task::none();
                }
                // `spawn()` only fork+execs the helper — it doesn't
                // wait for the browser — so a plain blocking call is
                // already non-blocking from our perspective.
                if let Err(e) = std::process::Command::new("xdg-open").arg(&url).spawn() {
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
                });
                Task::none()
            }
            Message::OpStartFailed(e) => {
                self.system.validation_message = Some(format!("Failed to start op: {e}"));
                Task::none()
            }
            Message::Progress(OpEvent::Progress(batch)) => {
                if let Some(op) = self.active_op.as_mut() {
                    for evt in &batch {
                        if let ProgressEvent::ExitSummary { code, .. } = evt {
                            op.final_code = Some(*code);
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
                if let Some(op) = self.active_op.as_mut() {
                    op.finished = true;
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
                    text("Failed to discover guix").size(24),
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
            return container(text("Discovering guix...").size(20))
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
            Tab::System => system::view(self),
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
        let handle = iced_image::Handle::from_bytes(bytes.to_vec());
        let img = iced_image(handle)
            .width(Length::Fill)
            .height(Length::Fill)
            .content_fit(iced::ContentFit::Contain);

        let close_btn = button(text("Close (Esc)").size(13))
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
        ]
        .spacing(2);

        // Brand mark — same isometric package icon as the README logo.
        let icon = svg(svg::Handle::from_memory(
            include_bytes!("../../assets/icon.svg").as_slice(),
        ))
        .width(Length::Fixed(32.0))
        .height(Length::Fixed(32.0));
        let brand = container(
            row![icon, text("Guix").size(18).font(styles::BOLD).color(TEXT),]
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

        let col = column![
            brand,
            primary,
            Space::new().height(Length::Fill),
            styles::separator(),
            Space::new().height(Length::Fixed(8.0)),
            settings,
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
        title: &'a str,
        action: Option<Element<'a, Message>>,
    ) -> Element<'a, Message> {
        let title_widget = text(title).size(24).color(TEXT);
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
        let title = format!("{} (op #{})", op.kind.label(), op.id.0);

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
            let cancel_btn = button(text("Cancel")).on_press_maybe(on_press);
            let cancel_element: Element<'_, Message> = if supports_cancel {
                cancel_btn.into()
            } else {
                tooltip(
                    cancel_btn,
                    container(text(CANCEL_PKEXEC_TOOLTIP))
                        .padding(6)
                        .style(container::rounded_box),
                    tooltip::Position::Top,
                )
                .into()
            };
            footer = footer.push(cancel_element).push(text("Running..."));
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
                .push(button(text("Close")).on_press(Message::DismissOverlay))
                .push(help_col);
        } else {
            let summary = match op.final_code {
                Some(0) => "Done.".to_string(),
                Some(code) => format!("Failed (exit {code})."),
                None => "Ended without exit summary.".to_string(),
            };
            footer = footer
                .push(button(text("Close")).on_press(Message::DismissOverlay))
                .push(text(summary));
        }
        let log_label = if self.show_log {
            "Hide log"
        } else {
            "Show log"
        };
        footer = footer
            .push(Space::new().width(Length::Fill))
            .push(button(text("Copy")).on_press(Message::CopyTerminalClicked))
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
        .unwrap_or_else(|| "Search failed.".to_string());
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
}
