//! Long-lived `guix repl -t machine` actor. Requests serialise through
//! a single in-flight slot — see NOTES.md.

pub(crate) mod actor;
pub(crate) mod framer;
pub(crate) mod op;

pub use actor::Repl;
