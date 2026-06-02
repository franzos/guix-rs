//! Two-scope Updates tab — each scope has its own fetch + apply pair
//! because reconfigure never auto-pulls. See NOTES.md "The two catalogs".

use std::path::Path;

use iced::widget::{button, column, container, row, text, tooltip, Column};
use iced::{Element, Font, Length};

use crate::app::{App, Message, PendingReconfigure};
use crate::settings::Tab;
use crate::styles::{self, BOLD, MUTED};
use crate::util::{humanize_age, short_hash};

pub fn view(app: &App) -> Element<'_, Message> {
    let busy = app.active_op.is_some();
    let source = app.settings.source_config_path.as_deref();
    let header = App::view_header(crate::t!("updates-title"), None);

    let sections = column![
        user_packages_section(app, busy),
        system_section(app, source, busy),
    ]
    .spacing(16)
    .width(Length::Fill);

    column![header, sections]
        .spacing(8)
        .height(Length::Fill)
        .into()
}

fn user_packages_section<'a>(app: &'a App, busy: bool) -> Element<'a, Message> {
    let blurb: Element<'a, Message> = text(crate::t!("updates-your-packages-blurb"))
        .size(13)
        .color(MUTED)
        .into();

    let summary = user_summary(app);

    let fetch_btn = primary_button(
        crate::t!("updates-fetch-latest"),
        "guix pull",
        Message::FetchUserCatalogClicked,
        busy,
        false,
    );
    let apply_btn = primary_button(
        crate::t!("updates-update-my-packages"),
        "guix package -u",
        Message::UpgradeClicked,
        busy,
        true,
    );
    let actions = row![fetch_btn, apply_btn].spacing(8);

    let body = column![blurb, summary, actions].spacing(10);
    section_card(crate::t!("updates-your-packages"), body.into())
}

fn system_section<'a>(app: &'a App, source: Option<&'a Path>, busy: bool) -> Element<'a, Message> {
    let blurb: Element<'a, Message> = text(crate::t!("updates-system-blurb"))
        .size(13)
        .color(MUTED)
        .into();

    let summary = system_summary(app);

    let source_display: Element<'a, Message> = match source {
        Some(p) => text(crate::t!(
            "updates-source-config",
            path = p.display().to_string()
        ))
        .size(12)
        .color(MUTED)
        .into(),
        None => text(crate::t!("updates-source-config-unset"))
            .size(12)
            .color(MUTED)
            .into(),
    };
    let open_system = button(text(crate::t!("common-open-settings")).size(12))
        .padding([6, 12])
        .style(styles::btn_ghost)
        .on_press(Message::TabSelected(Tab::System));
    let source_row = row![source_display, open_system]
        .spacing(10)
        .align_y(iced::Alignment::Center);

    let fetch_btn = primary_button(
        crate::t!("updates-fetch-system"),
        "pkexec guix pull",
        Message::FetchSystemCatalogClicked,
        busy,
        false,
    );

    let action_area: Element<'a, Message> =
        if let Some(pending) = app.system.pending_reconfigure.as_ref() {
            confirm_reconfigure_card(pending).into()
        } else {
            let apply_on_press: Option<Message> = if source.is_some() && !busy {
                Some(Message::ReconfigureClicked)
            } else {
                None
            };
            let apply_inner = button(text(crate::t!("updates-update-system")).size(13))
                .padding([8, 16])
                .style(styles::btn_primary)
                .on_press_maybe(apply_on_press);
            let apply_btn: Element<'a, Message> = tooltip(
                apply_inner,
                container(text(crate::t!("updates-update-system-tip")))
                    .padding(6)
                    .style(styles::card_flat),
                tooltip::Position::Top,
            )
            .into();
            row![fetch_btn, apply_btn].spacing(8).into()
        };

    let body = column![blurb, summary, source_row, action_area].spacing(10);
    section_card(crate::t!("updates-system"), body.into())
}

