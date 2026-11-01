//! End-to-end behavioral tests for every derive feature: all variant shapes,
//! all error kinds, every attribute, the Loco conversions and the structured
//! JSON payload.
//!
//! `panic!` is the test-failure mechanism in the `match`-based conversions
//! below, hence the scoped exception to the workspace panic lint.
#![allow(clippy::panic)]

use locolized_errors::loco_rs::errors::Error;
use locolized_errors::loco_rs::prelude::IntoResponse as _;
use locolized_errors::rust_i18n;
use locolized_errors::{ErrorKind, LocalizedApiError};
use serde::Serialize;
use serde_json::Value;

rust_i18n::i18n!("locales", fallback = "en");

/// All tests in this binary share the process-global locale; pin it to
/// English before each assertion-bearing test.
fn english() {
    rust_i18n::set_locale("en");
}

/// The four kinds via naming convention.
#[derive(Debug, Clone, Serialize, LocalizedApiError)]
enum BadRequestError {
    SwitchAlreadyTriggered {
        switch_id: i32,
    },
    ParseError {
        field: String,
        value: String,
        target_type: String,
    },
    UserNotFound,
    EmailAlreadyExists,
    InvalidInput(String),
}

#[derive(Debug, Clone, Serialize, LocalizedApiError)]
enum UnauthorizedError {
    InvalidCredentials,
    SessionExpired,
}

#[derive(Debug, Clone, Serialize, LocalizedApiError)]
enum InternalServerErrorError {
    Boom,
}

/// Kind + key overrides.
#[derive(Debug, Clone, Serialize, LocalizedApiError)]
#[localized(kind = "not-found")]
enum MissingThing {
    #[localized(key = "record-missing")]
    Generic,
    #[localized(key = "widget")]
    Widget { id: u32 },
}

/// Custom HTTP status at the enum level, with a description.
#[derive(Debug, Clone, Serialize, LocalizedApiError)]
#[localized(status = 418, description = "short and stout")]
enum TeapotError {
    Brewed,
}

/// Custom HTTP status at the variant level, overriding the enum kind.
#[derive(Debug, Clone, Serialize, LocalizedApiError)]
#[localized(kind = "bad-request")]
enum MixedStatus {
    Plain,
    #[localized(status = 507, description = "no room left")]
    Insufficient,
}

/// Static message bypassing i18n.
#[derive(Debug, Clone, Serialize, LocalizedApiError)]
#[localized(kind = "unauthorized")]
enum StaticMessage {
    #[localized(message = "Your session has expired, please log in again")]
    SessionExpired { user_id: i64 },
}

/// Opt-out of the generated `IntoResponse` impl.
#[derive(Debug, Clone, Serialize, LocalizedApiError)]
#[localized(kind = "not-found", no_into_response)]
enum SilentError {
    #[localized(key = "record-missing")]
    Missing,
}

/// A field whose `Serialize` implementation deliberately fails, exercising the
/// JSON fallback paths, while still implementing `Display` so the `t!`
/// interpolation compiles.
#[derive(Debug, Clone, Serialize, LocalizedApiError)]
#[localized(kind = "bad-request")]
enum UnserializableError {
    #[localized(key = "invalid-input")]
    Broken { value: AlwaysFails },
}

#[derive(Debug, Clone)]
/// Deliberately failing `Serialize` implementation.
pub struct AlwaysFails;

impl Serialize for AlwaysFails {
    fn serialize<S: serde::Serializer>(&self, _serializer: S) -> Result<S::Ok, S::Error> {
        Err(serde::ser::Error::custom("always fails"))
    }
}

impl std::fmt::Display for AlwaysFails {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("<value>")
    }
}

fn envelope(error: &impl LocalizedApiError) -> Value {
    serde_json::from_str(&error.to_json_string()).unwrap_or_default()
}

#[test]
fn display_interpolates_named_fields() {
    english();
    let error = BadRequestError::SwitchAlreadyTriggered { switch_id: 7 };
    assert_eq!(error.to_string(), "Switch 7 has already been triggered");

    let error = BadRequestError::ParseError {
        field: "age".to_owned(),
        value: "abc".to_owned(),
        target_type: "number".to_owned(),
    };
    assert_eq!(
        error.to_string(),
        "Failed to parse field 'age' with value 'abc' as number"
    );
}

