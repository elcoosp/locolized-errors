//! Code generation: turns a parsed [`syn::DeriveInput`] into the impl blocks
//! for `Display`, `std::error::Error`, `LocalizedApiError`, `From<E> for
//! loco_rs::Error` and (unless opted out) `IntoResponse`.
//!
//! `expand` is a pure function over `&DeriveInput`, which makes it directly
//! unit-testable without invoking rustc.
//!
//! Everything is emitted inside a `const _: () = { … }` block: the impls are
//! still global, but the two helper structs (`__Payload`, `__Params`) stay
//! scoped to this expansion, so several derives can coexist in one module.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Ident, LitStr, Variant};

use crate::attrs::{ContainerAttrs, VariantAttrs};
use crate::case::to_kebab_case;
use crate::kind::{KindPlan, custom_kind_expr, resolve_kind};

/// The per-variant data shared by the generated match arms.
struct VariantArms {
    message: TokenStream,
    kind: TokenStream,
    variant: TokenStream,
    key: TokenStream,
    params: TokenStream,
    stream_params: TokenStream,
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

    let mut message_arms = Vec::with_capacity(data_enum.variants.len());
    let mut kind_arms = Vec::with_capacity(data_enum.variants.len());
    let mut name_arms = Vec::with_capacity(data_enum.variants.len());
    let mut key_arms = Vec::with_capacity(data_enum.variants.len());
    let mut param_arms = Vec::with_capacity(data_enum.variants.len());
    let mut stream_param_arms = Vec::with_capacity(data_enum.variants.len());

    for variant in &data_enum.variants {
        let arms = variant_arms(variant, &plan, &container)?;
        message_arms.push(arms.message);
        kind_arms.push(arms.kind);
        name_arms.push(arms.variant);
        key_arms.push(arms.key);
        param_arms.push(arms.params);
        stream_param_arms.push(arms.stream_params);
    }

    let arms = TraitArms {
        message: message_arms,
        kind: kind_arms,
        variant: name_arms,
        key: key_arms,
        params: param_arms,
        stream_params: stream_param_arms,
    };
    let into_response_impl = into_response_impl(enum_ident, !container.no_into_response);
    let trait_impl = trait_impl(enum_ident, &arms);
    let payload_structs = payload_structs(enum_ident);
    let display_impl = display_impl(enum_ident);
    let from_impl = from_impl(enum_ident);

    Ok(quote! {
        const _: () = {
            #payload_structs
            #display_impl
            #trait_impl
            #from_impl
            #into_response_impl
        };
    })
}

/// Single-pass JSON payload views, scoped to this expansion so several
/// derives can coexist in one module.
fn payload_structs(enum_ident: &Ident) -> TokenStream {
    quote! {
        struct __Params<'a>(&'a #enum_ident);

        impl ::serde::Serialize for __Params<'_> {
            fn serialize<S: ::serde::Serializer>(
                &self,
                serializer: S,
            ) -> ::core::result::Result<S::Ok, S::Error> {
                <#enum_ident as ::locolized_errors::LocalizedApiError>::__serialize_params(
                    self.0,
                    serializer,
                )
            }
        }

        struct __Payload<'a>(&'a #enum_ident);

        impl ::serde::Serialize for __Payload<'_> {
            fn serialize<S: ::serde::Serializer>(
                &self,
                serializer: S,
            ) -> ::core::result::Result<S::Ok, S::Error> {
                use ::locolized_errors::LocalizedApiError as _;
                use ::serde::ser::SerializeStruct as _;
                let mut __state = serializer.serialize_struct("LocalizedApiError", 5)?;
                __state.serialize_field("kind", self.0.error_kind().as_kebab())?;
                __state.serialize_field("variant", self.0.error_variant())?;
                __state.serialize_field("key", self.0.error_key())?;
                __state.serialize_field("message", self.0.error_message().as_ref())?;
                __state.serialize_field("params", &__Params(self.0))?;
                __state.end()
            }
        }
    }
}

/// `Display` delegates to `error_message`: the localized text is produced
/// once per variant shape and rendered without format machinery. `Error`
/// comes along for free once `Display + Debug` hold.
fn display_impl(enum_ident: &Ident) -> TokenStream {
    quote! {
        #[automatically_derived]
        impl ::core::fmt::Display for #enum_ident {
            fn fmt(&self, __formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                use ::locolized_errors::LocalizedApiError as _;
                __formatter.write_str(self.error_message().as_ref())
            }
        }

        #[automatically_derived]
        impl ::core::error::Error for #enum_ident {}
    }
}

