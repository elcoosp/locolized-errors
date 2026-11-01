//! Parsing of the `#[localized(...)]` helper attribute.
//!
//! Both the enum-level and the variant-level attribute models live here. The
//! parser rejects unknown options, duplicate options and conflicting options
//! with spanned [`syn::Error`]s so users get precise compile diagnostics.

use syn::{Attribute, LitInt, LitStr, spanned::Spanned};

/// Options accepted on the enum: `#[localized(kind = ..., status = ...,
/// description = ..., locale = ..., no_into_response)]`.
#[derive(Debug, Default)]
pub(crate) struct ContainerAttrs {
    pub(crate) kind: Option<String>,
    pub(crate) status: Option<u16>,
    pub(crate) description: Option<String>,
    pub(crate) locale: Option<String>,
    pub(crate) no_into_response: bool,
}

/// Options accepted on variants: `#[localized(key = ..., message = ...,
/// status = ..., description = ..., locale = ...)]`.
#[derive(Debug, Default)]
pub(crate) struct VariantAttrs {
    pub(crate) key: Option<String>,
    pub(crate) message: Option<String>,
    pub(crate) status: Option<u16>,
    pub(crate) description: Option<String>,
    pub(crate) locale: Option<String>,
}

const CONTAINER_OPTIONS: &str = "`kind`, `status`, `description`, `locale`, `no_into_response`";
const VARIANT_OPTIONS: &str = "`key`, `message`, `status`, `description`, `locale`";

impl ContainerAttrs {
    /// Parse every `#[localized(...)]` attribute found on the enum.
    ///
    /// # Errors
    ///
    /// Returns a [`syn::Error`] for unknown options, duplicated options or the
    /// conflicting combination of `kind` and `status`.
    pub(crate) fn parse(attrs: &[Attribute]) -> syn::Result<Self> {
        let mut parsed = Self::default();
        let mut conflict_span = None;
        for attribute in attrs.iter().filter(|a| a.path().is_ident("localized")) {
            attribute.parse_nested_meta(|meta| {
                let path = &meta.path;
                if path.is_ident("kind") {
                    let value: LitStr = meta.value()?.parse()?;
                    set_once(&mut parsed.kind, "kind", value.value(), path.span())?;
                    conflict_span = Some(path.span());
                } else if path.is_ident("status") {
                    let status = parse_status(meta.value()?)?;
                    set_once(&mut parsed.status, "status", status, path.span())?;
                    conflict_span = Some(path.span());
                } else if path.is_ident("description") {
                    let value: LitStr = meta.value()?.parse()?;
                    set_once(
                        &mut parsed.description,
                        "description",
                        value.value(),
                        path.span(),
                    )?;
                } else if path.is_ident("locale") {
                    let value: LitStr = meta.value()?.parse()?;
                    set_once(&mut parsed.locale, "locale", value.value(), path.span())?;
                } else if path.is_ident("no_into_response") {
                    if parsed.no_into_response {
                        return Err(syn::Error::new(
                            path.span(),
                            "duplicate `localized(no_into_response)`",
                        ));
                    }
                    parsed.no_into_response = true;
                } else {
                    return Err(syn::Error::new(
                        path.span(),
                        format!("unknown `localized` option; expected one of {CONTAINER_OPTIONS}"),
                    ));
                }
                Ok(())
            })?;
        }
        if parsed.kind.is_some() && parsed.status.is_some() {
            return Err(syn::Error::new(
                conflict_span.unwrap_or_else(proc_macro2::Span::call_site),
                "`localized(kind = ...)` and `localized(status = ...)` are mutually exclusive",
            ));
        }
        Ok(parsed)
    }
}

