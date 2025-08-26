use std::fmt;
use crate::io::IoTaskId;

/// Errors that can occur during document operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentError {
    /// Document is not yet ready for operations (not loaded).
    DocumentNotReady,
    /// Document actor is in an invalid state.
    InvalidState(String),
    /// Document actor is stopped and cannot process operations.
    ActorStopped,
    /// Unexpected storage result for unknown task.
    UnexpectedStorageResult(IoTaskId),
    /// Unexpected announce policy completion for unknown task.
    UnexpectedPolicyCompletion(IoTaskId),
}

impl fmt::Display for DocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DocumentError::DocumentNotReady => {
                write!(f, "Document is not yet ready for operations")
            }
            DocumentError::InvalidState(msg) => {
                write!(f, "Invalid state: {msg}")
            }
            DocumentError::ActorStopped => {
                write!(f, "Document actor is stopped and cannot process operations")
            }
            DocumentError::UnexpectedStorageResult(task_id) => {
                write!(f, "Unexpected storage result for task {task_id:?}")
            }
            DocumentError::UnexpectedPolicyCompletion(task_id) => {
                write!(f, "Unexpected announce policy completion for task {task_id:?}")
            }
        }
    }
}

impl std::error::Error for DocumentError {}
