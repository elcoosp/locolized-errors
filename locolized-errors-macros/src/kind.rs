//! Resolution of the error kind for a derived enum.
//!
//! The kind is determined in three ways, in order of precedence:
//!
//! 1. explicit `#[localized(kind = "...")]` naming a known kind;
//! 2. explicit `#[localized(status = NNN)]` producing a custom kind;
//! 3. the enum name itself, after stripping a trailing `Error` suffix and
//!    converting to kebab-case (`BadRequestError` -> `bad-request`).

use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

use crate::attrs::ContainerAttrs;
use crate::case::to_kebab_case;

/// The four HTTP error kinds with a fixed mapping to loco's `Error` variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KnownKind {
    BadRequest,
    Unauthorized,
    NotFound,
    InternalServerError,
}

/// All kebab-case names accepted by `#[localized(kind = "...")]`, in the order
/// they are listed in diagnostics.
pub(crate) const KNOWN_KIND_KEYS: [&str; 4] = [
    "bad-request",
    "unauthorized",
    "not-found",
    "internal-server-error",
];

impl KnownKind {
    /// Look up a known kind by its kebab-case name.
    #[must_use]
    pub(crate) fn from_kebab(key: &str) -> Option<Self> {
        match key {
            "bad-request" => Some(Self::BadRequest),
            "unauthorized" => Some(Self::Unauthorized),
            "not-found" => Some(Self::NotFound),
            "internal-server-error" => Some(Self::InternalServerError),
            _ => None,
        }
    }

    /// The kebab-case name, also used as the i18n key segment and as the
    /// serialized form of [`locolized_errors::ErrorKind`].
    #[must_use]
    pub(crate) fn kebab(self) -> &'static str {
        match self {
            Self::BadRequest => "bad-request",
            Self::Unauthorized => "unauthorized",
            Self::NotFound => "not-found",
            Self::InternalServerError => "internal-server-error",
        }
    }

    /// Expression evaluating to the `locolized_errors::ErrorKind` variant.
    pub(crate) fn kind_expr(self) -> TokenStream {
        match self {
            Self::BadRequest => quote! { ::locolized_errors::ErrorKind::BadRequest },
            Self::Unauthorized => quote! { ::locolized_errors::ErrorKind::Unauthorized },
            Self::NotFound => quote! { ::locolized_errors::ErrorKind::NotFound },
            Self::InternalServerError => {
                quote! { ::locolized_errors::ErrorKind::InternalServerError }
            }
        }
    }
}

/// The resolved kind plan for a derived enum: the i18n key prefix plus the
/// default `ErrorKind` expression for its variants.
#[derive(Debug)]
pub(crate) struct KindPlan {
    /// kebab-case segment used as `errors.<key_prefix>.<variant>` key prefix.
    pub(crate) key_prefix: String,
    /// Expression evaluating to the default `ErrorKind` for variants.
    pub(crate) kind_expr: TokenStream,
}

/// Resolve the [`KindPlan`] for a derived enum.
///
/// # Errors
///
/// Returns a [`syn::Error`] pointing at `enum_ident` when the enum name does
/// not map to a known kind and no `#[localized(...)]` attribute resolves the
/// ambiguity, or when `kind` names something unknown.
pub(crate) fn resolve_kind(
    enum_ident: &Ident,
    container: &ContainerAttrs,
) -> syn::Result<KindPlan> {
    if let Some(kind_key) = container.kind.as_deref() {
        let Some(known) = KnownKind::from_kebab(kind_key) else {
            return Err(syn::Error::new(
                enum_ident.span(),
                format!(
                    "unknown `localized(kind = {kind_key:?})`; expected one of {} — \
                     use `localized(status = ...)` for a custom HTTP status",
                    KNOWN_KIND_KEYS.join(", "),
                ),
            ));
        };
        return Ok(KindPlan {
            key_prefix: known.kebab().to_owned(),
            kind_expr: known.kind_expr(),
        });
    }

    let derived = to_kebab_case(strip_error_suffix(&enum_ident.to_string()));

    if let Some(status) = container.status {
        return Ok(KindPlan {
            key_prefix: derived,
            kind_expr: custom_kind_expr(status, container.description.as_deref()),
        });
    }

    let Some(known) = KnownKind::from_kebab(&derived) else {
        return Err(syn::Error::new(
            enum_ident.span(),
            format!(
                "cannot determine the error kind of `{enum_ident}`: the kebab-cased name \
                 `{derived}` is not a known kind. Rename the enum (e.g. `...BadRequestError`), \
                 or annotate it with `#[localized(kind = \"...\")]` (one of {}) or \
                 `#[localized(status = ...)]`",
                KNOWN_KIND_KEYS.join(", "),
            ),
        ));
    };
    Ok(KindPlan {
        key_prefix: known.kebab().to_owned(),
        kind_expr: known.kind_expr(),
    })
}

