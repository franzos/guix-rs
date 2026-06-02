//! User-level channels editor — reads/writes `~/.config/guix/channels.scm`
//! (or the override set in Settings) via `libguix::ChannelsFile`.
//!
//! Mirrors `views/system.rs`: a resolved-path strip with a store-managed
//! banner when applicable, a list-with-per-row-action below (mirror of
//! `views/installed.rs`), and an inline "Add channel" form.

use iced::widget::{button, column, container, row, scrollable, text, text_input, Column, Space};
use iced::{Element, Font, Length};
use libguix::Channel;

use crate::app::{should_render_remove_warning, App, Message};
use crate::carrier::Carrier;
use crate::channels::{ChannelsSubMode, RollbackOffer};
use crate::settings::Tab;
use crate::styles::{self, BOLD, MUTED};
use guix_gui::discovery::{DiscoveredChannel, DiscoveredPackage};
use libguix::KnownBug;

pub fn view(app: &App) -> Element<'_, Message> {
    let refresh = button(text(crate::t!("common-refresh")).size(13))
        .padding([8, 16])
        .style(styles::btn_secondary)
        .on_press(Message::ChannelsRefresh);
    let header = App::view_header(crate::t!("channels-title"), Some(refresh.into()));

    // Plain-language intro — mirrors the muted subtitle pattern from
    // `views/home.rs`. The audience here is users who don't already
    // manage channels.scm declaratively.
    let intro = text(crate::t!("channels-intro")).size(13).color(MUTED);

    let path_strip = path_strip(app);
    let banner = store_managed_banner(app);
    let rollback = rollback_offer_card(app);
    // The rollback offer takes precedence over the post-write toast —
    // if a pull just failed, the toast that triggered it is stale and
    // would offer a confusing second-pull CTA.
    let toast = if rollback.is_some() {
        None
    } else {
        pending_toast(app)
    };

    // Sub-section heading. Today the tab only carries user-level
    // channels; when system-level editing arrives (Phase 4) a parallel
    // "SYSTEM CHANNELS" heading appears below with the same shape.
    let user_channels_heading = text(crate::t!("channels-section-user"))
        .size(12)
        .color(MUTED);

    let mut col: Column<'_, Message> = column![header, intro, user_channels_heading].spacing(8);

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
    if let Some(r) = rollback {
        col = col.push(r);
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
            text(crate::t!("channels-loading"))
                .size(13)
                .color(MUTED)
                .into()
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
    let mk = |label: String, mode: ChannelsSubMode| -> Element<'_, Message> {
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
            mk(
                crate::t!("channels-submode-installed"),
                ChannelsSubMode::Installed
            ),
            mk(
                crate::t!("channels-submode-discover"),
                ChannelsSubMode::Discover
            ),
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
            .unwrap_or_else(|| crate::t!("channels-default-path")),
    };

    let writable_badge: Element<'_, Message> = if let Some(f) = &app.channels.file {
        if f.is_store_managed {
            text(crate::t!("channels-store-managed"))
                .size(12)
                .color(styles::WARNING)
                .into()
        } else {
            text(crate::t!("channels-writable"))
                .size(12)
                .color(styles::SUCCESS)
                .into()
        }
    } else {
        text("").size(12).into()
    };

    let restore: Option<Element<'_, Message>> = restore_control(app);

    let mut row_widget = row![text(crate::t!("channels-file", path = resolved))
        .size(13)
        .color(MUTED),]
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
        let confirm = button(text(crate::t!("channels-confirm-restore")).size(11))
            .padding([4, 10])
            .style(styles::btn_secondary)
            .on_press(Message::ChannelsRestoreConfirmed);
        let cancel = button(text(crate::t!("common-cancel")).size(11))
            .padding([4, 10])
            .style(styles::btn_ghost)
            .on_press(Message::ChannelsRestoreCancelled);
        return Some(row![confirm, cancel].spacing(6).into());
    }
    let on_press = (!app.channels.saving).then_some(Message::ChannelsRestoreClicked);
    Some(
        button(text(crate::t!("channels-restore-last")).size(11))
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
    let open_settings = button(text(crate::t!("common-open-settings")).size(12))
        .padding([6, 12])
        .style(styles::btn_secondary)
        .on_press(Message::TabSelected(Tab::System));
    let body = column![
        text(crate::t!("channels-cant-edit-title"))
            .size(14)
            .font(BOLD),
        text(crate::t!(
            "channels-cant-edit-blurb",
            settings_tab = crate::t!("tab-system"),
            channels_tab = crate::t!("tab-channels")
        ))
        .size(13)
        .color(MUTED),
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
            container(text(crate::t!("channels-saving")).size(12).color(MUTED))
                .padding(10)
                .width(Length::Fill)
                .style(styles::card_flat)
                .into(),
        );
    }
    let msg = app.channels.last_message.as_ref()?;
    let dismiss = button(text(crate::t!("common-dismiss")).size(12))
        .padding([6, 12])
        .style(styles::btn_ghost)
        .on_press(Message::ChannelsToastDismissed);

    // Combined CTA when the Add originated from a Discover package row.
    // Two buttons — "Pull, then install <pkg>" (the happy path) and
    // "Pull only" (escape hatch if the user changed their mind).
    let row_widget = if let Some(pkg) = app.channels.post_apply_install_prompt.clone() {
        let pull_install =
            button(text(crate::t!("channels-pull-then-install", pkg = pkg.clone())).size(12))
                .padding([6, 12])
                .style(styles::btn_secondary)
                .on_press(Message::ChannelsToastPullAndInstallClicked(pkg.clone()));
        let pull_only = button(text(crate::t!("channels-pull-only")).size(12))
            .padding([6, 12])
            .style(styles::btn_ghost)
            .on_press(Message::ChannelsToastPullClicked);
        row![
            text(msg.clone()).size(13),
            Space::new().width(Length::Fill),
            pull_install,
            pull_only,
            dismiss,
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center)
    } else {
        let pull_btn = button(text(crate::t!("channels-pull-now")).size(12))
            .padding([6, 12])
            .style(styles::btn_secondary)
            .on_press(Message::ChannelsToastPullClicked);
        row![
            text(msg.clone()).size(13),
            Space::new().width(Length::Fill),
            pull_btn,
            dismiss,
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center)
    };

    Some(
        container(row_widget)
            .padding(12)
            .width(Length::Fill)
            .style(styles::card_flat)
            .into(),
    )
}

