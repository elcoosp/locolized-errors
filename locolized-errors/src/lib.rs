//! Localized API errors for the [Loco](https://loco.rs) web framework.
//!
//! `locolized_errors` eliminates the boilerplate of internationalized API
//! errors: derive [`LocalizedApiError`] on your error enums and you get
//! localized, parameterized messages, automatic error-kind detection,
//! conversion into Loco's native [`Error`], and structured JSON payloads for
//! machine clients.
//!
//! # Quick start
//!
//! 1. Derive the macro and initialize `rust_i18n` (via the re-export — no
//!    extra dependency needed):
//!
//! ```rust
//! use locolized_errors::LocalizedApiError;
//! use locolized_errors::rust_i18n;
//! use serde::Serialize;
//!
//! rust_i18n::i18n!("locales"); // loads ./locales/*.yml at compile time
//!
//! #[derive(Debug, Serialize, LocalizedApiError)]
//! #[localized(kind = "bad-request")]
//! pub enum UserError {
//!     #[localized(key = "user-not-found")]
//!     NotFound,
//!     TooManyAttempts { attempts: u32 },
//! }
//!
//! # fn main() {
//! // The message comes from `locales/en.yml`:
//! assert_eq!(
//!     UserError::TooManyAttempts { attempts: 3 }.to_string(),
//!     "Too many attempts: 3",
//! );
//!
//! // Convert into Loco's native error type with `.into()` or `.to_loco_error()`:
//! let loco_error: locolized_errors::loco_rs::errors::Error = UserError::NotFound.into();
//! # }
//! ```
//!
//! With `locales/en.yml`:
//!
//! ```yaml
//! en:
//!   errors:
//!     bad-request:
//!       user-not-found: "User not found"
//!       too-many-attempts: "Too many attempts: %{attempts}"
//! ```
//!
//! 2. Return errors from Loco controllers directly:
//!
//! ```ignore
//! async fn create_user(State(ctx): State<AppContext>) -> Result<Response> {
//!     let email = params.email.trim().to_string();
//!     if email.is_empty() {
//!         return Err(UserError::NotFound.into()); // converts automatically
//!     }
//!     // ...
//!     Ok(format::json(()))
//! }
//! ```
//!
//! # Error kinds
//!
//! Without attributes the kind is derived from the enum name:
//! `BadRequestError` -> [`ErrorKind::BadRequest`], `NotFoundError` ->
//! [`ErrorKind::NotFound`], and so on. Any name works with an explicit
//! `#[localized(kind = "...")]` or a custom `#[localized(status = 418)]`.
//!
//! # Structured payloads
//!
//! Besides localized messages, every error renders as machine-readable JSON
//! via [`LocalizedApiError::to_json`] / [`to_json_string`](LocalizedApiError::to_json_string),
//! and the generated [`IntoResponse`](loco_rs::prelude::IntoResponse) impl
//! serves exactly that payload with the matching HTTP status:
//!
//! ```rust
//! use locolized_errors::LocalizedApiError;
//! use locolized_errors::rust_i18n;
//! use serde::Serialize;
//!
//! rust_i18n::i18n!("locales");
//!
//! #[derive(Debug, Serialize, LocalizedApiError)]
//! #[localized(status = 418, description = "short and stout")]
//! pub enum TeapotError {
//!     Brewed,
//! }
//!
//! use locolized_errors::loco_rs::prelude::IntoResponse as _;
//!
//! # fn main() {
//! let payload = TeapotError::Brewed.to_json();
//! assert_eq!(payload["kind"], "custom");
//! assert_eq!(payload["variant"], "Brewed");
//! assert_eq!(payload["params"], serde_json::json!({}));
//!
//! let response = TeapotError::Brewed.into_response();
//! assert_eq!(response.status(), 418);
//! # }
//! ```
//!
//! # Requirements
//!
//! Your crate must call `rust_i18n::i18n!("locales")` (or the re-exported
//! `locolized_errors::rust_i18n::i18n!`) **before** any module using the
//! derive macro, because translation lookups expand inside your crate. Add
//! `#[derive(serde::Serialize)]` next to the derive: the trait requires
//! `Serialize` to build the `code`/`message` envelope and the structured
//! payload.

use std::fmt::Display;

use loco_rs::controller::ErrorDetail;
use loco_rs::errors::Error;
use serde::Serialize;
use serde_json::Value;

mod kind;

pub use kind::ErrorKind;

// Re-exported for the derive macro's generated code paths: downstream crates
// need neither `rust-i18n` nor `loco-rs` paths of their own to compile.
pub use loco_rs;
pub use locolized_errors_macros::LocalizedApiError;
pub use rust_i18n;
pub use serde_json;

/// Build a [`StatusCode`] from a literal already validated to `100..=599` by
/// the derive macro.
#[doc(hidden)]
#[must_use]
pub fn __status(code: u16) -> axum::http::StatusCode {
    axum::http::StatusCode::from_u16(code).unwrap_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
}

