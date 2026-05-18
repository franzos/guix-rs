//! User-level channels editor — reads/writes `~/.config/guix/channels.scm`
//! (or the override set in Settings) via `libguix::ChannelsFile`.
//!
//! Mirrors `views/system.rs`: a resolved-path strip with a store-managed
//! banner when applicable, a list-with-per-row-action below (mirror of
//! `views/installed.rs`), and an inline "Add channel" form.

use iced::widget::{button, column, container, row, scrollable, text, text_input, Column, Space};
use iced::{Element, Length};
use libguix::Channel;

use crate::app::{App, ChannelsSubMode, Message};
use crate::carrier::Carrier;
use crate::settings::Tab;
use crate::styles::{self, BOLD, MUTED};
use guix_gui::discovery::{DiscoveredChannel, DiscoveredPackage};

pub fn view(app: &App) -> Element<'_, Message> {
    let refresh = button(text("Refresh").size(13))
        .padding([8, 16])
        .style(styles::btn_secondary)
        .on_press(Message::ChannelsRefresh);
    let header = App::view_header("Channels", Some(refresh.into()));

    // Plain-language intro — mirrors the muted subtitle pattern from
    // `views/home.rs`. The audience here is users who don't already
    // manage channels.scm declaratively.
    let intro = text(
        "Channels are package sources for Guix. Adding a channel lets you install \
         software it provides. Removing one means its packages stop getting updates.",
    )
    .size(13)
    .color(MUTED);

    let path_strip = path_strip(app);
    let banner = store_managed_banner(app);
    let toast = pending_toast(app);

    let mut col: Column<'_, Message> = column![header, intro].spacing(8);

    // Sub-mode toggle is rendered ONLY when discovery is enabled. When
    // off, the page is byte-identical to the pre-discovery layout —
    // strict opt-in.
    if app.settings.discovery_enabled {
        col = col.push(sub_mode_toggle(app));
    }

    col = col.push(path_strip);
    if let Some(b) = banner {
        col = col.push(b);
    }
    if let Some(t) = toast {
        col = col.push(t);
    }

    // Branch on sub-mode. Without discovery enabled, `sub_mode` is
    // forced to `Installed` and the toggle isn't rendered above, so the
    // user can't reach the Discover branch from the UI.
    let in_discover =
        app.settings.discovery_enabled && app.channels.sub_mode == ChannelsSubMode::Discover;

    if in_discover {
        col = col.push(discover_body(app));
    } else {
        let body_inner: Element<'_, Message> = if app.channels.loading {
            text("Loading channels.scm...").size(13).color(MUTED).into()
        } else if let Some(err) = &app.channels.error {
            error_card(err)
        } else if app.channels.file.is_some() {
            installed_list(app)
        } else {
            empty_state()
        };
        col = col.push(body_inner);
        if let Some(footer) = inherited_footer(app) {
            col = col.push(footer);
        }
        let writable = app
            .channels
            .file
            .as_ref()
            .map(|f| !f.is_store_managed)
            .unwrap_or(true);
        // The Add form is gated by writability — the store-managed
        // banner already explains why it's gone, so a second muted card
        // would just add noise.
        if writable {
            col = col.push(add_channel_form(app));
        }
    }

    scrollable(col.spacing(12)).height(Length::Fill).into()
}

/// Segmented-control style sub-mode toggle. Only rendered when
/// `discovery_enabled` is true (caller-side gated).
fn sub_mode_toggle(app: &App) -> Element<'_, Message> {
    let mk = |label: &'static str, mode: ChannelsSubMode| -> Element<'_, Message> {
        let active = app.channels.sub_mode == mode;
        let style = if active {
            styles::btn_secondary
        } else {
            styles::btn_ghost
        };
        button(text(label).size(13))
            .padding([6, 14])
            .style(style)
            .on_press(Message::ChannelsSubModeSelected(mode))
            .into()
    };
    container(
        row![
            mk("Installed", ChannelsSubMode::Installed),
            mk("Discover", ChannelsSubMode::Discover),
        ]
        .spacing(4),
    )
    .padding(4)
    .into()
}

