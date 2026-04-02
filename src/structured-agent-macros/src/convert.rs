use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::Type as SynType;

use crate::types::{generic_arg, path_ident};

pub fn convert_result(ty: &SynType) -> syn::Result<TokenStream2> {
    to_expr_value(&quote! { __sa_result }, ty)
}

pub fn to_expr_value(val: &TokenStream2, ty: &SynType) -> syn::Result<TokenStream2> {
    let ident = path_ident(ty)?;
    Ok(match ident.to_string().as_str() {
        "String" => quote! { ::structured_agent_runtime::ExpressionValue::string(#val) },
        "bool" => quote! { ::structured_agent_runtime::ExpressionValue::boolean(#val) },
        "i64" => quote! { ::structured_agent_runtime::ExpressionValue::integer(#val) },
        "Option" => option_to_expr_value(val, ty)?,
        other => {
            return Err(syn::Error::new_spanned(
                ident,
                format!("unsupported return type: {other}"),
            ));
        }
    })
}

fn option_to_expr_value(val: &TokenStream2, ty: &SynType) -> syn::Result<TokenStream2> {
    let some_convert = to_expr_value(&quote! { __iv }, generic_arg(ty)?)?;
    Ok(quote! {
        match #val {
            None => ::structured_agent_runtime::ExpressionValue::option_none(),
            Some(__iv) => ::structured_agent_runtime::ExpressionValue::option_some(#some_convert),
        }
    })
}