/// `From<E> for loco_rs::Error`: the `?` / `.into()` route into controllers.
fn from_impl(enum_ident: &Ident) -> TokenStream {
    quote! {
        #[automatically_derived]
        impl ::core::convert::From<#enum_ident> for ::locolized_errors::loco_rs::errors::Error {
            fn from(error: #enum_ident) -> Self {
                use ::locolized_errors::LocalizedApiError as _;
                error.to_loco_error()
            }
        }
    }
}

/// The generated `IntoResponse`, unless opted out: the structured payload
/// bytes with the matching status and `content-type: application/json`.
fn into_response_impl(enum_ident: &Ident, enabled: bool) -> Option<TokenStream> {
    enabled.then(|| {
        quote! {
            #[automatically_derived]
            impl ::locolized_errors::loco_rs::prelude::IntoResponse for #enum_ident {
                fn into_response(
                    self,
                ) -> ::locolized_errors::loco_rs::prelude::Response {
                    use ::locolized_errors::LocalizedApiError as _;
                    ::locolized_errors::__response_from_json(
                        self.status_code(),
                        self.to_json_bytes(),
                    )
                }
            }
        }
    })
}

/// The per-variant match arms grouped for the trait impl.
struct TraitArms {
    message: Vec<TokenStream>,
    kind: Vec<TokenStream>,
    variant: Vec<TokenStream>,
    key: Vec<TokenStream>,
    params: Vec<TokenStream>,
    stream_params: Vec<TokenStream>,
}

/// The `LocalizedApiError` impl: the four generated primitives, the message,
/// the streaming params override and the single-pass payload methods.
fn trait_impl(enum_ident: &Ident, arms: &TraitArms) -> TokenStream {
    let TraitArms {
        message,
        kind,
        variant,
        key,
        params,
        stream_params,
    } = arms;
    let fallback = quote! {
        ::locolized_errors::__payload_tree(
            &self.error_kind(),
            self.error_variant(),
            self.error_key(),
            &self.error_message(),
            self.error_params(),
        )
    };
    quote! {
        #[automatically_derived]
        impl ::locolized_errors::LocalizedApiError for #enum_ident {
            #[inline]
            fn error_kind(&self) -> ::locolized_errors::ErrorKind {
                match self {
                    #(#kind)*
                }
            }

            #[inline]
            fn error_variant(&self) -> &'static ::core::primitive::str {
                match self {
                    #(#variant)*
                }
            }

            #[inline]
            fn error_key(&self) -> &'static ::core::primitive::str {
                match self {
                    #(#key)*
                }
            }

            #[inline]
            fn error_params(
                &self,
            ) -> ::std::vec::Vec<(&'static ::core::primitive::str, ::locolized_errors::serde_json::Value)> {
                match self {
                    #(#params)*
                }
            }

            #[inline]
            fn error_message(
                &self,
            ) -> ::std::borrow::Cow<'static, ::core::primitive::str> {
                match self {
                    #(#message)*
                }
            }

            #[doc(hidden)]
            #[inline]
            fn __serialize_params<S: ::serde::Serializer>(
                &self,
                __serializer: S,
            ) -> ::core::result::Result<S::Ok, S::Error> {
                use ::serde::ser::SerializeMap as _;
                match self {
                    #(#stream_params)*
                }
            }

            #[inline]
            fn to_json(&self) -> ::locolized_errors::serde_json::Value {
                use ::locolized_errors::LocalizedApiError as _;
                ::locolized_errors::serde_json::to_value(__Payload(self))
                    .unwrap_or_else(|_| #fallback)
            }

            #[inline]
            fn to_json_string(&self) -> ::std::string::String {
                use ::locolized_errors::LocalizedApiError as _;
                ::std::string::String::from_utf8(self.to_json_bytes())
                    .unwrap_or_default()
            }

            #[inline]
            fn to_json_bytes(&self) -> ::std::vec::Vec<::core::primitive::u8> {
                use ::locolized_errors::LocalizedApiError as _;
                ::locolized_errors::serde_json::to_vec(&__Payload(self))
                    .unwrap_or_else(|_| #fallback.to_string().into_bytes())
            }
        }
    }
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
    let message_arm = message_arm(
        variant_ident,
        &variant.fields,
        &attrs,
        &full_key,
        container.locale.as_deref(),
    );

    Ok(VariantArms {
        message: message_arm,
        kind: kind_arm,
        variant: variant_arm,
        key: key_arm,
        params: params_arm(variant_ident, &variant.fields),
        stream_params: stream_params_arm(variant_ident, &variant.fields),
    })
}

