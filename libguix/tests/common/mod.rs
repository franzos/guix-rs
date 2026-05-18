//! Shared test helpers — most importantly, a process-wide `Repl` actor
//! initialised once on first use. Per-test spawn would warmup the
//! Scheme helpers ~N times for nothing; one actor is plenty.

use std::time::Duration;

use libguix::{Guix, Repl};
use tokio::sync::OnceCell;

static SHARED_REPL: OnceCell<Option<Repl>> = OnceCell::const_new();

/// First call spawns + lightweight-warms; subsequent calls clone the
/// same handle. Returns `None` if `guix` isn't installed on the host —
/// tests should `match shared_repl_or_skip().await else { return; }`.
pub async fn shared_repl_or_skip() -> Option<Repl> {
    SHARED_REPL
        .get_or_init(|| async {
            let g = match Guix::discover().await {
                Ok(g) => g.with_repl_timeout(Duration::from_secs(60)),
                Err(e) => {
                    eprintln!("[channels tests] skipping: discover failed: {e}");
                    return None;
                }
            };
            let repl = match g.repl().await {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("[channels tests] skipping: repl spawn failed: {e}");
                    return None;
                }
            };
            if let Err(e) = repl.warmup_lightweight().await {
                eprintln!("[channels tests] skipping: lightweight warmup failed: {e}");
                return None;
            }
            Some(repl)
        })
        .await
        .clone()
}
