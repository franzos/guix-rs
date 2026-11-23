use iced::widget::{
    button, column, container, image, row, scrollable, text, text_input, toggler, Column, Row,
    Space,
};
use iced::{Element, Length};
use libguix::{InstalledPackage, PackageSummary, SandboxInfo};

use crate::app::{App, Message, PendingSwap, SearchError};
use crate::app_metadata::AppMetadata;
use crate::carrier::Carrier;
use crate::styles::{self, BOLD, MUTED};
use crate::views::sandbox::{install_state, permissions_block, InstallState, SessionType};

const SYNOPSIS_CHAR_LIMIT: usize = 120;

pub fn view(app: &App) -> Element<'_, Message> {
    let header = App::view_header(crate::t!("search-title"), None);

    let input = text_input(&crate::t!("search-placeholder"), &app.search.query)
        .on_input(Message::SearchInputChanged)
        .padding(10)
        .size(14)
        .width(Length::Fill);

    let warmup_hint: Element<'_, Message> = if !app.warmup_done {
        text(crate::t!("search-loading-catalog"))
            .size(11)
            .color(MUTED)
            .into()
    } else {
        text("").size(11).into()
    };

    let status: Element<'_, Message> = if app.search.searching {
        text(crate::t!("search-searching"))
            .size(12)
            .color(MUTED)
            .into()
    } else if app.search.error.is_some() {
        text("").size(12).into()
    } else {
        text(crate::t!(
            "search-results",
            count = app.search.results.len()
        ))
        .size(12)
        .color(MUTED)
        .into()
    };

    let truncated_hint: Element<'_, Message> = if app.search.truncated {
        text(crate::t!(
            "search-truncated",
            shown = app.search.last_limit,
            total = app.search.last_limit
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
    let label = text(crate::t!("search-error-label"))
        .size(12)
        .font(BOLD)
        .color(styles::DANGER);
    let summary = text(err.summary.clone()).size(12);
    let copy = button(text(crate::t!("search-copy-details")).size(12))
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
        return text(crate::t!("search-select-prompt"))
            .size(14)
            .color(MUTED)
            .into();
    };
    let Some(p) = app.search.results.get(i) else {
        return text(crate::t!("search-select-prompt"))
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
    } else {
        header = header.push(crate::fallback_icon::fallback_icon(&p.name, 48.0));
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
            row![
                text(crate::t!("search-homepage")).size(12).color(MUTED),
                link,
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        );
    }
    if !p.license.is_empty() {
        col = col.push(
            text(crate::t!("search-license", license = p.license.clone()))
                .size(12)
                .color(MUTED),
        );
    }
    if !p.outputs.is_empty() {
        col = col.push(
            text(crate::t!("search-outputs", outputs = p.outputs.join(", ")))
                .size(12)
                .color(MUTED),
        );
    }

    let busy = app.active_op.is_some();
    let state = install_state(&app.installed.packages, &p.name);
    let switch_on = sandbox_switch_on(state, app.search.sandboxed);
    if let Some(variant) = p.sandbox.as_ref() {
        col = col.push(
            toggler(switch_on)
                .label(crate::t!("sandbox-label"))
                .text_size(13)
                .on_toggle_maybe((!busy).then_some(Message::SandboxToggled)),
        );
        if switch_on {
            col = col.push(permissions_block(&variant.info, app.session));
        }
    }

    col = col.push(Space::new().height(Length::Fixed(4.0)));
    if let Some(pending) = app.search.pending_swap.as_ref() {
        let info = p.sandbox.as_ref().map(|v| &v.info);
        col = col.push(confirm_swap_card(pending, info, app.session, busy));
    } else {
        let action_btn = match pane_action(p, &app.installed.packages, app.search.sandboxed) {
            PaneAction::Remove(spec) => button(text(crate::t!("search-remove")).size(13))
                .padding([8, 20])
                .style(styles::btn_danger)
                .on_press_maybe((!busy).then(|| Message::RemoveRequested(spec))),
            PaneAction::Install(name) => button(text(crate::t!("search-install")).size(13))
                .padding([8, 20])
                .style(styles::btn_primary)
                .on_press_maybe((!busy).then(|| Message::InstallRequested(name))),
        };
        col = col.push(row![action_btn].spacing(8));
    }

    if app.settings.app_metadata.enabled {
        if let Some(strip) = screenshots_strip(metadata, cached.is_some()) {
            col = col.push(Space::new().height(Length::Fixed(8.0)));
            col = col.push(strip);
        }
    }

    col.into()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaneAction {
    Install(String),
    Remove(String),
}

/// Installed rows, either mode, only offer Remove so both modes never end up side by side.
pub fn pane_action(
    row: &PackageSummary,
    installed: &[InstalledPackage],
    sandboxed: bool,
) -> PaneAction {
    match install_state(installed, &row.name) {
        InstallState::Plain(ip) | InstallState::Sandboxed(ip) => PaneAction::Remove(ip.spec()),
        InstallState::NotInstalled => PaneAction::Install(match row.sandbox.as_ref() {
            Some(v) if sandboxed => v.name.clone(),
            _ => row.name.clone(),
        }),
    }
}

pub fn sandbox_switch_on(state: InstallState<'_>, sandboxed: bool) -> bool {
    match state {
        InstallState::NotInstalled => sandboxed,
        InstallState::Plain(_) => false,
        InstallState::Sandboxed(_) => true,
    }
}

fn confirm_swap_card<'a>(
    pending: &PendingSwap,
    info: Option<&SandboxInfo>,
    session: SessionType,
    busy: bool,
) -> Column<'a, Message> {
    let header = text(crate::t!("sandbox-swap-title", name = pending.base.clone()))
        .size(14)
        .font(BOLD);
    let blurb = if pending.to_sandboxed {
        crate::t!("sandbox-swap-to-sandboxed-blurb")
    } else {
        crate::t!("sandbox-swap-to-plain-blurb")
    };
    let mut col: Column<'a, Message> =
        column![header, text(blurb).size(12).color(MUTED)].spacing(6);
    if let Some(info) = info.filter(|_| pending.to_sandboxed) {
        col = col.push(permissions_block(info, session));
    }

    let confirm = button(text(crate::t!("sandbox-swap-confirm")).size(13))
        .padding([8, 16])
        .style(styles::btn_primary)
        .on_press_maybe((!busy).then_some(Message::SwapConfirmed));
    let cancel = button(text(crate::t!("common-cancel")).size(13))
        .padding([8, 16])
        .style(styles::btn_ghost)
        .on_press(Message::SwapCancelled);
    col.push(row![confirm, cancel].spacing(8))
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
                .map(|f| crate::t!("search-screenshots-flathub", id = f.component_id.clone()))
                .unwrap_or_else(|| crate::t!("search-screenshots-debian"));
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
            text(crate::t!("search-loading-media"))
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

