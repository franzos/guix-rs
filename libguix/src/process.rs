//! `SIGTERM` → 5s → `SIGKILL`. `try_wait()` before every `kill()` mitigates
//! PID-reuse; the SIGKILL escalation uses tokio's reaper.

use std::time::Duration;

use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;
use tokio::process::Child;
use tokio::time::timeout;

pub(crate) const KILL_GRACE: Duration = Duration::from_secs(5);

/// Exit code encoded `128 + signum` on signal-termination.
pub(crate) async fn graceful_kill(child: &mut Child) -> Option<i32> {
    match child.try_wait() {
        Ok(Some(status)) => return Some(status_to_code(status)),
        Ok(None) => {}
        Err(_) => return None,
    }

    let Some(pid) = child.id() else {
        return child.wait().await.ok().map(status_to_code);
    };
    let pid = Pid::from_raw(pid as i32);

    // ESRCH is fine — child may have exited between try_wait and kill.
    let _ = kill(pid, Signal::SIGTERM);

    match timeout(KILL_GRACE, child.wait()).await {
        Ok(Ok(status)) => Some(status_to_code(status)),
        Ok(Err(_)) => None,
        Err(_) => {
            let _ = child.start_kill();
            child.wait().await.ok().map(status_to_code)
        }
    }
}

fn status_to_code(status: std::process::ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = status.signal() {
            return 128 + sig;
        }
    }
    -1
}
