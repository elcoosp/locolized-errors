//! Locale-related behavior in its own test binary: `rust_i18n::set_locale`
//! is process-global, so pinned-locale tests must not race other tests.

use locolized_errors::rust_i18n;
use locolized_errors::{ErrorKind, LocalizedApiError};
use serde::Serialize;

rust_i18n::i18n!("locales", fallback = "en");

/// Enum-level locale pinning: every message renders in French regardless of
/// the process locale.
#[derive(Debug, Serialize, LocalizedApiError)]
#[localized(kind = "bad-request", locale = "fr")]
enum FrenchOnly {
    UserNotFound,
    TooManyAttempts { attempts: u32 },
}

/// Variant-level pinning wins over the enum-level default.
#[derive(Debug, Serialize, LocalizedApiError)]
#[localized(kind = "internal-server-error", locale = "fr")]
enum MixedLocales {
    VaultSealed,
    #[localized(locale = "en")]
    Boom,
}

#[test]
fn enum_level_locale_pins_translations() {
    rust_i18n::set_locale("en");
    assert_eq!(
        FrenchOnly::UserNotFound.to_string(),
        "Utilisateur introuvable"
    );
    assert_eq!(
        FrenchOnly::TooManyAttempts { attempts: 2 }.to_string(),
        "Trop de tentatives : 2"
    );
}

#[test]
fn variant_locale_overrides_enum_locale() {
    rust_i18n::set_locale("en");
    assert_eq!(
        MixedLocales::VaultSealed.to_string(),
        "Le coffre est scellé"
    );
    assert_eq!(
        MixedLocales::Boom.to_string(),
        "Something went wrong on our side"
    );
}

#[test]
fn fallback_locale_covers_missing_translations() {
    rust_i18n::set_locale("fr");
    // `errors.bad-request.email-already-exists` only exists in `en`.
    assert_eq!(
        BadRequestError::EmailAlreadyExists.to_string(),
        "Email already exists"
    );
    assert_eq!(
        BadRequestError::EmailAlreadyExists.error_kind(),
        ErrorKind::BadRequest
    );
}

/// Deliberately uses a key that only exists in the fallback locale.
#[derive(Debug, Serialize, LocalizedApiError)]
enum BadRequestError {
    EmailAlreadyExists,
}
