//! Library target for `guix-gui` — exists so integration tests under
//! `guix-gui/tests/` can import modules without going through the
//! binary's entrypoint. The full app still lives in `main.rs`; this file
//! re-exports the subset of modules with externally-relevant test
//! surface (currently: discovery).

pub mod discovery;
pub mod i18n;
