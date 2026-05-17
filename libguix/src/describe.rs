//! Read-only `guix describe` operations.

use std::path::PathBuf;

use crate::cmd::run_guix;
use crate::error::GuixError;
use crate::parsers::sexp;
use crate::types::Channel;

#[derive(Clone)]
pub struct DescribeOps {
    pub(crate) binary: PathBuf,
    pub(crate) profile: Option<PathBuf>,
}

impl DescribeOps {
    /// `guix describe -f channels` — the user's current channels.
    pub async fn channels(&self) -> Result<Vec<Channel>, GuixError> {
        let out = run_guix(
            &self.binary,
            self.profile.as_deref(),
            ["describe", "-f", "channels"],
        )
        .await?;
        let s = String::from_utf8_lossy(&out);
        sexp::parse_channels(&s)
    }
}
