use std::any::Any;
use std::fmt;
use std::panic::{catch_unwind, AssertUnwindSafe};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReconstructionFailure {
    workflow: &'static str,
    kind: ReconstructionFailureKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ReconstructionFailureKind {
    Error(String),
    Panic(String),
}

impl fmt::Display for ReconstructionFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ReconstructionFailureKind::Error(error) => {
                write!(formatter, "{} failed: {error}", self.workflow)
            }
            ReconstructionFailureKind::Panic(message) => {
                write!(
                    formatter,
                    "{} failed after an internal panic: {message}",
                    self.workflow
                )
            }
        }
    }
}

pub(super) fn contain_reconstruction<T>(
    workflow: &'static str,
    reconstruct: impl FnOnce() -> Result<T, String>,
) -> Result<T, ReconstructionFailure> {
    match catch_unwind(AssertUnwindSafe(reconstruct)) {
        Ok(Ok(candidate)) => Ok(candidate),
        Ok(Err(error)) => Err(ReconstructionFailure {
            workflow,
            kind: ReconstructionFailureKind::Error(error),
        }),
        Err(payload) => Err(ReconstructionFailure {
            workflow,
            kind: ReconstructionFailureKind::Panic(panic_message(payload.as_ref())),
        }),
    }
}

fn panic_message(payload: &(dyn Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|message| (*message).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "non-string panic payload".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconstruction_errors_and_panics_have_structured_context() {
        let error =
            contain_reconstruction::<()>("replay seek", || Err("bad command".into())).unwrap_err();
        assert_eq!(error.to_string(), "replay seek failed: bad command");

        let panic =
            contain_reconstruction::<()>("lab seek", || panic!("injected panic")).unwrap_err();
        assert_eq!(
            panic.to_string(),
            "lab seek failed after an internal panic: injected panic"
        );
    }
}
