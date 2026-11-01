//! Proc-macro implementation crate for [`locolized_errors`](https://docs.rs/locolized-errors).
//!
//! This crate is not meant to be used directly: import the
//! [`LocalizedApiError`](https://docs.rs/locolized_errors/latest/locolized_errors/derive.LocalizedApiError.html)
//! derive macro from the `locolized_errors` crate instead.
//!
//! The entry point below is intentionally thin: every piece of logic lives in
//! pure, unit-tested private modules:
//!
//! - `case`: identifier -> kebab-case conversion;
//! - `attrs`: the `#[localized(...)]` attribute model;
//! - `kind`: error-kind resolution;
//! - `expand`: pure `&DeriveInput -> TokenStream` code generation.

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

mod attrs;
mod case;
mod expand;
mod kind;

/// Derive `LocalizedApiError` on an error enum to get:
///
/// - `Display` rendering the localized message (`rust_i18n::t!`) for the
///   variant, with named fields interpolated as `%{field}` placeholders;
/// - `std::error::Error`;
/// - `locolized_errors::LocalizedApiError` (kind, key, params, JSON payload,
///   conversion to `loco_rs::Error`);
/// - `From<E> for loco_rs::Error` and `IntoResponse`, unless
///   `#[localized(no_into_response)]` is given.
///
/// Configure with the `#[localized(...)]` helper attribute:
///
/// ```ignore
/// #[derive(Debug, serde::Serialize, LocalizedApiError)]
/// #[localized(kind = "bad-request")]           // or: status = 418
/// pub enum UserError {
///     #[localized(key = "user-not-found")]     // custom i18n key segment
///     NotFound,
///     #[localized(message = "Too many attempts: %{attempts}")]
///     TooManyAttempts { attempts: u32 },       // `%{attempts}` interpolation
/// }
/// ```
///
/// # Panics
///
/// Never: every failure is reported as a spanned compile error.
#[proc_macro_derive(LocalizedApiError, attributes(localized))]
pub fn derive_localized_api_error(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand::expand(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
