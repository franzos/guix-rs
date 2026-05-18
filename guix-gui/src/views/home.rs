//! Discover-style landing page: curated tiles grouped by category.
//!
//! Each tile is a click target that jumps to Search with the package name
//! pre-queried — install/remove still happens via the Search detail pane,
//! so this view stays read-only and free of operation state.

use iced::widget::{button, column, container, image, row, text, Column, Row, Space};
use iced::{Alignment, Element, Length};

use crate::app::{App, IconCacheEntry, Message};
use crate::recommended::{self, RecommendedApp};
use crate::styles::{self, BOLD, MUTED, TEXT};

const TILES_PER_ROW: usize = 3;
const TILE_HEIGHT: f32 = 96.0;
const ICON_SIZE: f32 = 48.0;

pub fn view(app: &App) -> Element<'_, Message> {
    let header = App::view_header("Home", None);
    let subtitle = text(
        "A starting point — well-known applications available in Guix. \
         Open one to install, or use Search for the full catalogue.",
    )
    .size(13)
    .color(MUTED);

    let mut content: Column<'_, Message> = column![header, subtitle].spacing(8);

    for (cat, apps) in recommended::grouped() {
        // When metadata is enabled, hide tiles whose icon lookup
        // definitively failed — the Home tab is curated as "apps with an
        // icon", so a placeholder forever is worse than disappearing.
        // While metadata is disabled, show every curated tile.
        let visible: Vec<&RecommendedApp> = apps
            .into_iter()
            .filter(|ra| tile_is_visible(app, ra))
            .collect();
        if visible.is_empty() {
            continue;
        }
        content = content.push(Space::new().height(Length::Fixed(8.0)));
        content = content.push(text(cat.label()).size(16).font(BOLD).color(TEXT));
        content = content.push(category_grid(app, &visible));
    }

    iced::widget::scrollable(content.padding(iced::Padding {
        top: 0.0,
        right: 12.0,
        bottom: 8.0,
        left: 0.0,
    }))
    .height(Length::Fill)
    .into()
}

fn category_grid<'a>(app: &'a App, apps: &[&'static RecommendedApp]) -> Element<'a, Message> {
    let mut grid: Column<'a, Message> = Column::new().spacing(10);
    for chunk in apps.chunks(TILES_PER_ROW) {
        let mut row: Row<'a, Message> = Row::new().spacing(10);
        for ra in chunk {
            row = row.push(tile(app, ra));
        }
        // Pad the last row with empty space so partial rows don't grow
        // their tiles to fill the width.
        for _ in chunk.len()..TILES_PER_ROW {
            row = row.push(Space::new().width(Length::FillPortion(1)));
        }
        grid = grid.push(row);
    }
    grid.into()
}

fn tile<'a>(app: &'a App, ra: &'static RecommendedApp) -> Element<'a, Message> {
    let installed = app.installed.packages.iter().any(|p| p.name == ra.name);

    let icon: Element<'a, Message> = if app.settings.app_metadata.enabled {
        match app.home_icons.get(ra.name) {
            Some(IconCacheEntry::Done(Some(bytes))) => {
                let handle = image::Handle::from_bytes(bytes.clone());
                image(handle)
                    .width(Length::Fixed(ICON_SIZE))
                    .height(Length::Fixed(ICON_SIZE))
                    .into()
            }
            _ => icon_placeholder(),
        }
    } else {
        icon_placeholder()
    };

    let name_row = row![
        text(ra.name).size(15).font(BOLD).color(TEXT),
        Space::new().width(Length::Fill),
        installed_badge(installed),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    let body = column![name_row, text(ra.blurb).size(11).color(MUTED),].spacing(4);

    let inner = row![icon, body]
        .spacing(12)
        .align_y(Alignment::Start)
        .width(Length::Fill);

    // `button` only accepts a button-style closure, so the card framing
    // lives on the inner container and the button itself stays ghost.
    let card = container(inner)
        .padding(12)
        .width(Length::Fill)
        .height(Length::Fixed(TILE_HEIGHT))
        .style(styles::card);
    button(card)
        .padding(0)
        .width(Length::FillPortion(1))
        .style(styles::btn_ghost)
        .on_press(Message::HomeAppClicked(ra.name.to_string()))
        .into()
}

fn tile_is_visible(app: &App, ra: &RecommendedApp) -> bool {
    if !app.settings.app_metadata.enabled {
        return true;
    }
    !matches!(
        app.home_icons.get(ra.name),
        Some(IconCacheEntry::Done(None))
    )
}

fn icon_placeholder<'a>() -> Element<'a, Message> {
    // Reserve the same footprint as a real icon so tiles don't reflow
    // once metadata loads in.
    container(Space::new())
        .width(Length::Fixed(ICON_SIZE))
        .height(Length::Fixed(ICON_SIZE))
        .into()
}

fn installed_badge<'a>(installed: bool) -> Element<'a, Message> {
    if installed {
        text("installed").size(10).color(styles::SUCCESS).into()
    } else {
        text("").size(10).into()
    }
}