/// Post-pull-failure rollback CTA. Rendered in place of the "Pull now"
/// toast when `ChannelsState::rollback_offer` is set — the toast would
/// be stale at that point. The CTA never auto-restores; the user
/// confirms explicitly.
fn rollback_offer_card<'a>(app: &'a App) -> Option<Element<'a, Message>> {
    let offer: &'a RollbackOffer = app.channels.rollback_offer.as_ref()?;
    let dismiss = button(text(crate::t!("channels-keep-changes")).size(12))
        .padding([6, 12])
        .style(styles::btn_ghost)
        .on_press(Message::ChannelsRollbackDismissed);

    let title_label = match offer.bug {
        Some(KnownBug::ChannelShadow74396) => crate::t!("channels-pull-failed-shadow"),
        None => crate::t!("channels-pull-failed"),
    };

    let mut body_col: iced::widget::Column<'_, Message> = Column::new().spacing(4);
    body_col = body_col.push(text(title_label).size(14).font(BOLD).color(styles::DANGER));

    if offer.backup_path.is_some() {
        body_col = body_col.push(
            text(crate::t!("channels-rollback-blurb"))
                .size(12)
                .color(MUTED),
        );
    } else {
        // write_atomic always creates a .bak, so this branch is
        // defensive — but if we reach it we explain plainly that
        // there's nothing to restore.
        body_col = body_col.push(
            text(crate::t!("channels-rollback-none"))
                .size(12)
                .color(MUTED),
        );
    }

    // When a known bug is named, surface the canonical issue link
    // beneath the explanatory text. Read-only — clickable via the same
    // `OpenUrl` plumbing the package homepage CTA uses.
    if let Some(bug) = offer.bug {
        let url = bug.url();
        let link = button(text(url).size(11).color(styles::PRIMARY))
            .padding(0)
            .style(styles::btn_ghost)
            .on_press(Message::OpenUrl(url.to_string()));
        body_col = body_col.push(link);
    }

    let actions: iced::widget::Row<'_, Message> = if offer.backup_path.is_some() {
        let restore = button(text(crate::t!("channels-restore-previous")).size(12))
            .padding([6, 12])
            .style(styles::btn_secondary)
            .on_press(Message::ChannelsRollbackConfirmed);
        row![restore, dismiss].spacing(8)
    } else {
        // No backup — only the dismiss escape hatch is meaningful.
        row![button(text(crate::t!("common-dismiss")).size(12))
            .padding([6, 12])
            .style(styles::btn_ghost)
            .on_press(Message::ChannelsRollbackDismissed)]
        .spacing(8)
    };

    body_col = body_col.push(Space::new().height(4));
    body_col = body_col.push(actions);

    Some(
        container(body_col)
            .padding(14)
            .width(Length::Fill)
            .style(styles::card)
            .into(),
    )
}

