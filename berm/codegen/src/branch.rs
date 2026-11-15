use crate::schema;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, ext::IdentExt};

pub fn derive(input: &DeriveInput) -> syn::Result<TokenStream> {
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "a Branch has no generics",
        ));
    }
    let Data::Enum(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            input,
            "a Branch is an enum with one distinct payload type per case",
        ));
    };
    if data.variants.is_empty() {
        return Err(syn::Error::new_spanned(
            input,
            "a Branch needs at least one case",
        ));
    }
    let ident = &input.ident;
    let mut cases = Vec::new();
    let mut arms = Vec::new();
    let mut projections = Vec::new();
    for variant in &data.variants {
        let Fields::Unnamed(fields) = &variant.fields else {
            return Err(syn::Error::new_spanned(
                variant,
                "a Branch case wraps exactly one distinct payload type",
            ));
        };
        if fields.unnamed.len() != 1 {
            return Err(syn::Error::new_spanned(
                fields,
                "a Branch case wraps exactly one distinct payload type",
            ));
        }
        let ty = &fields.unnamed[0].ty;
        let variant = &variant.ident;
        let name = schema::snake(&variant.unraw().to_string());
        cases.push(quote!(::berm_lang::schema::Variant {
            name: #name, description: "", payload: Some(&<#ty as ::berm_lang::Schema>::SHAPE),
        }));
        arms.push(quote!(Self::#variant(_) => #name));
        projections.push(quote! {
            impl ::berm_lang::workflow::Projection<#ty> for #ident {
                const CASE: ::core::option::Option<&'static str> = ::core::option::Option::Some(#name);
                fn project(self) -> ::core::option::Option<#ty> {
                    match self {
                        Self::#variant(payload) => ::core::option::Option::Some(payload),
                        _ => ::core::option::Option::None,
                    }
                }
            }
        });
    }
    Ok(quote! {
        impl ::berm_lang::workflow::Branch for #ident {
            const CASES: &'static [::berm_lang::schema::Variant] = &[#(#cases),*];
            fn case(&self) -> &'static str {
                match self { #(#arms),* }
            }
        }
        #(#projections)*
    })
}