fn path_strip(app: &App) -> Element<'_, Message> {
    let resolved: String = match &app.channels.file {
        Some(f) => f.path.display().to_string(),
        None => app
            .settings
            .channels_source_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "~/.config/guix/channels.scm (default)".into()),
    };

    let writable_badge: Element<'_, Message> = if let Some(f) = &app.channels.file {
        if f.is_store_managed {
            text("store-managed (read-only)")
                .size(12)
                .color(styles::WARNING)
                .into()
        } else {
            text("writable").size(12).color(styles::SUCCESS).into()
        }
    } else {
        text("").size(12).into()
    };

    let restore: Option<Element<'_, Message>> = restore_control(app);

    let mut row_widget = row![text(format!("File: {resolved}")).size(13).color(MUTED),]
        .spacing(8)
        .align_y(iced::Alignment::Center);
    row_widget = row_widget.push(Space::new().width(Length::Fill));
    if let Some(r) = restore {
        row_widget = row_widget.push(r);
    }
    row_widget = row_widget.push(writable_badge);

    container(column![row_widget].spacing(4))
        .padding(14)
        .width(Length::Fill)
        .style(styles::card_flat)
        .into()
}

/// "Restore last backup" control — hidden entirely when no `.bak` is
/// known. Clicking enters a confirm/cancel latch (same pattern as the
/// per-row Remove flow) to avoid double-undo accidents.
fn restore_control(app: &App) -> Option<Element<'_, Message>> {
    app.channels.backup_path.as_ref()?;
    if app.channels.pending_restore {
        let confirm = button(text("Confirm restore").size(11))
            .padding([4, 10])
            .style(styles::btn_secondary)
            .on_press(Message::ChannelsRestoreConfirmed);
        let cancel = button(text("Cancel").size(11))
            .padding([4, 10])
            .style(styles::btn_ghost)
            .on_press(Message::ChannelsRestoreCancelled);
        return Some(row![confirm, cancel].spacing(6).into());
    }
    let on_press = (!app.channels.saving).then_some(Message::ChannelsRestoreClicked);
    Some(
        button(text("Restore last backup").size(11))
            .padding([4, 10])
            .style(styles::btn_ghost)
            .on_press_maybe(on_press)
            .into(),
    )
}

fn store_managed_banner(app: &App) -> Option<Element<'_, Message>> {
    let f = app.channels.file.as_ref()?;
    if !f.is_store_managed {
        return None;
    }
    // No section-level deep link in the existing tab nav — clicking jumps
    // to the Settings tab, the CHANNELS section sits at the bottom so the
    // user lands on the right input within a scroll.
    let open_settings = button(text("Open Settings").size(12))
        .padding([6, 12])
        .style(styles::btn_secondary)
        .on_press(Message::TabSelected(Tab::System));
    let body = column![
        text("This file can't be edited here").size(14).font(BOLD),
        text(
            "Your channels.scm is managed by `guix home` (or another tool) \
             and can't be edited directly. To use guix-gui for channel \
             changes, set a writable file in "
        )
        .size(13)
        .color(MUTED),
        text("Settings → Channels.").size(13).font(BOLD),
        Space::new().height(6),
        open_settings,
    ]
    .spacing(2);
    Some(
        container(body)
            .padding(14)
            .width(Length::Fill)
            .style(styles::card)
            .into(),
    )
}

