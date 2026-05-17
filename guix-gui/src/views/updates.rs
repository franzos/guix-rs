//! Two-scope Updates tab — each scope has its own fetch + apply pair
//! because reconfigure never auto-pulls. See NOTES.md "The two catalogs".

use std::path::Path;

use iced::widget::{button, column, container, row, text, tooltip, Column};
use iced::{Element, Length};

use crate::app::{App, Message};
use crate::settings::Tab;
use crate::styles::{self, BOLD, MUTED};
use crate::util::{humanize_age, short_hash};

pub fn view(app: &App) -> Element<'_, Message> {
    let busy = app.active_op.is_some();
    let source = app.settings.source_config_path.as_deref();
    let header = App::view_header("Updates", None);

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
    let blurb: Element<'a, Message> = text("Manage your user-level packages.")
        .size(13)
        .color(MUTED)
        .into();

    let summary = user_summary(app);

    let fetch_btn = primary_button(
        "Fetch latest catalog",
        "guix pull",
        Message::FetchUserCatalogClicked,
        busy,
        false,
    );
    let apply_btn = primary_button(
        "Update my packages",
        "guix package -u",
        Message::UpgradeClicked,
        busy,
        true,
    );
    let actions = row![fetch_btn, apply_btn].spacing(8);

    let body = column![blurb, summary, actions].spacing(10);
    section_card("Your packages", body.into())
}

fn system_section<'a>(app: &'a App, source: Option<&'a Path>, busy: bool) -> Element<'a, Message> {
    let blurb: Element<'a, Message> =
        text("Apply your system configuration. Requires admin authentication.")
            .size(13)
            .color(MUTED)
            .into();

    let summary = system_summary(app);

    let source_display: Element<'a, Message> = match source {
        Some(p) => text(format!("Source config: {}", p.display()))
            .size(12)
            .color(MUTED)
            .into(),
        None => text("Source config: (not set — open Settings to choose)")
            .size(12)
            .color(MUTED)
            .into(),
    };
    let open_system = button(text("Open Settings").size(12))
        .padding([6, 12])
        .style(styles::btn_ghost)
        .on_press(Message::TabSelected(Tab::System));
    let source_row = row![source_display, open_system]
        .spacing(10)
        .align_y(iced::Alignment::Center);

    let fetch_btn = primary_button(
        "Fetch system catalog",
        "pkexec guix pull",
        Message::FetchSystemCatalogClicked,
        busy,
        false,
    );

    let apply_on_press: Option<Message> = if source.is_some() && !busy {
        Some(Message::ReconfigureClicked)
    } else {
        None
    };
    let apply_inner = button(text("Update system").size(13))
        .padding([8, 16])
        .style(styles::btn_primary)
        .on_press_maybe(apply_on_press);
    let apply_btn: Element<'a, Message> = tooltip(
        apply_inner,
        container(text("pkexec guix system reconfigure"))
            .padding(6)
            .style(styles::card_flat),
        tooltip::Position::Top,
    )
    .into();

    let actions = row![fetch_btn, apply_btn].spacing(8);

    let _ = app;
    let body = column![blurb, summary, source_row, actions].spacing(10);
    section_card("System", body.into())
}

fn section_card<'a>(header_label: &'a str, body: Element<'a, Message>) -> Element<'a, Message> {
    let header = text(header_label).size(16).font(BOLD);
    let inner = column![header, body].spacing(10);
    container(inner)
        .padding(20)
        .width(Length::Fill)
        .style(styles::card)
        .into()
}

fn primary_button<'a>(
    label: &'a str,
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
    let btn = button(text(label).size(13))
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
        return text("Loading channels...").size(12).color(MUTED).into();
    }
    if let Some(e) = &app.updates.error {
        return text(format!("Error loading channels: {e}"))
            .size(12)
            .color(styles::DANGER)
            .into();
    }

    let last = match app.updates.mtimes.user_pull {
        Some(t) => format!("Last pulled: {}.", humanize_age(t)),
        None => "Last pulled: never.".to_string(),
    };

    let channels = if app.updates.channels.is_empty() {
        "Channels: (none discovered).".to_string()
    } else {
        let mut parts: Vec<String> = Vec::with_capacity(app.updates.channels.len());
        for c in &app.updates.channels {
            let h = c.commit.as_deref().map(short_hash).unwrap_or("(no commit)");
            parts.push(format!("{} {}", c.name, h));
        }
        format!("Channels: {}.", parts.join(", "))
    };

    let mut col: Column<'_, Message> = Column::new().spacing(2);
    col = col.push(text(last).size(12).color(MUTED));
    col = col.push(text(channels).size(12).color(MUTED));
    col.into()
}

fn system_summary(app: &App) -> Element<'_, Message> {
    let root = match app.updates.mtimes.root_pull {
        Some(t) => format!("Last pulled (root): {}.", humanize_age(t)),
        None => "Last pulled (root): never.".to_string(),
    };
    let reconf = match app.updates.mtimes.system_profile {
        Some(t) => format!("Last reconfigured: {}.", humanize_age(t)),
        None => "Last reconfigured: never (not a Guix System host?).".to_string(),
    };

    let mut col: Column<'_, Message> = Column::new().spacing(2);
    col = col.push(text(root).size(12).color(MUTED));
    col = col.push(text(reconf).size(12).color(MUTED));
    col.into()
}
