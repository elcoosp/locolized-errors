//! Code generation: turns a parsed [`syn::DeriveInput`] into the impl blocks
//! for `Display`, `std::error::Error`, `LocalizedApiError`, `From<E> for
//! loco_rs::Error` and (unless opted out) `IntoResponse`.
//!
//! `expand` is a pure function over `&DeriveInput`, which makes it directly
//! unit-testable without invoking rustc.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Ident, LitStr, Variant};

use crate::attrs::{ContainerAttrs, VariantAttrs};
use crate::case::to_kebab_case;
use crate::kind::{KindPlan, custom_kind_expr, resolve_kind};

/// The five match arms generated for one variant.
struct VariantArms {
    display: TokenStream,
    kind: TokenStream,
    variant: TokenStream,
    key: TokenStream,
    params: TokenStream,
}

/// Expand the `LocalizedApiError` derive for one input.
///
/// # Errors
///
/// Returns a [`syn::Error`] when the input is not an enum, when attributes are
/// invalid, or when the error kind cannot be resolved. The caller converts the
/// error into a `compile_error!` invocation.
pub(crate) fn expand(input: &DeriveInput) -> syn::Result<TokenStream> {
    let enum_ident = &input.ident;
    let Data::Enum(data_enum) = &input.data else {
        return Err(syn::Error::new_spanned(
            enum_ident,
            "`LocalizedApiError` can only be derived for enums",
        ));
    };

    let container = ContainerAttrs::parse(&input.attrs)?;
    let plan = resolve_kind(enum_ident, &container)?;

    let mut display_arms = Vec::with_capacity(data_enum.variants.len());
    let mut kind_arms = Vec::with_capacity(data_enum.variants.len());
    let mut name_arms = Vec::with_capacity(data_enum.variants.len());
    let mut key_arms = Vec::with_capacity(data_enum.variants.len());
    let mut param_arms = Vec::with_capacity(data_enum.variants.len());

    for variant in &data_enum.variants {
        let arms = variant_arms(variant, &plan, &container)?;
        display_arms.push(arms.display);
        kind_arms.push(arms.kind);
        name_arms.push(arms.variant);
        key_arms.push(arms.key);
        param_arms.push(arms.params);
    }

    let into_response_impl = (!container.no_into_response).then(|| {
        quote! {
            #[automatically_derived]
            impl ::locolized_errors::loco_rs::prelude::IntoResponse for #enum_ident {
                fn into_response(
                    self,
                ) -> ::locolized_errors::loco_rs::prelude::Response {
                    use ::locolized_errors::LocalizedApiError as _;
                    use ::locolized_errors::loco_rs::prelude::IntoResponse as _;
                    (self.status_code(), self.to_json_string()).into_response()
                }
            }
        }
    });

    Ok(quote! {
        #[automatically_derived]
        impl ::core::fmt::Display for #enum_ident {
            fn fmt(&self, __formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                match self {
                    #(#display_arms)*
                }
            }
        }

        #[automatically_derived]
        impl ::core::error::Error for #enum_ident {}

        #[automatically_derived]
        impl ::locolized_errors::LocalizedApiError for #enum_ident {
            #[inline]
            fn error_kind(&self) -> ::locolized_errors::ErrorKind {
                match self {
                    #(#kind_arms)*
                }
            }

            #[inline]
            fn error_variant(&self) -> &'static ::core::primitive::str {
                match self {
                    #(#name_arms)*
                }
            }

            #[inline]
            fn error_key(&self) -> &'static ::core::primitive::str {
                match self {
                    #(#key_arms)*
                }
            }

            #[inline]
            fn error_params(
                &self,
            ) -> ::std::vec::Vec<(&'static ::core::primitive::str, ::locolized_errors::serde_json::Value)> {
                match self {
                    #(#param_arms)*
                }
            }
        }

        #[automatically_derived]
        impl ::core::convert::From<#enum_ident> for ::locolized_errors::loco_rs::errors::Error {
            fn from(error: #enum_ident) -> Self {
                use ::locolized_errors::LocalizedApiError as _;
                error.to_loco_error()
            }
        }

        #into_response_impl
    })
}

