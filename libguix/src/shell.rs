//! `guix shell` — ephemeral environments and one-shot commands.

use std::path::PathBuf;

use crate::cmd::guix_cmd;
use crate::error::GuixError;
use crate::operation::{spawn_operation, Operation};
use crate::Guix;

#[derive(Clone)]
pub struct ShellOps {
    guix: Guix,
}

impl ShellOps {
    pub(crate) fn new(guix: Guix) -> Self {
        Self { guix }
    }

    /// One-shot: `guix shell PACKAGES... -- COMMAND ARGS...`.
    pub fn run(
        &self,
        packages: &[&str],
        command: &str,
        args: &[&str],
    ) -> Result<Operation, GuixError> {
        let mut b = self.builder();
        for p in packages {
            b = b.package(*p);
        }
        b.command(command, args).spawn()
    }

    pub fn builder(&self) -> ShellBuilder {
        ShellBuilder::new(self.guix.clone())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ShellArg {
    Package(String),
    DevelopmentPackage(String),
}

pub struct ShellBuilder {
    guix: Guix,
    items: Vec<ShellArg>,
    pending_dev: bool,
    manifest: Option<PathBuf>,
    pure: bool,
    container: bool,
    network: bool,
    preserve: Vec<String>,
    command: Option<(String, Vec<String>)>,
}

impl ShellBuilder {
    fn new(guix: Guix) -> Self {
        Self {
            guix,
            items: Vec::new(),
            pending_dev: false,
            manifest: None,
            pure: false,
            container: false,
            network: false,
            preserve: Vec::new(),
            command: None,
        }
    }

    /// Positional `PACKAGE`. If [`Self::development`] was called previously,
    /// this package is recorded as a development input (`-D NAME`) and the
    /// flag resets.
    pub fn package(mut self, name: impl Into<String>) -> Self {
        let n = name.into();
        if self.pending_dev {
            self.pending_dev = false;
            self.items.push(ShellArg::DevelopmentPackage(n));
        } else {
            self.items.push(ShellArg::Package(n));
        }
        self
    }

    pub fn packages<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for n in names {
            self = self.package(n);
        }
        self
    }

    /// Sets a "next package is a development input" flag. `guix shell -D P`
    /// brings in P's *inputs*, not P itself; the flag binds to the next
    /// [`Self::package`] call so argv order is preserved.
    pub fn development(mut self) -> Self {
        self.pending_dev = true;
        self
    }

    pub fn manifest(mut self, path: impl Into<PathBuf>) -> Self {
        self.manifest = Some(path.into());
        self
    }

    pub fn pure(mut self) -> Self {
        self.pure = true;
        self
    }

    pub fn container(mut self) -> Self {
        self.container = true;
        self
    }

    /// Only meaningful with [`Self::container`].
    pub fn network(mut self) -> Self {
        self.network = true;
        self
    }

    pub fn preserve(mut self, regex: impl Into<String>) -> Self {
        self.preserve.push(regex.into());
        self
    }

    /// Sets the trailing `-- COMMAND ARGS...`. Without this, the shell is
    /// interactive.
    pub fn command(mut self, cmd: impl Into<String>, args: &[&str]) -> Self {
        self.command = Some((cmd.into(), args.iter().map(|s| (*s).to_string()).collect()));
        self
    }

    pub fn spawn(self) -> Result<Operation, GuixError> {
        let argv = self.argv();
        let c = guix_cmd(
            self.guix.binary_path(),
            self.guix.profile_path(),
            true,
            &argv,
        );
        spawn_operation(c)
    }

    /// Composes the full argv (including the `shell` subcommand and any
    /// trailing `-- COMMAND ARGS...`). Exposed for unit tests.
    pub(crate) fn argv(&self) -> Vec<String> {
        let mut a: Vec<String> = vec!["shell".into()];

        if self.pure {
            a.push("--pure".into());
        }
        if self.container {
            a.push("-C".into());
        }
        if self.network {
            a.push("-N".into());
        }
        if let Some(m) = &self.manifest {
            a.push("-m".into());
            a.push(m.to_string_lossy().into_owned());
        }
        for r in &self.preserve {
            a.push("-E".into());
            a.push(r.clone());
        }
        for item in &self.items {
            match item {
                ShellArg::Package(n) => a.push(n.clone()),
                ShellArg::DevelopmentPackage(n) => {
                    a.push("-D".into());
                    a.push(n.clone());
                }
            }
        }
        if let Some((cmd, args)) = &self.command {
            a.push("--".into());
            a.push(cmd.clone());
            for arg in args {
                a.push(arg.clone());
            }
        }
        a
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration;

    fn fake_guix() -> Guix {
        Guix {
            binary: PathBuf::from("/nonexistent/fake-guix"),
            version: "0".into(),
            profile: None,
            repl: Arc::new(tokio::sync::OnceCell::new()),
            repl_timeout: Duration::from_secs(30),
        }
    }

    fn ops() -> ShellOps {
        ShellOps::new(fake_guix())
    }

    #[test]
    fn run_emits_packages_then_double_dash_then_command() {
        let argv = ops()
            .builder()
            .package("hello")
            .package("coreutils")
            .command("echo", &["hi", "world"])
            .argv();
        assert_eq!(
            argv,
            vec!["shell", "hello", "coreutils", "--", "echo", "hi", "world"]
        );
    }

    #[test]
    fn pure_container_network_flags_present() {
        let argv = ops()
            .builder()
            .pure()
            .container()
            .network()
            .package("hello")
            .argv();
        assert_eq!(argv, vec!["shell", "--pure", "-C", "-N", "hello"]);
    }

    #[test]
    fn development_binds_to_next_package_in_order() {
        let argv = ops()
            .builder()
            .development()
            .package("hello")
            .package("git")
            .argv();
        assert_eq!(argv, vec!["shell", "-D", "hello", "git"]);
    }

    #[test]
    fn manifest_and_multiple_preserve() {
        let argv = ops()
            .builder()
            .manifest("/tmp/m.scm")
            .preserve("^FOO_")
            .preserve("^BAR_")
            .argv();
        assert_eq!(
            argv,
            vec!["shell", "-m", "/tmp/m.scm", "-E", "^FOO_", "-E", "^BAR_"]
        );
    }

    #[test]
    fn packages_iter_appends_in_order() {
        let argv = ops().builder().packages(["a", "b", "c"]).argv();
        assert_eq!(argv, vec!["shell", "a", "b", "c"]);
    }

    #[test]
    fn development_resets_after_consumption() {
        let argv = ops()
            .builder()
            .development()
            .package("hello")
            .package("git")
            .development()
            .package("coreutils")
            .argv();
        assert_eq!(argv, vec!["shell", "-D", "hello", "git", "-D", "coreutils"]);
    }
}
