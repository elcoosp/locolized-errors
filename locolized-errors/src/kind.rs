//! The error kinds understood by [`locolized_errors`].
//!
//! loco-rs 1.x removed its own `ErrorKind`; this module is the successor:
//! a small enum that maps one-to-one onto loco's `Error` variants while
//! additionally supporting arbitrary custom HTTP statuses.

use std::fmt;

use axum::http::StatusCode;
use serde::Serialize;

/// The HTTP semantics of a localized error.
///
/// The four named kinds map onto the matching loco `Error` variants; [`ErrorKind::Custom`]
/// carries an arbitrary status code plus a description passed to Loco's
/// `ErrorDetail`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// Maps to `loco_rs::Error::BadRequest` (HTTP 400).
    BadRequest,
    /// Maps to `loco_rs::Error::Unauthorized` (HTTP 401).
    Unauthorized,
    /// Maps to `loco_rs::Error::NotFound` (HTTP 404).
    NotFound,
    /// Maps to `loco_rs::Error::InternalServerError` (HTTP 500).
    InternalServerError,
    /// Any other HTTP status: maps to `loco_rs::Error::CustomError` with the
    /// given status and description.
    Custom(StatusCode, String),
}

impl ErrorKind {
    /// The kebab-case name of the kind, as used in JSON payloads and as the
    /// `errors.<kind>` i18n key prefix.
    ///
    /// Custom kinds are named `"custom"`.
    #[must_use]
    pub const fn as_kebab(&self) -> &'static str {
        match self {
            Self::BadRequest => "bad-request",
            Self::Unauthorized => "unauthorized",
            Self::NotFound => "not-found",
            Self::InternalServerError => "internal-server-error",
            Self::Custom(..) => "custom",
        }
    }

    /// The HTTP status code this kind produces.
    #[must_use]
    pub const fn status_code(&self) -> StatusCode {
        match self {
            Self::BadRequest => StatusCode::BAD_REQUEST,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::InternalServerError => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Custom(status, _) => *status,
        }
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_kebab())
    }
}

impl Serialize for ErrorKind {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_kebab())
    }
}

impl From<ErrorKind> for StatusCode {
    fn from(kind: ErrorKind) -> Self {
        kind.status_code()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::ErrorKind;
    use crate::__status;
    use axum::http::StatusCode;

    #[test]
    fn kebab_names_and_statuses_match() {
        assert_eq!(ErrorKind::BadRequest.as_kebab(), "bad-request");
        assert_eq!(ErrorKind::Unauthorized.as_kebab(), "unauthorized");
        assert_eq!(ErrorKind::NotFound.as_kebab(), "not-found");
        assert_eq!(
            ErrorKind::InternalServerError.as_kebab(),
            "internal-server-error"
        );
        assert_eq!(
            ErrorKind::Custom(__status(418), String::new()).as_kebab(),
            "custom"
        );

        assert_eq!(ErrorKind::BadRequest.status_code(), 400);
        assert_eq!(ErrorKind::Unauthorized.status_code(), 401);
        assert_eq!(ErrorKind::NotFound.status_code(), 404);
        assert_eq!(ErrorKind::InternalServerError.status_code(), 500);
        assert_eq!(
            ErrorKind::Custom(__status(418), String::new()).status_code(),
            418
        );
    }

    #[test]
    fn display_matches_kebab() {
        assert_eq!(ErrorKind::BadRequest.to_string(), "bad-request");
        assert_eq!(
            ErrorKind::Custom(__status(418), "d".into()).to_string(),
            "custom"
        );
    }

    #[test]
    fn serializes_as_kebab_string() {
        let serialized = serde_json::to_string(&ErrorKind::Unauthorized).unwrap();
        assert_eq!(serialized, "\"unauthorized\"");
    }

    #[test]
    fn converts_into_status_code() {
        assert_eq!(StatusCode::from(ErrorKind::NotFound), StatusCode::NOT_FOUND);
    }
}