/// Confirmation card for `pkexec guix system reconfigure`. Lists the
/// config and every `-L` load path so the user authorises each
/// root-loaded module path explicitly.
fn confirm_reconfigure_card<'a>(pending: &'a PendingReconfigure) -> Column<'a, Message> {
    let header = text(crate::t!("updates-confirm-reconfigure"))
        .size(14)
        .font(BOLD);
    let blurb = text(crate::t!("updates-reconfigure-blurb"))
        .size(12)
        .color(MUTED);

    let cfg_label = text(crate::t!("updates-config")).size(12).color(MUTED);
    let cfg_value = text(pending.config_path.display().to_string())
        .size(12)
        .font(Font::MONOSPACE);

    let mut col: Column<'a, Message> = column![header, blurb, cfg_label, cfg_value].spacing(6);

    let lp_label = if pending.load_paths.is_empty() {
        text(crate::t!("updates-load-paths-none"))
            .size(12)
            .color(MUTED)
    } else {
        text(crate::t!(
            "updates-load-paths",
            count = pending.load_paths.len()
        ))
        .size(12)
        .color(MUTED)
    };
    col = col.push(lp_label);
    for p in &pending.load_paths {
        col = col.push(text(p.display().to_string()).size(12).font(Font::MONOSPACE));
    }

    let confirm = button(text(crate::t!("updates-confirm-reconfigure-btn")).size(13))
        .padding([8, 16])
        .style(styles::btn_primary)
        .on_press(Message::ReconfigureConfirmed);
    let cancel = button(text(crate::t!("common-cancel")).size(13))
        .padding([8, 16])
        .style(styles::btn_ghost)
        .on_press(Message::ReconfigureCancelled);
    col = col.push(row![confirm, cancel].spacing(8));

    col
}

fn section_card<'a>(
    header_label: impl Into<String>,
    body: Element<'a, Message>,
) -> Element<'a, Message> {
    let header = text(header_label.into()).size(16).font(BOLD);
    let inner = column![header, body].spacing(10);
    container(inner)
        .padding(20)
        .width(Length::Fill)
        .style(styles::card)
        .into()
}

fn primary_button<'a>(
    label: impl Into<String>,
    tip: &'a str,
    msg: Message,
    busy: bool,
    is_primary: bool,
) -> Element<'a, Message> {
    let style: fn(&iced::Theme, iced::widget::button::Status) -> iced::widget::button::Style =
        if is_primary {
            styles::btn_primary
        } else {
            styles::btn_secondary
        };
    let btn = button(text(label.into()).size(13))
        .padding([8, 16])
        .style(style)
        .on_press_maybe(if busy { None } else { Some(msg) });
    tooltip(
        btn,
        container(text(tip).size(12))
            .padding(6)
            .style(styles::card_flat),
        tooltip::Position::Top,
    )
    .into()
}

fn user_summary(app: &App) -> Element<'_, Message> {
    if app.updates.loading_channels {
        return text(crate::t!("updates-loading-channels"))
            .size(12)
            .color(MUTED)
            .into();
    }
    if let Some(e) = &app.updates.error {
        return text(crate::t!("updates-error-channels", error = e.clone()))
            .size(12)
            .color(styles::DANGER)
            .into();
    }

    let last = match app.updates.mtimes.user_pull {
        Some(t) => crate::t!("updates-last-pulled", age = humanize_age(t)),
        None => crate::t!("updates-last-pulled-never"),
    };

    let channels = if app.updates.channels.is_empty() {
        crate::t!("updates-channels-none")
    } else {
        let mut parts: Vec<String> = Vec::with_capacity(app.updates.channels.len());
        for c in &app.updates.channels {
            let h = c
                .commit
                .as_deref()
                .map(|c| short_hash(c).to_string())
                .unwrap_or_else(|| crate::t!("updates-channel-no-commit"));
            parts.push(format!("{} {}", c.name, h));
        }
        crate::t!("updates-channels", list = parts.join(", "))
    };

    let mut col: Column<'_, Message> = Column::new().spacing(2);
    col = col.push(text(last).size(12).color(MUTED));
    col = col.push(text(channels).size(12).color(MUTED));
    col.into()
}

fn system_summary(app: &App) -> Element<'_, Message> {
    let root = match app.updates.mtimes.root_pull {
        Some(t) => crate::t!("updates-last-pulled-root", age = humanize_age(t)),
        None => crate::t!("updates-last-pulled-root-never"),
    };
    let reconf = match app.updates.mtimes.system_profile {
        Some(t) => crate::t!("updates-last-reconfigured", age = humanize_age(t)),
        None => crate::t!("updates-last-reconfigured-never"),
    };

    let mut col: Column<'_, Message> = Column::new().spacing(2);
    col = col.push(text(root).size(12).color(MUTED));
    col = col.push(text(reconf).size(12).color(MUTED));
    col.into()
}
