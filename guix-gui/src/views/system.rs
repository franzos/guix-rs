use iced::widget::{button, column, container, row, scrollable, text, text_input, Column};
use iced::{Element, Length};

use crate::app::{App, Message};

pub fn view(app: &App) -> Element<'_, Message> {
    let current = match (
        &app.system.current_config_display,
        &app.system.current_config_error,
    ) {
        (Some(p), _) => text(format!("Current system config: {p}")).size(13),
        (_, Some(e)) => text(format!("Not on Guix System: {e}")).size(13),
        _ => text("Checking current system config...").size(13),
    };

    let banner: Option<Element<'_, Message>> = if app.settings.source_config_path.is_none() {
        Some(
            container(
                text(
                    "No system configuration file detected at /etc/config.scm or \
                     /etc/system.scm. Enter the path to your .scm configuration below.",
                )
                .size(12),
            )
            .padding(8)
            .style(container::rounded_box)
            .into(),
        )
    } else {
        None
    };

    let source = column![
        text("Source config (your editable .scm):").size(13),
        text_input("/home/you/dotfiles/config.scm", &app.system.source_input)
            .on_input(Message::SourceConfigChanged)
            .padding(6)
            .width(Length::Fill),
        row![
            button(text("Validate")).on_press(Message::SourceConfigValidate),
            text(app.system.validation_message.clone().unwrap_or_default()).size(12),
        ]
        .spacing(10),
    ]
    .spacing(6);

    let mut advanced = Column::new().spacing(4);
    advanced = advanced.push(text("Advanced: extra load paths").size(13));
    let auto = app.auto_load_path();
    advanced = advanced.push(
        text(match &auto {
            Some(p) => format!("Auto: {}", p.display()),
            None => "Auto: (set source config above)".into(),
        })
        .size(12),
    );
    for (i, p) in app.settings.custom_load_paths.iter().enumerate() {
        advanced = advanced.push(
            row![
                text(p.display().to_string()).size(12),
                button(text("Remove")).on_press(Message::LoadPathRemove(i)),
            ]
            .spacing(6),
        );
    }
    advanced = advanced.push(
        row![
            text_input("/path/to/extra/modules", &app.system.load_path_input)
                .on_input(Message::LoadPathInputChanged)
                .on_submit(Message::LoadPathAdd)
                .padding(6)
                .width(Length::Fill),
            button(text("+ Add load path")).on_press(Message::LoadPathAdd),
        ]
        .spacing(6),
    );

    let mut chans = Column::new().spacing(2);
    chans = chans.push(text("Channels:").size(14));
    for c in &app.updates.channels {
        let commit = c.commit.as_deref().unwrap_or("(no commit)");
        chans = chans.push(text(format!("  {}  {}  {}", c.name, commit, c.url)).size(12));
    }

    let mut col = Column::new().spacing(12).height(Length::Fill);
    col = col.push(current);
    if let Some(b) = banner {
        col = col.push(b);
    }
    col = col.push(source);
    col = col.push(advanced);
    col = col.push(scrollable(chans).height(Length::Fill));
    col.into()
}
