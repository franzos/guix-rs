//! GUI-side i18n shim over [`libguix::progress`] — the state machine lives
//! in the lib; here we map [`Stage`] / [`Failure`] to translated strings.

pub use libguix::progress::{BuildStatus, Failure, Stage, Summary as ProgressSummary};

pub trait StageLabel {
    fn label(self) -> String;
}

impl StageLabel for Stage {
    fn label(self) -> String {
        match self {
            Stage::Starting => crate::t!("stage-starting"),
            Stage::ChannelUpdate => crate::t!("stage-channel-update"),
            Stage::ComputingDeriv => crate::t!("stage-computing-deriv"),
            Stage::Downloading => crate::t!("stage-downloading"),
            Stage::Building => crate::t!("stage-building"),
            Stage::Profile => crate::t!("stage-profile"),
            Stage::Done => crate::t!("stage-done"),
            Stage::Failed => crate::t!("stage-failed"),
        }
    }
}

pub fn failure_text(f: &Failure) -> String {
    match f {
        Failure::Exit { code } => {
            let code: i32 = *code;
            crate::t!("app-failed-exit", code = code)
        }
        Failure::Build {
            name,
            log_path: Some(log),
        } => crate::t!(
            "stage-build-failed-log",
            name = name.clone(),
            log = log.clone()
        ),
        Failure::Build {
            name,
            log_path: None,
        } => crate::t!("stage-build-failed", name = name.clone()),
    }
}