impl VariantAttrs {
    /// Parse every `#[localized(...)]` attribute found on one variant.
    ///
    /// # Errors
    ///
    /// Returns a [`syn::Error`] for unknown or duplicated options; enum-level
    /// options (`kind`, `no_into_response`) are rejected with a hint.
    pub(crate) fn parse(attrs: &[Attribute]) -> syn::Result<Self> {
        let mut parsed = Self::default();
        for attribute in attrs.iter().filter(|a| a.path().is_ident("localized")) {
            attribute.parse_nested_meta(|meta| {
                let path = &meta.path;
                if path.is_ident("key") {
                    let value: LitStr = meta.value()?.parse()?;
                    set_once(&mut parsed.key, "key", value.value(), path.span())?;
                } else if path.is_ident("message") {
                    let value: LitStr = meta.value()?.parse()?;
                    set_once(&mut parsed.message, "message", value.value(), path.span())?;
                } else if path.is_ident("status") {
                    let status = parse_status(meta.value()?)?;
                    set_once(&mut parsed.status, "status", status, path.span())?;
                } else if path.is_ident("description") {
                    let value: LitStr = meta.value()?.parse()?;
                    set_once(&mut parsed.description, "description", value.value(), path.span())?;
                } else if path.is_ident("locale") {
                    let value: LitStr = meta.value()?.parse()?;
                    set_once(&mut parsed.locale, "locale", value.value(), path.span())?;
                } else if path.is_ident("kind") || path.is_ident("no_into_response") {
                    return Err(syn::Error::new(
                        path.span(),
                        format!("this option is only valid on the enum, not on variants; variant options are {VARIANT_OPTIONS}"),
                    ));
                } else {
                    return Err(syn::Error::new(
                        path.span(),
                        format!("unknown `localized` option; expected one of {VARIANT_OPTIONS}"),
                    ));
                }
                Ok(())
            })?;
        }
        Ok(parsed)
    }
}

/// Store `value` into `slot`, failing when the option was already given.
fn set_once<T>(
    slot: &mut Option<T>,
    name: &str,
    value: T,
    span: proc_macro2::Span,
) -> syn::Result<()> {
    if slot.is_some() {
        return Err(syn::Error::new(
            span,
            format!("duplicate `localized({name} = ...)`"),
        ));
    }
    *slot = Some(value);
    Ok(())
}

/// Parse `status = <HTTP status code>` and validate the `100..=599` range.
fn parse_status(stream: syn::parse::ParseStream<'_>) -> syn::Result<u16> {
    let value: LitInt = stream
        .parse()
        .map_err(|_| syn::Error::new(stream.span(), STATUS_RANGE_MESSAGE))?;
    let status = value
        .base10_parse::<u16>()
        .map_err(|_| syn::Error::new(value.span(), STATUS_RANGE_MESSAGE))?;
    if !(100..=599).contains(&status) {
        return Err(syn::Error::new(value.span(), STATUS_RANGE_MESSAGE));
    }
    Ok(status)
}