fn pending_toast(app: &App) -> Option<Element<'_, Message>> {
    if app.channels.saving {
        return Some(
            container(text("Saving...").size(12).color(MUTED))
                .padding(10)
                .width(Length::Fill)
                .style(styles::card_flat)
                .into(),
        );
    }
    let msg = app.channels.last_message.as_ref()?;
    let pull_btn = button(text("Pull now").size(12))
        .padding([6, 12])
        .style(styles::btn_secondary)
        .on_press(Message::FetchUserCatalogClicked);
    let dismiss = button(text("Dismiss").size(12))
        .padding([6, 12])
        .style(styles::btn_ghost)
        .on_press(Message::ChannelsToastDismissed);
    let row_widget = row![
        text(msg.clone()).size(13),
        Space::new().width(Length::Fill),
        pull_btn,
        dismiss,
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);
    Some(
        container(row_widget)
            .padding(12)
            .width(Length::Fill)
            .style(styles::card_flat)
            .into(),
    )
}

fn empty_state<'a>() -> Element<'a, Message> {
    let body = column![
        text("No channels.scm found").size(16).font(BOLD),
        text(
            "Add a channel below to create one. The file lives at \
             ~/.config/guix/channels.scm by default."
        )
        .size(12)
        .color(MUTED),
    ]
    .spacing(4);
    container(body)
        .padding(20)
        .width(Length::Fill)
        .style(styles::card)
        .into()
}

fn error_card<'a>(err: &'a str) -> Element<'a, Message> {
    container(
        column![
            text("Error").size(14).font(BOLD).color(styles::DANGER),
            text(err.to_string()).size(12).color(MUTED),
        ]
        .spacing(4),
    )
    .padding(14)
    .width(Length::Fill)
    .style(styles::card_flat)
    .into()
}

fn installed_list(app: &App) -> Element<'_, Message> {
    let f = app.channels.file.as_ref().expect("file present");
    let channels = f.list.channels();
    let writable = !f.is_store_managed;

    let header = row![text(format!(
        "{} channel{}",
        channels.len(),
        if channels.len() == 1 { "" } else { "s" }
    ))
    .size(12)
    .color(MUTED),];

    let mut rows: Column<'_, Message> = Column::new().spacing(8);
    for c in channels {
        rows = rows.push(channel_row(app, c, writable));
    }
    if channels.is_empty() {
        rows = rows.push(
            container(text("No channels in this file.").size(12).color(MUTED))
                .padding(14)
                .width(Length::Fill)
                .style(styles::card_flat),
        );
    }

    column![header, rows].spacing(8).into()
}

/// Lists channels that `guix describe` reports but the user's `channels.scm`
/// doesn't — i.e. transitive deps pulled in by declared channels (e.g. a
/// channel that depends on `nonguix` via its `.guix-channel` file). Returns
/// `None` when the difference is empty (nothing to show) or when either
/// set hasn't loaded yet (we'd be showing a misleading slice).
fn inherited_footer(app: &App) -> Option<Element<'_, Message>> {
    let file = app.channels.file.as_ref()?;
    let file_names: std::collections::HashSet<&str> = file
        .list
        .channels()
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    let inherited: Vec<&Channel> = app
        .updates
        .channels
        .iter()
        .filter(|c| !file_names.contains(c.name.as_str()))
        .collect();
    if inherited.is_empty() {
        return None;
    }

    let mut rows: Column<'_, Message> = Column::new().spacing(4);
    for c in inherited {
        let mut entry: Column<'_, Message> =
            Column::new().push(text(c.name.clone()).size(12).color(MUTED));
        if !c.url.is_empty() {
            entry = entry.push(text(c.url.clone()).size(11).color(MUTED));
        }
        rows = rows.push(entry.spacing(1));
    }

    let body = column![
        text("Also pulled in by your channels").size(13).font(BOLD),
        text("These come from the channels above and are managed by them.")
            .size(11)
            .color(MUTED),
        Space::new().height(4),
        rows,
    ]
    .spacing(2);
    Some(
        container(body)
            .padding(14)
            .width(Length::Fill)
            .style(styles::card_flat)
            .into(),
    )
}

