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
//! [`ErrorKind::NotFound`], `ForbiddenError` -> [`ErrorKind::Forbidden`], and
//! so on. Any name works with an explicit `#[localized(kind = "...")]` or a
//! custom `#[localized(status = 418)]`.
//!
//! # Structured payloads
//!
//! Besides localized messages, every error renders as machine-readable JSON
//! via [`LocalizedApiError::to_json`], [`to_json_string`](LocalizedApiError::to_json_string)
//! or [`to_json_bytes`](LocalizedApiError::to_json_bytes), and the generated
//! [`IntoResponse`](loco_rs::prelude::IntoResponse) impl serves exactly that
//! payload with the matching HTTP status and `content-type: application/json`:
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

use std::borrow::Cow;
use std::fmt::Display;

use loco_rs::controller::ErrorDetail;
use loco_rs::errors::Error;
use serde::Serialize;
use serde_json::Value;

mod kind;

pub use kind::ErrorKind;

// Re-exported for the derive macro's generated code paths: downstream crates
// need neither `rust-i18n`, `loco-rs` nor `axum` paths of their own to compile.
pub use axum;
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

/// Build an axum/loco response with `content-type: application/json` from an
/// already-serialized body. The derive's generated `IntoResponse` goes
/// through here so the payload bytes are produced in a single pass and never
/// re-copied by the body conversion.
#[doc(hidden)]
#[must_use]
pub fn __response_from_json(
    status: axum::http::StatusCode,
    body: Vec<u8>,
) -> loco_rs::prelude::Response {
    use loco_rs::prelude::IntoResponse as _;
    (
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        (status, axum::body::Body::from(body)),
    )
        .into_response()
}

/// Anything that renders as a localized API error.
///
/// Implemented for you by the [`LocalizedApiError`] derive macro; the first
/// five methods carry the generated per-variant data, everything else has
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

    /// The localized, parameterized message — the same text [`Display`]
    /// renders, without forcing a `String` allocation: unit variants and
    /// static `message = "..."` overrides return `Cow::Borrowed`, variants
    /// with interpolated fields return `Cow::Owned`.
    ///
    /// The derive overrides this; the default routes through [`Display`]
    /// (which allocates) so hand-written implementors keep working.
    fn error_message(&self) -> Cow<'static, str> {
        Cow::Owned(self.to_string())
    }

    /// The named fields of the variant, JSON-ready. Empty for unit and tuple
    /// variants (tuple variants cannot drive rust-i18n's named interpolation).
    ///
    /// Prefer [`to_json_bytes`](Self::to_json_bytes) /
    /// [`to_json_string`](Self::to_json_string) for payloads: the derive
    /// serializes those straight from the fields without materializing this
    /// `Vec` or per-field `Value`s.
    fn error_params(&self) -> Vec<(&'static str, Value)>;

    /// The HTTP status code this error produces.
    #[must_use]
    fn status_code(&self) -> axum::http::StatusCode {
        self.error_kind().status_code()
    }

    /// A structured JSON payload:
    /// `{"kind", "variant", "key", "message", "params"}`.
    ///
    /// The derive overrides this with a single-pass serialization; if a field
    /// fails to serialize, the payload degrades to the tree-based fallback
    /// where that field becomes `null`.
    #[must_use]
    fn to_json(&self) -> Value {
        serde_json::to_value(payload::Ref(self)).unwrap_or_else(|_| {
            __payload_tree(
                &self.error_kind(),
                self.error_variant(),
                self.error_key(),
                &self.error_message(),
                self.error_params(),
            )
        })
    }

    /// [`to_json`](Self::to_json), serialized.
    ///
    /// The derive overrides this: one pass from the variant fields into the
    /// output `String`, no intermediate `Value` tree.
    #[must_use]
    fn to_json_string(&self) -> String {
        String::from_utf8(self.to_json_bytes()).unwrap_or_default()
    }

    /// [`to_json`](Self::to_json) as UTF-8 bytes — the exact body the
    /// generated [`IntoResponse`](loco_rs::prelude::IntoResponse) serves.
    ///
    /// The derive overrides this: one pass from the variant fields into the
    /// output buffer, no intermediate `Value` tree.
    #[must_use]
    fn to_json_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(&payload::Ref(self)).unwrap_or_else(|_| {
            __payload_tree(
                &self.error_kind(),
                self.error_variant(),
                self.error_key(),
                &self.error_message(),
                self.error_params(),
            )
            .to_string()
            .into_bytes()
        })
    }

    /// Serialize the named fields of the variant as a JSON object, one entry
    /// per field. The derive overrides this to stream fields straight from
    /// the variant without materializing [`error_params`](Self::error_params).
    #[doc(hidden)]
    fn __serialize_params<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap as _;
        let params = self.error_params();
        let mut map = serializer.serialize_map(Some(params.len()))?;
        for (key, value) in params {
            map.serialize_entry(key, &value)?;
        }
        map.end()
    }

    /// Convert into Loco's native error type.
    ///
    /// The named kinds map onto their matching `loco_rs::Error` variants;
    /// `BadRequest` additionally embeds `{"code": <self>, "message": ...}` as
    /// loco expects; the remaining kinds map to `Error::CustomError` with the
    /// status and description.
    #[must_use]
    fn to_loco_error(&self) -> Error {
        let kind = self.error_kind();
        let message = self.error_message();
        match kind {
            ErrorKind::BadRequest => Error::BadRequest(bad_request_envelope(self, &message)),
            other => loco_error_from_kind(&other, message),
        }
    }
}

