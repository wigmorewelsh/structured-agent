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

fn type_contains_param(ty: &SynType, type_params: &[String]) -> bool {
    use crate::types::{generic_arg, path_ident};
    if let Ok(ident) = path_ident(ty)
        && type_params.contains(&ident.to_string())
    {
        return true;
    }
    if let Ok(inner) = generic_arg(ty) {
        return type_contains_param(inner, type_params);
    }
    false
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
    let ty = ret_ty.unwrap();
    let conv = convert_result(ty, type_params)?;
    if type_contains_param(ty, type_params) {
        Ok(quote! {
            let __sa_result = { #body };
            Ok(#conv)
        })
    } else {
        Ok(quote! {
            let __sa_result: #ty = { #body };
            Ok(#conv)
        })
    }
}

pub fn generate_native_function(attr: TokenStream2, input: ItemFn) -> syn::Result<TokenStream2> {
    let args: SaFnArgs = syn::parse2(attr)?;
    let type_params = &args.type_params;

    let fn_name = input.sig.ident.to_string();
    let fn_def_name = format_ident!("{}_native_def", fn_name);
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

    let doc_value = match doc {
        Some(ref d) => quote! { Some(#d.to_string()) },
        None => quote! { None },
    };

    let type_params_vec = if type_params.is_empty() {
        quote! { vec![] }
    } else {
        let tp_strs: Vec<&str> = type_params.iter().map(|s| s.as_str()).collect();
        quote! { vec![#(#tp_strs.to_string()),*] }
    };

    let type_param_count = type_params.len() as u32;
    let slot_indices: Vec<u32> =
        ((1 + type_param_count)..=(param_count as u32 + type_param_count)).collect();
    let params_slots = if slot_indices.is_empty() {
        quote! { vec![] }
    } else {
        quote! { vec![#(::structured_agent_il::Slot(#slot_indices)),*] }
    };

    Ok(quote! {
        pub fn #fn_def_name() -> ::structured_agent_il::NativeFunctionDef {
            let f = ::structured_agent_runtime::NativeFnPtr::new(
                move |args: Vec<::structured_agent_runtime::ExpressionValue>, agent: ::structured_agent_runtime::AgentHandle| -> ::std::pin::Pin<Box<dyn ::std::future::Future<Output = Result<::structured_agent_runtime::ExpressionValue, String>> + Send>> {
                    Box::pin(async move {
                        let _ = &agent;
                        if args.len() != #param_count {
                            return Err(format!(
                                "{} expects {} argument(s), got {}",
                                #fn_name, #param_count, args.len()
                            ));
                        }
                        #(#arg_extractions)*
                        #return_conversion
                    })
                }
            );
            let body = vec![
                ::structured_agent_il::Instruction::CallNative {
                    f,
                    params: #params_slots,
                    dest: ::structured_agent_il::Slot(0),
                },
                ::structured_agent_il::Instruction::Ret { var: ::structured_agent_il::Slot(0) },
            ];
            ::structured_agent_il::NativeFunctionDef::new(
                #fn_name.to_string(),
                vec![#(#param_constructions),*],
                #return_type_expr,
                #type_params_vec,
                #doc_value,
                body,
            )
        }
    })
}