/// Anything that renders as a localized API error.
///
/// Implemented for you by the [`LocalizedApiError`] derive macro; the first
/// four methods carry the generated per-variant data, everything else has
/// default implementations on top of them.
///
/// The trait requires `Serialize` so errors can serve both as the `code` of
/// the Loco `BadRequest` envelope and as structured JSON payloads — derive
/// `serde::Serialize` alongside.
pub trait LocalizedApiError: Display + Serialize + Send + Sync + 'static {
    /// The HTTP semantics of this error.
    fn error_kind(&self) -> ErrorKind;

    /// The variant name, e.g. `"TooManyAttempts"`.
    fn error_variant(&self) -> &'static str;

    /// The full translation key, e.g. `"errors.bad-request.too-many-attempts"`.
    fn error_key(&self) -> &'static str;

    /// The named fields of the variant, JSON-ready. Empty for unit and tuple
    /// variants (tuple variants cannot drive rust-i18n's named interpolation).
    fn error_params(&self) -> Vec<(&'static str, Value)>;

    /// The HTTP status code this error produces.
    #[must_use]
    fn status_code(&self) -> axum::http::StatusCode {
        self.error_kind().status_code()
    }

    /// A structured JSON payload:
    /// `{"kind", "variant", "key", "message", "params"}`.
    #[must_use]
    fn to_json(&self) -> Value {
        payload_json(
            &self.error_kind(),
            self.error_variant(),
            self.error_key(),
            self.to_string(),
            self.error_params(),
        )
    }

    /// [`to_json`](Self::to_json), serialized.
    #[must_use]
    fn to_json_string(&self) -> String {
        self.to_json().to_string()
    }

    /// Convert into Loco's native error type.
    ///
    /// The four named kinds map onto their matching `loco_rs::Error` variants;
    /// `BadRequest` additionally embeds `{"code": <self>, "message": ...}` as
    /// loco expects; `Custom` maps to `Error::CustomError` with the status and
    /// description.
    #[must_use]
    fn to_loco_error(&self) -> Error {
        let kind = self.error_kind();
        let message = self.to_string();
        match kind {
            ErrorKind::BadRequest => Error::BadRequest(bad_request_envelope(self, &message)),
            other => loco_error_from_kind(&other, message),
        }
    }
}

/// Assemble the structured JSON payload. Free function so the logic exists
/// exactly once, independent of the implementing error type.
fn payload_json(
    kind: &ErrorKind,
    variant: &str,
    key: &str,
    message: String,
    params: Vec<(&'static str, Value)>,
) -> Value {
    let mut object = serde_json::Map::with_capacity(5);
    object.insert("kind".to_owned(), Value::from(kind.as_kebab()));
    object.insert("variant".to_owned(), Value::from(variant));
    object.insert("key".to_owned(), Value::from(key));
    object.insert("message".to_owned(), Value::from(message));
    let params = params
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value))
        .collect::<serde_json::Map<String, Value>>();
    object.insert("params".to_owned(), Value::Object(params));
    Value::Object(object)
}

/// Map a non-BadRequest [`ErrorKind`] onto its loco error variant. Also a
/// total mapping over every kind (a plain `BadRequest` carries the message as
/// its body); `to_loco_error` routes `BadRequest` through the richer envelope
/// before calling this.
fn loco_error_from_kind(kind: &ErrorKind, message: String) -> Error {
    match kind {
        ErrorKind::BadRequest => Error::BadRequest(message),
        ErrorKind::Unauthorized => Error::Unauthorized(message),
        ErrorKind::NotFound => Error::NotFound,
        ErrorKind::InternalServerError => Error::InternalServerError,
        ErrorKind::Custom(status, description) => {
            Error::CustomError(*status, ErrorDetail::new(message, description.clone()))
        }
    }
}

/// Single-pass `{"code": <self>, "message": ...}` envelope for Loco's
/// `BadRequest`. Serializing `self` and splicing beats building a
/// `serde_json::Value` tree and walking it again.
fn bad_request_envelope(code: &(impl Serialize + ?Sized), message: &str) -> String {
    // `unwrap_or` evaluates the fallback eagerly, which also keeps every
    // line covered; `to_string` on `&str` cannot fail in practice.
    let code = serde_json::to_string(code).unwrap_or("null".to_owned());
    let message = serde_json::to_string(message).unwrap_or("\"\"".to_owned());
    format!("{{\"code\":{code},\"message\":{message}}}")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic, clippy::unwrap_used)]
    use super::{__status, ErrorKind, loco_error_from_kind};
    use crate::loco_rs::errors::Error;

    /// Compile-time proof that implementors may cross threads and task
    /// boundaries (the `Send + Sync + 'static` supertraits hold).
    #[test]
    fn error_kind_is_send_sync_static() {
        fn assert_send_sync_static<T: Send + Sync + 'static>() {}
        assert_send_sync_static::<ErrorKind>();
    }

    #[test]
    fn status_helper_builds_valid_codes() {
        assert_eq!(__status(418), 418);
    }

    /// `loco_error_from_kind` is a total mapping; the direct `BadRequest`
    /// route is exercised here, `to_loco_error` normally prefers the
    /// `code`/`message` envelope for that kind.
    #[test]
    fn loco_error_from_kind_is_total() {
        let error = loco_error_from_kind(&ErrorKind::BadRequest, "body".to_owned());
        assert_eq!(as_bad_request(&error).map(String::as_str), Some("body"));
        assert!(as_custom_error(&error).is_none());

        let error = loco_error_from_kind(
            &ErrorKind::Custom(__status(402), "fee".to_owned()),
            "msg".to_owned(),
        );
        assert!(as_bad_request(&error).is_none());
        assert_eq!(
            as_custom_error(&error).map(|(status, _)| status),
            Some(__status(402))
        );
        assert_eq!(
            as_custom_error(&error).and_then(|(_, detail)| detail.error.as_deref()),
            Some("msg")
        );
    }

    fn as_bad_request(error: &Error) -> Option<&String> {
        match error {
            Error::BadRequest(body) => Some(body),
            _ => None,
        }
    }

    fn as_custom_error(
        error: &Error,
    ) -> Option<(axum::http::StatusCode, &loco_rs::controller::ErrorDetail)> {
        match error {
            Error::CustomError(status, detail) => Some((*status, detail)),
            _ => None,
        }
    }
}
