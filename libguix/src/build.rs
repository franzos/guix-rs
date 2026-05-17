//! `guix build` — build packages without installing.
//!
//! Output is one `/gnu/store/...` path per line on stdout. The argv is
//! composed manually here; the same `Operation` machinery as the rest of
//! the crate streams events to the caller.

use std::path::PathBuf;

use futures_util::StreamExt;

use crate::cmd::guix_cmd;
use crate::error::GuixError;
use crate::operation::{spawn_operation, Operation};
use crate::types::{ProgressEvent, ProgressStream};
use crate::Guix;

#[derive(Clone)]
pub struct BuildOps {
    guix: Guix,
}

impl BuildOps {
    pub(crate) fn new(guix: Guix) -> Self {
        Self { guix }
    }

    /// One-shot: `guix build PACKAGES...`.
    pub fn run(&self, packages: &[&str]) -> Result<Operation, GuixError> {
        let mut b = self.builder();
        for p in packages {
            b = b.package(*p);
        }
        b.spawn()
    }

    pub fn builder(&self) -> BuildBuilder {
        BuildBuilder::new(self.guix.clone())
    }

    /// Drains `op`'s event stream to completion and returns every
    /// `/gnu/store/...` line emitted on stdout. Errors mirror
    /// [`Operation::await_completion`] semantics.
    pub async fn collect_store_paths(mut op: Operation) -> Result<Vec<PathBuf>, GuixError> {
        let mut paths: Vec<PathBuf> = Vec::new();
        let mut last_exit: Option<i32> = None;

        while let Some(batch) = op.events.next().await {
            for evt in batch {
                match evt {
                    ProgressEvent::Line {
                        stream: ProgressStream::Stdout,
                        text,
                        ..
                    } => {
                        if let Some(p) = parse_store_path(&text) {
                            paths.push(p);
                        }
                    }
                    ProgressEvent::StorePathListed { path } => {
                        if let Some(p) = parse_store_path(&path) {
                            paths.push(p);
                        }
                    }
                    ProgressEvent::ExitSummary { code, .. } => {
                        last_exit = Some(code);
                    }
                    _ => {}
                }
            }
        }

        match last_exit {
            Some(0) => Ok(paths),
            Some(code) => Err(GuixError::OperationFailed {
                code,
                stderr_tail: String::new(),
            }),
            None => Err(GuixError::Cancelled),
        }
    }

    /// Spawn and drain in one call. See [`Self::collect_store_paths`] for
    /// the streaming variant.
    pub async fn run_to_paths(&self, packages: &[&str]) -> Result<Vec<PathBuf>, GuixError> {
        let op = self.run(packages)?;
        Self::collect_store_paths(op).await
    }
}

/// Matches a bare `/gnu/store/...` token (no internal whitespace). Trims
/// leading whitespace so indented listings parse too. Returns `None` for
/// lines that mention store paths in passing (e.g. build phase chatter).
fn parse_store_path(text: &str) -> Option<PathBuf> {
    let t = text.trim();
    if !t.starts_with("/gnu/store/") {
        return None;
    }
    if t.split_whitespace().count() != 1 {
        return None;
    }
    Some(PathBuf::from(t))
}

pub struct BuildBuilder {
    guix: Guix,
    packages: Vec<String>,
    expressions: Vec<String>,
    files: Vec<PathBuf>,
    manifests: Vec<PathBuf>,
    derivations: bool,
    dry_run: bool,
    check: bool,
    log_file: bool,
    quiet: bool,
    root: Option<PathBuf>,
}

impl BuildBuilder {
    fn new(guix: Guix) -> Self {
        Self {
            guix,
            packages: Vec::new(),
            expressions: Vec::new(),
            files: Vec::new(),
            manifests: Vec::new(),
            derivations: false,
            dry_run: false,
            check: false,
            log_file: false,
            quiet: false,
            root: None,
        }
    }

    pub fn package(mut self, name: impl Into<String>) -> Self {
        self.packages.push(name.into());
        self
    }

