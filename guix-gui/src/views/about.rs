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

pub fn view(_app: &App) -> Element<'_, Message> {
    let header = App::view_header("About", None);

    let version_card = info_card(
        "Guix GUI",
        format!(
            "Version {}\nDesktop frontend for the Guix package manager.",
            env!("CARGO_PKG_VERSION")
        ),
    );

    let authors_card = section_card(
        "Authors",
        column![text("Franz Geffke <mail@gofranz.com>").size(13).color(TEXT)].spacing(4),
    );

    let source_card = section_card(
        "Source & contributions",
        column![
            text("Bug reports and pull requests are welcome.")
                .size(13)
                .color(MUTED),
            link_row(REPO_URL),
        ]
        .spacing(6),
    );

    let license_card = section_card(
        "License",
        column![
            text("Guix GUI is released under the GNU General Public License v3.0.")
                .size(13)
                .color(TEXT),
            text(
                "You may redistribute and modify it under the terms of that licence. \
                 See the LICENSE file in the repository for the full text."
            )
            .size(12)
            .color(MUTED),
        ]
        .spacing(6),
    );

    let third_party_card = section_card(
        "Third-party data",
        column![
            text(
                "Application icons and screenshots are fetched from external services \
                 when you enable third-party metadata in Settings. Trademarks, icons, \
                 and screenshots remain the property of their respective projects."
            )
            .size(12)
            .color(MUTED),
            link_row(FLATHUB_URL),
            link_row(DEBIAN_SCREENSHOTS_URL),
        ]
        .spacing(6),
    );

    let dependencies_card = section_card(
        "Built with",
        column![
            text(
                "libguix (MIT / Apache-2.0) · iced (MIT) · reqwest (MIT / Apache-2.0) · \
                 tokio (MIT) · serde (MIT / Apache-2.0) · directories (MIT / Apache-2.0) · \
                 vt100 (MIT) · tracing (MIT) · rustls (Apache-2.0 / ISC / MIT)"
            )
            .size(12)
            .color(MUTED),
            text("Licences of individual crates are listed in their respective repositories.")
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
        dependencies_card,
    ]
    .spacing(16);

    scrollable(body).height(Length::Fill).into()
}

fn section_card<'a>(title: &'a str, body: Column<'a, Message>) -> Element<'a, Message> {
    container(
        column![text(title).size(16).font(BOLD).color(TEXT), body].spacing(8),
    )
    .padding(20)
    .width(Length::Fill)
    .style(styles::card)
    .into()
}

fn info_card<'a>(title: &'a str, body: String) -> Element<'a, Message> {
    container(
        column![
            text(title).size(20).font(BOLD).color(TEXT),
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
