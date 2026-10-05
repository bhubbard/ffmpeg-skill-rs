// ============================================================================
// error.rs — Standard error kinds and exit codes for ffmpeg-skill-rs
// Part of ffmpeg-skill-rs
// ============================================================================

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Standard exit codes adhering to the ffmpeg-skill execution contract.
pub struct ExitCodes;

impl ExitCodes {
    pub const SUCCESS: i32 = 0;
    pub const FAILURE: i32 = 1;
    pub const UNDECIDED: i32 = 2; // Used in doctor when required capability status is unknown
    pub const TIMEOUT: i32 = 124;
    pub const MISSING_TOOL: i32 = 127;
    pub const INTERRUPTED: i32 = 130; // 128 + SIGINT
}

/// Category of error reported in the machine-readable error document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    Input,
    Ffmpeg,
    Output,
    MissingTool,
    Timeout,
    Verification,
    Interrupted,
}

impl std::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorKind::Input => write!(f, "input"),
            ErrorKind::Ffmpeg => write!(f, "ffmpeg"),
            ErrorKind::Output => write!(f, "output"),
            ErrorKind::MissingTool => write!(f, "missing_tool"),
            ErrorKind::Timeout => write!(f, "timeout"),
            ErrorKind::Verification => write!(f, "verification"),
            ErrorKind::Interrupted => write!(f, "interrupted"),
        }
    }
}

/// Machine-readable error payload embedded in tool failure JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorPayload {
    pub kind: ErrorKind,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

/// Unified error type for all ffmpeg-skill operations.
#[derive(Error, Debug)]
pub enum FfmpegSkillError {
    #[error("input error: {message}")]
    Input {
        message: String,
        hint: Option<String>,
    },

    #[error("ffmpeg error (exit {exit_code}): {message}")]
    Ffmpeg {
        exit_code: i32,
        message: String,
        hint: Option<String>,
    },

    #[error("output verification error: {message}")]
    Output {
        message: String,
        hint: Option<String>,
    },

    #[error("missing required binary '{tool}': {message}")]
    MissingTool {
        tool: String,
        message: String,
        hint: Option<String>,
    },

    #[error("execution timed out after {seconds}s")]
    Timeout {
        seconds: f64,
        hint: Option<String>,
    },

    #[error("compliance verification failed: {message}")]
    Verification {
        message: String,
        hint: Option<String>,
    },

    #[error("operation interrupted: {message}")]
    Interrupted { message: String },

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json serialization error: {0}")]
    Json(#[from] serde_json::Error),
}

impl FfmpegSkillError {
    pub fn kind(&self) -> ErrorKind {
        match self {
            Self::Input { .. } => ErrorKind::Input,
            Self::Ffmpeg { .. } => ErrorKind::Ffmpeg,
            Self::Output { .. } => ErrorKind::Output,
            Self::MissingTool { .. } => ErrorKind::MissingTool,
            Self::Timeout { .. } => ErrorKind::Timeout,
            Self::Verification { .. } => ErrorKind::Verification,
            Self::Interrupted { .. } => ErrorKind::Interrupted,
            Self::Io(_) => ErrorKind::Input,
            Self::Json(_) => ErrorKind::Input,
        }
    }

    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Input { .. } => ExitCodes::FAILURE,
            Self::Ffmpeg { exit_code, .. } => *exit_code,
            Self::Output { .. } => ExitCodes::FAILURE,
            Self::MissingTool { .. } => ExitCodes::MISSING_TOOL,
            Self::Timeout { .. } => ExitCodes::TIMEOUT,
            Self::Verification { .. } => ExitCodes::FAILURE,
            Self::Interrupted { .. } => ExitCodes::INTERRUPTED,
            Self::Io(_) => ExitCodes::FAILURE,
            Self::Json(_) => ExitCodes::FAILURE,
        }
    }

    pub fn hint(&self) -> Option<String> {
        match self {
            Self::Input { hint, .. }
            | Self::Ffmpeg { hint, .. }
            | Self::Output { hint, .. }
            | Self::MissingTool { hint, .. }
            | Self::Timeout { hint, .. }
            | Self::Verification { hint, .. } => hint.clone(),
            _ => None,
        }
    }

    pub fn to_payload(&self) -> ErrorPayload {
        ErrorPayload {
            kind: self.kind(),
            message: self.to_string(),
            hint: self.hint(),
        }
    }

    pub fn to_json_response(&self) -> serde_json::Value {
        serde_json::json!({
            "status": "failed",
            "error": self.to_payload(),
        })
    }

    pub fn input_error(msg: impl Into<String>) -> Self {
        Self::Input {
            message: msg.into(),
            hint: None,
        }
    }

    pub fn output_error(msg: impl Into<String>) -> Self {
        Self::Output {
            message: msg.into(),
            hint: None,
        }
    }

    pub fn verification_error(msg: impl Into<String>) -> Self {
        Self::Verification {
            message: msg.into(),
            hint: None,
        }
    }

    pub fn ffmpeg_error(
        msg: impl Into<String>,
        exit_code: Option<i32>,
        _args: Option<Vec<String>>,
    ) -> Self {
        Self::Ffmpeg {
            exit_code: exit_code.unwrap_or(ExitCodes::FAILURE),
            message: msg.into(),
            hint: None,
        }
    }
}