    pub fn packages<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for n in names {
            self.packages.push(n.into());
        }
        self
    }

    /// `-e EXPR`. Repeatable; combinable with [`Self::package`].
    pub fn expression(mut self, expr: impl Into<String>) -> Self {
        self.expressions.push(expr.into());
        self
    }

    /// `-f FILE`.
    pub fn file(mut self, path: impl Into<PathBuf>) -> Self {
        self.files.push(path.into());
        self
    }

    /// `-m FILE`.
    pub fn manifest(mut self, path: impl Into<PathBuf>) -> Self {
        self.manifests.push(path.into());
        self
    }

    /// `-d` — return derivation paths instead of building.
    pub fn derivations(mut self) -> Self {
        self.derivations = true;
        self
    }

    /// `-n` — show what would be built/downloaded without doing it.
    pub fn dry_run(mut self) -> Self {
        self.dry_run = true;
        self
    }

    /// `--check` — rebuild and compare to existing outputs.
    pub fn check(mut self) -> Self {
        self.check = true;
        self
    }

    /// `-r FILE` — register a GC root symlink at FILE.
    pub fn root(mut self, path: impl Into<PathBuf>) -> Self {
        self.root = Some(path.into());
        self
    }

    /// `--log-file` — return log file names instead of building.
    pub fn log_file(mut self) -> Self {
        self.log_file = true;
        self
    }

    /// `-q`.
    pub fn quiet(mut self) -> Self {
        self.quiet = true;
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

    /// Composes the full argv (including the `build` subcommand). Exposed
    /// for unit tests.
    pub(crate) fn argv(&self) -> Vec<String> {
        let mut a: Vec<String> = vec!["build".into()];

        if self.derivations {
            a.push("-d".into());
        }
        if self.dry_run {
            a.push("-n".into());
        }
        if self.check {
            a.push("--check".into());
        }
        if self.log_file {
            a.push("--log-file".into());
        }
        if self.quiet {
            a.push("-q".into());
        }
        if let Some(r) = &self.root {
            a.push("-r".into());
            a.push(r.to_string_lossy().into_owned());
        }
        for e in &self.expressions {
            a.push("-e".into());
            a.push(e.clone());
        }
        for f in &self.files {
            a.push("-f".into());
            a.push(f.to_string_lossy().into_owned());
        }
        for m in &self.manifests {
            a.push("-m".into());
            a.push(m.to_string_lossy().into_owned());
        }
        for p in &self.packages {
            a.push(p.clone());
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
    use tokio::process::Command;

    fn fake_guix() -> Guix {
        Guix {
            binary: PathBuf::from("/nonexistent/fake-guix"),
            version: "0".into(),
            profile: None,
            repl: Arc::new(tokio::sync::OnceCell::new()),
            repl_timeout: Duration::from_secs(30),
        }
    }

    fn ops() -> BuildOps {
        BuildOps::new(fake_guix())
    }

    #[test]
    fn run_emits_subcommand_and_packages() {
        let argv = ops().builder().package("hello").argv();
        assert_eq!(argv, vec!["build", "hello"]);
    }

    #[test]
    fn derivations_with_multiple_packages() {
        let argv = ops()
            .builder()
            .derivations()
            .packages(["hello", "coreutils"])
            .argv();
        assert_eq!(argv, vec!["build", "-d", "hello", "coreutils"]);
    }

    #[test]
    fn expression_file_manifest_all_present() {
        let argv = ops()
            .builder()
            .expression("(package foo)")
            .file("./pkg.scm")
            .manifest("./m.scm")
            .argv();
        assert_eq!(
            argv,
            vec![
                "build",
                "-e",
                "(package foo)",
                "-f",
                "./pkg.scm",
                "-m",
                "./m.scm",
            ]
        );
    }

    #[test]
    fn root_check_dry_run_log_file_flags() {
        let argv = ops()
            .builder()
            .check()
            .dry_run()
            .log_file()
            .root("/tmp/r")
            .package("hello")
            .argv();
        assert_eq!(
            argv,
            vec![
                "build",
                "-n",
                "--check",
                "--log-file",
                "-r",
                "/tmp/r",
                "hello",
            ]
        );
    }

    #[test]
    fn quiet_flag_present() {
        let argv = ops().builder().quiet().package("hello").argv();
        assert_eq!(argv, vec!["build", "-q", "hello"]);
    }

    #[test]
    fn expression_repeatable_and_combined_with_packages() {
        let argv = ops()
            .builder()
            .expression("(a)")
            .expression("(b)")
            .package("hello")
            .argv();
        assert_eq!(argv, vec!["build", "-e", "(a)", "-e", "(b)", "hello"]);
    }

    #[test]
    fn parses_store_path_token() {
        assert_eq!(
            parse_store_path("/gnu/store/abc-hello-2.12"),
            Some(PathBuf::from("/gnu/store/abc-hello-2.12"))
        );
        assert_eq!(
            parse_store_path("  /gnu/store/abc-hello-2.12  "),
            Some(PathBuf::from("/gnu/store/abc-hello-2.12"))
        );
    }

    #[test]
    fn rejects_non_store_lines_and_trailing_text() {
        assert_eq!(parse_store_path("hello world"), None);
        assert_eq!(
            parse_store_path("/gnu/store/abc-foo cached substitute"),
            None
        );
        assert_eq!(parse_store_path(""), None);
    }

    /// Smoke-tests `collect_store_paths` against a real subprocess
    /// (`/bin/echo`) so the event-pipeline contract is exercised end-to-end.
    /// No `guix` involvement.
    #[tokio::test]
    async fn collect_store_paths_drains_echoed_paths() {
        let mut c = Command::new("printf");
        c.arg("%s\n")
            .arg("/gnu/store/abc-hello")
            .arg("/gnu/store/def-coreutils")
            .arg("not a store path");
        let op = crate::__test_support::operation_from_command(c).expect("spawn printf");
        let paths = BuildOps::collect_store_paths(op).await.expect("drain ok");
        assert_eq!(
            paths,
            vec![
                PathBuf::from("/gnu/store/abc-hello"),
                PathBuf::from("/gnu/store/def-coreutils"),
            ]
        );
    }
}
