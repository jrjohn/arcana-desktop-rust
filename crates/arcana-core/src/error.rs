//! Application errors (C# `Arcana.Core.Common.AppError` / `ErrorCode`).

use std::fmt;

/// Stable numeric error codes, grouped by category (C# `ErrorCode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    // Network errors (1000-1999)
    NetworkUnavailable,
    ConnectionTimeout,
    ServerUnreachable,
    // Validation errors (2000-2999)
    ValidationFailed,
    InvalidEmail,
    RequiredFieldMissing,
    InvalidFormat,
    ValueOutOfRange,
    // Server errors (3000-3999)
    ServerError,
    NotFound,
    Unauthorized,
    Forbidden,
    BadRequest,
    // Authentication errors (4000-4999)
    AuthenticationFailed,
    TokenExpired,
    InvalidCredentials,
    AccountLocked,
    SessionExpired,
    // Data errors (5000-5999)
    ConflictError,
    DataCorruption,
    DuplicateEntry,
    ReferentialIntegrity,
    // Database errors (6000-6999)
    DatabaseError,
    MigrationFailed,
    ConnectionFailed,
    QueryFailed,
    // Plugin errors (7000-7999)
    PluginLoadFailed,
    PluginNotFound,
    PluginIncompatible,
    PluginDisabled,
    // File errors (8000-8999)
    FileNotFound,
    FileAccessDenied,
    FileCorrupted,
    // Unknown (9000-9999)
    Unknown,
}

impl ErrorCode {
    /// Every code, in declaration order.
    pub const ALL: [Self; 34] = [
        Self::NetworkUnavailable,
        Self::ConnectionTimeout,
        Self::ServerUnreachable,
        Self::ValidationFailed,
        Self::InvalidEmail,
        Self::RequiredFieldMissing,
        Self::InvalidFormat,
        Self::ValueOutOfRange,
        Self::ServerError,
        Self::NotFound,
        Self::Unauthorized,
        Self::Forbidden,
        Self::BadRequest,
        Self::AuthenticationFailed,
        Self::TokenExpired,
        Self::InvalidCredentials,
        Self::AccountLocked,
        Self::SessionExpired,
        Self::ConflictError,
        Self::DataCorruption,
        Self::DuplicateEntry,
        Self::ReferentialIntegrity,
        Self::DatabaseError,
        Self::MigrationFailed,
        Self::ConnectionFailed,
        Self::QueryFailed,
        Self::PluginLoadFailed,
        Self::PluginNotFound,
        Self::PluginIncompatible,
        Self::PluginDisabled,
        Self::FileNotFound,
        Self::FileAccessDenied,
        Self::FileCorrupted,
        Self::Unknown,
    ];

    /// The numeric value, identical to the C# enum.
    #[must_use]
    pub fn value(self) -> i32 {
        match self {
            Self::NetworkUnavailable => 1000,
            Self::ConnectionTimeout => 1001,
            Self::ServerUnreachable => 1002,
            Self::ValidationFailed => 2000,
            Self::InvalidEmail => 2001,
            Self::RequiredFieldMissing => 2002,
            Self::InvalidFormat => 2003,
            Self::ValueOutOfRange => 2004,
            Self::ServerError => 3000,
            Self::NotFound => 3001,
            Self::Unauthorized => 3002,
            Self::Forbidden => 3003,
            Self::BadRequest => 3004,
            Self::AuthenticationFailed => 4000,
            Self::TokenExpired => 4001,
            Self::InvalidCredentials => 4002,
            Self::AccountLocked => 4003,
            Self::SessionExpired => 4004,
            Self::ConflictError => 5000,
            Self::DataCorruption => 5001,
            Self::DuplicateEntry => 5002,
            Self::ReferentialIntegrity => 5003,
            Self::DatabaseError => 6000,
            Self::MigrationFailed => 6001,
            Self::ConnectionFailed => 6002,
            Self::QueryFailed => 6003,
            Self::PluginLoadFailed => 7000,
            Self::PluginNotFound => 7001,
            Self::PluginIncompatible => 7002,
            Self::PluginDisabled => 7003,
            Self::FileNotFound => 8000,
            Self::FileAccessDenied => 8001,
            Self::FileCorrupted => 8002,
            Self::Unknown => 9000,
        }
    }

    /// Reverse of [`ErrorCode::value`].
    #[must_use]
    pub fn from_value(value: i32) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.value() == value)
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}({})", self.value())
    }
}

/// Application error (C# `abstract record AppError` and its seven sealed cases).
///
/// C# carries an optional `Exception InnerException`; here `inner` keeps the inner error's
/// message, which keeps the type `Clone + Eq` (exceptions are not comparable values).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AppError {
    #[error("{message}")]
    Network {
        code: ErrorCode,
        message: String,
        inner: Option<String>,
    },
    #[error("{message}")]
    Validation {
        code: ErrorCode,
        message: String,
        errors: Vec<String>,
    },
    #[error("{message}")]
    Server {
        code: ErrorCode,
        message: String,
        status_code: Option<u16>,
    },
    #[error("{message}")]
    Data {
        code: ErrorCode,
        message: String,
        inner: Option<String>,
    },
    #[error("{message}")]
    Auth { code: ErrorCode, message: String },
    #[error("{message}")]
    Plugin {
        code: ErrorCode,
        message: String,
        plugin_id: Option<String>,
    },
    #[error("{message}")]
    Unknown {
        message: String,
        inner: Option<String>,
    },
}

