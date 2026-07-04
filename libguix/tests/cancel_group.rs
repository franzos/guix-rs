//! Regression: cancelling a repl op must kill the whole process group,
//! not just the direct child. `guix pull` forks git/substitute workers
//! that inherit fd 3 (the event pipe); if cancel signals only the leader,
//! those workers keep the pipe open and the reader never sees EOF, so the
//! operation hangs until they exit on their own (the ~100s freeze bug).
//!
//! The fake `guix repl` shim below reproduces that shape: it forks a
//! background worker that inherits fd 3 and sleeps, writes one event, then
//! the leader itself sleeps. Both sleeps far outlast the test's post-cancel
//! deadline, so reaching `ExitSummary` quickly proves the group was killed.

use std::time::Duration;

use futures_util::StreamExt;
use libguix::{Guix, ProgressEvent};

#[tokio::test(flavor = "multi_thread")]
async fn cancel_kills_worker_holding_event_pipe() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bin_dir = dir.path().join("bin");
    std::fs::create_dir_all(&bin_dir).expect("mkdir bin");
    let guix_path = bin_dir.join("guix");

    // Worker (`sleep 30 &`) inherits fd 3 and holds it open; the leader
    // then blocks in its own `sleep 30`. Neither exits within the test's
    // deadline, so only a group-wide signal can end the operation fast.
    let script = r#"#!/bin/sh
if [ "$1" = "--version" ]; then
    echo "guix (Guix) 9999-01-01.00"
    exit 0
fi
cat > /dev/null &
sleep 30 &
printf '(build-started "/gnu/store/abc-foo.drv" "-" "x86_64-linux" "")\n' >&3
sleep 30
"#;
    std::fs::write(&guix_path, script).expect("write shim");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&guix_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&guix_path, perms).expect("chmod");
    }

    let old_profile = std::env::var_os("GUIX_PROFILE");
    std::env::set_var("GUIX_PROFILE", dir.path());

    let restore = |old: Option<std::ffi::OsString>| {
        if let Some(p) = old {
            std::env::set_var("GUIX_PROFILE", p);
        } else {
            std::env::remove_var("GUIX_PROFILE");
        }
    };

    let g = match Guix::discover().await {
        Ok(g) => g,
        Err(e) => {
            restore(old_profile);
            panic!("fake-guix discover failed: {e}");
        }
    };
    assert_eq!(
        g.binary(),
        guix_path.as_path(),
        "discover should resolve fake guix"
    );

    let mut op = g.pull().user().expect("pull().user()");
    let cancel = op.take_cancel().expect("cancel handle");

    // Wait for the first event so the worker/leader are actually running.
    let first = tokio::time::timeout(Duration::from_secs(10), op.events_mut().next())
        .await
        .expect("no event before deadline");
    restore(old_profile);
    assert!(first.is_some(), "expected at least one event from the shim");

    // Cancel and require the operation to finish well under the 30s sleeps.
    // Without group kill, the fd-3 worker survives and this times out.
    let done = tokio::time::timeout(Duration::from_secs(12), async {
        let _ = cancel.cancel().await;
        let mut tail = Vec::new();
        while let Some(batch) = op.events_mut().next().await {
            tail.extend(batch);
        }
        tail
    })
    .await
    .expect("cancel did not reap the process group before the deadline");

    assert!(
        matches!(done.last(), Some(ProgressEvent::ExitSummary { .. })),
        "expected ExitSummary as final event, got {:?}",
        done.last()
    );
}