const STATUS_RANGE_MESSAGE: &str = "`status` must be an integer HTTP status code in `100..=599`";

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use quote::quote;
    use syn::{Attribute, parse_quote};

    fn container_attrs(tokens: &proc_macro2::TokenStream) -> Vec<Attribute> {
        let input: syn::DeriveInput = parse_quote! {
            #tokens
            enum Example {}
        };
        input.attrs
    }

    fn variant_attrs(tokens: &proc_macro2::TokenStream) -> Vec<Attribute> {
        let variant: syn::Variant = parse_quote! { #tokens First };
        variant.attrs
    }

    #[test]
    fn parses_container_options() {
        let attrs = container_attrs(&quote! {
            #[localized(kind = "bad-request", locale = "fr")]
            #[localized(no_into_response)]
        });
        let parsed = super::ContainerAttrs::parse(&attrs).unwrap();
        assert_eq!(parsed.kind.as_deref(), Some("bad-request"));
        assert_eq!(parsed.locale.as_deref(), Some("fr"));
        assert!(parsed.no_into_response);
        assert!(parsed.status.is_none());
    }

    #[test]
    fn parses_status_and_description() {
        let attrs = container_attrs(
            &quote! { #[localized(status = 418, description = "short and stout")] },
        );
        let parsed = super::ContainerAttrs::parse(&attrs).unwrap();
        assert_eq!(parsed.status, Some(418));
        assert_eq!(parsed.description.as_deref(), Some("short and stout"));
    }

    #[test]
    fn rejects_unknown_container_option() {
        let attrs = container_attrs(&quote! { #[localized(bogus)] });
        let error = super::ContainerAttrs::parse(&attrs).unwrap_err();
        assert!(
            error.to_string().contains("unknown `localized` option"),
            "{error}"
        );
    }

    #[test]
    fn rejects_duplicate_container_option() {
        let attrs =
            container_attrs(&quote! { #[localized(locale = "en")] #[localized(locale = "fr")] });
        let error = super::ContainerAttrs::parse(&attrs).unwrap_err();
        assert!(error.to_string().contains("duplicate"), "{error}");
    }

    #[test]
    fn rejects_duplicate_kind_option() {
        let attrs = container_attrs(
            &quote! { #[localized(kind = "bad-request")] #[localized(kind = "not-found")] },
        );
        let error = super::ContainerAttrs::parse(&attrs).unwrap_err();
        assert!(error.to_string().contains("duplicate"), "{error}");
    }

    #[test]
    fn rejects_duplicate_status_option() {
        let attrs =
            container_attrs(&quote! { #[localized(status = 418)] #[localized(status = 503)] });
        let error = super::ContainerAttrs::parse(&attrs).unwrap_err();
        assert!(error.to_string().contains("duplicate"), "{error}");
    }

    #[test]
    fn rejects_duplicate_description_option() {
        let attrs = container_attrs(
            &quote! { #[localized(description = "a")] #[localized(description = "b")] },
        );
        let error = super::ContainerAttrs::parse(&attrs).unwrap_err();
        assert!(error.to_string().contains("duplicate"), "{error}");
    }

    #[test]
    fn rejects_kind_and_status_conflict() {
        let attrs = container_attrs(&quote! { #[localized(kind = "bad-request", status = 418)] });
        let error = super::ContainerAttrs::parse(&attrs).unwrap_err();
        assert!(error.to_string().contains("mutually exclusive"), "{error}");
    }

    #[test]
    fn rejects_out_of_range_status() {
        let attrs = container_attrs(&quote! { #[localized(status = 99)] });
        let error = super::ContainerAttrs::parse(&attrs).unwrap_err();
        assert!(error.to_string().contains("100..=599"), "{error}");
    }

    #[test]
    fn rejects_non_numeric_status() {
        let attrs = container_attrs(&quote! { #[localized(status = "teapot")] });
        let error = super::ContainerAttrs::parse(&attrs).unwrap_err();
        assert!(
            error.to_string().contains("integer HTTP status code"),
            "{error}"
        );
    }

    #[test]
    fn parses_variant_options() {
        let attrs = variant_attrs(&quote! {
            #[localized(key = "custom-key", message = "static", status = 503, locale = "de")]
        });
        let parsed = super::VariantAttrs::parse(&attrs).unwrap();
        assert_eq!(parsed.key.as_deref(), Some("custom-key"));
        assert_eq!(parsed.message.as_deref(), Some("static"));
        assert_eq!(parsed.status, Some(503));
        assert_eq!(parsed.locale.as_deref(), Some("de"));
    }

    #[test]
    fn rejects_duplicate_no_into_response() {
        let attrs = container_attrs(&quote! {
            #[localized(no_into_response)] #[localized(no_into_response)]
        });
        let error = super::ContainerAttrs::parse(&attrs).unwrap_err();
        assert!(error.to_string().contains("duplicate"), "{error}");
    }

    #[test]
    fn rejects_enum_only_options_on_variants() {
        let attrs = variant_attrs(&quote! { #[localized(kind = "bad-request")] });
        let error = super::VariantAttrs::parse(&attrs).unwrap_err();
        assert!(
            error.to_string().contains("only valid on the enum"),
            "{error}"
        );
    }

    #[test]
    fn rejects_duplicate_variant_options() {
        for tokens in [
            quote! { #[localized(key = "a")] #[localized(key = "b")] },
            quote! { #[localized(message = "m")] #[localized(message = "n")] },
            quote! { #[localized(status = 418)] #[localized(status = 503)] },
            quote! { #[localized(locale = "en")] #[localized(locale = "fr")] },
            quote! { #[localized(description = "x")] #[localized(description = "y")] },
        ] {
            let attrs = variant_attrs(&tokens);
            let error = super::VariantAttrs::parse(&attrs).unwrap_err();
            assert!(error.to_string().contains("duplicate"), "{error}");
        }
    }

    #[test]
    fn rejects_unknown_variant_option() {
        let attrs = variant_attrs(&quote! { #[localized(nope = 1)] });
        let error = super::VariantAttrs::parse(&attrs).unwrap_err();
        assert!(
            error.to_string().contains("unknown `localized` option"),
            "{error}"
        );
    }

    #[test]
    fn ignores_unrelated_attributes() {
        let attrs =
            container_attrs(&quote! { #[derive(Debug)] #[serde(rename_all = "kebab-case")] });
        assert!(super::ContainerAttrs::parse(&attrs).is_ok());
    }
}
