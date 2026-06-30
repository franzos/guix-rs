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
use iced::Font;
use settings::Settings;
use tracing_subscriber::EnvFilter;

// Bundle the fonts so the UI renders identically on any machine, with no
// dependency on system fonts or a working fontconfig. DejaVu covers every
// shipped locale except CJK, which still falls back to system fonts.
const DEJAVU: &[u8] = include_bytes!("../assets/fonts/DejaVuSans.ttf");
const DEJAVU_BOLD: &[u8] = include_bytes!("../assets/fonts/DejaVuSans-Bold.ttf");
const DEJAVU_MONO: &[u8] = include_bytes!("../assets/fonts/DejaVuSansMono.ttf");

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
        .default_font(Font::with_name("DejaVu Sans"))
        .font(DEJAVU)
        .font(DEJAVU_BOLD)
        .font(DEJAVU_MONO)
        .run()
}
