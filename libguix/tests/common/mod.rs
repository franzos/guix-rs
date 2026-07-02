//! Shared test helpers. Each `#[tokio::test]` runs on its own runtime, so
//! a cross-test cached actor would die when the first test's runtime tears
//! down. We spawn a fresh `Repl` per call instead; the Scheme helpers are
//! primed at actor bootstrap, so no warmup step is needed.

use std::time::Duration;

use libguix::{Guix, Repl};

/// Spawns a `Repl` for the calling test, or returns `None` if `guix` isn't
/// installed on the host — tests should
/// `match shared_repl_or_skip().await else { return; }`.
pub async fn shared_repl_or_skip() -> Option<Repl> {
    let g = match Guix::discover().await {
        Ok(g) => g.with_repl_timeout(Duration::from_secs(60)),
        Err(e) => {
            eprintln!("[channels tests] skipping: discover failed: {e}");
            return None;
        }
    };
    match g.repl().await {
        Ok(r) => Some(r),
        Err(e) => {
            eprintln!("[channels tests] skipping: repl spawn failed: {e}");
            None
        }
    }
}
