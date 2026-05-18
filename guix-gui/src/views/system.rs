use iced::widget::{button, checkbox, column, container, row, scrollable, text, text_input, Space};
use iced::{Element, Length};

use crate::app::{App, Message};
use crate::settings::Tab;
use crate::styles::{self, BOLD, MUTED};

pub fn view(app: &App) -> Element<'_, Message> {
    let header = App::view_header("Settings", None);

    // -- GENERAL section --
    let current_label: Element<'_, Message> = match (
        &app.system.current_config_display,
        &app.system.current_config_error,
    ) {
        (Some(p), _) => text(format!("Current system config: {p}"))
            .size(13)
            .color(MUTED)
            .into(),
        (_, Some(e)) => text(format!("Not on Guix System: {e}"))
            .size(13)
            .color(MUTED)
            .into(),
        _ => text("Checking current system config...")
            .size(13)
            .color(MUTED)
            .into(),
    };

    let banner: Option<Element<'_, Message>> = if app.settings.source_config_path.is_none() {
        Some(
            container(
                text(
                    "No system configuration file detected at /etc/config.scm or \
                     /etc/system.scm. Enter the path to your .scm configuration below.",
                )
                .size(12)
                .color(MUTED),
            )
            .padding(10)
            .style(styles::card_flat)
            .into(),
        )
    } else {
        None
    };

    let validate_btn = button(text("Validate").size(13))
        .padding([8, 16])
        .style(styles::btn_secondary)
        .on_press(Message::SourceConfigValidate);
    let validation: Element<'_, Message> =
        text(app.system.validation_message.clone().unwrap_or_default())
            .size(12)
            .color(MUTED)
            .into();

    let source_content = column![
        text("Source config").size(16).font(BOLD),
        text("Path to your editable .scm system configuration.")
            .size(12)
            .color(MUTED),
        Space::new().height(4),
        text_input("/home/you/dotfiles/config.scm", &app.system.source_input)
            .on_input(Message::SourceConfigChanged)
            .padding(8)
            .size(13)
            .width(Length::Fill),
        Space::new().height(4),
        row![validate_btn, validation]
            .spacing(12)
            .align_y(iced::Alignment::Center),
    ]
    .spacing(4);

    let source_card = container(source_content)
        .padding(20)
        .width(Length::Fill)
        .style(styles::card);

    let mut general_section = column![text("GENERAL").size(12).color(MUTED)].spacing(8);
    general_section = general_section.push(
        container(current_label)
            .padding(20)
            .width(Length::Fill)
            .style(styles::card),
    );
    if let Some(b) = banner {
        general_section = general_section.push(b);
    }
    general_section = general_section.push(source_card);

    // -- ADVANCED section: load paths --
    let auto = app.auto_load_path();
    let auto_line = text(match &auto {
        Some(p) => format!("Auto: {}", p.display()),
        None => "Auto: (set source config above)".into(),
    })
    .size(12)
    .color(MUTED);

    let mut advanced_inner = column![
        text("Extra load paths").size(16).font(BOLD),
        text("Additional directories to search when resolving Scheme imports.")
            .size(12)
            .color(MUTED),
        Space::new().height(4),
        auto_line,
    ]
    .spacing(4);

    for (i, p) in app.settings.custom_load_paths.iter().enumerate() {
        let remove_btn = button(text("Remove").size(11))
            .padding([4, 10])
            .style(styles::btn_ghost)
            .on_press(Message::LoadPathRemove(i));
        advanced_inner = advanced_inner.push(
            row![
                text(p.display().to_string()).size(12),
                Space::new().width(Length::Fill),
                remove_btn,
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        );
    }

    let add_btn = button(text("+ Add").size(13))
        .padding([8, 16])
        .style(styles::btn_secondary)
        .on_press(Message::LoadPathAdd);
    advanced_inner = advanced_inner.push(
        row![
            text_input("/path/to/extra/modules", &app.system.load_path_input)
                .on_input(Message::LoadPathInputChanged)
                .on_submit(Message::LoadPathAdd)
                .padding(8)
                .size(13)
                .width(Length::Fill),
            add_btn,
        ]
        .spacing(8),
    );

    let advanced_section = column![
        text("ADVANCED").size(12).color(MUTED),
        container(advanced_inner)
            .padding(20)
            .width(Length::Fill)
            .style(styles::card),
    ]
    .spacing(8);

    // -- CHANNELS section --
    // Summary card only — full per-channel editing lives in the Channels
    // tab. The count must mirror the Channels tab (the file set), not
    // `app.updates.channels` (the `guix describe` set) — those diverge
    // whenever a user's channels declare transitive deps.
    let summary = match app.channels.file.as_ref() {
        Some(f) => {
            let count = f.list.channels().len();
            if count == 0 {
                "No channels configured.".to_string()
            } else {
                format!(
                    "{count} channel{} configured.",
                    if count == 1 { "" } else { "s" }
                )
            }
        }
        None => "Channels configured: —".to_string(),
    };
    let open_channels = button(text("Open Channels tab").size(13))
        .padding([8, 16])
        .style(styles::btn_secondary)
        .on_press(Message::TabSelected(Tab::Channels));
    let channels_inner = column![
        text("Channels").size(16).font(BOLD),
        text("Manage user-level channels in the dedicated tab.")
            .size(12)
            .color(MUTED),
        Space::new().height(4),
        text(summary).size(12).color(MUTED),
        Space::new().height(4),
        open_channels,
    ]
    .spacing(4);

    // User channels source path override — lives in the CHANNELS section
    // (the "User" prefix is forward-compatible with the system-level
    // equivalent coming in Phase 4). Empty value clears the override and
    // falls back to `~/.config/guix/channels.scm`.
    let has_override = app.settings.channels_source_path.is_some();
    let use_default_btn = {
        let on_press = if has_override {
            Some(Message::ChannelsSourcePathUseDefault)
        } else {
            None
        };
        button(text("Use default").size(12))
            .padding([6, 12])
            .style(styles::btn_ghost)
            .on_press_maybe(on_press)
    };
    let channels_source_inner = column![
        text("User channels source path").size(16).font(BOLD),
        text(
            "Override for ~/.config/guix/channels.scm. Required when the \
             default path is managed by `guix home` (resolves into /gnu/store)."
        )
        .size(12)
        .color(MUTED),
        Space::new().height(4),
        row![
            text_input(
                "/home/you/dotfiles/channels.scm (leave empty for default)",
                &app.system.channels_source_input,
            )
            .on_input(Message::ChannelsSourcePathChanged)
            .padding(8)
            .size(13)
            .width(Length::Fill),
            use_default_btn,
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
    ]
    .spacing(4);
    let channels_source_card = container(channels_source_inner)
        .padding(20)
        .width(Length::Fill)
        .style(styles::card);

    // Discovery opt-in — strict gate for the Discover sub-mode. When
    // off, nothing related to discovery renders anywhere in the app.
    let discovery_check = checkbox(app.settings.discovery_enabled)
        .on_toggle(Message::DiscoveryEnabledToggled)
        .size(16);
    let discovery_inner = column![
        text("Discovery").size(16).font(BOLD),
        row![
            discovery_check,
            text("Browse channels and packages from toys.whereis.social").size(14),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
        text(
            "Opt-in. Requires network access. When off, discovery does not \
             appear anywhere in the app."
        )
        .size(12)
        .color(MUTED),
    ]
    .spacing(4);
    let discovery_card = container(discovery_inner)
        .padding(20)
        .width(Length::Fill)
        .style(styles::card);

    let channels_section = column![
        text("CHANNELS").size(12).color(MUTED),
        container(channels_inner)
            .padding(20)
            .width(Length::Fill)
            .style(styles::card),
        channels_source_card,
        discovery_card,
    ]
    .spacing(8);

    // -- METADATA section: third-party icons + screenshots --
    let meta = &app.settings.app_metadata;
    let sub_enabled = meta.enabled;

    let labeled_check = |label: &'static str,
                         checked: bool,
                         enabled: bool,
                         on_toggle: fn(bool) -> Message|
     -> Element<'_, Message> {
        let cb = checkbox(checked)
            .on_toggle_maybe(enabled.then_some(on_toggle))
            .size(16);
        let label_color = if enabled { styles::TEXT } else { MUTED };
        row![cb, text(label).size(14).color(label_color)]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .into()
    };

    let cache_path_hint: Element<'_, Message> = match app.metadata_client.cache_root() {
        Some(p) => text(format!("Cache directory: {}", p.display()))
            .size(11)
            .color(MUTED)
            .into(),
        None => text("Cache directory: (no XDG cache dir found — using in-memory only)")
            .size(11)
            .color(MUTED)
            .into(),
    };
    let clear_btn = button(text("Clear cache").size(13))
        .padding([8, 16])
        .style(styles::btn_secondary)
        .on_press(Message::ClearMetadataCacheClicked);
    let cache_feedback: Element<'_, Message> =
        text(app.system.cache_action_message.clone().unwrap_or_default())
            .size(12)
            .color(MUTED)
            .into();

    let metadata_inner = column![
        text("Icons & screenshots").size(16).font(BOLD),
        text(
            "Fetch icons and screenshots from third-party catalogs for selected \
             search results. Opt-in; requires network access."
        )
        .size(12)
        .color(MUTED),
        Space::new().height(4),
        labeled_check(
            "Enable third-party metadata",
            meta.enabled,
            true,
            Message::AppMetadataEnabledToggled,
        ),
        Space::new().height(4),
        labeled_check(
            "Flathub (flathub.org)",
            meta.use_flathub,
            sub_enabled,
            Message::AppMetadataFlathubToggled,
        ),
        labeled_check(
            "screenshots.debian.net",
            meta.use_debian_screenshots,
            sub_enabled,
            Message::AppMetadataDebianToggled,
        ),
        Space::new().height(8),
        text("Cache").size(13).font(BOLD),
        text("Icons and screenshots are saved on disk for up to a year. Clear it if an icon looks wrong upstream.")
            .size(12)
            .color(MUTED),
        cache_path_hint,
        Space::new().height(4),
        row![clear_btn, cache_feedback]
            .spacing(12)
            .align_y(iced::Alignment::Center),
    ]
    .spacing(6);

    let metadata_section = column![
        text("METADATA").size(12).color(MUTED),
        container(metadata_inner)
            .padding(20)
            .width(Length::Fill)
            .style(styles::card),
    ]
    .spacing(8);

    let body = column![
        header,
        general_section,
        metadata_section,
        advanced_section,
        channels_section
    ]
    .spacing(16);

    scrollable(body).height(Length::Fill).into()
}