fn channel_row<'a>(app: &'a App, ch: &'a Channel, writable: bool) -> Element<'a, Message> {
    let name_row = row![
        text(ch.name.clone()).size(14).font(BOLD),
        Space::new().width(Length::Fill),
        per_row_action(app, ch, writable),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    let mut details: Column<'_, Message> = Column::new().spacing(2);
    details = details.push(text(ch.url.clone()).size(12).color(MUTED));
    if let Some(b) = &ch.branch {
        details = details.push(text(format!("branch: {b}")).size(11).color(MUTED));
    }
    if let Some(c) = &ch.commit {
        details = details.push(text(format!("commit: {c}")).size(11).color(MUTED));
    }
    if ch.introduction_commit.is_some() {
        let fpr = ch
            .introduction_fingerprint
            .as_deref()
            .unwrap_or("(no fingerprint)");
        details = details.push(text(format!("introduction: {fpr}")).size(11).color(MUTED));
    } else {
        details = details.push(text("introduction: (none)").size(11).color(styles::WARNING));
    }

    let body = column![name_row, details].spacing(4);
    container(body)
        .padding(14)
        .width(Length::Fill)
        .style(styles::card_flat)
        .into()
}

fn per_row_action<'a>(app: &'a App, ch: &'a Channel, writable: bool) -> Element<'a, Message> {
    // The `guix` channel is locked — removing it would break the user's
    // setup. We don't render an action for it.
    if ch.name == "guix" {
        return text("locked").size(11).color(MUTED).into();
    }
    let pending = app.channels.pending_remove.as_deref() == Some(ch.name.as_str());
    if pending {
        let confirm = button(text("Confirm remove").size(11))
            .padding([4, 10])
            .style(styles::btn_danger)
            .on_press(Message::ChannelsRemoveConfirmed(ch.name.clone()));
        let cancel = button(text("Cancel").size(11))
            .padding([4, 10])
            .style(styles::btn_ghost)
            .on_press(Message::ChannelsRemoveCancelled);
        return row![confirm, cancel].spacing(6).into();
    }
    let on_press = if writable && !app.channels.saving {
        Some(Message::ChannelsRemoveClicked(ch.name.clone()))
    } else {
        None
    };
    button(text("Remove").size(11))
        .padding([4, 10])
        .style(styles::btn_ghost)
        .on_press_maybe(on_press)
        .into()
}

fn add_channel_form(app: &App) -> Element<'_, Message> {
    let writable = app
        .channels
        .file
        .as_ref()
        .map(|f| !f.is_store_managed)
        .unwrap_or(true);
    let form = &app.channels.add_form;

    let name_input = text_input("e.g. nonguix", &form.name)
        .on_input(Message::ChannelsAddNameChanged)
        .padding(8)
        .size(13)
        .width(Length::Fill);
    let url_input = text_input("https://gitlab.com/nonguix/nonguix", &form.url)
        .on_input(Message::ChannelsAddUrlChanged)
        .padding(8)
        .size(13)
        .width(Length::Fill);
    let branch_input = text_input("master (optional)", &form.branch)
        .on_input(Message::ChannelsAddBranchChanged)
        .padding(8)
        .size(13)
        .width(Length::Fill);
    let commit_input = text_input("commit hash (optional)", &form.commit)
        .on_input(Message::ChannelsAddCommitChanged)
        .padding(8)
        .size(13)
        .width(Length::Fill);
    let intro_commit_input = text_input("introduction commit hash", &form.intro_commit)
        .on_input(Message::ChannelsAddIntroCommitChanged)
        .padding(8)
        .size(13)
        .width(Length::Fill);
    let intro_fpr_input = text_input(
        "OpenPGP fingerprint (e.g. 2A39 3FFF 68F4 EF7A 3D29 ...)",
        &form.intro_fpr,
    )
    .on_input(Message::ChannelsAddIntroFprChanged)
    .padding(8)
    .size(13)
    .width(Length::Fill);

    let submit_enabled = writable
        && !app.channels.saving
        && !form.name.trim().is_empty()
        && !form.url.trim().is_empty()
        && !form.intro_commit.trim().is_empty()
        && !form.intro_fpr.trim().is_empty();
    let submit_btn = button(text("Add channel").size(13))
        .padding([8, 16])
        .style(styles::btn_secondary)
        .on_press_maybe(if submit_enabled {
            Some(Message::ChannelsAddSubmitted)
        } else {
            None
        });

    let mut content = column![
        text("Add a channel").size(16).font(BOLD),
        text("All fields are stored verbatim; introduction commit + fingerprint are required.")
            .size(12)
            .color(MUTED),
        Space::new().height(4),
        labeled("Name", name_input),
        labeled("URL", url_input),
        labeled("Branch", branch_input),
        labeled("Commit", commit_input),
        labeled("Introduction commit", intro_commit_input),
        labeled("Introduction fingerprint", intro_fpr_input),
    ]
    .spacing(6);

    if let Some(msg) = &app.channels.validation_message {
        content = content.push(text(msg.clone()).size(12).color(styles::DANGER));
    }

    content = content.push(submit_btn);

    container(content)
        .padding(20)
        .width(Length::Fill)
        .style(styles::card)
        .into()
}

