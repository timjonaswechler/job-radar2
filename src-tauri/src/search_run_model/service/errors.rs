use super::super::SourceRunStatus;

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceExecutionError {
    Failed(String),
    Cancelled(String),
}

impl SourceExecutionError {
    pub(super) fn status(&self) -> SourceRunStatus {
        match self {
            Self::Failed(_) => SourceRunStatus::Failed,
            Self::Cancelled(_) => SourceRunStatus::Cancelled,
        }
    }

    pub(super) fn message(&self) -> String {
        match self {
            Self::Failed(message) | Self::Cancelled(message) => message.clone(),
        }
    }
}
