<div align="center">
  <img src="docs/logo.png" alt="locolized_errors Logo" width="300"/>
  <p>
    <strong>Localized Errors for Loco</strong><br/>
    A Rust macro for generating localized API errors with seamless integration into the Loco web framework.
  </p>
  <p>
    <img src="https://img.shields.io/badge/Rust-1.94%20%7C%202024-000000?style=flat-square&logo=rust" alt="Rust"/>
    <img src="https://img.shields.io/crates/v/locolized_errors?style=flat-square" alt="Crates.io"/>
    <a href="https://github.com/elcoosp/locolized-errors/actions"><img src="https://img.shields.io/github/actions/workflow/status/elcoosp/locolized-errors/ci.yml?branch=main&style=flat-square&label=CI" alt="CI"/></a>
    <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue?style=flat-square" alt="License MIT"/></a>
    <img src="https://img.shields.io/badge/Framework-Loco%201.x-000000?style=flat-square" alt="Loco Framework"/>
  </p>
</div>

---

`locolized_errors` eliminates boilerplate when building internationalized APIs in Rust. By simply deriving a macro on your error enums, you get automatic error kind detection, field interpolation, conversion into Loco's native error types, structured JSON payloads, and a ready-to-use `IntoResponse` implementation.

## Features

- **Automatic error kind detection**: derives HTTP error kinds from enum names (`BadRequestError` → `ErrorKind::BadRequest`), overridable per enum or per variant. Nine named kinds are built in (400/401/403/404/409/422/429/500/503), plus arbitrary custom statuses.
- **Internationalization ready**: built-in integration with `rust_i18n` for localized, parameterized error messages — `rust-i18n` is re-exported, so it is **not** a direct dependency of your app.
- **Seamless Loco integration**: direct `Into<loco_rs::Error>` conversion for clean controller logic.
- **Structured JSON payloads, streamed in a single pass**: `to_json()` / `to_json_string()` / `to_json_bytes()` produce `{kind, variant, key, message, params}` for SPA/mobile frontends — serialized straight from the variant fields, no intermediate tree.
- **Axum/Loco `IntoResponse`**: error enums can be returned straight from handlers, rendering the JSON payload with the matching HTTP status and `content-type: application/json` (opt-out available).
- **Flexible variants**: unit, named, and tuple variants supported; named fields interpolate into messages.
- **Lean by default**: no database stack (`loco-rs` pulled without `with-db`), trimmed `syn` features — 58 fewer crates and ~20 % faster clean builds than 0.2.0 for non-Loco consumers.

## Installation

```toml
[dependencies]
locolized_errors = "0.2"
```

MSRV: Rust **1.94** (required by `loco-rs` 1.x), edition 2024.

## Quick Start

### 1. Initialize translations and define your errors

Derive `LocalizedApiError` and `serde::Serialize` on your error enums. Call `i18n!` **before** the modules using the derive (translation lookups expand inside your crate). Named fields automatically become interpolation parameters.

```rust
use locolized_errors::LocalizedApiError;
use locolized_errors::rust_i18n;
use serde::Serialize;

rust_i18n::i18n!("locales"); // loads ./locales/*.yml at compile time

#[derive(Debug, Serialize, LocalizedApiError)]
#[localized(kind = "bad-request")]
pub enum UserError {
    #[localized(key = "user-not-found")]
    NotFound,
    TooManyAttempts { attempts: u32 },
}

#[derive(Debug, Serialize, LocalizedApiError)]
#[localized(status = 418, description = "short and stout")]
pub enum TeapotError {
    Brewed,
}

# fn main() {
rust_i18n::set_locale("en");
assert_eq!(UserError::TooManyAttempts { attempts: 3 }.to_string(), "Too many attempts: 3");
assert_eq!(UserError::NotFound.error_kind(), locolized_errors::ErrorKind::BadRequest);
# }
```

### 2. Configure localization

Create translation files in the `locales` directory. Keys follow the pattern `errors.<kebab-case-kind>.<kebab-case-variant>`; placeholders use rust-i18n's `%{field}` syntax.

**`locales/en.yml`**

```yaml
errors.user-not-found: "User not found"
errors.too-many-attempts: "Too many attempts: %{attempts}"
```

**`locales/fr.yml`**

