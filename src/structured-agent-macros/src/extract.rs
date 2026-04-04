use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::Type as SynType;

use crate::types::{generic_arg, path_ident};

pub fn extract_arg(idx: usize, ty: &SynType, type_params: &[String]) -> syn::Result<TokenStream2> {
    extract_value(&quote! { args[#idx] }, ty, type_params)
}

pub fn extract_value(
    val: &TokenStream2,
    ty: &SynType,
    type_params: &[String],
) -> syn::Result<TokenStream2> {
    let ident = path_ident(ty)?;
    let ident_str = ident.to_string();

    if type_params.contains(&ident_str) {
        return Ok(quote! { #val.clone() });
    }

    match ident_str.as_str() {
        "String" => Ok(quote! { #val.as_string().map_err(|e| e.to_string())?.to_string() }),
        "bool" => Ok(quote! { #val.as_boolean().map_err(|e| e.to_string())? }),
        "i64" => Ok(quote! { #val.as_integer().map_err(|e| e.to_string())? }),
        "Vec" => {
            let inner = generic_arg(ty)?;
            if let Ok(inner_ident) = path_ident(inner)
                && type_params.contains(&inner_ident.to_string())
            {
                return Ok(quote! { #val.as_list_elements().map_err(|e| e.to_string())? });
            }
            Err(syn::Error::new_spanned(
                ident,
                "Vec<T> where T is not a type param is not supported as an argument type",
            ))
        }
        "Option" => {
            let inner = generic_arg(ty)?;
            if let Ok(inner_ident) = path_ident(inner)
                && type_params.contains(&inner_ident.to_string())
            {
                return Ok(quote! { #val.as_option().map_err(|e| e.to_string())? });
            }
            extract_option(val, ty, type_params)
        }
        other => Err(syn::Error::new_spanned(
            ident,
            format!("unsupported arg type: {other}"),
        )),
    }
}

fn extract_option(
    val: &TokenStream2,
    ty: &SynType,
    type_params: &[String],
) -> syn::Result<TokenStream2> {
    let inner_extract = extract_value(&quote! { __iv }, generic_arg(ty)?, type_params)?;
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
