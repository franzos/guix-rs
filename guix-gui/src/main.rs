//! Binary entry point for `guix-gui`.

mod app;
mod app_metadata;
mod carrier;
mod channels;
mod i18n;
mod operation_subscription;
mod progress_summary;
mod recommended;
mod settings;
mod styles;
mod terminal_buffer;
mod util;
mod views;

use app::App;
use settings::Settings;
use tracing_subscriber::EnvFilter;

fn main() -> iced::Result {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .try_init();

    // Resolve the locale before the first frame. `App::new` reloads
    // `Settings` (cheap), so we only read the override here.
    let language = Settings::load().language;
    i18n::select_language(&i18n::requested_languages(language.as_deref()));

    iced::application(App::new, App::update, App::view)
        .title(App::title)
        .subscription(App::subscription)
        .theme(App::theme)
        .run()
}
