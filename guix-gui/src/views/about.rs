//! Project metadata, contributors, source link, and license notices.
//!
//! Plain content view — no async state, no message routing beyond
//! `OpenUrl` for the homepage / repository / data-source links.

use iced::widget::{button, column, container, scrollable, text, Column};
use iced::{Element, Length};

use crate::app::{App, Message};
use crate::styles::{self, BOLD, MUTED, PRIMARY, TEXT};

const REPO_URL: &str = "https://github.com/franzos/guix-rs";
const FLATHUB_URL: &str = "https://flathub.org";
const DEBIAN_SCREENSHOTS_URL: &str = "https://screenshots.debian.net";
const TOYS_WHEREIS_URL: &str = "https://toys.whereis.social";

pub fn view(_app: &App) -> Element<'_, Message> {
    let header = App::view_header(crate::t!("about-title"), None);

    let version_card = info_card(
        crate::t!("app-title"),
        format!(
            "{}\n{}",
            crate::t!("about-version", version = env!("CARGO_PKG_VERSION")),
            crate::t!("about-tagline"),
        ),
    );

    let authors_card = section_card(
        crate::t!("about-authors"),
        column![text("Franz Geffke <mail@gofranz.com>").size(13).color(TEXT)].spacing(4),
    );

    let source_card = section_card(
        crate::t!("about-source"),
        column![
            text(crate::t!("about-source-blurb")).size(13).color(MUTED),
            link_row(REPO_URL),
        ]
        .spacing(6),
    );

    let license_card = section_card(
        crate::t!("about-license"),
        column![
            text(crate::t!("about-license-line")).size(13).color(TEXT),
            text(crate::t!("about-license-detail"))
                .size(12)
                .color(MUTED),
        ]
        .spacing(6),
    );

    let third_party_card = section_card(
        crate::t!("about-third-party"),
        column![
            text(crate::t!("about-third-party-blurb"))
                .size(12)
                .color(MUTED),
            link_row(FLATHUB_URL),
            link_row(DEBIAN_SCREENSHOTS_URL),
        ]
        .spacing(6),
    );

    let discovery_card = section_card(
        crate::t!("about-channel-discovery"),
        column![
            text(crate::t!("about-channel-discovery-blurb"))
                .size(12)
                .color(MUTED),
            link_row(TOYS_WHEREIS_URL),
        ]
        .spacing(6),
    );

    let dependencies_card = section_card(
        crate::t!("about-built-with"),
        column![
            text(
                "libguix (MIT / Apache-2.0) · iced (MIT) · reqwest (MIT / Apache-2.0) · \
                 tokio (MIT) · serde (MIT / Apache-2.0) · directories (MIT / Apache-2.0) · \
                 vt100 (MIT) · tracing (MIT) · rustls (Apache-2.0 / ISC / MIT)"
            )
            .size(12)
            .color(MUTED),
            text(crate::t!("about-built-with-detail"))
                .size(11)
                .color(MUTED),
        ]
        .spacing(6),
    );

    let body = column![
        header,
        version_card,
        authors_card,
        source_card,
        license_card,
        third_party_card,
        discovery_card,
        dependencies_card,
    ]
    .spacing(16);

    scrollable(body).height(Length::Fill).into()
}

fn section_card<'a>(title: impl Into<String>, body: Column<'a, Message>) -> Element<'a, Message> {
    container(column![text(title.into()).size(16).font(BOLD).color(TEXT), body].spacing(8))
        .padding(20)
        .width(Length::Fill)
        .style(styles::card)
        .into()
}

fn info_card<'a>(title: impl Into<String>, body: String) -> Element<'a, Message> {
    container(
        column![
            text(title.into()).size(20).font(BOLD).color(TEXT),
            text(body).size(13).color(MUTED),
        ]
        .spacing(6),
    )
    .padding(20)
    .width(Length::Fill)
    .style(styles::card)
    .into()
}

fn link_row<'a>(url: &'a str) -> Element<'a, Message> {
    button(text(url).size(13).color(PRIMARY))
        .padding(0)
        .style(styles::btn_ghost)
        .on_press(Message::OpenUrl(url.to_string()))
        .into()
}
