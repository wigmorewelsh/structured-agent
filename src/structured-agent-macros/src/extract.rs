use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::Type as SynType;

use crate::types::{generic_arg, path_ident};

pub fn extract_arg(idx: usize, ty: &SynType) -> syn::Result<TokenStream2> {
    extract_value(&quote! { args[#idx] }, ty)
}

pub fn extract_value(val: &TokenStream2, ty: &SynType) -> syn::Result<TokenStream2> {
    let ident = path_ident(ty)?;
    Ok(match ident.to_string().as_str() {
        "String" => quote! { #val.as_string().map_err(|e| e.to_string())?.to_string() },
        "bool" => quote! { #val.as_boolean().map_err(|e| e.to_string())? },
        "i64" => quote! { #val.as_integer().map_err(|e| e.to_string())? },
        "Option" => extract_option(val, ty)?,
        other => {
            return Err(syn::Error::new_spanned(
                ident,
                format!("unsupported arg type: {other}"),
            ));
        }
    })
}

fn extract_option(val: &TokenStream2, ty: &SynType) -> syn::Result<TokenStream2> {
    let inner_extract = extract_value(&quote! { __iv }, generic_arg(ty)?)?;
    Ok(quote! {
        {
            let __opt = #val.as_option().map_err(|e| e.to_string())?;
            match __opt {
                None => None,
                Some(__iv) => Some(#inner_extract),
            }
        }
    })
}
