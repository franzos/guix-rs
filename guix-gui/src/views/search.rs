use iced::widget::{
    button, column, container, image, row, scrollable, text, text_input, Column, Row, Space,
};
use iced::{Element, Length};
use libguix::PackageSummary;

use crate::app::{App, Message, SearchError};
use crate::app_metadata::AppMetadata;
use crate::carrier::Carrier;
use crate::styles::{self, BOLD, MUTED};

const SYNOPSIS_CHAR_LIMIT: usize = 120;

pub fn view(app: &App) -> Element<'_, Message> {
    let header = App::view_header("Search", None);

    let input = text_input("Search packages...", &app.search.query)
        .on_input(Message::SearchInputChanged)
        .padding(10)
        .size(14)
        .width(Length::Fill);

    let warmup_hint: Element<'_, Message> = if !app.warmup_done {
        text("Loading package catalog...")
            .size(11)
            .color(MUTED)
            .into()
    } else {
        text("").size(11).into()
    };

    let status: Element<'_, Message> = if app.search.searching {
        text("Searching...").size(12).color(MUTED).into()
    } else if app.search.error.is_some() {
        text("").size(12).into()
    } else {
        text(format!("{} results", app.search.results.len()))
            .size(12)
            .color(MUTED)
            .into()
    };

    let truncated_hint: Element<'_, Message> = if app.search.truncated {
        text(format!(
            "Showing first {n} of \u{2265}{n} matches; refine your query.",
            n = app.search.last_limit
        ))
        .size(11)
        .color(MUTED)
        .into()
    } else {
        text("").size(11).into()
    };

    let error_banner: Element<'_, Message> = match &app.search.error {
        Some(err) => search_error_banner(err),
        None => text("").size(11).into(),
    };

    let panes = row![
        container(scrollable(result_list(
            &app.search.results,
            app.search.selected
        )))
        .width(Length::FillPortion(2))
        .height(Length::Fill)
        .padding(12)
        .style(styles::card),
        container(scrollable(detail_pane(app)))
            .width(Length::FillPortion(3))
            .height(Length::Fill)
            .padding(16)
            .style(styles::card),
    ]
    .spacing(12)
    .height(Length::Fill);

    column![
        header,
        input,
        warmup_hint,
        status,
        truncated_hint,
        error_banner,
        panes
    ]
    .spacing(8)
    .height(Length::Fill)
    .into()
}

fn search_error_banner(err: &SearchError) -> Element<'_, Message> {
    let label = text("Search error:")
        .size(12)
        .font(BOLD)
        .color(styles::DANGER);
    let summary = text(err.summary.clone()).size(12);
    let copy = button(text("Copy details").size(12))
        .padding([6, 12])
        .style(styles::btn_ghost)
        .on_press(Message::SearchErrorCopy);
    container(
        row![label, summary, Space::new().width(Length::Fill), copy]
            .spacing(8)
            .align_y(iced::Alignment::Center),
    )
    .padding(10)
    .width(Length::Fill)
    .style(styles::card_flat)
    .into()
}

fn result_list(results: &[PackageSummary], selected: Option<usize>) -> Element<'_, Message> {
    let mut col = Column::new().spacing(4);
    for (i, p) in results.iter().enumerate() {
        let row_content = column![
            text(p.name.clone()).font(BOLD).size(14),
            text(truncate(&p.synopsis, SYNOPSIS_CHAR_LIMIT)).size(11),
        ]
        .spacing(2);
        let btn = button(row_content)
            .padding(6)
            .width(Length::Fill)
            .style(styles::result_row_btn(selected == Some(i)))
            .on_press(Message::SearchResultSelected(i));
        col = col.push(btn);
    }
    col.into()
}

