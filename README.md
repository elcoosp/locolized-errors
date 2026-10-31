<div align="center">
  <img src="docs/logo.png" alt="locolized_errors Logo" width="300"/>
  <p>
    <strong>Localized Errors for Loco</strong><br/>
    A Rust macro for generating localized API errors with seamless integration into the Loco web framework.
  </p>
  <p>
    <img src="https://img.shields.io/badge/Rust-1.94%20%7C%202024-000000?style=flat-square&logo=rust" alt="Rust"/>
    <img src="https://img.shields.io/crates/v/locolized_errors?style=flat-square" alt="Crates.io"/>
    <img src="https://img.shields.io/badge/License-MIT-blue?style=flat-square" alt="License MIT"/>
    <img src="https://img.shields.io/badge/Framework-Loco-000000?style=flat-square" alt="Loco Framework"/>
    <img src="https://img.shields.io/badge/i18n-rust__i18n-FF6F00?style=flat-square" alt="rust_i18n"/>
  </p>
</div>

---

`locolized_errors` eliminates boilerplate when building internationalized APIs in Rust. By simply deriving a macro on your error enums, you get automatic error kind detection, field interpolation, and direct conversion to Loco's native error types.

## Features

- **Automatic Error Kind Detection**: Derives HTTP error types directly from enum names (e.g., `BadRequestError` → `ErrorKind::BadRequest`).
- **Internationalization Ready**: Built-in integration with `rust_i18n` for localized, parameterized error messages.
- **Seamless Loco Integration**: Direct `Into<loco_rs::prelude::Error>` conversion for clean controller logic.
- **Flexible Variants**: Full support for unit, named, and tuple variants with automatic field interpolation.

## Installation

Add `locolized_errors` to your `Cargo.toml`:

```toml
[dependencies]
locolized_errors = "0.1"
```

## Quick Start

### 1. Define Your Errors

Derive `LocalizedApiError` on your custom error enums. Named fields will automatically be available for message interpolation.

```rust
use locolized_errors::LocalizedApiError;

#[derive(Debug, LocalizedApiError)]
pub enum BadRequestError {
    SwitchAlreadyTriggered { switch_id: i32 },
    ParseError {
        field: String,
        value: String,
        target_type: String,
    },
    UserNotFound,
    EmailAlreadyExists,
}

#[derive(Debug, LocalizedApiError)]
pub enum UnauthorizedError {
    InvalidCredentials,
    SessionExpired,
}
```

### 2. Configure Localization

Create your translation files in the `locales` directory. The macro expects keys matching the pattern `errors.<kebab-case-kind>.<kebab-case-variant>`.

**`locales/en.yml`**
```yaml
errors:
  bad-request:
    switch-already-triggered: "Switch {{ switch_id }} has already been triggered"
    parse-error: "Failed to parse field '{{ field }}' with value '{{ value }}' as {{ target_type }}"
    user-not-found: "User not found"
    email-already-exists: "Email already exists"
  
  unauthorized:
    invalid-credentials: "Invalid credentials"
    session-expired: "Session has expired"
```

**`locales/fr.yml`**
```yaml
errors:
  bad-request:
    switch-already-triggered: "L'interrupteur {{ switch_id }} a déjà été déclenché"
    user-not-found: "Utilisateur non trouvé"
    # ...
```

### 3. Use in Loco Controllers

Return your custom errors directly from your handlers. The macro implements `From<YourError> for loco_rs::prelude::Error`, allowing clean `.into()` conversions.

```rust
use loco_rs::prelude::*;
use crate::errors::BadRequestError; // Your defined enum

pub async fn create_user(
    State(ctx): State<AppContext>,
    Json(params): Json<CreateUserRequest>,
) -> Result<Json<UserResponse>> {
    if params.email.is_empty() {
        return Err(BadRequestError::ParseError {
            field: "email".to_string(),
            value: "".to_string(),
            target_type: "non-empty string".to_string(),
        }.into()); // Automatically converts to Loco Error
    }
    
    if User::find_by_email(&ctx.db, &params.email).await.is_ok() {
        return Err(BadRequestError::EmailAlreadyExists.into());
    }
    
    // ... create user logic
    Ok(Json(user.into()))
}
```

> [!TIP]
> **Explicit Conversion**  
> If you prefer explicit over implicit, you can also call `.to_loco_error()` instead of `.into()`:
> ```rust
> return Err(BadRequestError::UserNotFound.to_loco_error());
> ```

## How It Works

### Error Kind Mapping
The macro automatically maps standard error enum suffixes to Loco's `ErrorKind`:

| Enum Name Suffix | Mapped `ErrorKind` |
| :--- | :--- |
| `BadRequestError` | `ErrorKind::BadRequest` |
| `UnauthorizedError` | `ErrorKind::Unauthorized` |
| `NotFoundError` | `ErrorKind::NotFound` |
| `InternalServerError` | `ErrorKind::InternalServerError` |

### Field Interpolation
Named fields in your enum variants are automatically extracted and passed to the `rust_i18n` translation engine.

> [!NOTE]
> **Interpolation Example**  
> Given the error:
> ```rust
> BadRequestError::ParseError {
>     field: "age".to_string(),
>     value: "invalid".to_string(), 
>     target_type: "number".to_string(),
> }
> ```
> The macro looks up the key `errors.bad-request.parse-error` and injects the variables `{{ field }}`, `{{ value }}`, and `{{ target_type }}` into the localized string.

## Requirements

To use this crate effectively, ensure your project includes:
- [`loco_rs`](https://crates.io/crates/loco-rs): The target web framework.
- [`rust_i18n`](https://crates.io/crates/rust_i18n): For localization management.
- [`convert_case`](https://crates.io/crates/convert_case): Used internally by the macro for kebab-case conversion.
