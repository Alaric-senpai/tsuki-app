//! Application error type shared by commands and services.
//!
//! Errors cross the IPC boundary serialized as `{ code, message, detail }`.
//! `message` is safe to show to the user; `detail` is technical and copyable.

use serde::ser::{Serialize, SerializeStruct, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{message}")]
    Config { message: String, detail: String },
    #[error("{message}")]
    Io { message: String, detail: String },
    /// Reserved for the SQLite layer; constructed once `db/` lands.
    #[allow(dead_code)]
    #[error("{message}")]
    Db { message: String, detail: String },
    /// A remote catalog source could not be read or resolved.
    #[error("{message}")]
    Source { message: String, detail: String },
    /// yt-dlp or the download pipeline failed.
    #[error("{message}")]
    Download { message: String, detail: String },
    #[error("{message}")]
    Internal { message: String, detail: String },
}

impl AppError {
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config {
            message: message.into(),
            detail: String::new(),
        }
    }

    pub fn config_with(message: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::Config {
            message: message.into(),
            detail: detail.into(),
        }
    }

    /// Reserved for call sites that have no technical detail to attach.
    #[allow(dead_code)]
    pub fn io(message: impl Into<String>) -> Self {
        Self::Io {
            message: message.into(),
            detail: String::new(),
        }
    }

    pub fn io_with(message: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::Io {
            message: message.into(),
            detail: detail.into(),
        }
    }

    /// Reserved for the SQLite layer; constructed once `db/` lands.
    #[allow(dead_code)]
    pub fn db(message: impl Into<String>) -> Self {
        Self::Db {
            message: message.into(),
            detail: String::new(),
        }
    }

    /// Reserved for the SQLite layer; constructed once `db/` lands.
    #[allow(dead_code)]
    pub fn db_with(message: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::Db {
            message: message.into(),
            detail: detail.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
            detail: String::new(),
        }
    }

    pub fn internal_with(message: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
            detail: detail.into(),
        }
    }

    pub fn source_with(message: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::Source {
            message: message.into(),
            detail: detail.into(),
        }
    }

    pub fn download(message: impl Into<String>) -> Self {
        Self::Download {
            message: message.into(),
            detail: String::new(),
        }
    }

    pub fn download_with(message: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::Download {
            message: message.into(),
            detail: detail.into(),
        }
    }

    /// Stable machine-readable category for the frontend.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Config { .. } => "config",
            Self::Io { .. } => "io",
            Self::Db { .. } => "db",
            Self::Source { .. } => "source",
            Self::Download { .. } => "download",
            Self::Internal { .. } => "internal",
        }
    }

    pub fn message(&self) -> &str {
        match self {
            Self::Config { message, .. }
            | Self::Io { message, .. }
            | Self::Db { message, .. }
            | Self::Source { message, .. }
            | Self::Download { message, .. }
            | Self::Internal { message, .. } => message,
        }
    }

    pub fn detail(&self) -> &str {
        match self {
            Self::Config { detail, .. }
            | Self::Io { detail, .. }
            | Self::Db { detail, .. }
            | Self::Source { detail, .. }
            | Self::Download { detail, .. }
            | Self::Internal { detail, .. } => detail,
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("AppError", 3)?;
        state.serialize_field("code", self.code())?;
        state.serialize_field("message", self.message())?;
        state.serialize_field("detail", self.detail())?;
        state.end()
    }
}
