//! Binary entry point for `guix-gui`.

mod app;
mod app_metadata;
mod carrier;
mod channels;
mod operation_subscription;
mod progress_summary;
mod recommended;
mod settings;
mod styles;
mod terminal_buffer;
mod util;
mod views;

use app::App;
use tracing_subscriber::EnvFilter;

fn main() -> iced::Result {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .try_init();

    iced::application(App::new, App::update, App::view)
        .title(App::title)
        .subscription(App::subscription)
        .theme(App::theme)
        .run()
}