/// Expression for `ErrorKind::Custom(status, description)`.
///
/// The macro has validated `status` to be in `100..=599`, so the runtime
/// helper cannot fall back in practice.
pub(crate) fn custom_kind_expr(status: u16, description: Option<&str>) -> TokenStream {
    let description = description.unwrap_or("");
    quote! {
        ::locolized_errors::ErrorKind::Custom(
            ::locolized_errors::__status(#status),
            ::std::string::String::from(#description),
        )
    }
}

/// Strip a trailing `Error` suffix: `BadRequestError` -> `BadRequest`.
fn strip_error_suffix(name: &str) -> &str {
    name.strip_suffix("Error").unwrap_or(name)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::{KNOWN_KIND_KEYS, KnownKind, resolve_kind, strip_error_suffix};
    use crate::attrs::ContainerAttrs;
    use quote::quote;
    use syn::parse_quote;

    fn resolve_for(
        name: &str,
        tokens: &proc_macro2::TokenStream,
    ) -> Result<super::KindPlan, syn::Error> {
        let ident = syn::Ident::new(name, proc_macro2::Span::call_site());
        let input: syn::DeriveInput = parse_quote! {
            #tokens
            enum #ident {}
        };
        let container = ContainerAttrs::parse(&input.attrs)?;
        resolve_kind(&input.ident, &container)
    }

    #[test]
    fn known_kind_round_trip() {
        for key in KNOWN_KIND_KEYS {
            let kind = KnownKind::from_kebab(key).unwrap();
            assert_eq!(kind.kebab(), key);
        }
        assert!(KnownKind::from_kebab("teapot").is_none());
    }

    #[test]
    fn strips_error_suffix() {
        assert_eq!(strip_error_suffix("BadRequestError"), "BadRequest");
        assert_eq!(strip_error_suffix("Unauthorized"), "Unauthorized");
        assert_eq!(strip_error_suffix("Error"), "");
    }

    #[test]
    fn derives_kind_from_enum_name() {
        for (name, expected) in [
            ("BadRequestError", "bad-request"),
            ("UnauthorizedError", "unauthorized"),
            ("NotFoundError", "not-found"),
            ("InternalServerErrorError", "internal-server-error"),
            ("Unauthorized", "unauthorized"),
        ] {
            let plan = resolve_for(name, &quote! {}).unwrap();
            assert_eq!(plan.key_prefix, expected, "enum: {name}");
        }
    }

    #[test]
    fn kind_attribute_wins_over_name() {
        let plan = resolve_for(
            "SomethingElse",
            &quote! { #[localized(kind = "not-found")] },
        )
        .unwrap();
        assert_eq!(plan.key_prefix, "not-found");
    }

    #[test]
    fn status_attribute_creates_custom_kind() {
        let plan = resolve_for(
            "TeapotError",
            &quote! { #[localized(status = 418, description = "stout")] },
        )
        .unwrap();
        assert_eq!(plan.key_prefix, "teapot");
        let rendered = plan.kind_expr.to_string();
        assert!(rendered.contains("__status (418"), "{rendered}");
        assert!(rendered.contains("\"stout\""), "{rendered}");
    }

    #[test]
    fn unknown_name_is_a_friendly_error() {
        let error = resolve_for("TeapotError", &quote! {}).unwrap_err();
        let message = error.to_string();
        assert!(
            message.contains("cannot determine the error kind"),
            "{message}"
        );
        assert!(message.contains("teapot"), "{message}");
        assert!(message.contains("localized(kind"), "{message}");
    }

    #[test]
    fn unknown_kind_attribute_is_an_error() {
        let error =
            resolve_for("TeapotError", &quote! { #[localized(kind = "teapot")] }).unwrap_err();
        assert!(
            error.to_string().contains("unknown `localized(kind"),
            "{error}"
        );
    }

    #[test]
    fn custom_kind_expr_without_description() {
        let expr = super::custom_kind_expr(503, None).to_string();
        assert!(expr.contains("\"\""), "{expr}");
    }
}