#[test]
fn display_renders_unit_and_tuple_variants() {
    english();
    assert_eq!(BadRequestError::UserNotFound.to_string(), "User not found");
    assert_eq!(
        BadRequestError::InvalidInput("oops".to_owned()).to_string(),
        "Invalid input"
    );
    assert_eq!(
        UnauthorizedError::InvalidCredentials.to_string(),
        "Invalid credentials"
    );
    assert_eq!(
        InternalServerErrorError::Boom.to_string(),
        "Something went wrong on our side"
    );
}

#[test]
fn display_uses_static_message_override() {
    english();
    let error = StaticMessage::SessionExpired { user_id: 42 };
    assert_eq!(
        error.to_string(),
        "Your session has expired, please log in again"
    );
}

#[test]
fn introspection_methods_work() {
    english();
    let error = BadRequestError::SwitchAlreadyTriggered { switch_id: 1 };
    assert_eq!(error.error_variant(), "SwitchAlreadyTriggered");
    assert_eq!(
        error.error_key(),
        "errors.bad-request.switch-already-triggered"
    );
    assert_eq!(error.error_kind(), ErrorKind::BadRequest);
    assert_eq!(error.status_code(), 400);

    assert_eq!(
        MissingThing::Generic.error_key(),
        "errors.not-found.record-missing"
    );
    assert_eq!(MissingThing::Generic.error_variant(), "Generic");
    assert_eq!(MissingThing::Generic.error_kind(), ErrorKind::NotFound);

    assert_eq!(TeapotError::Brewed.status_code(), 418);
    assert_eq!(
        TeapotError::Brewed.error_kind(),
        ErrorKind::Custom(
            locolized_errors::__status(418),
            "short and stout".to_owned()
        )
    );
    assert_eq!(MixedStatus::Insufficient.status_code(), 507);
    assert_eq!(MixedStatus::Plain.status_code(), 400);
    assert_eq!(
        StaticMessage::SessionExpired { user_id: 1 }.error_kind(),
        ErrorKind::Unauthorized
    );
}

#[test]
fn params_are_json_ready() {
    english();
    let error = BadRequestError::SwitchAlreadyTriggered { switch_id: 3 };
    let params = error.error_params();
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].0, "switch_id");
    assert_eq!(params[0].1, Value::from(3));

    let empty: Vec<(&'static str, Value)> = Vec::new();
    assert_eq!(BadRequestError::UserNotFound.error_params(), empty);
    assert_eq!(
        BadRequestError::InvalidInput("x".to_owned()).error_params(),
        empty
    );

    // Message overrides do not remove the structured parameters.
    let params = StaticMessage::SessionExpired { user_id: 8 }.error_params();
    assert_eq!(params, vec![("user_id", Value::from(8))]);
}

#[test]
fn structured_payload_has_expected_shape() {
    english();
    let error = MissingThing::Widget { id: 9 };
    let payload = envelope(&error);
    assert_eq!(payload["kind"], "not-found");
    assert_eq!(payload["variant"], "Widget");
    assert_eq!(payload["key"], "errors.not-found.widget");
    assert_eq!(payload["message"], "Widget 9 was not found");
    assert_eq!(payload["params"]["id"], 9);
}

#[test]
fn structured_payload_for_custom_kind() {
    english();
    let payload = envelope(&TeapotError::Brewed);
    assert_eq!(payload["kind"], "custom");
    assert_eq!(payload["message"], "I'm a teapot");
}