fn labeled<'a>(
    label: &'a str,
    input: iced::widget::TextInput<'a, Message>,
) -> Element<'a, Message> {
    column![text(label).size(12).color(MUTED), input]
        .spacing(2)
        .into()
}

// --- Discover sub-mode --------------------------------------------------
//
// Rendered only when `app.settings.discovery_enabled` is true AND the
// active sub-mode is Discover. Both invariants are enforced by the top-
// level `view`; this section never has to re-check the toggle.

fn discover_body(app: &App) -> Element<'_, Message> {
    let mut col: Column<'_, Message> = Column::new().spacing(12);

    col = col.push(discover_search_bar(app));

    if let Some(err) = &app.channels.discover_error {
        col = col.push(discover_error_line(err));
    }

    // Confirmation card takes over the action area when a row's "Add"
    // has been clicked — mirrors the Installed sub-mode's
    // remove-confirmation pattern.
    if let Some(ch) = &app.channels.discover_pending_add {
        col = col.push(confirm_add_card(app, ch));
    }

    let query_active = !app.channels.discover_query.trim().is_empty();
    if query_active {
        col = col.push(packages_panel(app));
    }
    col = col.push(channels_panel(app));

    col.into()
}

fn discover_search_bar(app: &App) -> Element<'_, Message> {
    let input = text_input(
        "Search packages or channels...",
        &app.channels.discover_query,
    )
    .on_input(Message::DiscoverQueryChanged)
    .padding(8)
    .size(13)
    .width(Length::Fill);
    container(input).padding(4).width(Length::Fill).into()
}

fn discover_error_line(err: &str) -> Element<'_, Message> {
    container(text(err.to_string()).size(12).color(MUTED))
        .padding(10)
        .width(Length::Fill)
        .style(styles::card_flat)
        .into()
}

fn packages_panel(app: &App) -> Element<'_, Message> {
    let header_text = if app.channels.discover_packages_loading {
        "Searching...".to_string()
    } else {
        format!(
            "{} package result{}",
            app.channels.discover_packages.len(),
            if app.channels.discover_packages.len() == 1 {
                ""
            } else {
                "s"
            },
        )
    };
    let mut rows: Column<'_, Message> = Column::new().spacing(6);

    // Group hits by providing channel so the rendering shows which
    // channel each package belongs to in a structured way.
    use std::collections::BTreeMap;
    let mut grouped: BTreeMap<&str, Vec<&DiscoveredPackage>> = BTreeMap::new();
    for p in &app.channels.discover_packages {
        grouped.entry(p.channel.as_str()).or_default().push(p);
    }
    for (channel, pkgs) in grouped {
        rows = rows.push(text(format!("from {channel}")).size(12).color(MUTED));
        for p in pkgs {
            rows = rows.push(package_row(app, p));
        }
    }

    let title = text("Packages").size(14).font(BOLD);
    let inner = column![title, text(header_text).size(12).color(MUTED), rows].spacing(6);
    container(inner)
        .padding(14)
        .width(Length::Fill)
        .style(styles::card)
        .into()
}