```yaml
errors.user-not-found: "Utilisateur introuvable"
errors.too-many-attempts: "Trop de tentatives : %{attempts}"
```

> [!TIP]
> **Fallback locales** — initialize with `i18n!("locales", fallback = "en")` so missing translations degrade gracefully.

### 3. Use in Loco controllers

Return your custom errors directly from handlers — the macro implements `From<YourError> for loco_rs::Error`, so `.into()` / `?` just work:

```rust,ignore
use loco_rs::prelude::*;

pub async fn create_user(State(_ctx): State<AppContext>) -> Result<Response> {
    Err(UserError::TooManyAttempts { attempts: 3 }.into()) // converts automatically
}
```

> [!TIP]
> **Explicit conversion** — if you prefer explicit over implicit, call `.to_loco_error()` instead of `.into()`.

### 4. Structured JSON payloads

Every error renders as machine-readable JSON, and the generated `IntoResponse` serves exactly that payload with the right status code:

```rust
use locolized_errors::loco_rs::prelude::IntoResponse as _;
use locolized_errors::LocalizedApiError;
use locolized_errors::rust_i18n;
use serde::Serialize;

rust_i18n::i18n!("locales");

#[derive(Debug, Serialize, LocalizedApiError)]
#[localized(status = 418, description = "short and stout")]
pub enum TeapotError {
    Brewed,
}

# fn main() {
let payload = TeapotError::Brewed.to_json();
assert_eq!(payload["kind"], "custom");
assert_eq!(payload["variant"], "Brewed");
assert_eq!(payload["params"], serde_json::json!({}));

let response = TeapotError::Brewed.into_response();
assert_eq!(response.status(), 418);
# }
```

## Attributes

The `#[localized(...)]` helper attribute configures the derive.

**Enum level:**

| Option | Example | Effect |
| :--- | :--- | :--- |
| `kind` | `#[localized(kind = "unauthorized")]` | Explicit kind: `bad-request`, `unauthorized`, `forbidden`, `not-found`, `conflict`, `unprocessable-entity`, `too-many-requests`, `internal-server-error`, `service-unavailable` |
| `status` | `#[localized(status = 418)]` | Map the whole enum to `ErrorKind::Custom` with that HTTP status |
| `description` | `#[localized(description = "...")]` | Description carried into Loco's `ErrorDetail` for custom statuses |
| `locale` | `#[localized(locale = "fr")]` | Pin the translation locale for every variant |
| `no_into_response` | `#[localized(no_into_response)]` | Skip the generated `IntoResponse` impl |

**Variant level:**

| Option | Example | Effect |
| :--- | :--- | :--- |
| `key` | `#[localized(key = "user-not-found")]` | Override the kebab-case key segment |
| `message` | `#[localized(message = "Static text")]` | Static message bypassing the i18n lookup |
| `status` / `description` / `locale` | — | Per-variant overrides (e.g. one variant with a different HTTP status) |

Without attributes, the kind derives from the enum name (`BadRequestError` → `bad-request`, `NotFoundError` → `not-found`, ...). Unknown names produce a compile error with actionable hints instead of a panic.

## How It Works

### Error kind mapping

| Enum name (or `kind` value) | Mapped `ErrorKind` | HTTP | Loco `Error` variant |
| :--- | :--- | :--- | :--- |
| `...BadRequestError` / `bad-request` | `ErrorKind::BadRequest` | 400 | `BadRequest` (with `{"code", "message"}` envelope) |
| `...UnauthorizedError` / `unauthorized` | `ErrorKind::Unauthorized` | 401 | `Unauthorized` |
| `...ForbiddenError` / `forbidden` | `ErrorKind::Forbidden` | 403 | `CustomError` |
| `...NotFoundError` / `not-found` | `ErrorKind::NotFound` | 404 | `NotFound` |
| `...ConflictError` / `conflict` | `ErrorKind::Conflict` | 409 | `CustomError` |
| `...UnprocessableEntityError` / `unprocessable-entity` | `ErrorKind::UnprocessableEntity` | 422 | `CustomError` |
| `...TooManyRequestsError` / `too-many-requests` | `ErrorKind::TooManyRequests` | 429 | `CustomError` |
| `...InternalServerErrorError` / `internal-server-error` | `ErrorKind::InternalServerError` | 500 | `InternalServerError` |
| `...ServiceUnavailableError` / `service-unavailable` | `ErrorKind::ServiceUnavailable` | 503 | `CustomError` |
| `#[localized(status = NNN)]` | `ErrorKind::Custom` | NNN | `CustomError` (with `ErrorDetail`) |

