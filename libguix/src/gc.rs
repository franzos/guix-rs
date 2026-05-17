//! `guix gc` — store-wide garbage collection.

use std::time::Duration;

use crate::cmd::guix_cmd;
use crate::error::GuixError;
use crate::operation::{spawn_operation, Operation};
use crate::Guix;

#[derive(Clone)]
pub struct GcOps {
    guix: Guix,
}

impl GcOps {
    pub(crate) fn new(guix: Guix) -> Self {
        Self { guix }
    }

    /// Does **not** thread `-p` — `gc` is store-wide.
    pub fn run(&self, opts: GcOptions) -> Result<Operation, GuixError> {
        let mut args: Vec<String> = vec!["gc".into()];
        if let Some(free) = opts.free_space {
            args.push(format!("-F{}", free.as_bytes()));
        }
        if let Some(d) = opts.delete_unused_after {
            args.push(format!("--delete-generations={}s", d.as_secs()));
        }
        if opts.verify {
            args.push("--verify".into());
        }
        let c = guix_cmd(
            self.guix.binary_path(),
            self.guix.profile_path(),
            false,
            &args,
        );
        spawn_operation(c)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ByteSize(u64);

impl ByteSize {
    pub fn bytes(n: u64) -> Self {
        Self(n)
    }
    pub fn megabytes(n: u64) -> Self {
        Self(n * 1_000_000)
    }
    pub fn gigabytes(n: u64) -> Self {
        Self(n * 1_000_000_000)
    }
    pub fn as_bytes(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Default)]
pub struct GcOptions {
    pub free_space: Option<ByteSize>,
    pub delete_unused_after: Option<Duration>,
    pub verify: bool,
}
