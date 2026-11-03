//! Criterion benchmarks: the 0.2.0 implementations (copied verbatim into the
//! `legacy_*` helpers below) against the 0.3.0 API, on the same enums and
//! locale data. This isolates the algorithmic changes from machine noise.
//!
//! Run with `cargo bench` (results are also summarized in
//! `docs/BENCHMARKS.md`).

use std::borrow::Cow;
use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use locolized_errors::loco_rs::errors::Error;
use locolized_errors::loco_rs::prelude::IntoResponse as _;
use locolized_errors::rust_i18n;
use locolized_errors::{ErrorKind, LocalizedApiError};
use serde::Serialize;
use serde_json::Value;

rust_i18n::i18n!("locales", fallback = "en");

fn english() {
    rust_i18n::set_locale("en");
}

#[derive(Debug, Clone, Serialize, LocalizedApiError)]
enum BadRequestError {
    SwitchAlreadyTriggered { switch_id: i32 },
    UserNotFound,
}

#[derive(Debug, Clone, Serialize, LocalizedApiError)]
#[localized(status = 418, description = "short and stout")]
enum TeapotError {
    Brewed,
}

// ---------------------------------------------------------------------------
// 0.2.0 implementations, verbatim (before this iteration)
// ---------------------------------------------------------------------------

fn legacy_payload_json(
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

fn legacy_to_json(error: &BadRequestError) -> Value {
    legacy_payload_json(
        &error.error_kind(),
        error.error_variant(),
        error.error_key(),
        error.to_string(),
        error.error_params(),
    )
}

fn legacy_to_json_string(error: &BadRequestError) -> String {
    legacy_to_json(error).to_string()
}

fn legacy_bad_request_envelope(code: &(impl Serialize + ?Sized), message: &str) -> String {
    // 0.2.0 used `unwrap_or` (eager fallback allocation on every call).
    let code = serde_json::to_string(code).unwrap_or("null".to_owned());
    let message = serde_json::to_string(message).unwrap_or("\"\"".to_owned());
    format!("{{\"code\":{code},\"message\":{message}}}")
}

fn legacy_to_loco_error(error: &BadRequestError) -> Error {
    let message = error.to_string();
    Error::BadRequest(legacy_bad_request_envelope(error, &message))
}

/// 0.2.0-style custom kind: allocates an owned description on every call.
fn legacy_custom_kind() -> ErrorKind {
    ErrorKind::Custom(
        locolized_errors::__status(418),
        Cow::Owned(std::string::String::from("short and stout")),
    )
}

/// 0.2.0-style `Display`: formatting machinery around the `t!` result.
fn legacy_display(error: &BadRequestError) -> String {
    match error {
        BadRequestError::SwitchAlreadyTriggered { switch_id } => format!(
            "{}",
            rust_i18n::t!(
                "errors.bad-request.switch-already-triggered",
                switch_id = switch_id
            )
        ),
        BadRequestError::UserNotFound => {
            format!("{}", rust_i18n::t!("errors.bad-request.user-not-found"))
        }
    }
}

/// Legacy `IntoResponse`: `(status, to_json_string()).into_response()` —
/// builds the `Value` tree, serializes it to `String`, then hands a
/// `text/plain`-typed body to axum.
fn legacy_into_response(error: &BadRequestError) -> locolized_errors::loco_rs::prelude::Response {
    (
        error.error_kind().status_code(),
        legacy_to_json_string(error),
    )
        .into_response()
}

fn body_bytes(response: locolized_errors::loco_rs::prelude::Response) -> usize {
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread().build() else {
        unreachable!("current-thread runtime construction cannot fail");
    };
    runtime.block_on(async {
        match http_body_util::BodyExt::collect(response.into_body()).await {
            Ok(collected) => collected.to_bytes().len(),
            Err(_) => unreachable!("collecting an in-memory body cannot fail"),
        }
    })
}

// ---------------------------------------------------------------------------
// Benchmarks
// ---------------------------------------------------------------------------

fn bench_display(c: &mut Criterion) {
    english();
    let mut group = c.benchmark_group("display");

    let unit = BadRequestError::UserNotFound;
    group.bench_function("unit/legacy", |b| {
        b.iter(|| legacy_display(black_box(&unit)));
    });
    group.bench_function("unit/current", |b| b.iter(|| black_box(&unit).to_string()));

    let params = BadRequestError::SwitchAlreadyTriggered { switch_id: 3 };
    group.bench_function("interpolated/legacy", |b| {
        b.iter(|| legacy_display(black_box(&params)));
    });
    group.bench_function("interpolated/current", |b| {
        b.iter(|| black_box(&params).to_string());
    });
    group.finish();
}

fn bench_error_kind(c: &mut Criterion) {
    let mut group = c.benchmark_group("error_kind");
    group.bench_function("custom/legacy_string", |b| {
        b.iter(|| {
            let kind = legacy_custom_kind();
            assert_eq!(kind.status_code(), 418);
            kind
        });
    });
    group.bench_function("custom/current_cow", |b| {
        b.iter(|| {
            let kind = black_box(&TeapotError::Brewed).error_kind();
            assert_eq!(kind.status_code(), 418);
            kind
        });
    });
    group.finish();
}

fn bench_payload(c: &mut Criterion) {
    english();
    let mut group = c.benchmark_group("payload");
    let error = BadRequestError::SwitchAlreadyTriggered { switch_id: 7 };

    group.bench_function("to_json/legacy_tree", |b| {
        b.iter(|| legacy_to_json(black_box(&error)));
    });
    group.bench_function("to_json/current_stream", |b| {
        b.iter(|| black_box(&error).to_json());
    });

    group.bench_function("to_json_string/legacy_double_pass", |b| {
        b.iter(|| legacy_to_json_string(black_box(&error)));
    });
    group.bench_function("to_json_string/current_single_pass", |b| {
        b.iter(|| black_box(&error).to_json_string());
    });

    group.bench_function("to_json_bytes/current_single_pass", |b| {
        b.iter(|| black_box(&error).to_json_bytes());
    });

    group.bench_function("error_params/introspection", |b| {
        b.iter(|| black_box(&error).error_params());
    });
    group.finish();
}

fn bench_loco_error(c: &mut Criterion) {
    english();
    let mut group = c.benchmark_group("loco_error");
    let error = BadRequestError::SwitchAlreadyTriggered { switch_id: 7 };

    group.bench_function("bad_request/legacy", |b| {
        b.iter(|| legacy_to_loco_error(black_box(&error)));
    });
    group.bench_function("bad_request/current", |b| {
        b.iter(|| black_box(&error).to_loco_error());
    });
    group.finish();
}

fn bench_into_response(c: &mut Criterion) {
    english();
    let mut group = c.benchmark_group("into_response");
    let error = BadRequestError::SwitchAlreadyTriggered { switch_id: 7 };

    // Sanity: both paths must produce the same status and payload size.
    assert_eq!(legacy_into_response(&error).status(), error.status_code());
    assert_eq!(
        body_bytes(legacy_into_response(&error)),
        body_bytes(error.clone().into_response())
    );

    group.bench_function("legacy_tree_string", |b| {
        b.iter(|| body_bytes(legacy_into_response(black_box(&error))));
    });
    group.bench_function("current_single_pass_bytes", |b| {
        b.iter(|| body_bytes(black_box(&error).clone().into_response()));
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_display,
    bench_error_kind,
    bench_payload,
    bench_loco_error,
    bench_into_response,
);
criterion_main!(benches);
