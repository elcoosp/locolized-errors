# Performance & API Design Notes — 0.3.0 iteration

Research + brainstorm backing the 0.3.0 performance release. Every claim below is
verified against the vendored sources of `rust-i18n 4.2.4`, `axum 0.8.9` and
`loco-rs 1.2` in the local cargo registry, and against the 0.2.0 codebase.

## Findings on the 0.2.0 code

| # | Finding | Severity | Fix |
| :-- | :--- | :--- | :--- |
| 1 | The generated `IntoResponse` returns `(status, String)`; axum's `IntoResponse for String` sets `content-type: text/plain; charset=utf-8`, so **the JSON error payload was served as `text/plain`**. | correctness bug | Explicit `application/json` header via a tested `__response_from_json` helper. |
| 2 | `to_json_string()` builds a `serde_json::Value` tree (`payload_json`) and then re-serializes it — two passes, 5 key `String`s, a params `Map`, then a full `Value::to_string` re-walk. | hot-path cost | Generated `Serialize` payload struct streamed **once** by `serde_json` straight into the output buffer. |
| 3 | `error_params()` materializes a `Vec` plus one `serde_json::to_value` per field (each `String` field is copied into a fresh `Value::String`) — and the old JSON path used it even when the caller only wanted bytes. | hot-path cost | New hidden `__serialize_params` streams fields **directly from the variant** into the serializer (`SerializeMap`); `error_params()` remains as the introspection/fallback API. |
| 4 | `ErrorKind::Custom(StatusCode, String)` — the macro emitted `String::from(description)`, so **every `error_kind()` call on a custom-status variant heap-allocated**; it also made `ErrorKind` non-`Clone`-cheap and forced `description.clone()` in `to_loco_error`. | hot-path cost | `Custom(StatusCode, Cow<'static, str>)`; the macro emits `Cow::Borrowed("…")` — zero allocation per call. *(breaking)* |
| 5 | `bad_request_envelope` used `unwrap_or` whose fallbacks (`"null".to_owned()`, `"\"\"".to_owned()`) are evaluated **eagerly on every success path**. | micro | `unwrap_or_else` — fallbacks only materialize when serialization actually fails. |
| 6 | `Display` rendered via `write!(f, "{}", …)` — full formatting machinery for a single string. | micro | Generated `Display` now calls `error_message()` and `Formatter::write_str`. |
| 7 | `Display` was the only way to obtain the localized message; `to_string()` forced a `String` even when the translation is a borrowed `&'static str`. | API + perf | New `error_message() -> Cow<'static, str>` (default: `to_string()`; overridden by the derive). `t!` without args returns `Cow::Borrowed` from the compile-time-embedded backend — zero allocation, verified in `rust-i18n-macro-4.2.4/src/tr.rs`. |
| 8 | `loco-rs` was pulled with `features = ["with-db"]` although the crate only touches `errors`, `controller::ErrorDetail` and `prelude::IntoResponse` — none of which are feature-gated (verified in loco-rs 1.2 sources). Every downstream build compiled sea-orm + sea-orm-migration + sqlx for nothing. | compile time | Drop `with-db` (unification still enables it for real Loco apps; non-Loco consumers skip the whole db tree). |
| 9 | `syn` was pulled with `features = ["full", "extra-traits"]`; the macro parses only items/attributes (no expressions/statements) and its own structs contain no `syn` types needing `Debug`. | compile time | Drop both features. |
| 10 | The runtime crate's `serde` dependency enabled `derive`, which the library itself never uses (only the `Serialize` trait bound). | compile time (marginal) | Runtime dep without `derive`; tests keep a dev-dependency with `derive`. |
| 11 | JSON `"kind"` for any custom-status enum is the opaque `"custom"`, and the i18n prefix falls back to the raw enum name. | API gap | Five new named kinds — `forbidden` (403), `unprocessable-entity` (422), `too-many-requests` (429), `conflict` (409), `service-unavailable` (503) — giving semantic JSON kinds and `errors.<kind>.*` key prefixes. *(breaking: `ErrorKind` gains variants)* |

## What stays as-is (checked, already SOTA)

- **rust-i18n 4.2.4 internals**: `t!` with no args is a single backend lookup returning
  `Cow::Borrowed`; with args it runs `replace_patterns_fast`, a single-pass byte state
  machine with one output allocation. Bypassing it would mean depending on
  `#[doc(hidden)]` internals — rejected as fragile.
- **Keys are `&'static str` literals** baked in by the macro (no runtime formatting).
- **BadRequest envelope** is already single-pass splice of two pre-escaped strings.
- **`release` profile** already tuned (`lto = "thin"`, `codegen-units = 1`, `strip`).
- **No-std**: still out of scope (axum/loco are std-bound).

## Design of the streaming payload

For every derived enum the macro now generates (inside a `const _: () = { … }` block
for name hygiene):

1. `__Params<'a>(&'a Enum)` — `Serialize` impl delegating to the generated
   `__serialize_params`, which serializes each named field **straight from `self`**
   (`serialize_map`/`serialize_entry`), no intermediate `Value`, no `Vec`.
2. `__Payload<'a>(&'a Enum)` — `Serialize` impl emitting
   `{"kind","variant","key","message","params"}` in one pass.
3. Overrides of `to_json_bytes` / `to_json_string` / `to_json` on the trait impl,
   all going through `__Payload` (single pass), with a graceful fallback to the
   tree path (per-field `Value::Null` degradation, preserved semantics for
   unserializable fields — covered by tests).
4. `IntoResponse` delegating to `__response_from_json(status, to_json_bytes())`
   which sets `content-type: application/json` and builds the body from the byte
   buffer with zero extra copies (`Body::from(Vec<u8>)`).

Field-order note: the streaming payload emits keys in documentation order
(`kind, variant, key, message, params`); the fallback tree path still emits
`serde_json::Map` (sorted) order. JSON object key order is not semantic; the
structured shape and values are identical and both are tested.

## Benchmark methodology

`benches/perf.rs` (criterion) benchmarks the 0.2.0 implementations — copied verbatim
into the bench as `legacy_*` functions operating on the same enums and locale data —
against the 0.3.0 API. This isolates the algorithmic change from machine noise.
`into_response` is measured by collecting the full body (`http_body_util::BodyExt`).
Compile-time impact is measured as a clean `cargo build` of the workspace plus a
`cargo tree` diff. Results live in `docs/BENCHMARKS.md`.
