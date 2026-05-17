use iced::font::Weight;
use iced::widget::{button, column, container, row, scrollable, text, text_input, Column, Space};
use iced::{Element, Font, Length};
use libguix::PackageSummary;

use crate::app::{App, Message, SearchError};

const SYNOPSIS_CHAR_LIMIT: usize = 120;

const BOLD: Font = Font {
    weight: Weight::Bold,
    ..Font::DEFAULT
};

pub fn view(app: &App) -> Element<'_, Message> {
    let input = text_input("Search packages...", &app.search.query)
        .on_input(Message::SearchInputChanged)
        .padding(8)
        .width(Length::Fill);

    let warmup_hint: Element<'_, Message> = if !app.warmup_done {
        text("Loading package catalog...")
            .size(11)
            .style(text::secondary)
            .into()
    } else {
        text("").size(11).into()
    };

    let status: Element<'_, Message> = if app.search.searching {
        text("Searching...").size(12).into()
    } else if app.search.error.is_some() {
        text("").size(12).into()
    } else {
        text(format!("{} results", app.search.results.len()))
            .size(12)
            .into()
    };

    let truncated_hint: Element<'_, Message> = if app.search.truncated {
        text(format!(
            "Showing first {n} of \u{2265}{n} matches; refine your query.",
            n = app.search.last_limit
        ))
        .size(11)
        .into()
    } else {
        text("").size(11).into()
    };

    let error_banner: Element<'_, Message> = match &app.search.error {
        Some(err) => search_error_banner(err),
        None => text("").size(11).into(),
    };

    let list = result_list(&app.search.results, app.search.selected);
    let detail = detail_pane(app);

    column![
        input,
        warmup_hint,
        status,
        truncated_hint,
        error_banner,
        row![
            container(scrollable(list))
                .width(Length::FillPortion(2))
                .height(Length::Fill),
            container(scrollable(detail))
                .width(Length::FillPortion(3))
                .height(Length::Fill),
        ]
        .spacing(12)
        .height(Length::Fill),
    ]
    .spacing(8)
    .height(Length::Fill)
    .into()
}

fn search_error_banner(err: &SearchError) -> Element<'_, Message> {
    let label = text("Search error:").size(12).font(BOLD);
    let summary = text(err.summary.clone()).size(12);
    let copy = button(text("Copy details").size(12)).on_press(Message::SearchErrorCopy);
    container(
        row![label, summary, Space::new().width(Length::Fill), copy]
            .spacing(8)
            .align_y(iced::Alignment::Center),
    )
    .padding(6)
    .width(Length::Fill)
    .style(container::rounded_box)
    .into()
}

fn result_list(results: &[PackageSummary], _selected: Option<usize>) -> Element<'_, Message> {
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
            .on_press(Message::SearchResultSelected(i));
        col = col.push(btn);
    }
    col.into()
}

fn detail_pane(app: &App) -> Element<'_, Message> {
    let Some(i) = app.search.selected else {
        return text("Select a package to see details.").size(14).into();
    };
    let Some(p) = app.search.results.get(i) else {
        return text("Select a package to see details.").size(14).into();
    };

    let header = row![
        text(p.name.clone()).font(BOLD).size(20),
        text(p.version.clone()).size(14),
    ]
    .spacing(8);

    let mut col = column![header, text(p.synopsis.clone()).size(14)].spacing(8);

    if !p.description.is_empty() {
        col = col.push(text(p.description.clone()).size(12));
    }
    if !p.homepage.is_empty() {
        col = col.push(text(format!("homepage: {}", p.homepage)).size(12));
    }
    if !p.license.is_empty() {
        col = col.push(text(format!("license: {}", p.license)).size(12));
    }
    if !p.outputs.is_empty() {
        col = col.push(text(format!("outputs: {}", p.outputs.join(", "))).size(12));
    }

    let already_installed = app.installed.packages.iter().any(|ip| ip.name == p.name);
    let busy = app.active_op.is_some();
    let action_btn = if already_installed {
        let msg = (!busy).then(|| Message::RemoveRequested(p.name.clone()));
        button(text("Remove")).on_press_maybe(msg)
    } else {
        let msg = (!busy).then(|| Message::InstallRequested(p.name.clone()));
        button(text("Install")).on_press_maybe(msg)
    };
    col = col.push(row![action_btn].spacing(8));

    col.into()
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
