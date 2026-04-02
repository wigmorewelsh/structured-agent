use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{GenericArgument, Ident, PathArguments, Type as SynType};

pub fn is_unit_type(ty: &SynType) -> bool {
    matches!(ty, SynType::Tuple(t) if t.elems.is_empty())
}

pub fn path_ident(ty: &SynType) -> syn::Result<&Ident> {
    if let SynType::Path(tp) = ty
        && tp.qself.is_none()
        && tp.path.segments.len() == 1
    {
        return Ok(&tp.path.segments[0].ident);
    }
    Err(syn::Error::new_spanned(ty, "expected a simple type name"))
}

pub fn generic_arg(ty: &SynType) -> syn::Result<&SynType> {
    if let SynType::Path(tp) = ty
        && let Some(seg) = tp.path.segments.last()
        && let PathArguments::AngleBracketed(ab) = &seg.arguments
        && let Some(GenericArgument::Type(inner)) = ab.args.first()
    {
        return Ok(inner);
    }
    Err(syn::Error::new_spanned(ty, "type has no generic argument"))
}

pub fn map_type_to_runtime(ty: &SynType) -> syn::Result<TokenStream2> {
    if is_unit_type(ty) {
        return Ok(quote! { ::structured_agent_runtime::Type::unit() });
    }
    let ident = path_ident(ty)?;
    Ok(match ident.to_string().as_str() {
        "String" => quote! { ::structured_agent_runtime::Type::string() },
        "bool" => quote! { ::structured_agent_runtime::Type::boolean() },
        "i64" => quote! { ::structured_agent_runtime::Type::int() },
        "Option" => {
            let inner = map_type_to_runtime(generic_arg(ty)?)?;
            quote! { ::structured_agent_runtime::Type::option(#inner) }
        }
        "Vec" => {
            let inner = map_type_to_runtime(generic_arg(ty)?)?;
            quote! { ::structured_agent_runtime::Type::list(#inner) }
        }
        other => {
            return Err(syn::Error::new_spanned(
                ident,
                format!("unsupported type: {other}"),
            ));
        }
    })
}
