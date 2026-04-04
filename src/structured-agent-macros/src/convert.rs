use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::Type as SynType;

use crate::types::{generic_arg, path_ident};

pub fn convert_result(ty: &SynType, type_params: &[String]) -> syn::Result<TokenStream2> {
    to_expr_value(&quote! { __sa_result }, ty, type_params)
}

pub fn to_expr_value(
    val: &TokenStream2,
    ty: &SynType,
    type_params: &[String],
) -> syn::Result<TokenStream2> {
    let ident = path_ident(ty)?;
    let ident_str = ident.to_string();

    if type_params.contains(&ident_str) {
        return Ok(quote! { #val });
    }

    match ident_str.as_str() {
        "String" => Ok(quote! { ::structured_agent_runtime::ExpressionValue::string(#val) }),
        "bool" => Ok(quote! { ::structured_agent_runtime::ExpressionValue::boolean(#val) }),
        "i64" => Ok(quote! { ::structured_agent_runtime::ExpressionValue::integer(#val) }),
        "Option" => option_to_expr_value(val, ty, type_params),
        "Vec" => {
            let inner = generic_arg(ty)?;
            if let Ok(inner_ident) = path_ident(inner)
                && type_params.contains(&inner_ident.to_string())
            {
                return Ok(quote! {
                    ::structured_agent_runtime::ExpressionValue::from_elements(#val).map_err(|e| e)?
                });
            }
            Err(syn::Error::new_spanned(
                ident,
                "Vec<T> where T is not a type param is not supported as a return type",
            ))
        }
        other => Err(syn::Error::new_spanned(
            ident,
            format!("unsupported return type: {other}"),
        )),
    }
}

fn option_to_expr_value(
    val: &TokenStream2,
    ty: &SynType,
    type_params: &[String],
) -> syn::Result<TokenStream2> {
    let some_convert = to_expr_value(&quote! { __iv }, generic_arg(ty)?, type_params)?;
    Ok(quote! {
        match #val {
            None => ::structured_agent_runtime::ExpressionValue::option_none(),
            Some(__iv) => ::structured_agent_runtime::ExpressionValue::option_some(#some_convert),
        }
    })
}
