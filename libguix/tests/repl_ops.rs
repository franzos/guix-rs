//! REPL-native install/remove/upgrade ops, exercised via a fake-`guix` shim.

// `ENV_LOCK` is held across `.await` on purpose — it serialises tests that
// mutate `$GUIX_PROFILE`, which would otherwise race. The awaits don't
// re-enter env-var code, so there's no deadlock risk.
#![allow(clippy::await_holding_lock)]

use std::sync::Mutex;
use std::time::Duration;

use futures_util::StreamExt;
use libguix::{Guix, ProgressEvent};

/// Serialise tests that mutate `$GUIX_PROFILE` — `Guix::discover`
/// reads it on every call, and parallel tests racing on env vars
/// resolve each other's fake-guix binaries.
static ENV_LOCK: Mutex<()> = Mutex::new(());

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

/// fd-3 plumbing test for `install`. We swap in a fake `guix`
/// binary that prints a handful of build events to fd 3 and exits 0.
/// The fake doesn't parse the Scheme payload — it just demonstrates
/// the parent-side wiring works for an install-shaped invocation the
/// same way it does for pull.
#[tokio::test(flavor = "multi_thread")]
async fn install_fd3_pipe_carries_events() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().expect("tempdir");
    let bin_dir = dir.path().join("bin");
    std::fs::create_dir_all(&bin_dir).expect("mkdir bin");
    let guix_path = bin_dir.join("guix");

    let script = r#"#!/bin/sh
if [ "$1" = "--version" ]; then
    echo "guix (Guix) 9999-01-01.00"
    exit 0
fi
cat > /dev/null
{
  printf '(build-started "/gnu/store/abc-hello.drv" "-" "x86_64-linux" "")\n'
  printf '(download-started "/gnu/store/hello-out" "https://ci/hello" "9999")\n'
  printf '(build-succeeded "/gnu/store/abc-hello.drv")\n'
  printf '(done 0)\n'
} >&3
exit 0
"#;
    std::fs::write(&guix_path, script).expect("write");
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

    let mut op = g.package().install(&["hello"]).expect("install");

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
        matches!(
            events.last(),
            Some(ProgressEvent::ExitSummary { code: 0, .. })
        ),
        "expected clean ExitSummary, got {:?}",
        events.last()
    );
}

/// Validation: package names with control characters are rejected
/// before the REPL is even spawned. Uses the fake-guix shim purely so
/// `Guix::discover` succeeds; the spawn itself must short-circuit.
#[tokio::test(flavor = "multi_thread")]
async fn install_rejects_invalid_arg() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().expect("tempdir");
    let bin_dir = dir.path().join("bin");
    std::fs::create_dir_all(&bin_dir).expect("mkdir bin");
    let guix_path = bin_dir.join("guix");
    let script = r#"#!/bin/sh
[ "$1" = "--version" ] && { echo "guix (Guix) 9999-01-01.00"; exit 0; }
exit 1
"#;
    std::fs::write(&guix_path, script).expect("write");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&guix_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&guix_path, perms).expect("chmod");
    }
    let old_profile = std::env::var_os("GUIX_PROFILE");
    std::env::set_var("GUIX_PROFILE", dir.path());

    let g = Guix::discover().await.expect("discover");
    let err = g
        .package()
        .install(&["bad\nname"])
        .err()
        .expect("must reject newline");
    let msg = err.to_string();
    assert!(msg.contains("invalid package name"), "got: {msg}");

    if let Some(p) = old_profile {
        std::env::set_var("GUIX_PROFILE", p);
    } else {
        std::env::remove_var("GUIX_PROFILE");
    }
}

/// Forgiving smoke test against a real `guix`. We start `install`
/// for the tiny `hello` package, give it a brief moment to emit
/// *something*, then cancel — same pattern as
/// `pull_repl_spawns_against_real_guix`. We do **not** wait for the
/// install to complete, so the user's profile stays untouched in
/// practice (the daemon may have started a build computation but no
/// new generation is written until `guix-package` finalises).
///
/// We assert only that the plumbing produced at least one event and
/// reaches an `ExitSummary`. Skipped if `guix` is unavailable.
#[tokio::test(flavor = "multi_thread")]
async fn install_spawns_against_real_guix() {
    let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let g = match Guix::discover().await {
        Ok(g) => g,
        Err(e) => {
            eprintln!("skipping: `guix` not available: {e}");
            return;
        }
    };

    let mut op = match g.package().install(&["hello"]) {
        Ok(op) => op,
        Err(e) => {
            eprintln!("skipping: install spawn failed: {e}");
            return;
        }
    };
    let cancel = op.take_cancel().expect("cancel handle");

    let events = drain_with_timeout(&mut op, Duration::from_secs(2)).await;

    // Kill the REPL child so we don't leave `guix-package` running.
    let _ = cancel.cancel().await;

    let mut tail = Vec::new();
    while let Some(batch) = op.events_mut().next().await {
        tail.extend(batch);
    }
    let mut all = events;
    all.extend(tail);

    eprintln!("install produced {} events", all.len());
    for e in all.iter().take(5) {
        eprintln!("  - {e:?}");
    }

    assert!(
        matches!(all.last(), Some(ProgressEvent::ExitSummary { .. })),
        "expected ExitSummary as final event, got {:?}",
        all.last()
    );
}