fn detail_pane(app: &App) -> Element<'_, Message> {
    let Some(i) = app.search.selected else {
        return text("Select a package to see details.")
            .size(14)
            .color(MUTED)
            .into();
    };
    let Some(p) = app.search.results.get(i) else {
        return text("Select a package to see details.")
            .size(14)
            .color(MUTED)
            .into();
    };

    let cached = app.metadata_cache.get(&p.name);
    let metadata = cached.and_then(|opt| opt.as_ref());

    let mut header = row![].spacing(10).align_y(iced::Alignment::Center);
    if let Some(icon_bytes) = metadata
        .and_then(|m| m.flathub.as_ref())
        .and_then(|f| f.icon_bytes.as_ref())
    {
        let handle = image::Handle::from_bytes(icon_bytes.clone());
        header = header.push(
            image(handle)
                .width(Length::Fixed(48.0))
                .height(Length::Fixed(48.0)),
        );
    }
    header = header.push(text(p.name.clone()).font(BOLD).size(20));
    header = header.push(text(p.version.clone()).size(14).color(MUTED));

    let mut col = column![header, text(p.synopsis.clone()).size(14)].spacing(8);

    if !p.description.is_empty() {
        col = col.push(text(p.description.clone()).size(12));
    }
    if !p.homepage.is_empty() {
        // Only turn the URL itself into a link — keeps the "homepage:"
        // prefix non-clickable so the surrounding text doesn't act like
        // a button hitbox.
        let link = button(text(p.homepage.clone()).size(12).color(styles::PRIMARY))
            .padding(0)
            .style(styles::btn_ghost)
            .on_press(Message::OpenUrl(p.homepage.clone()));
        col = col.push(
            row![text("homepage:").size(12).color(MUTED), link,]
                .spacing(6)
                .align_y(iced::Alignment::Center),
        );
    }
    if !p.license.is_empty() {
        col = col.push(
            text(format!("license: {}", p.license))
                .size(12)
                .color(MUTED),
        );
    }
    if !p.outputs.is_empty() {
        col = col.push(
            text(format!("outputs: {}", p.outputs.join(", ")))
                .size(12)
                .color(MUTED),
        );
    }

    let already_installed = app.installed.packages.iter().any(|ip| ip.name == p.name);
    let busy = app.active_op.is_some();
    let action_btn = if already_installed {
        let msg = (!busy).then(|| Message::RemoveRequested(p.name.clone()));
        button(text("Remove").size(13))
            .padding([8, 20])
            .style(styles::btn_danger)
            .on_press_maybe(msg)
    } else {
        let msg = (!busy).then(|| Message::InstallRequested(p.name.clone()));
        button(text("Install").size(13))
            .padding([8, 20])
            .style(styles::btn_primary)
            .on_press_maybe(msg)
    };
    col = col.push(Space::new().height(Length::Fixed(4.0)));
    col = col.push(row![action_btn].spacing(8));

    if app.settings.app_metadata.enabled {
        if let Some(strip) = screenshots_strip(metadata, cached.is_some()) {
            col = col.push(Space::new().height(Length::Fixed(8.0)));
            col = col.push(strip);
        }
    }

    col.into()
}

/// Horizontal scroll of screenshot thumbnails when at least one is
/// available. Returns `None` when nothing's been fetched yet or both
/// catalogs missed — silence is better than an empty placeholder for an
/// opt-in feature.
fn screenshots_strip<'a>(
    metadata: Option<&'a AppMetadata>,
    cache_present: bool,
) -> Option<Element<'a, Message>> {
    match metadata {
        Some(m) if !m.is_empty() => {
            let mut rendered: Row<'a, Message> = Row::new().spacing(8);
            let mut count = 0;
            let push_thumb = |row: Row<'a, Message>, bytes: &Vec<u8>| -> Row<'a, Message> {
                let handle = image::Handle::from_bytes(bytes.clone());
                let thumb = image(handle)
                    .width(Length::Fixed(240.0))
                    .height(Length::Fixed(150.0));
                // Wrap in a no-padding button so the whole thumbnail is
                // a click target. `style: ghost` keeps the visual flat.
                let btn = button(thumb)
                    .padding(0)
                    .style(styles::btn_ghost)
                    .on_press(Message::LightboxOpened(Carrier::new(bytes.clone())));
                row.push(btn)
            };
            if let Some(fh) = m.flathub.as_ref() {
                for s in &fh.screenshots {
                    if let Some(bytes) = &s.bytes {
                        rendered = push_thumb(rendered, bytes);
                        count += 1;
                    }
                }
            }
            for s in &m.debian_screenshots {
                if let Some(bytes) = &s.bytes {
                    rendered = push_thumb(rendered, bytes);
                    count += 1;
                }
            }
            if count == 0 {
                return None;
            }
            let attribution = m
                .flathub
                .as_ref()
                .map(|f| format!("Screenshots via Flathub ({})", f.component_id))
                .unwrap_or_else(|| "Screenshots via screenshots.debian.net".to_string());
            Some(
                column![
                    text(attribution).size(11).color(MUTED),
                    scrollable(rendered).direction(scrollable::Direction::Horizontal(
                        scrollable::Scrollbar::default()
                    )),
                ]
                .spacing(4)
                .into(),
            )
        }
        Some(_) => None,
        None if cache_present => Some(
            text("Loading icons / screenshots...")
                .size(11)
                .color(MUTED)
                .into(),
        ),
        None => None,
    }
}

fn truncate(s: &str, n: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= n {
        s.to_owned()
    } else {
        let mut out: String = s.chars().take(n).collect();
        out.push('…');
        out
    }
}
