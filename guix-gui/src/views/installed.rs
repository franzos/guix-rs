use iced::font::Weight;
use iced::widget::{button, column, row, scrollable, text, Column, Space};
use iced::{Element, Font, Length};

use crate::app::{App, Message};

const SECONDARY_LINE_CHAR_LIMIT: usize = 90;

const BOLD: Font = Font {
    weight: Weight::Bold,
    ..Font::DEFAULT
};

pub fn view(app: &App) -> Element<'_, Message> {
    let header = row![
        text(format!(
            "{} installed packages",
            app.installed.packages.len()
        ))
        .size(14),
        button(text("Refresh")).on_press(Message::InstalledRefresh),
    ]
    .spacing(12);

    let status: Element<'_, Message> = if app.installed.refreshing {
        text("Loading...").size(12).into()
    } else if let Some(err) = &app.installed.error {
        text(format!("Error: {err}")).size(12).into()
    } else {
        text("").size(12).into()
    };

    let mut rows = Column::new().spacing(6);
    for p in &app.installed.packages {
        let store_path = p.store_path.display().to_string();
        let prefix = format!("{} ({}) ", p.version, p.output);
        let budget = SECONDARY_LINE_CHAR_LIMIT.saturating_sub(prefix.chars().count());
        let path_display = truncate_path_left(&store_path, budget.max(8));
        let secondary = format!("{prefix}{path_display}");

        let card = column![
            text(p.name.clone()).font(BOLD).size(14),
            text(secondary).size(11),
        ]
        .spacing(2);

        rows = rows.push(
            row![
                card,
                Space::new().width(Length::Fill),
                button(text("Remove")).on_press(Message::RemoveRequested(p.name.clone())),
            ]
            .spacing(8)
            .width(Length::Fill),
        );
    }

    column![header, status, scrollable(rows).height(Length::Fill)]
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
