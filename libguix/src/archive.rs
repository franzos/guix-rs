//! `guix archive` operations. Currently only `--authorize`, which the
//! installer runs already-root to trust a substitute server's public key.

use std::path::PathBuf;
use std::process::Stdio;

use tokio::io::AsyncWriteExt;

use crate::error::GuixError;
use crate::options::{privileged_guix_cmd, Privilege};

/// `binary` is only consulted under [`Privilege::AlreadyRoot`]; the `pkexec`
/// path always targets the trusted-path guix (see `cmd::POLKIT_GUIX_PATH`).
#[derive(Clone)]
pub struct ArchiveOps {
    binary: PathBuf,
}

impl ArchiveOps {
    pub(crate) fn new(binary: PathBuf) -> Self {
        Self { binary }
    }

    /// `guix archive --authorize` — appends the given ACL entry (a public-key
    /// s-expression, read on stdin) to the store's trusted-keys ACL. Needs
    /// root: the installer runs [`Privilege::AlreadyRoot`].
    pub async fn authorize(&self, key: &str, privilege: Privilege) -> Result<(), GuixError> {
        if privilege == Privilege::Pkexec {
            crate::system::preflight_auth_agent()?;
        }

        let (mut cmd, _classifier) = privileged_guix_cmd(privilege, &self.binary, &build_args())?;
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(GuixError::Spawn)?;
        // Take + drop stdin so guix sees EOF and stops waiting for input.
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| GuixError::Internal("archive: child stdin not piped".into()))?;
        stdin
            .write_all(key.as_bytes())
            .await
            .map_err(GuixError::Io)?;
        stdin.write_all(b"\n").await.map_err(GuixError::Io)?;
        stdin.flush().await.map_err(GuixError::Io)?;
        drop(stdin);

        let out = child.wait_with_output().await.map_err(GuixError::Spawn)?;
        if out.status.success() {
            return Ok(());
        }
        Err(GuixError::NonZeroExit {
            code: out.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }
}

fn build_args() -> Vec<String> {
    vec!["archive".into(), "--authorize".into()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorize_args_are_archive_authorize() {
        assert_eq!(build_args(), vec!["archive", "--authorize"]);
    }

    /// A fake guix that ignores argv, drains stdin to a file, and exits by
    /// the byte count — proving `authorize` feeds `<key>\n` then closes stdin.
    #[cfg(unix)]
    #[tokio::test]
    async fn authorize_feeds_key_to_stdin_via_fake_binary() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::tempdir().expect("tempdir");
        let captured = tmp.path().join("stdin.txt");
        let script = tmp.path().join("fake-guix");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\ncat > '{}'\n[ -s '{}' ] && exit 0 || exit 7\n",
                captured.display(),
                captured.display(),
            ),
        )
        .expect("write script");
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("chmod");

        let ops = ArchiveOps::new(script);
        // ETXTBSY: a sibling test's fork/exec can briefly hold a write fd to
        // our freshly-written fake binary. Retry past that race.
        let mut tries = 0;
        loop {
            match ops
                .authorize("(public-key ...)", Privilege::AlreadyRoot)
                .await
            {
                Ok(()) => break,
                Err(GuixError::Spawn(e))
                    if e.raw_os_error() == Some(libc::ETXTBSY) && tries < 20 =>
                {
                    tries += 1;
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                }
                Err(e) => panic!("fake guix should exit 0 after consuming stdin: {e:?}"),
            }
        }

        let got = std::fs::read_to_string(&captured).expect("read captured stdin");
        assert_eq!(got, "(public-key ...)\n");
    }
}
