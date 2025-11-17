# Localized Errors for Loco

A Rust macro for generating localized API errors with seamless integration into the Loco web framework.

## Features

- **Automatic Error Kind Detection**: Derives error types from enum names (e.g., `BadRequestError` becomes `bad-request`)
- **Internationalization Support**: Built-in integration with `rust_i18n` for localized error messages
- **Loco Framework Ready**: Direct conversion to Loco's error types
- **Flexible Error Variants**: Support for unit, named, and tuple variants with field interpolation

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
locolized_errors = "0.1"
```

## Usage

### Basic Example

```rust
use locolized_errors::LocalizedApiError;

#[derive(Debug, LocalizedApiError)]
pub enum BadRequestError {
    SwitchAlreadyTriggered {
        switch_id: i32,
    },
    SwitchNotFound {
        switch_id: i32,
    },
    UserNotFound,
    InvalidToken,
}
```

### Advanced Example with Multiple Error Types

```rust
#[derive(Debug, LocalizedApiError)]
pub enum BadRequestError {
    SwitchAlreadyTriggered { switch_id: i32 },
    SwitchNotFound { switch_id: i32 },
    SwitchNotActive { switch_id: i32 },
    ParseError {
        field: String,
        value: String,
        target_type: String,
    },
    UserNotFound,
    UserAlreadyRecipient,
    InvalidToken,
    TokenExpired,
    InvalidInvitation,
    InvitationSentToDifferentEmail,
    InvitationExpired,
    EmailAlreadyExists,
    PendingInvitationExists,
}

#[derive(Debug, LocalizedApiError)]
pub enum UnauthorizedError {
    InvalidCredentials,
    SessionExpired,
}

#[derive(Debug, LocalizedApiError)]
pub enum NotFoundError {
    UserNotFound { user_id: i32 },
    ResourceNotFound { resource_id: String },
}
```

## Localization Files

Create localization files in your `locales` directory:

### `en.yml`
```yaml
errors:
  bad-request:
    switch-already-triggered: "Switch {{ switch_id }} has already been triggered"
    switch-not-found: "Switch {{ switch_id }} not found"
    switch-not-active: "Switch {{ switch_id }} is not active"
    parse-error: "Failed to parse field '{{ field }}' with value '{{ value }}' as {{ target_type }}"
    user-not-found: "User not found"
    user-already-recipient: "User is already a recipient"
    invalid-token: "Invalid token"
    token-expired: "Token has expired"
    invalid-invitation: "Invalid invitation"
    invitation-sent-to-different-email: "Invitation was sent to a different email"
    invitation-expired: "Invitation has expired"
    email-already-exists: "Email already exists"
    pending-invitation-exists: "Pending invitation already exists"
  
  unauthorized:
    invalid-credentials: "Invalid credentials"
    session-expired: "Session has expired"
  
  not-found:
    user-not-found: "User {{ user_id }} not found"
    resource-not-found: "Resource {{ resource_id }} not found"
```

### `fr.yml`
```yaml
errors:
  bad-request:
    switch-already-triggered: "L'interrupteur {{ switch_id }} a déjà été déclenché"
    user-not-found: "Utilisateur non trouvé"
    # ... other French translations
```

## Integration with Loco Controllers

```rust
use loco_rs::prelude::*;
use locolized_errors::LocalizedApiError;

pub async fn create_user(
    State(ctx): State<AppContext>,
    Json(params): Json<CreateUserRequest>,
) -> Result<Json<UserResponse>> {
    // Validate input
    if params.email.is_empty() {
        return Err(BadRequestError::ParseError {
            field: "email".to_string(),
            value: "".to_string(),
            target_type: "non-empty string".to_string(),
        }.into());
    }
    
    // Check if user exists
    if User::find_by_email(&ctx.db, &params.email).await.is_ok() {
        return Err(BadRequestError::EmailAlreadyExists.into());
    }
    
    // Create user logic...
    Ok(Json(user.into()))
}
```

## Error Conversion

The macro automatically implements `From<YourError> for loco_rs::prelude::Error`:

```rust
// These are equivalent:
return Err(BadRequestError::UserNotFound.into());
// or
return Err(BadRequestError::UserNotFound.to_loco_error());
```

## Error Kinds

The macro automatically maps enum names to error kinds:

- `BadRequestError` → `ErrorKind::BadRequest`
- `UnauthorizedError` → `ErrorKind::Unauthorized` 
- `NotFoundError` → `ErrorKind::NotFound`
- `InternalServerError` → `ErrorKind::InternalServerError`

## Field Interpolation

Named fields in error variants are automatically interpolated into localization messages:

```rust
// This error:
BadRequestError::ParseError {
    field: "age".to_string(),
    value: "invalid".to_string(), 
    target_type: "number".to_string(),
}

// Uses this localization key with field interpolation:
// errors.bad-request.parse-error
// With values: field = "age", value = "invalid", target_type = "number"
```

## Requirements

- `rust_i18n` for localization
- `loco_rs` framework
- `convert_case` for case conversion

## License

MIT
