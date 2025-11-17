use convert_case::ccase;
use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, parse_macro_input};

#[proc_macro_derive(LocalizedApiError)]
pub fn derive_localized_api_error(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let enum_name = name.to_string();

    // Automatically derive error_kind from enum name
    // BadRequestError -> "bad_request", UnauthorizedError -> "unauthorized", etc.
    let error_kind = derive_error_kind(&enum_name);

    let (display_arms, error_kind_arms) = match &input.data {
        Data::Enum(data_enum) => {
            let mut display_arms = Vec::new();
            let mut error_kind_arms = Vec::new();

            for variant in &data_enum.variants {
                let variant_ident = &variant.ident;
                let variant_str = variant_ident.to_string();
                let error_key = format!("errors.{}.{}", error_kind, ccase!(kebab, &variant_str));

                match &variant.fields {
                    Fields::Unit => {
                        display_arms.push(quote! {
                            #name::#variant_ident => {
                                use rust_i18n::t;
                                write!(f, "{}", t!(#error_key))
                            }
                        });
                        error_kind_arms.push(quote! {
                            #name::#variant_ident => #error_kind,
                        });
                    }
                    Fields::Named(fields_named) => {
                        let field_idents: Vec<_> =
                            fields_named.named.iter().map(|f| &f.ident).collect();
                        let field_names: Vec<String> = field_idents
                            .iter()
                            .map(|ident| ident.as_ref().map(|i| i.to_string()).unwrap_or_default())
                            .collect();

                        // Create the t! macro call with field assignments
                        let field_assignments: Vec<_> = field_idents
                            .iter()
                            .zip(field_names.iter())
                            .map(|(ident, name)| {
                                quote! { #name = #ident }
                            })
                            .collect();

                        display_arms.push(quote! {
                            #name::#variant_ident { #(#field_idents),* } => {
                                use rust_i18n::t;
                                write!(f, "{}", t!(#error_key, #(#field_assignments),*))
                            }
                        });
                        error_kind_arms.push(quote! {
                            #name::#variant_ident { .. } => #error_kind,
                        });
                    }
                    Fields::Unnamed(fields_unnamed) => {
                        // Handle tuple variants
                        let field_indices: Vec<_> = (0..fields_unnamed.unnamed.len())
                            .map(syn::Index::from)
                            .collect();

                        display_arms.push(quote! {
                            #name::#variant_ident ( #(#field_indices),* ) => {
                                use rust_i18n::t;
                                write!(f, "{}", t!(#error_key))
                            }
                        });
                        error_kind_arms.push(quote! {
                            #name::#variant_ident ( .. ) => #error_kind,
                        });
                    }
                }
            }

            (display_arms, error_kind_arms)
        }
        _ => panic!("LocalizedApiError can only be derived for enums"),
    };

    let expanded = quote! {
        impl std::fmt::Display for #name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    #(#display_arms)*
                }
            }
        }

        impl std::error::Error for #name {}

        impl locolized_errors::LocalizedApiError for #name {
            fn error_kind(&self) -> locolized_errors::ErrorKind {
                match self {
                    #(#error_kind_arms)*
                }
            }
        }

        impl From<#name> for loco_rs::prelude::Error {
            fn from(err: #name) -> Self {
                err.to_loco_error()
            }
        }
    };

    TokenStream::from(expanded)
}

fn derive_error_kind(enum_name: &str) -> String {
    // Remove "Error" suffix if present and convert to snake_case
    let base_name = if enum_name.ends_with("Error") {
        &enum_name[..enum_name.len() - 5]
    } else {
        enum_name
    };
    ccase!(kebab, base_name)
}
