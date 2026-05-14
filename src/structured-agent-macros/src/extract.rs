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
        _ => Ok(quote! { #val.downcast_clone::<#ident>().map_err(|e| e.to_string())? }),
    }
}
