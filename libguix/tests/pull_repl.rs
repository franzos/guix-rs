//! `Guix::pull_repl` via fake-`guix` shim — won't mutate the user catalog.

use std::time::Duration;

use futures_util::StreamExt;
use libguix::{Guix, ProgressEvent};

/// Helper: drain events with a wall-clock deadline so a stuck pipe
/// doesn't hang the test runner.
async fn drain_with_timeout(op: &mut libguix::Operation, total: Duration) -> Vec<ProgressEvent> {
    let deadline = tokio::time::Instant::now() + total;
    let mut out = Vec::new();
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, op.events_mut().next()).await {
            Ok(Some(batch)) => out.extend(batch),
            Ok(None) => break,
            Err(_) => break,
        }
    }
    out
}

/// Smoke test against a real `guix` on the host. We start `pull_repl`,
/// give it a couple of seconds to emit *something* (banner stderr,
/// `(error …)` events from misconfiguration, or real build events if
/// the daemon is reachable), then cancel it.
///
/// We don't assert exit-success: a real pull would actually run, which
/// we don't want from a test. We only assert that the plumbing produced
/// at least one `ProgressEvent` and reaches an `ExitSummary`.
#[tokio::test(flavor = "multi_thread")]
async fn pull_repl_spawns_against_real_guix() {
    let g = match Guix::discover().await {
        Ok(g) => g,
        Err(e) => {
            eprintln!("skipping: `guix` not available: {e}");
            return;
        }
    };

    let mut op = match g.pull().user() {
        Ok(op) => op,
        Err(e) => {
            eprintln!("skipping: pull_repl spawn failed: {e}");
            return;
        }
    };
    let cancel = op.take_cancel().expect("cancel handle");

    // Give it a short window to start producing events. We don't want
    // a real pull to actually complete here — 2 seconds is enough to
    // see the banner / first event / error from the REPL.
    let events = drain_with_timeout(&mut op, Duration::from_secs(3)).await;

    // Kill the REPL child so we don't leave a `guix pull` running.
    let _ = cancel.cancel().await;

    // Drain the rest (should end with ExitSummary).
    let mut tail = Vec::new();
    while let Some(batch) = op.events_mut().next().await {
        tail.extend(batch);
    }
    let mut all = events;
    all.extend(tail);

    eprintln!("pull_repl produced {} events", all.len());
    for e in all.iter().take(5) {
        eprintln!("  - {e:?}");
    }

    // The driver always emits an ExitSummary as the final event.
    assert!(
        matches!(all.last(), Some(ProgressEvent::ExitSummary { .. })),
        "expected ExitSummary as final event, got {:?}",
        all.last()
    );
}

/// End-to-end fd-3 plumbing test. Use a fake `guix` binary that, when
/// invoked as `guix repl -t machine`, ignores stdin and writes a few
/// hand-crafted s-expressions to fd 3 and exits.
///
/// This validates the parent-side pipe wiring, the fd-3 → mpsc reader,
/// the event-mapping table, and the standard operation pipeline
/// (coalescer + ExitSummary) — all without touching the user's catalog
/// or requiring a working guix daemon.
#[tokio::test(flavor = "multi_thread")]
async fn pull_repl_fd3_pipe_carries_events() {
    // Build a fake `guix` shim with a script that writes events to fd 3.
    let dir = tempfile::tempdir().expect("tempdir");
    let bin_dir = dir.path().join("bin");
    std::fs::create_dir_all(&bin_dir).expect("mkdir bin");
    let guix_path = bin_dir.join("guix");

    // Make `Guix::discover` resolve our fake by setting `GUIX_PROFILE`
    // (its first-priority candidate). We also rewrite the shim to
    // handle `--version` for the discover pre-flight.
    let script = r#"#!/bin/sh
if [ "$1" = "--version" ]; then
    echo "guix (Guix) 9999-01-01.00"
    exit 0
fi
cat > /dev/null
{
  printf '(build-started "/gnu/store/abc-foo.drv" "-" "x86_64-linux" "")\n'
  printf '(download-started "/gnu/store/xyz" "https://ci/x" "12345")\n'
  printf '(substituter-started "/gnu/store/xyz" "substitute")\n'
  printf '(build-succeeded "/gnu/store/abc-foo.drv")\n'
} >&3
exit 0
"#;
    std::fs::write(&guix_path, script).expect("rewrite");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&guix_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&guix_path, perms).expect("chmod");
    }

    let old_profile = std::env::var_os("GUIX_PROFILE");
    std::env::set_var("GUIX_PROFILE", dir.path());

    let g = match Guix::discover().await {
        Ok(g) => g,
        Err(e) => {
            if let Some(p) = old_profile {
                std::env::set_var("GUIX_PROFILE", p);
            } else {
                std::env::remove_var("GUIX_PROFILE");
            }
            panic!("fake-guix discover failed: {e}");
        }
    };
    assert_eq!(
        g.binary(),
        guix_path.as_path(),
        "discover should resolve our fake guix"
    );

    let mut op = g.pull().user().expect("pull().user()");
    // Drain — fake exits quickly. We drain to channel close so the
    // ExitSummary lands in the captured set; a 15s outer timeout
    // protects against a stuck pipe.
    let drain_fut = async {
        let mut out = Vec::new();
        while let Some(batch) = op.events_mut().next().await {
            out.extend(batch);
        }
        out
    };
    let events = tokio::time::timeout(Duration::from_secs(15), drain_fut)
        .await
        .expect("drain timed out");

    if let Some(p) = old_profile {
        std::env::set_var("GUIX_PROFILE", p);
    } else {
        std::env::remove_var("GUIX_PROFILE");
    }

    // Should observe at least the three typed events + a Line for the
    // unmapped substituter-started + ExitSummary at the end.
    for e in &events {
        eprintln!("event: {e:?}");
    }
    let kinds: Vec<&'static str> = events
        .iter()
        .map(|e| match e {
            ProgressEvent::BuildStart { .. } => "BuildStart",
            ProgressEvent::BuildDone { .. } => "BuildDone",
            ProgressEvent::BuildFailed { .. } => "BuildFailed",
            ProgressEvent::SubstituteDownload { .. } => "SubstituteDownload",
            ProgressEvent::Line { .. } => "Line",
            ProgressEvent::ExitSummary { .. } => "ExitSummary",
            _ => "Other",
        })
        .collect();
    eprintln!("got event kinds: {kinds:?}");

    assert!(
        kinds.contains(&"BuildStart"),
        "expected BuildStart, kinds={kinds:?}"
    );
    assert!(
        kinds.contains(&"SubstituteDownload"),
        "expected SubstituteDownload, kinds={kinds:?}"
    );
    assert!(
        kinds.contains(&"BuildDone"),
        "expected BuildDone, kinds={kinds:?}"
    );
    assert!(
        kinds.contains(&"ExitSummary"),
        "expected ExitSummary, kinds={kinds:?}"
    );
}