/// Assemble the structured JSON payload as a `Value` tree. Free function so
/// the fallback logic exists exactly once, independent of the implementing
/// error type; fields that fail to serialize degrade to `Value::Null`.
#[doc(hidden)]
#[must_use]
pub fn __payload_tree(
    kind: &ErrorKind,
    variant: &str,
    key: &str,
    message: &str,
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

/// Generic single-pass payload views shared by the default trait methods
/// (hand-written implementors). The derive generates its own equivalents
/// scoped to each enum.
mod payload {
    use serde::Serialize;

    use crate::LocalizedApiError;

    /// Single-pass view of an implementor for `serde_json`.
    pub(super) struct Ref<'a, E: ?Sized>(pub(super) &'a E);

    impl<E: LocalizedApiError + ?Sized> Serialize for Ref<'_, E> {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            use serde::ser::SerializeStruct as _;
            let mut state = serializer.serialize_struct("LocalizedApiError", 5)?;
            state.serialize_field("kind", self.0.error_kind().as_kebab())?;
            state.serialize_field("variant", self.0.error_variant())?;
            state.serialize_field("key", self.0.error_key())?;
            state.serialize_field("message", self.0.error_message().as_ref())?;
            state.serialize_field("params", &Params(self.0))?;
            state.end()
        }
    }

    /// Delegates to [`LocalizedApiError::__serialize_params`], which the
    /// derive overrides to stream fields straight from the variant.
    pub(super) struct Params<'a, E: ?Sized>(pub(super) &'a E);

    impl<E: LocalizedApiError + ?Sized> Serialize for Params<'_, E> {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            self.0.__serialize_params(serializer)
        }
    }
}

/// Map a non-BadRequest [`ErrorKind`] onto its loco error variant. Also a
/// total mapping over every kind (a plain `BadRequest` carries the message as
/// its body); `to_loco_error` routes `BadRequest` through the richer envelope
/// before calling this.
fn loco_error_from_kind(kind: &ErrorKind, message: Cow<'_, str>) -> Error {
    match kind {
        ErrorKind::BadRequest => Error::BadRequest(message.into_owned()),
        ErrorKind::Unauthorized => Error::Unauthorized(message.into_owned()),
        ErrorKind::NotFound => Error::NotFound,
        ErrorKind::InternalServerError => Error::InternalServerError,
        ErrorKind::Custom(status, description) => Error::CustomError(
            *status,
            ErrorDetail::new(message.as_ref(), description.as_ref()),
        ),
        ErrorKind::Forbidden
        | ErrorKind::Conflict
        | ErrorKind::UnprocessableEntity
        | ErrorKind::TooManyRequests
        | ErrorKind::ServiceUnavailable => {
            Error::CustomError(kind.status_code(), ErrorDetail::new(message.as_ref(), ""))
        }
    }
}

