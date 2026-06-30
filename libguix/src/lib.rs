//! Unofficial Rust client library for GNU Guix.

#![deny(rust_2018_idioms)]
#![warn(clippy::pedantic)]
#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::needless_pass_by_value,
    clippy::too_many_lines,
    clippy::doc_markdown,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap,
    clippy::if_not_else,
    clippy::single_match_else,
    clippy::uninlined_format_args,
    clippy::needless_continue,
    clippy::collapsible_if,
    clippy::struct_excessive_bools,
    clippy::similar_names,
    clippy::return_self_not_must_use,
    clippy::unused_async,
    clippy::map_unwrap_or,
    clippy::manual_let_else,
    // tokio::select!'s `_ = future =>` arms trip 1.93's `() = ...` preference.
    clippy::ignored_unit_patterns
)]

#[cfg(feature = "tracing")]
macro_rules! trace_debug {
    ($($t:tt)*) => { ::tracing::debug!($($t)*) };
}
#[cfg(not(feature = "tracing"))]
macro_rules! trace_debug {
    ($($t:tt)*) => {};
}
#[cfg(feature = "tracing")]
macro_rules! trace_warn {
    ($($t:tt)*) => { ::tracing::warn!($($t)*) };
}
#[cfg(not(feature = "tracing"))]
macro_rules! trace_warn {
    ($($t:tt)*) => {};
}

pub(crate) use trace_debug;
pub(crate) use trace_warn;

mod archive;
mod build;
mod channels;
mod cmd;
mod describe;
mod discover;
mod error;
mod gc;
mod installed;
mod operation;
mod options;
mod package;
mod parsers;
mod process;
// Public so external GUIs (e.g. the installer) can reuse the state machine.
pub mod progress;
mod pull;
mod repl;
mod retry;
mod shell;
mod system;
mod types;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

pub use archive::ArchiveOps;
pub use build::{BuildBuilder, BuildOps};
pub use channels::{ChannelOp, ChannelsError, ChannelsFile};
pub use describe::DescribeOps;
pub use discover::{Discovered, MIN_GUIX_VERSION_DATE};
pub use error::{GuixError, PolkitFailure};
pub use gc::{ByteSize, GcOps, GcOptions};
pub use installed::{InstalledOps, UNKNOWN_BUCKET};
pub use operation::{CancelHandle, EventStream, Operation};
pub use options::{BuildOptions, Privilege};
pub use package::{PackageOps, SearchFastResult, DEFAULT_SEARCH_LIMIT};
pub use parsers::sexp::{parse_channels_list, ChannelsList};
pub use pull::{PullOps, SystemPullOptions};
pub use repl::Repl;
pub use retry::{run_with_retry, RetryPolicy};
pub use shell::{ShellBuilder, ShellOps};
pub use system::{
    auth_agent_present, InitOptions, ReconfigureOptions, SystemOps, CURRENT_SYSTEM_CONFIG,
};
pub use types::{
    Channel, Generation, InstalledPackage, KnownBug, PackageDetail, PackageSummary, ProgressEvent,
    ProgressStream,
};

#[doc(hidden)]
pub mod __test_support {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration;

    use tokio::process::Command;

    use crate::error::GuixError;
    use crate::operation::{spawn_operation_with, ExitClassifier, Operation};
    use crate::pull::PullOps;
    use crate::system::SystemOps;
    use crate::Guix;

    pub fn operation_from_command(cmd: Command) -> Result<Operation, GuixError> {
        crate::operation::spawn_operation(cmd)
    }

    pub fn pkexec_operation_from_command(cmd: Command) -> Result<Operation, GuixError> {
        spawn_operation_with(cmd, ExitClassifier::Pkexec)
    }

    pub fn system_ops() -> SystemOps {
        SystemOps::new_for_tests()
    }

    pub fn pull_ops_with_fake_binary() -> PullOps {
        let g = Guix {
            binary: PathBuf::from("/nonexistent/fake-guix"),
            version: "0".into(),
            profile: None,
            repl: Arc::new(tokio::sync::OnceCell::new()),
            repl_timeout: Duration::from_secs(30),
        };
        PullOps::new(g)
    }
}

pub const DEFAULT_REPL_TIMEOUT: Duration = Duration::from_secs(30);

/// Construct via [`Guix::discover`].
#[derive(Clone)]
pub struct Guix {
    binary: PathBuf,
    version: String,
    profile: Option<PathBuf>,
    repl: Arc<tokio::sync::OnceCell<Repl>>,
    repl_timeout: Duration,
}

impl Guix {
    pub async fn discover() -> Result<Self, GuixError> {
        let d = discover::discover().await?;
        Ok(Self {
            binary: d.binary,
            version: d.version,
            profile: None,
            repl: Arc::new(tokio::sync::OnceCell::new()),
            repl_timeout: DEFAULT_REPL_TIMEOUT,
        })
    }

    pub fn with_profile(mut self, profile: impl Into<PathBuf>) -> Self {
        self.profile = Some(profile.into());
        self
    }

    /// Must be called before the first `repl()` — the cached actor
    /// keeps the existing timeout otherwise.
    pub fn with_repl_timeout(mut self, timeout: Duration) -> Self {
        self.repl_timeout = timeout;
        self
    }

    pub fn binary(&self) -> &Path {
        &self.binary
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn package(&self) -> PackageOps {
        PackageOps::new(self.clone())
    }

    pub fn describe(&self) -> DescribeOps {
        DescribeOps {
            binary: self.binary.clone(),
            profile: self.profile.clone(),
        }
    }

    /// The `pkexec` path targets `/run/current-system/profile/bin/guix`;
    /// `self.binary` is threaded for the already-root (installer) path.
    pub fn system(&self) -> SystemOps {
        SystemOps::new(self.binary.clone())
    }

    /// The `pkexec` path targets `/run/current-system/profile/bin/guix`;
    /// `self.binary` is threaded for the already-root (installer) path.
    pub fn archive(&self) -> ArchiveOps {
        ArchiveOps::new(self.binary.clone())
    }

    pub fn pull(&self) -> PullOps {
        PullOps::new(self.clone())
    }

    pub fn installed(&self) -> InstalledOps {
        InstalledOps::new(self.clone())
    }

    pub fn gc(&self) -> GcOps {
        GcOps::new(self.clone())
    }

    pub fn shell(&self) -> ShellOps {
        ShellOps::new(self.clone())
    }

    pub fn build(&self) -> BuildOps {
        BuildOps::new(self.clone())
    }

    pub async fn repl(&self) -> Result<Repl, GuixError> {
        let binary = self.binary.clone();
        let timeout = self.repl_timeout;
        let r = self
            .repl
            .get_or_try_init(|| async move { Repl::spawn(binary, timeout).await })
            .await?;
        Ok(r.clone())
    }

    /// Returns `None` if [`Guix::repl`] hasn't completed yet — lets sync
    /// contexts fire-and-forget `interrupt()` without spawning.
    pub fn repl_if_ready(&self) -> Option<Repl> {
        self.repl.get().cloned()
    }

    pub fn system_profile_path() -> PathBuf {
        PathBuf::from(format!("{}/system", system::GUIX_PROFILES_ROOT))
    }

    pub(crate) fn binary_path(&self) -> &Path {
        &self.binary
    }

    pub(crate) fn profile_path(&self) -> Option<&Path> {
        self.profile.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_profile_path_layout() {
        assert_eq!(
            Guix::system_profile_path(),
            PathBuf::from("/var/guix/profiles/system")
        );
    }
}
