use proc_macro2::{Ident, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::{Attribute, Block, FnArg, ItemFn, LitStr, Pat, ReturnType, Token, Type as SynType};

use crate::convert::convert_result;
use crate::extract::extract_arg;
use crate::types::{is_unit_type, map_type_to_runtime};

struct SaFnArgs {
    type_params: Vec<String>,
}

impl syn::parse::Parse for SaFnArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        if input.is_empty() {
            return Ok(Self {
                type_params: vec![],
            });
        }
        let ident: Ident = input.parse()?;
        if ident != "type_params" {
            return Err(syn::Error::new(ident.span(), "expected `type_params`"));
        }
        let _eq: Token![=] = input.parse()?;
        let s: LitStr = input.parse()?;
        let type_params = s
            .value()
            .split(',')
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .collect();
        Ok(Self { type_params })
    }
}

pub fn to_pascal_case(s: &str) -> String {
    s.split('_').map(capitalize_first).collect()
}

fn capitalize_first(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

pub fn fn_struct_ident(fn_name: &str) -> Ident {
    format_ident!("{}Function", to_pascal_case(fn_name))
}

fn doc_line(attr: &Attribute) -> Option<String> {
    if !attr.path().is_ident("doc") {
        return None;
    }
    if let syn::Meta::NameValue(nv) = &attr.meta
        && let syn::Expr::Lit(el) = &nv.value
        && let syn::Lit::Str(s) = &el.lit
    {
        return Some(s.value().trim().to_string());
    }
    None
}

fn extract_doc(attrs: &[Attribute]) -> Option<String> {
    let lines: Vec<String> = attrs.iter().filter_map(doc_line).collect();
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

fn extract_params(
    inputs: &syn::punctuated::Punctuated<FnArg, syn::token::Comma>,
) -> Vec<(Ident, SynType)> {
    inputs
        .iter()
        .filter_map(|arg| {
            if let FnArg::Typed(pt) = arg
                && let Pat::Ident(pi) = &*pt.pat
            {
                return Some((pi.ident.clone(), (*pt.ty).clone()));
            }
            None
        })
        .collect()
}

fn build_param_constructions(
    params: &[(Ident, SynType)],
    type_params: &[String],
) -> syn::Result<Vec<TokenStream2>> {
    params
        .iter()
        .map(|(name, ty)| {
            let name_str = name.to_string();
            let rt = map_type_to_runtime(ty, type_params)?;
            Ok(quote! { ::structured_agent_runtime::Parameter::new(#name_str.to_string(), #rt) })
        })
        .collect()
}

fn build_arg_extractions(
    params: &[(Ident, SynType)],
    type_params: &[String],
) -> syn::Result<Vec<TokenStream2>> {
    params
        .iter()
        .enumerate()
        .map(|(i, (name, ty))| {
            let extraction = extract_arg(i, ty, type_params)?;
            Ok(quote! { let #name = #extraction; })
        })
        .collect()
}

fn return_type(output: &ReturnType) -> Option<&SynType> {
    match output {
        ReturnType::Default => None,
        ReturnType::Type(_, ty) => Some(ty.as_ref()),
    }
}

fn build_return_conversion(
    ret_ty: Option<&SynType>,
    body: &Block,
    type_params: &[String],
) -> syn::Result<TokenStream2> {
    if ret_ty.is_none_or(is_unit_type) {
        return Ok(quote! {
            { #body };
            Ok(::structured_agent_runtime::ExpressionValue::unit())
        });
    }
    let conv = convert_result(ret_ty.unwrap(), type_params)?;
    Ok(quote! {
        let __sa_result = { #body };
        Ok(#conv)
    })
}

pub fn generate_native_function(attr: TokenStream2, input: ItemFn) -> syn::Result<TokenStream2> {
    let args: SaFnArgs = syn::parse2(attr)?;
    let type_params = &args.type_params;

    let fn_name = input.sig.ident.to_string();
    let struct_name = fn_struct_ident(&fn_name);
    let params = extract_params(&input.sig.inputs);
    let param_count = params.len();
    let doc = extract_doc(&input.attrs);

    let param_constructions = build_param_constructions(&params, type_params)?;
    let ret_ty = return_type(&input.sig.output);
    let return_type_expr = ret_ty.map_or_else(
        || Ok(quote! { ::structured_agent_runtime::Type::unit() }),
        |t| map_type_to_runtime(t, type_params),
    )?;
    let arg_extractions = build_arg_extractions(&params, type_params)?;
    let return_conversion = build_return_conversion(ret_ty, &input.block, type_params)?;
    let doc_fn = doc
        .map(|d| quote! { fn documentation(&self) -> Option<&str> { Some(#d) } })
        .unwrap_or_default();

    let type_params_field = if !type_params.is_empty() {
        quote! { type_params: Vec<String>, }
    } else {
        quote! {}
    };

    let type_params_init = if !type_params.is_empty() {
        let tp_strs: Vec<&str> = type_params.iter().map(|s| s.as_str()).collect();
        quote! { type_params: vec![#(#tp_strs.to_string()),*], }
    } else {
        quote! {}
    };

    let type_params_fn = if !type_params.is_empty() {
        quote! {
            fn type_params(&self) -> &[String] {
                &self.type_params
            }
        }
    } else {
        quote! {}
    };

    Ok(quote! {
        #[derive(Debug)]
        pub struct #struct_name {
            parameters: Vec<::structured_agent_runtime::Parameter>,
            return_type: ::structured_agent_runtime::Type,
            #type_params_field
        }

        impl Default for #struct_name {
            fn default() -> Self { Self::new() }
        }

        impl #struct_name {
            pub fn new() -> Self {
                Self {
                    parameters: vec![#(#param_constructions),*],
                    return_type: #return_type_expr,
                    #type_params_init
                }
            }
        }

        #[::async_trait::async_trait]
        impl ::structured_agent_runtime::NativeFunction for #struct_name {
            fn name(&self) -> &str { #fn_name }

            fn parameters(&self) -> &[::structured_agent_runtime::Parameter] {
                &self.parameters
            }

            fn return_type(&self) -> &::structured_agent_runtime::Type {
                &self.return_type
            }

            #type_params_fn

            #doc_fn

            async fn execute(
                &self,
                args: Vec<::structured_agent_runtime::ExpressionValue>,
                agent: &::structured_agent_runtime::AgentHandle,
            ) -> Result<::structured_agent_runtime::ExpressionValue, String> {
                let _ = &agent;
                if args.len() != #param_count {
                    return Err(format!(
                        "{} expects {} argument(s), got {}",
                        #fn_name, #param_count, args.len()
                    ));
                }
                #(#arg_extractions)*
                #return_conversion
            }
        }
    })
}
