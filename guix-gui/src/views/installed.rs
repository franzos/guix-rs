use iced::widget::{button, column, container, row, scrollable, text, Column, Space};
use iced::{Element, Length};

use crate::app::{App, Message};
use crate::styles::{self, BOLD, MUTED};

const SECONDARY_LINE_CHAR_LIMIT: usize = 90;

pub fn view(app: &App) -> Element<'_, Message> {
    let refresh = button(text(crate::t!("common-refresh")).size(13))
        .padding([8, 16])
        .style(styles::btn_secondary)
        .on_press(Message::InstalledRefresh);
    let header = App::view_header(crate::t!("installed-title"), Some(refresh.into()));

    let count: Element<'_, Message> = text(crate::t!(
        "installed-count",
        count = app.installed.packages.len()
    ))
    .size(12)
    .color(MUTED)
    .into();

    let status: Element<'_, Message> = if app.installed.refreshing {
        text(crate::t!("installed-loading"))
            .size(12)
            .color(MUTED)
            .into()
    } else if let Some(err) = &app.installed.error {
        text(crate::t!("installed-error", error = err.clone()))
            .size(12)
            .color(styles::DANGER)
            .into()
    } else {
        text("").size(12).into()
    };

    let mut rows = Column::new().spacing(8);
    for p in &app.installed.packages {
        let store_path = p.store_path.display().to_string();
        let prefix = format!("{} ({}) ", p.version, p.output);
        let budget = SECONDARY_LINE_CHAR_LIMIT.saturating_sub(prefix.chars().count());
        let path_display = truncate_path_left(&store_path, budget.max(8));
        let secondary = format!("{prefix}{path_display}");

        let info = column![
            text(p.name.clone()).font(BOLD).size(14),
            text(secondary).size(11).color(MUTED),
        ]
        .spacing(2);

        let remove_btn = button(text(crate::t!("common-remove")).size(12))
            .padding([6, 14])
            .style(styles::btn_danger)
            .on_press(Message::RemoveRequested(p.name.clone()));

        let card = container(
            row![info, Space::new().width(Length::Fill), remove_btn]
                .spacing(8)
                .align_y(iced::Alignment::Center)
                .width(Length::Fill),
        )
        .padding(14)
        .width(Length::Fill)
        .style(styles::card_flat);

        rows = rows.push(card);
    }

    column![header, count, status, scrollable(rows).height(Length::Fill)]
        .spacing(8)
        .height(Length::Fill)
        .into()
}

/// Left-truncate so the package name tail survives.
fn truncate_path_left(s: &str, n: usize) -> String {
    let count = s.chars().count();
    if count <= n {
        return s.to_owned();
    }
    let keep = n.saturating_sub(1);
    let skip = count.saturating_sub(keep);
    let tail: String = s.chars().skip(skip).collect();
    format!("\u{2026}{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_path_left_short_passthrough() {
        assert_eq!(truncate_path_left("/gnu/store/x", 20), "/gnu/store/x");
    }

    #[test]
    fn truncate_path_left_keeps_tail() {
        let p = "/gnu/store/abcdef-wpa-supplicant-2.10";
        let t = truncate_path_left(p, 20);
        assert!(
            t.starts_with('\u{2026}'),
            "expected leading ellipsis: {t:?}"
        );
        assert!(
            t.ends_with("wpa-supplicant-2.10"),
            "expected tail preserved: {t:?}"
        );
        assert!(t.chars().count() <= 20);
    }
}
