use std::fmt;

use serde_json::Value;

pub type CommandResult<T> = Result<T, CommandError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitStatus {
    Failure,
    Blocked,
}

impl ExitStatus {
    pub fn code(self) -> u8 {
        match self {
            Self::Failure => 1,
            Self::Blocked => 2,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CommandError {
    exit_status: ExitStatus,
    code: &'static str,
    message: String,
    hint: &'static str,
    details: Option<Value>,
}

impl CommandError {
    pub fn failure(code: &'static str, message: impl Into<String>, hint: &'static str) -> Self {
        Self {
            exit_status: ExitStatus::Failure,
            code,
            message: message.into(),
            hint,
            details: None,
        }
    }

    pub fn blocked(code: &'static str, message: impl Into<String>, hint: &'static str) -> Self {
        Self {
            exit_status: ExitStatus::Blocked,
            code,
            message: message.into(),
            hint,
            details: None,
        }
    }

    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn exit_status(&self) -> ExitStatus {
        self.exit_status
    }

    pub fn code(&self) -> &str {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn hint(&self) -> &str {
        self.hint
    }

    pub fn details(&self) -> Option<&Value> {
        self.details.as_ref()
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CommandError {}
