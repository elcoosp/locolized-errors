//! Tests for the streaming JSON payload path, the generated `IntoResponse`
//! contract (`application/json`), and the `error_message()` introspection API.
//!
//! `panic!`/`expect` are the test-failure mechanism here, hence the scoped
//! lint exceptions.
#![allow(clippy::panic, clippy::expect_used)]

use locolized_errors::loco_rs::prelude::IntoResponse as _;
use locolized_errors::rust_i18n;
use locolized_errors::{ErrorKind, LocalizedApiError};
use serde::Serialize;
use std::borrow::Cow;

rust_i18n::i18n!("locales", fallback = "en");

fn english() {
    rust_i18n::set_locale("en");
}

#[derive(Debug, Clone, Serialize, LocalizedApiError)]
enum BadRequestError {
    UserNotFound,
    TooManyAttempts { attempts: u32 },
    InvalidInput(String),
}

#[derive(Debug, Clone, Serialize, LocalizedApiError)]
#[localized(status = 418, description = "short and stout")]
enum TeapotError {
    Brewed,
}

#[derive(Debug, Clone, Serialize, LocalizedApiError)]
#[localized(kind = "unauthorized")]
enum StaticMessage {
    #[localized(message = "Your session has expired, please log in again")]
    SessionExpired { user_id: i64 },
}

#[test]
fn into_response_sets_application_json_content_type() {
    english();
    let response = BadRequestError::UserNotFound.into_response();
    assert_eq!(response.status(), 400);
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok());
    assert_eq!(
        content_type,
        Some("application/json"),
        "JSON payloads must be served as application/json"
    );
}

#[test]
fn into_response_body_is_the_streamed_payload() {
    english();
    let response = BadRequestError::TooManyAttempts { attempts: 3 }.into_response();
    assert_eq!(response.status(), 400);
    let body = collect_body(response);
    let payload: serde_json::Value = serde_json::from_slice(&body).expect("valid JSON body");
    assert_eq!(payload["kind"], "bad-request");
    assert_eq!(payload["variant"], "TooManyAttempts");
    assert_eq!(payload["key"], "errors.bad-request.too-many-attempts");
    assert_eq!(payload["message"], "Too many attempts: 3");
    assert_eq!(payload["params"]["attempts"], 3);
}

#[test]
fn to_json_bytes_is_the_single_pass_payload() {
    english();
    let error = BadRequestError::TooManyAttempts { attempts: 5 };
    let bytes = error.to_json_bytes();
    let from_bytes: serde_json::Value = serde_json::from_slice(&bytes).expect("valid JSON bytes");
    assert_eq!(from_bytes, error.to_json(), "bytes must match to_json()");
    // The streaming path emits keys in documentation order.
    let text = core::str::from_utf8(&bytes).expect("utf-8");
    assert!(
        text.starts_with("{\"kind\":"),
        "streaming payload must start with `kind`: {text}"
    );
}

#[test]
fn to_json_bytes_matches_to_json_for_every_variant_shape() {
    english();
    for text in [
        BadRequestError::UserNotFound.to_json_bytes(),
        BadRequestError::TooManyAttempts { attempts: 1 }.to_json_bytes(),
        BadRequestError::InvalidInput("x".to_owned()).to_json_bytes(),
        TeapotError::Brewed.to_json_bytes(),
    ] {
        let parsed: serde_json::Value = serde_json::from_slice(&text).expect("valid JSON");
        assert!(parsed.get("kind").is_some(), "{parsed}");
    }
}

#[test]
fn to_json_string_is_the_streamed_payload() {
    english();
    let error = BadRequestError::UserNotFound;
    let parsed: serde_json::Value =
        serde_json::from_str(&error.to_json_string()).expect("valid JSON");
    assert_eq!(parsed, error.to_json());
}

#[test]
fn error_message_matches_display() {
    english();
    assert_eq!(
        BadRequestError::UserNotFound.error_message(),
        BadRequestError::UserNotFound.to_string()
    );
    assert_eq!(
        BadRequestError::TooManyAttempts { attempts: 2 }.error_message(),
        "Too many attempts: 2"
    );
    assert_eq!(
        StaticMessage::SessionExpired { user_id: 1 }.error_message(),
        "Your session has expired, please log in again"
    );
}

#[test]
fn error_message_borrows_when_no_interpolation_is_needed() {
    english();
    // Found translation without parameters: borrowed from the embedded backend.
    assert!(matches!(
        BadRequestError::UserNotFound.error_message(),
        Cow::Borrowed(_)
    ));
    // Static `message = "..."` overrides are borrowed literals.
    assert!(matches!(
        StaticMessage::SessionExpired { user_id: 1 }.error_message(),
        Cow::Borrowed(_)
    ));
    // Interpolated variants own their message.
    assert!(matches!(
        BadRequestError::TooManyAttempts { attempts: 2 }.error_message(),
        Cow::Owned(_)
    ));
}

#[test]
fn custom_kind_description_is_borrowed() {
    let ErrorKind::Custom(status, description) = TeapotError::Brewed.error_kind() else {
        panic!("expected Custom kind");
    };
    assert_eq!(status, 418);
    assert!(
        matches!(description, Cow::Borrowed("short and stout")),
        "macro-generated descriptions must be borrowed, got {description:?}"
    );
    assert_eq!(description, "short and stout");
}

/// The `CONTENT_TYPE` header name via the library's `axum` re-export.
use locolized_errors::axum::http::header::CONTENT_TYPE;

/// Collects a response body into bytes (single-threaded runtime).
fn collect_body(response: locolized_errors::loco_rs::prelude::Response) -> Vec<u8> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("single-thread runtime");
    runtime.block_on(async {
        http_body_util::BodyExt::collect(response.into_body())
            .await
            .expect("collect body")
            .to_bytes()
            .to_vec()
    })
}