### Field interpolation

Named fields are forwarded to `rust_i18n::t!` as named parameters:

```rust,ignore
UserError::TooManyAttempts { attempts: 3 }.to_string();
// key: `errors.bad-request.too-many-attempts`
// translation: "Too many attempts: %{attempts}" → "Too many attempts: 3"
```

> [!NOTE]
> Unit and tuple variants render their translation without parameters — rust-i18n's interpolation requires named parameters, so use named fields for dynamic values. The README of previous versions incorrectly claimed tuple-variant interpolation; this limitation is now documented.

### Trait surface

The derive implements `Display`, `std::error::Error`, `IntoResponse`, `From<E> for loco_rs::Error`, and the `LocalizedApiError` trait, which exposes:

| Method | Purpose |
| :--- | :--- |
| `error_kind()` | The resolved `ErrorKind` |
| `error_variant()` | Variant name (e.g. `"TooManyAttempts"`) |
| `error_key()` | Full translation key (e.g. `"errors.bad-request.too-many-attempts"`) |
| `error_message()` | Localized message as `Cow<'static, str>` — borrowed when no interpolation is needed |
| `error_params()` | Named fields as JSON-ready `(&str, Value)` pairs (introspection) |
| `status_code()` | HTTP status for the kind |
| `to_json()` / `to_json_string()` / `to_json_bytes()` | Structured payload, streamed in a single pass |
| `to_loco_error()` | Conversion into `loco_rs::Error` |

## Performance

The 0.3.0 iteration made every payload path single-pass (no intermediate
`serde_json::Value` tree, no per-field materialization), removed the per-call
allocation on custom-status kinds and the double serialization in
`IntoResponse`, and cut the dependency graph by 58 crates. Measured numbers
and methodology: [docs/BENCHMARKS.md](docs/BENCHMARKS.md); the design notes
and findings: [docs/PERFORMANCE.md](docs/PERFORMANCE.md).

## 0.3.0 breaking changes

- `ErrorKind::Custom(StatusCode, String)` is now `Custom(StatusCode, Cow<'static, str>)` — the derive emits borrowed descriptions and never allocates; runtime constructors can still own.
- The generated `IntoResponse` now sets `content-type: application/json` (0.2.0 unintentionally served the JSON payload as `text/plain`).
- `ErrorKind` gained the `Forbidden`, `Conflict`, `UnprocessableEntity`, `TooManyRequests` and `ServiceUnavailable` variants (match exhaustively? add the new arms).
- `LocalizedApiError` gained `error_message()` (defaulted) and the hidden `__serialize_params` hook (defaulted); derive users are unaffected.
- `loco_rs` is re-exported without the `with-db` feature: code reaching db types through `locolized_errors::loco_rs` must depend on `loco-rs` directly (Loco apps already do).

## Requirements

- [`loco_rs`](https://crates.io/crates/loco-rs) 1.x — the target web framework (hard dependency, re-exported as `locolized_errors::loco_rs`).
- `rust-i18n` — re-exported as `locolized_errors::rust_i18n`; no direct dependency needed, but your crate must call `i18n!("locales")` (see Quick Start).
- `serde` with `derive` — derive `serde::Serialize` alongside the macro: the trait requires `Serialize` to build the `code`/`message` envelope and structured payloads.

## Development

```bash
cargo test --workspace                # unit + integration + trybuild + doctests
cargo clippy --workspace --all-targets -- -D warnings
cargo llvm-cov --workspace --all-targets   # coverage
cargo bench --bench perf              # criterion: legacy (0.2.0) vs current
```

CI (`.github/workflows/`) runs rustfmt, pedantic clippy (zero warnings, no allows), tests, rustdoc, an MSRV job, and coverage. Releases are automated: `prepare-release.yml` computes the next version from conventional commits with [git-cliff](https://git-cliff.org), regenerates `CHANGELOG.md`, and pushes a `v*` tag; `release.yml` builds, generates the changelog, creates the GitHub release and (optionally) publishes to crates.io.

## License

MIT — see [LICENSE](LICENSE).