/// Generate the five match arms for one variant.
fn variant_arms(
    variant: &Variant,
    plan: &KindPlan,
    container: &ContainerAttrs,
) -> syn::Result<VariantArms> {
    let attrs = VariantAttrs::parse(&variant.attrs)?;
    let variant_ident = &variant.ident;
    let variant_name = variant_ident.to_string();
    let key_segment = attrs
        .key
        .clone()
        .unwrap_or_else(|| to_kebab_case(&variant_name));
    let full_key = format!("errors.{}.{}", plan.key_prefix, key_segment);
    let full_key_literal = LitStr::new(&full_key, variant_ident.span());

    let kind_expr = attrs.status.map_or_else(
        || plan.kind_expr.clone(),
        |status| {
            custom_kind_expr(
                status,
                attrs
                    .description
                    .as_deref()
                    .or(container.description.as_deref()),
            )
        },
    );
    let kind_arm = wildcard_arm(variant_ident, &variant.fields, &kind_expr);

    let variant_name_literal = LitStr::new(&variant_name, variant_ident.span());
    let variant_arm = wildcard_arm(
        variant_ident,
        &variant.fields,
        &quote! { #variant_name_literal },
    );

    let key_arm = wildcard_arm(
        variant_ident,
        &variant.fields,
        &quote! { #full_key_literal },
    );
    let display_arm = display_arm(
        variant_ident,
        &variant.fields,
        &attrs,
        &full_key,
        container.locale.as_deref(),
    );

    Ok(VariantArms {
        display: display_arm,
        kind: kind_arm,
        variant: variant_arm,
        key: key_arm,
        params: params_arm(variant_ident, &variant.fields),
    })
}

/// Build the `Display` match arm for one variant.
///
/// - unit and tuple variants render their (non-interpolated) translation;
/// - named fields are forwarded to `t!` as named parameters;
/// - a `message = "..."` override replaces the `t!` lookup entirely.
fn display_arm(
    variant_ident: &Ident,
    fields: &Fields,
    attrs: &VariantAttrs,
    full_key: &str,
    container_locale: Option<&str>,
) -> TokenStream {
    let body = match attrs.message.as_deref() {
        Some(message) => {
            let literal = LitStr::new(message, variant_ident.span());
            quote! { #literal }
        }
        None => t_call(
            full_key,
            attrs.locale.as_deref().or(container_locale),
            &interpolation_args(fields),
        ),
    };
    match fields {
        Fields::Unit => {
            quote! { Self::#variant_ident => ::core::write!(__formatter, "{}", #body), }
        }
        Fields::Named(fields_named) => {
            let idents: Vec<&Ident> = fields_named
                .named
                .iter()
                .filter_map(|field| field.ident.as_ref())
                .collect();
            if attrs.message.is_some() {
                quote! { Self::#variant_ident { .. } => ::core::write!(__formatter, "{}", #body), }
            } else {
                quote! {
                    Self::#variant_ident { #(#idents),* } =>
                        ::core::write!(__formatter, "{}", #body),
                }
            }
        }
        Fields::Unnamed(fields_unnamed) => {
            let wildcards: Vec<TokenStream> = (0..fields_unnamed.unnamed.len())
                .map(|_| quote! { _ })
                .collect();
            quote! { Self::#variant_ident(#(#wildcards),*) => ::core::write!(__formatter, "{}", #body), }
        }
    }
}

/// Named interpolation arguments for `t!`: one per named field, matching the
/// `%{field_name}` placeholders in the translation file.
fn interpolation_args(fields: &Fields) -> Vec<TokenStream> {
    match fields {
        Fields::Named(fields_named) => fields_named
            .named
            .iter()
            .filter_map(|field| field.ident.as_ref())
            .map(|ident| quote! { #ident = #ident })
            .collect(),
        _ => Vec::new(),
    }
}

/// Build the `rust_i18n::t!` invocation for a key, optional locale and named
/// arguments. The path goes through the `locolized_errors` re-export so
/// downstream crates do not need `rust-i18n` as a direct dependency; the
/// macro still expands inside the *user's* crate, which is where `i18n!`
/// must have been initialized.
fn t_call(key: &str, locale: Option<&str>, args: &[TokenStream]) -> TokenStream {
    let key_literal = LitStr::new(key, proc_macro2::Span::call_site());
    match locale {
        Some(locale) => {
            let locale_literal = LitStr::new(locale, proc_macro2::Span::call_site());
            quote! { ::locolized_errors::rust_i18n::t!(#key_literal, locale = #locale_literal, #(#args,)*) }
        }
        None if args.is_empty() => quote! { ::locolized_errors::rust_i18n::t!(#key_literal) },
        None => quote! { ::locolized_errors::rust_i18n::t!(#key_literal, #(#args,)*) },
    }
}

/// Match arm binding nothing: `=> expr` for every variant shape.
fn wildcard_arm(variant_ident: &Ident, fields: &Fields, expr: &TokenStream) -> TokenStream {
    let pattern = match fields {
        Fields::Unit => quote! { Self::#variant_ident },
        Fields::Named(_) => quote! { Self::#variant_ident { .. } },
        Fields::Unnamed(_) => quote! { Self::#variant_ident (..) },
    };
    quote! { #pattern => #expr, }
}

/// Match arm producing the JSON-ready parameter list: named fields serialize
/// to `("field", Value)` pairs; unit and tuple variants carry no parameters.
fn params_arm(variant_ident: &Ident, fields: &Fields) -> TokenStream {
    match fields {
        Fields::Named(fields_named) => {
            let idents: Vec<&Ident> = fields_named
                .named
                .iter()
                .filter_map(|field| field.ident.as_ref())
                .collect();
            let names = idents
                .iter()
                .map(|ident| LitStr::new(&ident.to_string(), ident.span()));
            quote! {
                Self::#variant_ident { #(#idents),* } => ::std::vec![
                    #((#names, ::locolized_errors::serde_json::to_value(#idents).unwrap_or_default())),*
                ],
            }
        }
        Fields::Unit => quote! { Self::#variant_ident => ::std::vec::Vec::new(), },
        Fields::Unnamed(_) => quote! { Self::#variant_ident (..) => ::std::vec::Vec::new(), },
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::expand;
    use quote::quote;
    use syn::parse_quote;

    fn expand_src(tokens: &proc_macro2::TokenStream) -> Result<String, syn::Error> {
        let input: syn::DeriveInput = parse_quote! { #tokens };
        expand(&input).map(|stream| stream.to_string())
    }

    #[test]
    fn generates_all_impls_for_named_variants() {
        let output = expand_src(&quote! {
            #[derive(Debug, serde::Serialize)]
            pub enum BadRequestError {
                SwitchAlreadyTriggered { switch_id: i32 },
                UserNotFound,
            }
        })
        .unwrap();
        for fragment in [
            "impl :: core :: fmt :: Display for BadRequestError",
            "impl :: core :: error :: Error for BadRequestError",
            "impl :: locolized_errors :: LocalizedApiError for BadRequestError",
            "impl :: core :: convert :: From < BadRequestError > for :: locolized_errors :: loco_rs :: errors :: Error",
            "impl :: locolized_errors :: loco_rs :: prelude :: IntoResponse for BadRequestError",
            "\"errors.bad-request.switch-already-triggered\"",
            "\"errors.bad-request.user-not-found\"",
            "switch_id = switch_id",
        ] {
            assert!(
                output.contains(fragment),
                "missing {fragment} in:\n{output}"
            );
        }
    }

    #[test]
    fn tuple_variants_render_without_interpolation() {
        let output = expand_src(&quote! {
            #[derive(Debug, serde::Serialize)]
            pub enum BadRequestError { Invalid(String), Pair(String, u32) }
        })
        .unwrap();
        assert!(output.contains("Self :: Invalid (_) =>"), "{output}");
        assert!(output.contains("Self :: Pair (_ , _) =>"), "{output}");
        assert!(output.contains("Self :: Invalid (..) =>"), "{output}");
        assert!(
            !output.contains("to_value"),
            "tuple fields must not become params: {output}"
        );
    }

    #[test]
    fn unit_variant_params_are_empty() {
        let output = expand_src(&quote! {
            #[derive(Debug, serde::Serialize)]
            pub enum NotFoundError { NothingHere }
        })
        .unwrap();
        assert!(
            output.contains("Self :: NothingHere => :: std :: vec :: Vec :: new ()"),
            "{output}"
        );
    }

    #[test]
    fn named_variant_params_serialize_fields() {
        let output = expand_src(&quote! {
            #[derive(Debug, serde::Serialize)]
            pub enum BadRequestError { ParseError { field: String } }
        })
        .unwrap();
        assert!(
            output.contains(
                "\"field\" , :: locolized_errors :: serde_json :: to_value (field) . unwrap_or_default ()"
            ),
            "{output}"
        );
    }

    #[test]
    fn kind_attribute_overrides_prefix() {
        let output = expand_src(&quote! {
            #[localized(kind = "unauthorized")]
            pub enum Whatever { Nope }
        })
        .unwrap();
        assert!(output.contains("\"errors.unauthorized.nope\""), "{output}");
        assert!(output.contains("ErrorKind :: Unauthorized"), "{output}");
    }

    #[test]
    fn status_attribute_creates_custom_kind() {
        let output = expand_src(&quote! {
            #[localized(status = 418, description = "stout")]
            pub enum TeapotError { Brewed }
        })
        .unwrap();
        assert!(output.contains("__status (418"), "{output}");
        assert!(output.contains("\"stout\""), "{output}");
        assert!(output.contains("\"errors.teapot.brewed\""), "{output}");
    }

    #[test]
    fn variant_status_overrides_enum_kind() {
        let output = expand_src(&quote! {
            #[localized(kind = "bad-request")]
            pub enum Mixed { #[localized(status = 507)] Insufficient }
        })
        .unwrap();
        assert!(output.contains("__status (507"), "{output}");
        assert!(output.contains("ErrorKind :: Custom"), "{output}");
        assert!(
            output.contains("\"errors.bad-request.insufficient\""),
            "{output}"
        );
    }

    #[test]
    fn message_override_bypasses_t_macro() {
        let output = expand_src(&quote! {
            #[localized(kind = "bad-request")]
            pub enum Static { #[localized(message = "plain old message")] Plain { x: u8 } }
        })
        .unwrap();
        assert!(output.contains("\"plain old message\""), "{output}");
        assert!(output.contains("Self :: Plain { .. } =>"), "{output}");
        assert!(
            !output.contains("rust_i18n :: t !"),
            "message override must bypass t!: {output}"
        );
    }

    #[test]
    fn key_override_renames_segment() {
        let output = expand_src(&quote! {
            #[localized(kind = "bad-request")]
            pub enum Custom { #[localized(key = "renamed")] Original }
        })
        .unwrap();
        assert!(
            output.contains("\"errors.bad-request.renamed\""),
            "{output}"
        );
    }

    #[test]
    fn locale_is_forwarded_to_t() {
        let output = expand_src(&quote! {
            #[localized(kind = "bad-request", locale = "fr")]
            pub enum French { Salut }
        })
        .unwrap();
        assert!(output.contains("locale = \"fr\""), "{output}");
    }

    #[test]
    fn variant_locale_wins_over_enum_locale() {
        let output = expand_src(&quote! {
            #[localized(kind = "bad-request", locale = "fr")]
            pub enum Mixed { #[localized(locale = "de")] Thing }
        })
        .unwrap();
        assert!(output.contains("locale = \"de\""), "{output}");
    }

    #[test]
    fn no_into_response_skips_impl() {
        let output = expand_src(&quote! {
            #[localized(kind = "bad-request", no_into_response)]
            pub enum Silent { Quiet }
        })
        .unwrap();
        assert!(!output.contains("IntoResponse"), "{output}");
    }

    #[test]
    fn non_enum_input_is_rejected() {
        let error = expand_src(&quote! {
            #[derive(Debug, serde::Serialize)]
            pub struct Oops { field: u8 }
        })
        .unwrap_err();
        assert!(
            error.to_string().contains("only be derived for enums"),
            "{error}"
        );
    }

    #[test]
    fn unknown_kind_error_is_actionable() {
        let error = expand_src(&quote! { pub enum TeapotError { Brewed } }).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("cannot determine the error kind"),
            "{error}"
        );
    }

    #[test]
    fn bad_attribute_error_propagates() {
        let error = expand_src(&quote! {
            #[localized(bogus)]
            pub enum BadRequestError { X }
        })
        .unwrap_err();
        assert!(
            error.to_string().contains("unknown `localized` option"),
            "{error}"
        );
    }
}
