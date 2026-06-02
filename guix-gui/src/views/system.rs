use iced::widget::{
    button, checkbox, column, container, pick_list, row, scrollable, text, text_input, Space,
};
use iced::{Element, Length};
use unic_langid::LanguageIdentifier;

use crate::app::{App, Message};
use crate::settings::Tab;
use crate::styles::{self, BOLD, MUTED};

/// One entry in the language picker. `System` follows the OS locale;
/// `Locale` pins a specific embedded catalogue.
#[derive(Debug, Clone, PartialEq, Eq)]
enum LangChoice {
    System,
    Locale(LanguageIdentifier),
}

impl std::fmt::Display for LangChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LangChoice::System => f.write_str(&crate::t!("settings-language-system")),
            LangChoice::Locale(id) => write!(f, "{id}"),
        }
    }
}

pub fn view(app: &App) -> Element<'_, Message> {
    let header = App::view_header(crate::t!("system-title"), None);

    // -- SYSTEM section --
    let current_label: Element<'_, Message> = match (
        &app.system.current_config_display,
        &app.system.current_config_error,
    ) {
        (Some(p), _) => text(crate::t!("system-current-config", path = p.clone()))
            .size(13)
            .color(MUTED)
            .into(),
        (_, Some(e)) => text(crate::t!("system-not-guix", error = e.clone()))
            .size(13)
            .color(MUTED)
            .into(),
        _ => text(crate::t!("system-checking-config"))
            .size(13)
            .color(MUTED)
            .into(),
    };

    let banner: Option<Element<'_, Message>> = if app.settings.source_config_path.is_none() {
        Some(
            container(
                text(crate::t!("system-no-config-banner"))
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

    let validate_btn = button(text(crate::t!("system-validate")).size(13))
        .padding([8, 16])
        .style(styles::btn_secondary)
        .on_press(Message::SourceConfigValidate);
    let validation: Element<'_, Message> =
        text(app.system.validation_message.clone().unwrap_or_default())
            .size(12)
            .color(MUTED)
            .into();

    let source_content = column![
        text(crate::t!("system-config-heading")).size(16).font(BOLD),
        text(crate::t!("system-config-blurb")).size(12).color(MUTED),
        Space::new().height(4),
        text_input(
            &crate::t!("system-config-placeholder"),
            &app.system.source_input
        )
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

    // Extra load paths — additional dirs to search when resolving Scheme
    // imports. Logically part of the system-config story, so it lives in
    // the SYSTEM section below the system-config card.
    let auto = app.auto_load_path();
    let auto_line = text(match &auto {
        Some(p) => crate::t!("system-load-paths-auto", path = p.display().to_string()),
        None => crate::t!("system-load-paths-auto-unset"),
    })
    .size(12)
    .color(MUTED);

    let mut load_paths_inner = column![
        text(crate::t!("system-load-paths-heading"))
            .size(16)
            .font(BOLD),
        text(crate::t!("system-load-paths-blurb"))
            .size(12)
            .color(MUTED),
        Space::new().height(4),
        auto_line,
    ]
    .spacing(4);

    for (i, p) in app.settings.custom_load_paths.iter().enumerate() {
        let remove_btn = button(text(crate::t!("common-remove")).size(11))
            .padding([4, 10])
            .style(styles::btn_ghost)
            .on_press(Message::LoadPathRemove(i));
        load_paths_inner = load_paths_inner.push(
            row![
                text(p.display().to_string()).size(12),
                Space::new().width(Length::Fill),
                remove_btn,
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        );
    }

    let add_btn = button(text(crate::t!("system-add")).size(13))
        .padding([8, 16])
        .style(styles::btn_secondary)
        .on_press(Message::LoadPathAdd);
    load_paths_inner = load_paths_inner.push(
        row![
            text_input(
                &crate::t!("system-load-paths-placeholder"),
                &app.system.load_path_input
            )
            .on_input(Message::LoadPathInputChanged)
            .on_submit(Message::LoadPathAdd)
            .padding(8)
            .size(13)
            .width(Length::Fill),
            add_btn,
        ]
        .spacing(8),
    );

    let load_paths_card = container(load_paths_inner)
        .padding(20)
        .width(Length::Fill)
        .style(styles::card);

    // Language picker — selecting an option re-localises the UI live
    // because `view()` re-evaluates every `t!` on each render.
    let mut lang_options = vec![LangChoice::System];
    lang_options.extend(
        crate::i18n::available_locales()
            .into_iter()
            .map(LangChoice::Locale),
    );
    let selected = match app.settings.language.as_deref() {
        Some(tag) => tag
            .parse::<LanguageIdentifier>()
            .ok()
            .map(LangChoice::Locale)
            .unwrap_or(LangChoice::System),
        None => LangChoice::System,
    };
    let lang_picker = pick_list(lang_options, Some(selected), |choice| match choice {
        LangChoice::System => Message::LanguageSelected(None),
        LangChoice::Locale(id) => Message::LanguageSelected(Some(id.to_string())),
    })
    .padding(8)
    .text_size(13);
    let language_inner = column![
        text(crate::t!("settings-language")).size(16).font(BOLD),
        Space::new().height(4),
        lang_picker,
    ]
    .spacing(4);
    let language_card = container(language_inner)
        .padding(20)
        .width(Length::Fill)
        .style(styles::card);

    let mut system_section = column![text(crate::t!("system-section-system"))
        .size(12)
        .color(MUTED)]
    .spacing(8);
    system_section = system_section.push(
        container(current_label)
            .padding(20)
            .width(Length::Fill)
            .style(styles::card),
    );
    if let Some(b) = banner {
        system_section = system_section.push(b);
    }
    system_section = system_section.push(language_card);
    system_section = system_section.push(source_card);
    system_section = system_section.push(load_paths_card);

    // -- CHANNELS section --
    // Summary card only — full per-channel editing lives in the Channels
    // tab. The count must mirror the Channels tab (the file set), not
    // `app.updates.channels` (the `guix describe` set) — those diverge
    // whenever a user's channels declare transitive deps.
    let summary = match app.channels.file.as_ref() {
        Some(f) => {
            let count = f.list.channels().len();
            if count == 0 {
                crate::t!("system-channels-none")
            } else {
                crate::t!("system-channels-configured", count = count)
            }
        }
        None => crate::t!("system-channels-unknown"),
    };
    let open_channels = button(text(crate::t!("system-open-channels")).size(13))
        .padding([8, 16])
        .style(styles::btn_secondary)
        .on_press(Message::TabSelected(Tab::Channels));
    let channels_inner = column![
        text(crate::t!("system-channels-heading"))
            .size(16)
            .font(BOLD),
        text(crate::t!("system-channels-blurb"))
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
        button(text(crate::t!("system-use-default")).size(12))
            .padding([6, 12])
            .style(styles::btn_ghost)
            .on_press_maybe(on_press)
    };
    let channels_source_inner = column![
        text(crate::t!("system-channels-source-heading"))
            .size(16)
            .font(BOLD),
        text(crate::t!("system-channels-source-blurb"))
            .size(12)
            .color(MUTED),
        Space::new().height(4),
        row![
            text_input(
                &crate::t!("system-channels-source-placeholder"),
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

    let channels_section = column![
        text(crate::t!("system-section-user-channels"))
            .size(12)
            .color(MUTED),
        container(channels_inner)
            .padding(20)
            .width(Length::Fill)
            .style(styles::card),
        channels_source_card,
    ]
    .spacing(8);

    // -- METADATA section: third-party icons + screenshots --
    let meta = &app.settings.app_metadata;
    let sub_enabled = meta.enabled;

    let labeled_check = |label: String,
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
        Some(p) => text(crate::t!(
            "system-cache-dir",
            path = p.display().to_string()
        ))
        .size(11)
        .color(MUTED)
        .into(),
        None => text(crate::t!("system-cache-dir-none"))
            .size(11)
            .color(MUTED)
            .into(),
    };
    let clear_btn = button(text(crate::t!("system-clear-cache")).size(13))
        .padding([8, 16])
        .style(styles::btn_secondary)
        .on_press(Message::ClearMetadataCacheClicked);
    let cache_feedback: Element<'_, Message> =
        text(app.system.cache_action_message.clone().unwrap_or_default())
            .size(12)
            .color(MUTED)
            .into();

    let metadata_inner = column![
        text(crate::t!("system-metadata-heading"))
            .size(16)
            .font(BOLD),
        text(crate::t!("system-metadata-blurb"))
            .size(12)
            .color(MUTED),
        Space::new().height(4),
        labeled_check(
            crate::t!("system-metadata-enable"),
            meta.enabled,
            true,
            Message::AppMetadataEnabledToggled,
        ),
        Space::new().height(4),
        labeled_check(
            crate::t!("system-metadata-flathub"),
            meta.use_flathub,
            sub_enabled,
            Message::AppMetadataFlathubToggled,
        ),
        labeled_check(
            crate::t!("system-metadata-debian"),
            meta.use_debian_screenshots,
            sub_enabled,
            Message::AppMetadataDebianToggled,
        ),
        Space::new().height(8),
        text(crate::t!("system-cache-heading")).size(13).font(BOLD),
        text(crate::t!("system-cache-blurb")).size(12).color(MUTED),
        cache_path_hint,
        Space::new().height(4),
        row![clear_btn, cache_feedback]
            .spacing(12)
            .align_y(iced::Alignment::Center),
    ]
    .spacing(6);

    // Discovery opt-in — strict gate for the Discover sub-mode on the
    // Channels tab. When off, nothing related to discovery renders
    // anywhere in the app. Lives in METADATA alongside icons/screenshots
    // because conceptually it's the same shape of feature: opt-in,
    // network-touching, augments what's reachable from the Guix CLI.
    let discovery_check = checkbox(app.settings.discovery_enabled)
        .on_toggle(Message::DiscoveryEnabledToggled)
        .size(16);
    let discovery_inner = column![
        text(crate::t!("system-discovery-heading"))
            .size(16)
            .font(BOLD),
        row![
            discovery_check,
            text(crate::t!("system-discovery-toggle")).size(14),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
        text(crate::t!("system-discovery-blurb"))
            .size(12)
            .color(MUTED),
    ]
    .spacing(4);
    let discovery_card = container(discovery_inner)
        .padding(20)
        .width(Length::Fill)
        .style(styles::card);

    let metadata_section = column![
        text(crate::t!("system-section-metadata"))
            .size(12)
            .color(MUTED),
        container(metadata_inner)
            .padding(20)
            .width(Length::Fill)
            .style(styles::card),
        discovery_card,
    ]
    .spacing(8);

    let body = column![header, channels_section, system_section, metadata_section,].spacing(16);

    scrollable(body).height(Length::Fill).into()
}