/// Build the `error_message` match arm for one variant.
///
/// - unit and tuple variants render their (non-interpolated) translation,
///   which `t!` returns as a borrowed `Cow` when found;
/// - named fields are forwarded to `t!` as named parameters;
/// - a `message = "..."` override is a borrowed static literal.
fn message_arm(
    variant_ident: &Ident,
    fields: &Fields,
    attrs: &VariantAttrs,
    full_key: &str,
    container_locale: Option<&str>,
) -> TokenStream {
    let body = match attrs.message.as_deref() {
        Some(message) => {
            let literal = LitStr::new(message, variant_ident.span());
            quote! { ::std::borrow::Cow::Borrowed(#literal) }
        }
        None => t_call(
            full_key,
            attrs.locale.as_deref().or(container_locale),
            &interpolation_args(fields),
        ),
    };
    match fields {
        Fields::Unit => quote! { Self::#variant_ident => #body, },
        Fields::Named(fields_named) => {
            let idents: Vec<&Ident> = fields_named
                .named
                .iter()
                .filter_map(|field| field.ident.as_ref())
                .collect();
            if attrs.message.is_some() {
                quote! { Self::#variant_ident { .. } => #body, }
            } else {
                quote! {
                    Self::#variant_ident { #(#idents),* } => #body,
                }
            }
        }
        Fields::Unnamed(fields_unnamed) => {
            let wildcards: Vec<TokenStream> = (0..fields_unnamed.unnamed.len())
                .map(|_| quote! { _ })
                .collect();
            quote! { Self::#variant_ident(#(#wildcards),*) => #body, }
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
/// must have been initialized. The expansion unifies to
/// `Cow<'static, str>`: borrowed for parameterless hits, owned when
/// interpolated.
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

/// Match arm streaming the JSON-ready parameter object: named fields
/// serialize straight from the variant (`serialize_entry`, no intermediate
/// `Value`, no `Vec`); unit and tuple variants emit an empty object.
fn stream_params_arm(variant_ident: &Ident, fields: &Fields) -> TokenStream {
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
            let length = idents.len();
            quote! {
                Self::#variant_ident { #(#idents),* } => {
                    let mut __map = __serializer.serialize_map(Some(#length))?;
                    #(__map.serialize_entry(#names, #idents)?;)*
                    __map.end()
                }
            }
        }
        Fields::Unit => quote! {
            Self::#variant_ident => __serializer.serialize_map(Some(0))?.end(),
        },
        Fields::Unnamed(_) => quote! {
            Self::#variant_ident (..) => __serializer.serialize_map(Some(0))?.end(),
        },
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
    fn display_routes_through_error_message() {
        let output = expand_src(&quote! {
            pub enum BadRequestError { UserNotFound }
        })
        .unwrap();
        assert!(
            output.contains("__formatter . write_str (self . error_message () . as_ref ())",),
            "{output}"
        );
        // The generated Display no longer carries per-variant arms.
        let display = output
            .split("impl :: core :: fmt :: Display")
            .nth(1)
            .unwrap()
            .split("impl :: core :: error :: Error")
            .next()
            .unwrap();
        assert!(display.contains("write_str"), "{display}");
        assert!(
            !display.contains("match self"),
            "Display must delegate to error_message: {display}"
        );
    }

    #[test]
    fn message_arms_return_cow() {
        let output = expand_src(&quote! {
            #[localized(kind = "bad-request")]
            pub enum Mixed {
                #[localized(message = "static text")]
                Static { x: u8 },
                Dynamic { attempts: u32 },
                Unit,
                Tuple(u8),
            }
        })
        .unwrap();
        assert!(
            output.contains(":: std :: borrow :: Cow :: Borrowed (\"static text\")"),
            "{output}"
        );
        assert!(
            output.contains("fn error_message (& self ,) -> :: std :: borrow :: Cow < 'static , :: core :: primitive :: str >"),
            "{output}"
        );
        assert!(output.contains("attempts = attempts"), "{output}");
    }

    #[test]
    fn params_stream_straight_from_fields() {
        let output = expand_src(&quote! {
            #[localized(kind = "bad-request")]
            pub enum BadRequestError { ParseError { field: String, value: String }, Unit, Tuple(String) }
        })
        .unwrap();
        assert!(
            output.contains("__map . serialize_entry (\"field\" , field) ? ;"),
            "{output}"
        );
        assert!(
            output.contains("__map . serialize_entry (\"value\" , value) ? ;"),
            "{output}"
        );
        assert!(
            output.contains("Self :: Unit => __serializer . serialize_map (Some (0)) ? . end ()"),
            "{output}"
        );
        // Named fields stream directly from the variant inside
        // `__serialize_params`; only the `error_params` introspection arm may
        // materialize `Value`s.
        let streaming = output
            .split("fn __serialize_params")
            .nth(1)
            .unwrap()
            .split("fn to_json")
            .next()
            .unwrap();
        assert!(
            !streaming.contains("to_value"),
            "streaming params must not materialize Values: {streaming}"
        );
    }

    #[test]
    fn payload_overrides_use_single_pass_serialization() {
        let output = expand_src(&quote! {
            pub enum NotFoundError { NothingHere }
        })
        .unwrap();
        for fragment in [
            "struct __Payload < 'a > (& 'a NotFoundError)",
            "serialize_struct (\"LocalizedApiError\" , 5)",
            "fn to_json_bytes (& self) -> :: std :: vec :: Vec < :: core :: primitive :: u8 >",
            ":: locolized_errors :: serde_json :: to_vec (& __Payload (self))",
            "__payload_tree",
        ] {
            assert!(
                output.contains(fragment),
                "missing {fragment} in:\n{output}"
            );
        }
    }

    #[test]
    fn into_response_streams_bytes_with_json_content_type() {
        let output = expand_src(&quote! {
            pub enum BadRequestError { X }
        })
        .unwrap();
        assert!(
            output.contains(
                ":: locolized_errors :: __response_from_json (self . status_code () , self . to_json_bytes () ,)",
            ),
            "{output}"
        );
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
        // Tuple variants carry no parameters on either params path.
        let streaming = output
            .split("fn __serialize_params")
            .nth(1)
            .unwrap()
            .split("fn to_json")
            .next()
            .unwrap();
        assert!(
            !streaming.contains("unwrap_or_default"),
            "tuple fields must not become params: {streaming}"
        );
        let introspection = output
            .split("fn error_params")
            .nth(1)
            .unwrap()
            .split("fn error_message")
            .next()
            .unwrap();
        assert!(
            !introspection.contains("unwrap_or_default"),
            "tuple fields must not become params: {introspection}"
        );
    }

    #[test]
    fn unit_variant_params_are_an_empty_object() {
        let output = expand_src(&quote! {
            #[derive(Debug, serde::Serialize)]
            pub enum NotFoundError { NothingHere }
        })
        .unwrap();
        assert!(
            output.contains(
                "Self :: NothingHere => __serializer . serialize_map (Some (0)) ? . end ()"
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
    fn extended_kinds_resolve() {
        for (kind, variant) in [
            ("forbidden", "Forbidden"),
            ("conflict", "Conflict"),
            ("unprocessable-entity", "UnprocessableEntity"),
            ("too-many-requests", "TooManyRequests"),
            ("service-unavailable", "ServiceUnavailable"),
        ] {
            let output = expand_src(&quote! {
                #[localized(kind = #kind)]
                pub enum Whatever { Nope }
            })
            .unwrap();
            assert!(
                output.contains(&format!("ErrorKind :: {variant}")),
                "kind {kind} in:\n{output}"
            );
            assert!(
                output.contains(&format!("\"errors.{kind}.nope\"")),
                "kind {kind} key in:\n{output}"
            );
        }
    }

    #[test]
    fn status_attribute_creates_custom_kind() {
        let output = expand_src(&quote! {
            #[localized(status = 418, description = "stout")]
            pub enum TeapotError { Brewed }
        })
        .unwrap();
        assert!(output.contains("__status (418"), "{output}");
        assert!(output.contains("Cow :: Borrowed (\"stout\")"), "{output}");
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