#[cfg(test)]
pub(crate) mod tests {
    use std::path::PathBuf;

    use libguix::{SandboxPolicy, SandboxVariant};

    use super::*;

    fn info(base: &str, output: &str) -> SandboxInfo {
        SandboxInfo {
            base: base.to_owned(),
            output: output.to_owned(),
            policy: SandboxPolicy::default(),
        }
    }

    /// `output` is the wrapped output of `name` when `variant` is set.
    pub(crate) fn summary(name: &str, variant: Option<&str>, output: &str) -> PackageSummary {
        PackageSummary {
            name: name.to_owned(),
            version: "1.0".to_owned(),
            synopsis: String::new(),
            description: String::new(),
            homepage: String::new(),
            license: String::new(),
            outputs: vec![output.to_owned()],
            sandbox: variant.map(|v| SandboxVariant {
                name: v.to_owned(),
                info: info(name, output),
            }),
        }
    }

    pub(crate) fn installed(
        name: &str,
        output: &str,
        sandbox: Option<(&str, &str)>,
    ) -> InstalledPackage {
        InstalledPackage {
            name: name.to_owned(),
            version: "1.0".to_owned(),
            output: output.to_owned(),
            store_path: PathBuf::from("/gnu/store/x"),
            sandbox: sandbox.map(|(b, o)| info(b, o)),
        }
    }

    #[test]
    fn not_installed_installs_selected_mode() {
        let row = summary("transmission", Some("transmission-sandboxed"), "gui");
        assert_eq!(
            pane_action(&row, &[], false),
            PaneAction::Install("transmission".to_owned())
        );
        assert_eq!(
            pane_action(&row, &[], true),
            PaneAction::Install("transmission-sandboxed".to_owned())
        );
    }

    #[test]
    fn row_without_variant_ignores_switch() {
        let row = summary("hello", None, "out");
        assert_eq!(
            pane_action(&row, &[], true),
            PaneAction::Install("hello".to_owned())
        );
        let list = vec![installed("hello", "out", None)];
        assert_eq!(
            pane_action(&row, &list, false),
            PaneAction::Remove("hello:out".to_owned())
        );
    }

    #[test]
    fn only_variant_installed_offers_remove_of_variant() {
        let row = summary("transmission", Some("transmission-sandboxed"), "gui");
        let list = vec![installed(
            "transmission-sandboxed",
            "out",
            Some(("transmission", "gui")),
        )];
        assert!(matches!(
            install_state(&list, &row.name),
            InstallState::Sandboxed(_)
        ));
        for sandboxed in [false, true] {
            assert_eq!(
                pane_action(&row, &list, sandboxed),
                PaneAction::Remove("transmission-sandboxed:out".to_owned())
            );
        }
    }

    #[test]
    fn plain_installed_offers_remove_of_plain() {
        let row = summary("transmission", Some("transmission-sandboxed"), "gui");
        let list = vec![installed("transmission", "gui", None)];
        assert_eq!(
            pane_action(&row, &list, true),
            PaneAction::Remove("transmission:gui".to_owned())
        );
    }

    #[test]
    fn switch_position() {
        let plain = installed("foo", "out", None);
        let boxed = installed("foo-sandboxed", "out", Some(("foo", "out")));
        assert!(!sandbox_switch_on(InstallState::NotInstalled, false));
        assert!(sandbox_switch_on(InstallState::NotInstalled, true));
        assert!(!sandbox_switch_on(InstallState::Plain(&plain), true));
        assert!(sandbox_switch_on(InstallState::Sandboxed(&boxed), false));
    }
}
