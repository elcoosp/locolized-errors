//! The error kinds understood by [`locolized_errors`].
//!
//! loco-rs 1.x removed its own `ErrorKind`; this module is the successor:
//! a small enum that maps one-to-one onto loco's `Error` variants while
//! additionally supporting arbitrary custom HTTP statuses.

use std::borrow::Cow;
use std::fmt;

use axum::http::StatusCode;
use serde::Serialize;

/// The HTTP semantics of a localized error.
///
/// The named kinds map onto the matching loco `Error` variants — the four
/// kinds loco names directly, plus the extended set introduced in 0.3.0
/// (which maps onto `Error::CustomError` with its HTTP status) — and
/// [`ErrorKind::Custom`] carries an arbitrary status code plus a description
/// passed to Loco's `ErrorDetail`.
///
/// The description is a `Cow<'static, str>` so macro-generated kinds hold a
/// borrowed literal (zero allocation per [`ErrorKind`] value) while runtime
/// construction can still own its description.
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
    /// Maps to `loco_rs::Error::CustomError` with HTTP 403.
    Forbidden,
    /// Maps to `loco_rs::Error::CustomError` with HTTP 409.
    Conflict,
    /// Maps to `loco_rs::Error::CustomError` with HTTP 422.
    UnprocessableEntity,
    /// Maps to `loco_rs::Error::CustomError` with HTTP 429.
    TooManyRequests,
    /// Maps to `loco_rs::Error::CustomError` with HTTP 503.
    ServiceUnavailable,
    /// Any other HTTP status: maps to `loco_rs::Error::CustomError` with the
    /// given status and description.
    Custom(StatusCode, Cow<'static, str>),
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
            Self::Forbidden => "forbidden",
            Self::Conflict => "conflict",
            Self::UnprocessableEntity => "unprocessable-entity",
            Self::TooManyRequests => "too-many-requests",
            Self::ServiceUnavailable => "service-unavailable",
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
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::Conflict => StatusCode::CONFLICT,
            Self::UnprocessableEntity => StatusCode::UNPROCESSABLE_ENTITY,
            Self::TooManyRequests => StatusCode::TOO_MANY_REQUESTS,
            Self::ServiceUnavailable => StatusCode::SERVICE_UNAVAILABLE,
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
    use crate::__response_from_json;
    use crate::__status;
    use axum::http::StatusCode;
    use std::borrow::Cow;

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
            ErrorKind::Custom(__status(418), Cow::Borrowed("")).as_kebab(),
            "custom"
        );

        assert_eq!(ErrorKind::BadRequest.status_code(), 400);
        assert_eq!(ErrorKind::Unauthorized.status_code(), 401);
        assert_eq!(ErrorKind::NotFound.status_code(), 404);
        assert_eq!(ErrorKind::InternalServerError.status_code(), 500);
        assert_eq!(
            ErrorKind::Custom(__status(418), Cow::Borrowed("")).status_code(),
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
    fn named_kinds_cover_the_extended_set() {
        assert_eq!(ErrorKind::Forbidden.as_kebab(), "forbidden");
        assert_eq!(ErrorKind::Forbidden.status_code(), 403);
        assert_eq!(ErrorKind::Conflict.as_kebab(), "conflict");
        assert_eq!(ErrorKind::Conflict.status_code(), 409);
        assert_eq!(
            ErrorKind::UnprocessableEntity.as_kebab(),
            "unprocessable-entity"
        );
        assert_eq!(ErrorKind::UnprocessableEntity.status_code(), 422);
        assert_eq!(ErrorKind::TooManyRequests.as_kebab(), "too-many-requests");
        assert_eq!(ErrorKind::TooManyRequests.status_code(), 429);
        assert_eq!(
            ErrorKind::ServiceUnavailable.as_kebab(),
            "service-unavailable"
        );
        assert_eq!(ErrorKind::ServiceUnavailable.status_code(), 503);
    }

    #[test]
    fn serializes_as_kebab_string() {
        let serialized = serde_json::to_string(&ErrorKind::Unauthorized).unwrap();
        assert_eq!(serialized, "\"unauthorized\"");
        let serialized = serde_json::to_string(&ErrorKind::TooManyRequests).unwrap();
        assert_eq!(serialized, "\"too-many-requests\"");
    }

    #[test]
    fn converts_into_status_code() {
        assert_eq!(StatusCode::from(ErrorKind::NotFound), StatusCode::NOT_FOUND);
        assert_eq!(
            StatusCode::from(ErrorKind::ServiceUnavailable),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }

    #[test]
    fn response_from_json_sets_status_header_and_body() {
        let response = __response_from_json(__status(418), b"{\"ok\":true}".to_vec());
        assert_eq!(response.status(), 418);
        assert_eq!(
            response
                .headers()
                .get(axum::http::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("application/json")
        );
        let body = collect(response);
        assert_eq!(body, b"{\"ok\":true}".to_vec());
    }

    /// Collects the response body through a minimal single-thread runtime.
    fn collect(response: crate::loco_rs::prelude::Response) -> Vec<u8> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        runtime.block_on(async {
            http_body_util::BodyExt::collect(response.into_body())
                .await
                .unwrap()
                .to_bytes()
                .to_vec()
        })
    }
}