impl AppError {
    /// `AppError.Data(code, message)` without an inner error.
    #[must_use]
    pub fn data(code: ErrorCode, message: impl Into<String>) -> Self {
        Self::Data {
            code,
            message: message.into(),
            inner: None,
        }
    }

    /// `AppError.Validation(ValidationFailed, message, errors)`.
    #[must_use]
    pub fn validation(message: impl Into<String>, errors: Vec<String>) -> Self {
        Self::Validation {
            code: ErrorCode::ValidationFailed,
            message: message.into(),
            errors,
        }
    }

    /// The error code (C# `AppError.Code`). `Unknown` always reports [`ErrorCode::Unknown`].
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Network { code, .. }
            | Self::Validation { code, .. }
            | Self::Server { code, .. }
            | Self::Data { code, .. }
            | Self::Auth { code, .. }
            | Self::Plugin { code, .. } => *code,
            Self::Unknown { .. } => ErrorCode::Unknown,
        }
    }

    /// The developer-facing message (C# `AppError.Message`).
    #[must_use]
    pub fn message(&self) -> &str {
        match self {
            Self::Network { message, .. }
            | Self::Validation { message, .. }
            | Self::Server { message, .. }
            | Self::Data { message, .. }
            | Self::Auth { message, .. }
            | Self::Plugin { message, .. }
            | Self::Unknown { message, .. } => message,
        }
    }

    /// Message shown to the user (C# `ToUserMessage`): validation errors joined by newlines when
    /// there are any, otherwise the message.
    #[must_use]
    pub fn to_user_message(&self) -> String {
        match self {
            Self::Validation { errors, .. } if !errors.is_empty() => errors.join("\n"),
            other => other.message().to_owned(),
        }
    }
}

/// Result alias used across the workspace.
///
/// C# `Result<T>` / `Result` (with `Match`, `Map`, `Bind`, `GetValueOrDefault`, `OnSuccess`,
/// `OnFailure`) is exactly `std::result::Result<T, AppError>` with `match`, `map`, `and_then`,
/// `unwrap_or`, `inspect`, `inspect_err`; non-generic `Result` is `AppResult<()>`.
pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_have_csharp_values_and_round_trip() {
        assert_eq!(ErrorCode::NetworkUnavailable.value(), 1000);
        assert_eq!(ErrorCode::ValueOutOfRange.value(), 2004);
        assert_eq!(ErrorCode::BadRequest.value(), 3004);
        assert_eq!(ErrorCode::SessionExpired.value(), 4004);
        assert_eq!(ErrorCode::ReferentialIntegrity.value(), 5003);
        assert_eq!(ErrorCode::QueryFailed.value(), 6003);
        assert_eq!(ErrorCode::PluginDisabled.value(), 7003);
        assert_eq!(ErrorCode::FileCorrupted.value(), 8002);
        assert_eq!(ErrorCode::Unknown.value(), 9000);
        for c in ErrorCode::ALL {
            assert_eq!(ErrorCode::from_value(c.value()), Some(c));
        }
        assert_eq!(ErrorCode::from_value(42), None);
        assert_eq!(ErrorCode::NotFound.to_string(), "NotFound(3001)");
    }

    #[test]
    fn every_case_exposes_code_and_message() {
        let cases = [
            AppError::Network {
                code: ErrorCode::ConnectionTimeout,
                message: "n".into(),
                inner: Some("io".into()),
            },
            AppError::validation("v", vec![]),
            AppError::Server {
                code: ErrorCode::ServerError,
                message: "s".into(),
                status_code: Some(500),
            },
            AppError::data(ErrorCode::DuplicateEntry, "d"),
            AppError::Auth {
                code: ErrorCode::TokenExpired,
                message: "a".into(),
            },
            AppError::Plugin {
                code: ErrorCode::PluginNotFound,
                message: "p".into(),
                plugin_id: Some("x".into()),
            },
            AppError::Unknown {
                message: "u".into(),
                inner: None,
            },
        ];
        let codes: Vec<_> = cases.iter().map(AppError::code).collect();
        assert_eq!(
            codes,
            [
                ErrorCode::ConnectionTimeout,
                ErrorCode::ValidationFailed,
                ErrorCode::ServerError,
                ErrorCode::DuplicateEntry,
                ErrorCode::TokenExpired,
                ErrorCode::PluginNotFound,
                ErrorCode::Unknown
            ]
        );
        let msgs: Vec<_> = cases.iter().map(ToString::to_string).collect();
        assert_eq!(msgs, ["n", "v", "s", "d", "a", "p", "u"]);
        assert_eq!(cases[3].to_user_message(), "d");
    }

    #[test]
    fn validation_user_message_joins_errors_or_falls_back() {
        let e = AppError::validation("bad", vec!["a".into(), "b".into()]);
        assert_eq!(e.to_user_message(), "a\nb");
        assert_eq!(e.message(), "bad");
        assert_eq!(AppError::validation("bad", vec![]).to_user_message(), "bad");
    }
}