/// Single-pass `{"code": <self>, "message": ...}` envelope for Loco's
/// `BadRequest`. Serializing `self` and splicing beats building a
/// `serde_json::Value` tree and walking it again.
fn bad_request_envelope(code: &(impl Serialize + ?Sized), message: &str) -> String {
    // `unwrap_or_else` keeps the fallback allocations off the success path.
    let code = serde_json::to_string(code).unwrap_or_else(|_| "null".to_owned());
    let message = serde_json::to_string(message).unwrap_or_else(|_| "\"\"".to_owned());
    format!("{{\"code\":{code},\"message\":{message}}}")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic, clippy::unwrap_used)]
    use super::{__status, ErrorKind, LocalizedApiError, loco_error_from_kind};
    use crate::loco_rs::errors::Error;
    use serde::Serialize;
    use serde_json::Value;
    use std::borrow::Cow;

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
        let error = loco_error_from_kind(&ErrorKind::BadRequest, "body".into());
        assert_eq!(as_bad_request(&error).map(String::as_str), Some("body"));
        assert!(as_custom_error(&error).is_none());

        let error = loco_error_from_kind(
            &ErrorKind::Custom(__status(402), Cow::Borrowed("fee")),
            "msg".into(),
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
        assert_eq!(
            as_custom_error(&error).and_then(|(_, detail)| detail.description.as_deref()),
            Some("fee")
        );
    }

    /// The extended named kinds map onto `CustomError` with their status.
    #[test]
    fn extended_kinds_map_onto_custom_error() {
        for kind in [
            ErrorKind::Forbidden,
            ErrorKind::Conflict,
            ErrorKind::UnprocessableEntity,
            ErrorKind::TooManyRequests,
            ErrorKind::ServiceUnavailable,
        ] {
            let expected = kind.status_code();
            let error = loco_error_from_kind(&kind, "msg".into());
            let (status, detail) = as_custom_error(&error)
                .unwrap_or_else(|| panic!("expected CustomError for {kind:?}"));
            assert_eq!(status, expected);
            assert_eq!(detail.error.as_deref(), Some("msg"));
        }
    }

    /// `Display` supertrait users still get the message through the default
    /// `error_message` (`Cow::Owned(self.to_string())`).
    #[test]
    fn envelope_falls_back_when_serialization_fails() {
        struct Null;
        impl Serialize for Null {
            fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
                Err(serde::ser::Error::custom("always fails"))
            }
        }
        let envelope = super::bad_request_envelope(&Null, "\"quoted\"");
        assert_eq!(envelope, "{\"code\":null,\"message\":\"\\\"quoted\\\"\"}");
    }

    /// A hand-written implementor (no derive) exercising every default trait
    /// method: `error_message` routes through `Display`, the payload goes
    /// through the generic single-pass `payload::Ref` view.
    struct ManualError {
        code: u16,
    }

    impl std::fmt::Display for ManualError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(formatter, "manual error {}", self.code)
        }
    }

    impl Serialize for ManualError {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            serializer.serialize_u16(self.code)
        }
    }

    impl LocalizedApiError for ManualError {
        fn error_kind(&self) -> ErrorKind {
            ErrorKind::Custom(__status(402), Cow::Owned("payment required".to_owned()))
        }

        fn error_variant(&self) -> &'static str {
            "ManualError"
        }

        fn error_key(&self) -> &'static str {
            "errors.custom.manual"
        }

        fn error_params(&self) -> Vec<(&'static str, Value)> {
            vec![("code", Value::from(self.code))]
        }
    }

    #[test]
    fn manual_implementor_gets_every_default() {
        let error = ManualError { code: 7 };
        assert_eq!(error.error_message(), "manual error 7");
        assert!(matches!(error.error_message(), Cow::Owned(_)));
        assert_eq!(error.status_code(), 402);

        // The `Serialize` supertrait stays usable on its own (the BadRequest
        // envelope depends on it).
        assert_eq!(serde_json::to_string(&error).unwrap(), "7");

        let payload = error.to_json();
        assert_eq!(payload["kind"], "custom");
        assert_eq!(payload["variant"], "ManualError");
        assert_eq!(payload["key"], "errors.custom.manual");
        assert_eq!(payload["message"], "manual error 7");
        assert_eq!(payload["params"]["code"], 7);

        assert_eq!(
            serde_json::from_str::<Value>(&error.to_json_string()).unwrap(),
            payload
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&error.to_json_bytes()).unwrap(),
            payload
        );

        let loco_error = error.to_loco_error();
        let (status, detail) = as_custom_error(&loco_error).unwrap();
        assert_eq!(status, 402);
        assert_eq!(detail.error.as_deref(), Some("manual error 7"));
        assert_eq!(detail.description.as_deref(), Some("payment required"));
    }

    /// A hand-written implementor whose `__serialize_params` always fails,
    /// exercising the default-method fallbacks: the payload degrades to the
    /// tree path where every parameter becomes its `error_params()` value.
    struct ManualBrokenError;

    impl std::fmt::Display for ManualBrokenError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("broken")
        }
    }

    impl Serialize for ManualBrokenError {
        fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom("always fails"))
        }
    }

    impl LocalizedApiError for ManualBrokenError {
        fn error_kind(&self) -> ErrorKind {
            ErrorKind::BadRequest
        }

        fn error_variant(&self) -> &'static str {
            "ManualBrokenError"
        }

        fn error_key(&self) -> &'static str {
            "errors.bad-request.broken"
        }

        fn error_params(&self) -> Vec<(&'static str, Value)> {
            vec![("kept", Value::from(true))]
        }

        fn __serialize_params<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom("params always fail"))
        }
    }

    #[test]
    fn broken_params_fall_back_to_the_tree_payload() {
        let error = ManualBrokenError;
        let payload = error.to_json();
        assert_eq!(payload["kind"], "bad-request");
        assert_eq!(payload["message"], "broken");
        assert_eq!(payload["params"]["kept"], true);

        let bytes = error.to_json_bytes();
        assert_eq!(serde_json::from_slice::<Value>(&bytes).unwrap(), payload);
        assert_eq!(
            serde_json::from_str::<Value>(&error.to_json_string()).unwrap(),
            payload
        );

        // The BadRequest envelope degrades `code` to `null` when `self`
        // fails to serialize, while the message survives.
        let loco_error = error.to_loco_error();
        assert_eq!(
            as_bad_request(&loco_error).map(String::as_str),
            Some("{\"code\":null,\"message\":\"broken\"}")
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