fn empty_state<'a>() -> Element<'a, Message> {
    let body = column![
        text(crate::t!("channels-empty-title")).size(16).font(BOLD),
        text(crate::t!("channels-empty-blurb"))
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
            text(crate::t!("channels-error"))
                .size(14)
                .font(BOLD)
                .color(styles::DANGER),
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

    let header = row![text(crate::t!("channels-count", count = channels.len()))
        .size(12)
        .color(MUTED),];

    let mut rows: Column<'_, Message> = Column::new().spacing(8);
    for c in channels {
        rows = rows.push(channel_row(app, c, writable));
    }
    if channels.is_empty() {
        rows = rows.push(
            container(
                text(crate::t!("channels-none-in-file"))
                    .size(12)
                    .color(MUTED),
            )
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
        text(crate::t!("channels-inherited-title"))
            .size(13)
            .font(BOLD),
        text(crate::t!("channels-inherited-blurb"))
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
        details = details.push(
            text(crate::t!("channels-branch", branch = b.clone()))
                .size(11)
                .color(MUTED),
        );
    }
    if let Some(c) = &ch.commit {
        details = details.push(
            text(crate::t!("channels-commit", commit = c.clone()))
                .size(11)
                .color(MUTED),
        );
    }
    if ch.introduction_commit.is_some() {
        let fpr = ch
            .introduction_fingerprint
            .clone()
            .unwrap_or_else(|| crate::t!("channels-no-fingerprint"));
        details = details.push(
            text(crate::t!("channels-introduction", fpr = fpr))
                .size(11)
                .color(MUTED),
        );
    } else {
        details = details.push(
            text(crate::t!("channels-introduction-none"))
                .size(11)
                .color(styles::WARNING),
        );
    }

    let mut body: Column<'_, Message> = column![name_row, details].spacing(4);
    if let Some(dialog) = remove_warning_dialog(app, ch) {
        body = body.push(Space::new().height(6));
        body = body.push(dialog);
    }
    container(body)
        .padding(14)
        .width(Length::Fill)
        .style(styles::card_flat)
        .into()
}

/// Returns the expanded Remove warning dialog when:
/// - this channel is the one with `pending_remove`, AND
/// - the introspection cache reports at least one installed package
///   sourced from this channel.
///
/// All other states (no pending, cache not loaded, cache empty for
/// this channel) → `None`. The existing inline Confirm/Cancel in
/// `per_row_action` handles those.
///
/// The dialog is informational — it never blocks the Remove. Whether 1
/// package or 100 are affected, the user can proceed via "Remove
/// channel anyway". See TODO.md "warn, don't block".
fn remove_warning_dialog<'a>(app: &'a App, ch: &'a Channel) -> Option<Element<'a, Message>> {
    if !should_render_remove_warning(
        app.channels.pending_remove.as_deref(),
        app.channels.installed_by_channel.as_ref(),
        &ch.name,
    ) {
        return None;
    }
    // We treat the `(unknown)` bucket as "no info" for the named
    // channel; the dialog only surfaces real channel names.
    let affected = app
        .channels
        .installed_by_channel
        .as_ref()
        .and_then(|m| m.get(&ch.name))?;

    let title = text(crate::t!("channels-remove-title", name = ch.name.clone()))
        .size(14)
        .font(BOLD);
    let count = affected.len();
    let intro = text(crate::t!("channels-remove-intro", count = count))
        .size(12)
        .color(MUTED);

    // Bullet list of `name (version)` — no cap, scrollable when long.
    let mut pkg_list: Column<'_, Message> = Column::new().spacing(2);
    for p in affected {
        pkg_list = pkg_list.push(text(format!("• {} ({})", p.name, p.version)).size(12));
    }
    let pkg_list_el: Element<'_, Message> = if affected.len() > 8 {
        scrollable(pkg_list).height(Length::Fixed(180.0)).into()
    } else {
        pkg_list.into()
    };

    let explainer = text(crate::t!("channels-remove-explainer"))
        .size(12)
        .color(MUTED);

    // Command snippet — `name1 name2 ...` for copy-paste. We render
    // it inside a `card_flat` container so it visually reads as a
    // copyable block. iced text isn't selectable, but the snippet is
    // short enough that copying via a contextual UI gesture (e.g. the
    // overlay Copy button pattern) is overkill here; users can
    // re-type a 2-3-word command without much trouble.
    let names_joined: String = affected
        .iter()
        .map(|p| p.name.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let cmd_label = text(crate::t!("channels-remove-cmd-label"))
        .size(12)
        .color(MUTED);
    let cmd_snippet = container(text(format!("guix package -r {names_joined}")).size(12))
        .padding(8)
        .width(Length::Fill)
        .style(styles::card_flat);

    let confirm = button(text(crate::t!("channels-remove-anyway")).size(12))
        .padding([6, 12])
        .style(styles::btn_danger)
        .on_press(Message::ChannelsRemoveConfirmed(ch.name.clone()));
    let cancel = button(text(crate::t!("common-cancel")).size(12))
        .padding([6, 12])
        .style(styles::btn_ghost)
        .on_press(Message::ChannelsRemoveCancelled);
    let actions = row![confirm, cancel].spacing(8);

    let dialog = column![
        title,
        intro,
        pkg_list_el,
        Space::new().height(4),
        explainer,
        Space::new().height(4),
        cmd_label,
        cmd_snippet,
        Space::new().height(6),
        actions,
    ]
    .spacing(4);

    Some(
        container(dialog)
            .padding(12)
            .width(Length::Fill)
            .style(styles::card)
            .into(),
    )
}

fn per_row_action<'a>(app: &'a App, ch: &'a Channel, writable: bool) -> Element<'a, Message> {
    // The `guix` channel is locked — removing it would break the user's
    // setup. We don't render an action for it.
    if ch.name == "guix" {
        return text(crate::t!("channels-locked"))
            .size(11)
            .color(MUTED)
            .into();
    }
    let pending = app.channels.pending_remove.as_deref() == Some(ch.name.as_str());
    if pending {
        // If the warning dialog is going to render below the row, its
        // own Remove/Cancel actions take precedence — skip the inline
        // pair to avoid two side-by-side confirms. The branch matches
        // the `remove_warning_dialog` predicate.
        let warning_will_render = should_render_remove_warning(
            app.channels.pending_remove.as_deref(),
            app.channels.installed_by_channel.as_ref(),
            &ch.name,
        );
        if warning_will_render {
            return text(crate::t!("channels-see-warning"))
                .size(11)
                .color(MUTED)
                .into();
        }
        let confirm = button(text(crate::t!("channels-confirm-remove")).size(11))
            .padding([4, 10])
            .style(styles::btn_danger)
            .on_press(Message::ChannelsRemoveConfirmed(ch.name.clone()));
        let cancel = button(text(crate::t!("common-cancel")).size(11))
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
    button(text(crate::t!("common-remove")).size(11))
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

    let submit_enabled = writable
        && !app.channels.saving
        && !form.name.trim().is_empty()
        && !form.url.trim().is_empty()
        && !form.intro_commit.trim().is_empty()
        && !form.intro_fpr.trim().is_empty();
    // Pressing Enter in any field submits the form when the required
    // fields are filled. Matches the LoadPathAdd input on the System tab.
    let submit_on_enter = || {
        if submit_enabled {
            Some(Message::ChannelsAddSubmitted)
        } else {
            None
        }
    };

    let name_input = text_input(&crate::t!("channels-add-name-placeholder"), &form.name)
        .on_input(Message::ChannelsAddNameChanged)
        .on_submit_maybe(submit_on_enter())
        .padding(8)
        .size(13)
        .width(Length::Fill);
    let url_input = text_input(&crate::t!("channels-add-url-placeholder"), &form.url)
        .on_input(Message::ChannelsAddUrlChanged)
        .on_submit_maybe(submit_on_enter())
        .padding(8)
        .size(13)
        .width(Length::Fill);
    let branch_input = text_input(&crate::t!("channels-add-branch-placeholder"), &form.branch)
        .on_input(Message::ChannelsAddBranchChanged)
        .on_submit_maybe(submit_on_enter())
        .padding(8)
        .size(13)
        .width(Length::Fill);
    let commit_input = text_input(&crate::t!("channels-add-commit-placeholder"), &form.commit)
        .on_input(Message::ChannelsAddCommitChanged)
        .on_submit_maybe(submit_on_enter())
        .padding(8)
        .size(13)
        .width(Length::Fill);
    let intro_commit_input = text_input(
        &crate::t!("channels-add-intro-commit-placeholder"),
        &form.intro_commit,
    )
    .on_input(Message::ChannelsAddIntroCommitChanged)
    .on_submit_maybe(submit_on_enter())
    .padding(8)
    .size(13)
    .width(Length::Fill);
    let intro_fpr_input = text_input(
        &crate::t!("channels-add-intro-fpr-placeholder"),
        &form.intro_fpr,
    )
    .on_input(Message::ChannelsAddIntroFprChanged)
    .on_submit_maybe(submit_on_enter())
    .padding(8)
    .size(13)
    .width(Length::Fill);
    let submit_btn = button(text(crate::t!("channels-add-btn")).size(13))
        .padding([8, 16])
        .style(styles::btn_secondary)
        .on_press_maybe(if submit_enabled {
            Some(Message::ChannelsAddSubmitted)
        } else {
            None
        });

    let mut content = column![
        text(crate::t!("channels-add-heading")).size(16).font(BOLD),
        text(crate::t!("channels-add-blurb")).size(12).color(MUTED),
        Space::new().height(4),
        labeled(crate::t!("channels-add-name"), name_input),
        labeled(crate::t!("channels-add-url"), url_input),
        labeled(crate::t!("channels-add-branch"), branch_input),
        labeled(crate::t!("channels-add-commit"), commit_input),
        labeled(crate::t!("channels-add-intro-commit"), intro_commit_input),
        labeled(crate::t!("channels-add-intro-fpr"), intro_fpr_input),
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
    label: impl Into<String>,
    input: iced::widget::TextInput<'a, Message>,
) -> Element<'a, Message> {
    column![text(label.into()).size(12).color(MUTED), input]
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
        &crate::t!("channels-discover-placeholder"),
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
        crate::t!("channels-searching")
    } else {
        crate::t!(
            "channels-package-results",
            count = app.channels.discover_packages.len()
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
        rows = rows.push(
            text(crate::t!("channels-from", channel = channel.to_string()))
                .size(12)
                .color(MUTED),
        );
        for p in pkgs {
            rows = rows.push(package_row(app, p));
        }
    }

    let title = text(crate::t!("channels-packages")).size(14).font(BOLD);
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
        // Channel already added — direct install via the existing
        // install plumbing (same message the Search tab uses).
        let on_press = (!app.channels.saving).then(|| Message::InstallRequested(p.name.clone()));
        button(text(crate::t!("channels-install")).size(11))
            .padding([4, 10])
            .style(styles::btn_secondary)
            .on_press_maybe(on_press)
            .into()
    } else {
        let parsed = providing.and_then(|d| d.to_channel());
        let on_press = match (writable, parsed) {
            (true, Some(ch)) => Some(Message::DiscoverAddAndInstallClicked(
                Carrier::new(ch),
                p.name.clone(),
            )),
            _ => None,
        };
        button(text(crate::t!("channels-add-and-install")).size(11))
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
        text(crate::t!("channels-no-synopsis"))
            .size(11)
            .color(MUTED)
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
    let title = text(crate::t!("channels-channels-heading"))
        .size(14)
        .font(BOLD);

    let body_inner: Element<'_, Message> = if app.channels.discover_channels_loading {
        text(crate::t!("channels-loading-discover"))
            .size(12)
            .color(MUTED)
            .into()
    } else if app.channels.discover_channels.is_empty() {
        text(crate::t!("channels-no-introduced"))
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

    let count_line = text(crate::t!(
        "channels-available",
        count = app.channels.discover_channels.len()
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
        text(crate::t!("channels-already-added"))
            .size(11)
            .color(MUTED)
            .into()
    } else {
        let on_press = match (writable, parsed.clone()) {
            (true, Some(ch)) => Some(Message::DiscoverAddClicked(Carrier::new(ch))),
            _ => None,
        };
        button(text(crate::t!("channels-add")).size(11))
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
        text(crate::t!("channels-pkgs", count = c.packages_count))
            .size(11)
            .color(MUTED),
        text(crate::t!("channels-svcs", count = c.services_count))
            .size(11)
            .color(MUTED),
        cta,
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    let url_line = text(c.url.clone()).size(11).color(MUTED);
    let fpr_line = if fpr_short.is_empty() {
        text(crate::t!("channels-intro-dash")).size(11).color(MUTED)
    } else {
        text(crate::t!("channels-intro-short", fpr = fpr_short))
            .size(11)
            .color(MUTED)
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

    // Monospace so the user can eyeball commit/fingerprint values.
    let mono_line = |label: &str, value: &str| -> Element<'_, Message> {
        row![
            text(label.to_string())
                .size(12)
                .color(MUTED)
                .width(Length::Fixed(140.0)),
            text(value.to_string())
                .size(12)
                .font(Font::MONOSPACE)
                .width(Length::Fill),
        ]
        .spacing(8)
        .into()
    };

    let mut details: Column<'_, Message> = Column::new().spacing(2);
    details = details.push(line(&crate::t!("channels-field-name"), &ch.name));
    details = details.push(line(&crate::t!("channels-field-url"), &ch.url));
    if let Some(b) = &ch.branch {
        details = details.push(line(&crate::t!("channels-field-branch"), b));
    }
    if let Some(c) = &ch.commit {
        details = details.push(mono_line(&crate::t!("channels-field-commit"), c));
    }
    if let Some(c) = &ch.introduction_commit {
        details = details.push(mono_line(&crate::t!("channels-field-intro-commit"), c));
    }
    if let Some(fpr) = &ch.introduction_fingerprint {
        details = details.push(mono_line(&crate::t!("channels-field-intro-fpr"), fpr));
    }

    let confirm_tooltip = if !writable {
        crate::t!("channels-set-writable-tooltip")
    } else {
        String::new()
    };
    let confirm = button(text(crate::t!("channels-add-btn")).size(13))
        .padding([8, 16])
        .style(styles::btn_secondary)
        .on_press_maybe(
            (writable && !app.channels.saving).then_some(Message::DiscoverAddConfirmed),
        );
    let cancel = button(text(crate::t!("common-cancel")).size(13))
        .padding([8, 16])
        .style(styles::btn_ghost)
        .on_press(Message::DiscoverAddCancelled);

    let mut actions = row![confirm, cancel].spacing(8);
    if !confirm_tooltip.is_empty() {
        actions = actions.push(text(confirm_tooltip).size(11).color(MUTED));
    }

    // Surface the catalog the intro values came from — they decide
    // what `guix pull` trusts going forward.
    let provenance = column![
        text(crate::t!("channels-provenance")).size(12).color(MUTED),
        text(crate::t!(
            "channels-supplied-by",
            source = guix_gui::discovery::TOYS_API
        ))
        .size(12)
        .font(Font::MONOSPACE),
    ]
    .spacing(2);

    let trust_warning = text(crate::t!("channels-trust-warning"))
        .size(12)
        .color(styles::WARNING);

    let body = column![
        text(crate::t!("channels-confirm-add-title"))
            .size(14)
            .font(BOLD),
        text(crate::t!("channels-confirm-add-blurb"))
            .size(12)
            .color(MUTED),
        Space::new().height(6),
        provenance,
        Space::new().height(4),
        trust_warning,
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
