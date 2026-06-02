//! Fluent-backed localization. Catalogues are embedded via `rust-embed`;
//! the active locale follows the system default plus a persisted in-app
//! override (override wins). Only `en` ships today.

use std::sync::OnceLock;

use i18n_embed::fluent::{fluent_language_loader, FluentLanguageLoader};
use i18n_embed::{DesktopLanguageRequester, LanguageLoader};
use rust_embed::RustEmbed;
use unic_langid::LanguageIdentifier;

#[derive(RustEmbed)]
#[folder = "i18n/"]
struct Localizations;

static LOADER: OnceLock<FluentLanguageLoader> = OnceLock::new();

pub fn loader() -> &'static FluentLanguageLoader {
    LOADER.get_or_init(|| {
        let loader = fluent_language_loader!();
        loader
            .load_fallback_language(&Localizations)
            .expect("embedded `en` fallback catalogue must load");
        // No RTL locales ship yet; Fluent's BiDi isolate marks around every
        // interpolated value can render as tofu in some shapers. Re-enable
        // when RTL support lands.
        loader.set_use_isolating(false);
        loader
    })
}

/// `t!("key")` / `t!("key", arg = value)` — thin wrapper over `fl!` bound
/// to the shared loader. Keys must be string literals so the catalogue is
/// validated at compile time.
#[macro_export]
macro_rules! t {
    ($($args:tt)*) => {
        ::i18n_embed_fl::fl!($crate::i18n::loader(), $($args)*)
    };
}

pub fn available_locales() -> Vec<LanguageIdentifier> {
    loader()
        .available_languages(&Localizations)
        .unwrap_or_default()
}

pub fn select_language(requested: &[LanguageIdentifier]) {
    if let Err(e) = i18n_embed::select(loader(), &Localizations, requested) {
        tracing::warn!(target: "guix_gui", "locale selection failed: {e}");
    }
}

/// Build the priority-ordered locale list: a valid in-app override first,
/// then the system's preference chain, with `en` pinned as a final
/// backstop. De-duped, order preserved.
pub fn requested_languages(override_tag: Option<&str>) -> Vec<LanguageIdentifier> {
    let mut out: Vec<LanguageIdentifier> = Vec::new();

    if let Some(tag) = override_tag {
        match tag.parse::<LanguageIdentifier>() {
            Ok(id) => out.push(id),
            Err(e) => {
                tracing::warn!(target: "guix_gui", "ignoring invalid language override {tag:?}: {e}");
            }
        }
    }

    out.extend(DesktopLanguageRequester::requested_languages());

    let en: LanguageIdentifier = "en".parse().expect("`en` is a valid language tag");
    if !out.contains(&en) {
        out.push(en);
    }

    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loader_resolves_en() {
        let en: LanguageIdentifier = "en".parse().unwrap();
        assert!(loader().current_languages().contains(&en));
    }

    #[test]
    fn app_title_non_empty() {
        assert!(!t!("app-title").is_empty());
    }

    #[test]
    fn override_is_first_and_en_last() {
        let langs = requested_languages(Some("de-DE"));
        let de: LanguageIdentifier = "de".parse().unwrap();
        let en: LanguageIdentifier = "en".parse().unwrap();
        assert_eq!(langs.first().map(|l| l.language), Some(de.language));
        assert_eq!(langs.last(), Some(&en));
    }

    #[test]
    fn invalid_override_does_not_panic() {
        let langs = requested_languages(Some("not a tag"));
        let en: LanguageIdentifier = "en".parse().unwrap();
        assert_eq!(langs.last(), Some(&en));
    }
}