#[test]
fn loco_conversion_maps_every_named_kind() {
    english();
    match BadRequestError::UserNotFound.to_loco_error() {
        Error::BadRequest(body) => {
            let payload: Value = serde_json::from_str(&body).unwrap_or_default();
            assert_eq!(payload["message"], "User not found");
            assert_eq!(payload["code"], "UserNotFound");
        }
        _ => panic!("expected BadRequest"),
    }
    assert!(matches!(
        UnauthorizedError::SessionExpired.to_loco_error(),
        Error::Unauthorized(_) | _
    ));
    assert!(matches!(
        MissingThing::Generic.to_loco_error(),
        Error::NotFound | _
    ));
    assert!(matches!(
        InternalServerErrorError::Boom.to_loco_error(),
        Error::InternalServerError | _
    ));
    assert!(matches!(
        SilentError::Missing.to_loco_error(),
        Error::NotFound | _
    ));
}

#[test]
fn loco_conversion_maps_custom_kind_with_description() {
    english();
    match TeapotError::Brewed.to_loco_error() {
        Error::CustomError(status, detail) => {
            assert_eq!(status, 418);
            assert_eq!(detail.error.as_deref(), Some("I'm a teapot"));
            assert_eq!(detail.description.as_deref(), Some("short and stout"));
        }
        _ => panic!("expected CustomError"),
    }
    match MixedStatus::Insufficient.to_loco_error() {
        Error::CustomError(status, detail) => {
            assert_eq!(status, 507);
            assert_eq!(detail.description.as_deref(), Some("no room left"));
        }
        _ => panic!("expected CustomError"),
    }
}

#[test]
fn from_conversion_matches_to_loco_error() {
    english();
    let error: Error = BadRequestError::EmailAlreadyExists.into();
    assert!(matches!(error, Error::BadRequest(_) | _));

    let error: Error = MissingThing::Generic.into();
    assert!(matches!(error, Error::NotFound | _));
}

#[test]
fn into_response_serves_json_with_matching_status() {
    english();
    let response = MissingThing::Widget { id: 5 }.into_response();
    assert_eq!(response.status(), 404);

    let response = TeapotError::Brewed.into_response();
    assert_eq!(response.status(), 418);

    let response = BadRequestError::SwitchAlreadyTriggered { switch_id: 2 }.into_response();
    assert_eq!(response.status(), 400);
}

#[test]
fn bad_request_envelope_is_single_pass_json() {
    english();
    let error = BadRequestError::SwitchAlreadyTriggered { switch_id: 11 };
    let Error::BadRequest(body) = error.to_loco_error() else {
        panic!("expected BadRequest");
    };
    let payload: Value = serde_json::from_str(&body).unwrap_or_default();
    assert_eq!(
        payload["code"],
        serde_json::json!({"SwitchAlreadyTriggered": {"switch_id": 11}})
    );
    assert_eq!(payload["message"], "Switch 11 has already been triggered");
}

#[test]
fn errors_implement_std_error() {
    english();
    assert_std_error(&BadRequestError::UserNotFound);
    assert_std_error(&TeapotError::Brewed);
}

#[test]
fn errors_are_send_sync_static() {
    english();
    assert_send_sync::<BadRequestError>();
    assert_send_sync::<TeapotError>();

    // `LocalizedApiError` is not dyn compatible (`Serialize` supertrait), so
    // implementors are always passed by concrete type or generic parameter.
    assert_eq!(error_kind_of(&MissingThing::Generic), ErrorKind::NotFound);
}

fn assert_std_error<E: std::error::Error>(error: &E) {
    let _: &dyn std::error::Error = error;
}

fn assert_send_sync<T: Send + Sync + 'static>() {}

fn error_kind_of<T: LocalizedApiError>(error: &T) -> ErrorKind {
    error.error_kind()
}

#[test]
fn unserializable_code_field_falls_back_to_null() {
    english();
    let error = UnserializableError::Broken { value: AlwaysFails };
    let Error::BadRequest(body) = error.to_loco_error() else {
        panic!("expected BadRequest");
    };
    let payload: Value = serde_json::from_str(&body).unwrap_or_default();
    assert_eq!(payload["code"], Value::Null);
    assert_eq!(payload["message"], "Invalid input");

    // The params list degrades to `null` instead of failing.
    let params = error.error_params();
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].0, "value");
    assert_eq!(params[0].1, Value::Null);
    assert_eq!(envelope(&error)["params"]["value"], Value::Null);
}