fn package_row<'a>(app: &'a App, p: &'a DiscoveredPackage) -> Element<'a, Message> {
    // The providing channel's `DiscoveredChannel` — used to resolve
    // the full subscription snippet when the Add CTA needs to construct
    // the `Channel` (via `to_channel`).
    let providing = app
        .channels
        .discover_channels
        .iter()
        .find(|c| c.name == p.channel);

    // Already-present check: the in-Installed set is the file list
    // exactly as the user sees it on the other sub-mode.
    let already_present = app
        .channels
        .file
        .as_ref()
        .map(|f| f.list.channels().iter().any(|c| c.name == p.channel))
        .unwrap_or(false);

    let writable = app
        .channels
        .file
        .as_ref()
        .map(|f| !f.is_store_managed)
        .unwrap_or(true);

    let cta: Element<'_, Message> = if already_present {
        // Phase 2b ships Add only — wiring the actual install is a
        // Phase 3 follow-up; surface the affordance as disabled so
        // the user knows it's coming.
        button(text("Install").size(11))
            .padding([4, 10])
            .style(styles::btn_ghost)
            .into()
    } else {
        let parsed = providing.and_then(|d| d.to_channel());
        let on_press = match (writable, parsed) {
            (true, Some(ch)) => Some(Message::DiscoverAddClicked(Carrier::new(ch))),
            _ => None,
        };
        button(text("Add channel & install").size(11))
            .padding([4, 10])
            .style(styles::btn_secondary)
            .on_press_maybe(on_press)
            .into()
    };

    let title = row![
        text(p.name.clone()).size(13).font(BOLD),
        text(format!(" {}", p.version)).size(11).color(MUTED),
        Space::new().width(Length::Fill),
        cta,
    ]
    .spacing(4)
    .align_y(iced::Alignment::Center);

    let synopsis = if p.synopsis.is_empty() {
        text("(no synopsis)").size(11).color(MUTED)
    } else {
        text(p.synopsis.clone()).size(11).color(MUTED)
    };

    let inner = column![title, synopsis].spacing(2);
    container(inner)
        .padding(10)
        .width(Length::Fill)
        .style(styles::card_flat)
        .into()
}

fn channels_panel(app: &App) -> Element<'_, Message> {
    let title = text("Channels").size(14).font(BOLD);

    let body_inner: Element<'_, Message> = if app.channels.discover_channels_loading {
        text("Loading channels...").size(12).color(MUTED).into()
    } else if app.channels.discover_channels.is_empty() {
        text("No introduced channels were returned.")
            .size(12)
            .color(MUTED)
            .into()
    } else {
        // Sort by packagesCount descending — biggest-impact channels
        // first. We clone the references; underlying storage stays put.
        let mut sorted: Vec<&DiscoveredChannel> = app.channels.discover_channels.iter().collect();
        sorted.sort_by(|a, b| b.packages_count.cmp(&a.packages_count));

        let mut rows: Column<'_, Message> = Column::new().spacing(6);
        for c in sorted {
            rows = rows.push(channel_discover_row(app, c));
        }
        rows.into()
    };

    let count_line = text(format!(
        "{} channel{} available",
        app.channels.discover_channels.len(),
        if app.channels.discover_channels.len() == 1 {
            ""
        } else {
            "s"
        }
    ))
    .size(12)
    .color(MUTED);

    let inner = column![title, count_line, body_inner].spacing(6);
    container(inner)
        .padding(14)
        .width(Length::Fill)
        .style(styles::card)
        .into()
}

fn channel_discover_row<'a>(app: &'a App, c: &'a DiscoveredChannel) -> Element<'a, Message> {
    let writable = app
        .channels
        .file
        .as_ref()
        .map(|f| !f.is_store_managed)
        .unwrap_or(true);
    let already_present = app
        .channels
        .file
        .as_ref()
        .map(|f| f.list.channels().iter().any(|ch| ch.name == c.name))
        .unwrap_or(false);

    let parsed = c.to_channel();
    let cta: Element<'_, Message> = if already_present {
        text("already added").size(11).color(MUTED).into()
    } else {
        let on_press = match (writable, parsed.clone()) {
            (true, Some(ch)) => Some(Message::DiscoverAddClicked(Carrier::new(ch))),
            _ => None,
        };
        button(text("Add").size(11))
            .padding([4, 10])
            .style(styles::btn_secondary)
            .on_press_maybe(on_press)
            .into()
    };

    // Truncate the fingerprint to 20 chars so it fits visually next to
    // the other badges. The confirmation card shows the full value.
    let fpr_short: String = parsed
        .as_ref()
        .and_then(|ch| ch.introduction_fingerprint.as_ref())
        .map(|f| f.chars().take(20).collect::<String>())
        .unwrap_or_default();

    let title = row![
        text(c.name.clone()).size(13).font(BOLD),
        Space::new().width(Length::Fill),
        text(format!("{} pkgs", c.packages_count))
            .size(11)
            .color(MUTED),
        text(format!("{} svcs", c.services_count))
            .size(11)
            .color(MUTED),
        cta,
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    let url_line = text(c.url.clone()).size(11).color(MUTED);
    let fpr_line = if fpr_short.is_empty() {
        text("intro: —").size(11).color(MUTED)
    } else {
        text(format!("intro: {fpr_short}...")).size(11).color(MUTED)
    };

    let inner = column![title, url_line, fpr_line].spacing(2);
    container(inner)
        .padding(10)
        .width(Length::Fill)
        .style(styles::card_flat)
        .into()
}

fn confirm_add_card<'a>(app: &'a App, ch: &'a Channel) -> Element<'a, Message> {
    let writable = app
        .channels
        .file
        .as_ref()
        .map(|f| !f.is_store_managed)
        .unwrap_or(true);

    let line = |label: &str, value: &str| -> Element<'_, Message> {
        row![
            text(label.to_string())
                .size(12)
                .color(MUTED)
                .width(Length::Fixed(140.0)),
            text(value.to_string()).size(12),
        ]
        .spacing(8)
        .into()
    };

    let mut details: Column<'_, Message> = Column::new().spacing(2);
    details = details.push(line("name", &ch.name));
    details = details.push(line("url", &ch.url));
    if let Some(b) = &ch.branch {
        details = details.push(line("branch", b));
    }
    if let Some(c) = &ch.commit {
        details = details.push(line("commit", c));
    }
    if let Some(c) = &ch.introduction_commit {
        details = details.push(line("intro commit", c));
    }
    if let Some(fpr) = &ch.introduction_fingerprint {
        details = details.push(line("intro fingerprint", fpr));
    }

    let confirm_tooltip = if !writable {
        "Set a writable file in Settings"
    } else {
        ""
    };
    let confirm = button(text("Add channel").size(13))
        .padding([8, 16])
        .style(styles::btn_secondary)
        .on_press_maybe(
            (writable && !app.channels.saving).then_some(Message::DiscoverAddConfirmed),
        );
    let cancel = button(text("Cancel").size(13))
        .padding([8, 16])
        .style(styles::btn_ghost)
        .on_press(Message::DiscoverAddCancelled);

    let mut actions = row![confirm, cancel].spacing(8);
    if !confirm_tooltip.is_empty() {
        actions = actions.push(text(confirm_tooltip).size(11).color(MUTED));
    }

    let body = column![
        text("Confirm channel add").size(14).font(BOLD),
        text(
            "This will append the channel to your channels.scm and validate \
             the file before saving."
        )
        .size(12)
        .color(MUTED),
        Space::new().height(6),
        details,
        Space::new().height(6),
        actions,
    ]
    .spacing(4);

    container(body)
        .padding(14)
        .width(Length::Fill)
        .style(styles::card)
        .into()
}
