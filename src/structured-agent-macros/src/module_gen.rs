use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{Item, ItemMod, ItemTrait, Token, TraitItem};

use crate::fn_gen::{generate_native_function, to_pascal_case};
use crate::types::map_type_to_runtime;

struct SaImplArgs {
    type_name: String,
    trait_name: Option<String>,
}

impl syn::parse::Parse for SaImplArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let type_ident: syn::Ident = input.parse()?;
        let trait_name = if input.peek(Token![for]) {
            let _: Token![for] = input.parse()?;
            let trait_ident: syn::Ident = input.parse()?;
            Some(trait_ident.to_string())
        } else {
            None
        };
        Ok(Self {
            type_name: type_ident.to_string(),
            trait_name,
        })
    }
}

struct PartitionResult {
    mod_items: Vec<TokenStream2>,
    fn_def_calls: Vec<TokenStream2>,
    trait_constructions: Vec<TokenStream2>,
    impl_constructions: Vec<TokenStream2>,
}

pub fn generate_module(input: ItemMod) -> syn::Result<TokenStream2> {
    let mod_name = &input.ident;
    let mod_name_str = mod_name.to_string();
    let module_struct = format_ident!("{}Module", to_pascal_case(&mod_name_str));
    let vis = &input.vis;

    let items = match input.content {
        Some((_, items)) => items,
        None => {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "#[sa_module] requires an inline mod block",
            ));
        }
    };

    let PartitionResult {
        mod_items,
        fn_def_calls,
        trait_constructions,
        impl_constructions,
    } = partition_items(items)?;

    let trait_override = if trait_constructions.is_empty() {
        None
    } else {
        Some(quote! {
            fn native_traits(&self) -> Vec<::structured_agent_il::NativeTraitDecl> {
                vec![#(#trait_constructions),*]
            }
        })
    };

    let impl_override = if impl_constructions.is_empty() {
        None
    } else {
        Some(quote! {
            fn native_impls(&self) -> Vec<::structured_agent_il::NativeImplDecl> {
                vec![#(#impl_constructions),*]
            }
        })
    };

    Ok(quote! {
        #vis mod #mod_name {
            #(#mod_items)*

            pub struct #module_struct;

            impl ::structured_agent_il::Module for #module_struct {
                fn name(&self) -> &str {
                    #mod_name_str
                }

                fn native_functions(&self) -> Vec<::structured_agent_il::NativeFunctionDef> {
                    vec![#(#fn_def_calls),*]
                }

                #trait_override
                #impl_override
            }
        }
    })
}

fn partition_items(items: Vec<Item>) -> syn::Result<PartitionResult> {
    let mut mod_items = Vec::new();
    let mut fn_def_calls = Vec::new();
    let mut trait_constructions = Vec::new();
    let mut impl_constructions = Vec::new();

    for item in items {
        match item {
            Item::Fn(mut item_fn) => {
                let sa_fn_pos = item_fn
                    .attrs
                    .iter()
                    .position(|a| a.path().is_ident("sa_fn"));
                if let Some(pos) = sa_fn_pos {
                    let attr_tokens = match &item_fn.attrs[pos].meta {
                        syn::Meta::List(ml) => ml.tokens.clone(),
                        _ => proc_macro2::TokenStream::new(),
                    };
                    item_fn.attrs.remove(pos);
                    let fn_name = item_fn.sig.ident.to_string();
                    let def_fn_name = format_ident!("{}_native_def", fn_name);
                    fn_def_calls.push(quote! { #def_fn_name() });
                    mod_items.push(generate_native_function(attr_tokens, item_fn)?);
                    continue;
                }
                mod_items.push(quote! { #item_fn });
            }
            Item::Trait(item_trait) => {
                let sa_trait_pos = item_trait
                    .attrs
                    .iter()
                    .position(|a| a.path().is_ident("sa_trait"));
                if sa_trait_pos.is_some() {
                    trait_constructions.push(trait_to_decl_construction(&item_trait)?);
                    continue;
                }
                mod_items.push(quote! { #item_trait });
            }
            Item::Mod(item_mod) => {
                let sa_impl_pos = item_mod
                    .attrs
                    .iter()
                    .position(|a| a.path().is_ident("sa_impl"));
                if let Some(pos) = sa_impl_pos {
                    let attr_tokens = match &item_mod.attrs[pos].meta {
                        syn::Meta::List(ml) => ml.tokens.clone(),
                        _ => {
                            return Err(syn::Error::new_spanned(
                                &item_mod.ident,
                                "#[sa_impl] requires arguments, e.g. #[sa_impl(Int for ToString)]",
                            ));
                        }
                    };
                    let impl_args: SaImplArgs = syn::parse2(attr_tokens)?;
                    let (submod_code, construction) = impl_mod_to_decl(impl_args, item_mod)?;
                    mod_items.push(submod_code);
                    impl_constructions.push(construction);
                    continue;
                }
                mod_items.push(quote! { #item_mod });
            }
            other => {
                mod_items.push(quote! { #other });
            }
        }
    }

    Ok(PartitionResult {
        mod_items,
        fn_def_calls,
        trait_constructions,
        impl_constructions,
    })
}

fn trait_to_decl_construction(item_trait: &ItemTrait) -> syn::Result<TokenStream2> {
    let name = item_trait.ident.to_string();
    let self_type_params = vec!["Self".to_string()];
    let mut fn_constructions = Vec::new();

    for item in &item_trait.items {
        if let TraitItem::Fn(method) = item {
            let fn_name = method.sig.ident.to_string();

            let params: Vec<(syn::Ident, syn::Type)> = method
                .sig
                .inputs
                .iter()
                .filter_map(|arg| {
                    if let syn::FnArg::Typed(pt) = arg
                        && let syn::Pat::Ident(pi) = &*pt.pat
                    {
                        return Some((pi.ident.clone(), (*pt.ty).clone()));
                    }
                    None
                })
                .collect();

            let param_constructions: Vec<TokenStream2> = params
                .iter()
                .map(|(param_name, ty)| {
                    let name_str = param_name.to_string();
                    let rt = map_type_to_runtime(ty, &self_type_params)?;
                    Ok(quote! {
                        ::structured_agent_runtime::Parameter::new(#name_str.to_string(), #rt)
                    })
                })
                .collect::<syn::Result<_>>()?;

            let ret_ty = match &method.sig.output {
                syn::ReturnType::Default => None,
                syn::ReturnType::Type(_, ty) => Some(ty.as_ref()),
            };
            let return_type_expr = ret_ty.map_or_else(
                || Ok(quote! { ::structured_agent_runtime::Type::unit() }),
                |t| map_type_to_runtime(t, &self_type_params),
            )?;

            fn_constructions.push(quote! {
                ::structured_agent_il::NativeTraitFnDecl {
                    name: #fn_name.to_string(),
                    parameters: vec![#(#param_constructions),*],
                    return_type: #return_type_expr,
                }
            });
        }
    }

    Ok(quote! {
        ::structured_agent_il::NativeTraitDecl {
            name: #name.to_string(),
            functions: vec![#(#fn_constructions),*],
        }
    })
}

fn impl_mod_to_decl(
    impl_args: SaImplArgs,
    item_mod: ItemMod,
) -> syn::Result<(TokenStream2, TokenStream2)> {
    let mod_ident = &item_mod.ident;
    let items = match item_mod.content {
        Some((_, items)) => items,
        None => {
            return Err(syn::Error::new_spanned(
                &item_mod.ident,
                "#[sa_impl] requires an inline mod block",
            ));
        }
    };

    let mut generated_fns = Vec::new();
    let mut fn_def_names = Vec::new();

    for item in items {
        if let Item::Fn(mut item_fn) = item {
            let sa_fn_pos = item_fn
                .attrs
                .iter()
                .position(|a| a.path().is_ident("sa_fn"));
            if let Some(pos) = sa_fn_pos {
                let attr_tokens = match &item_fn.attrs[pos].meta {
                    syn::Meta::List(ml) => ml.tokens.clone(),
                    _ => proc_macro2::TokenStream::new(),
                };
                item_fn.attrs.remove(pos);
                let fn_name = item_fn.sig.ident.to_string();
                fn_def_names.push(format_ident!("{}_native_def", fn_name));
                generated_fns.push(generate_native_function(attr_tokens, item_fn)?);
            }
        }
    }

    let def_constructions: Vec<TokenStream2> = fn_def_names
        .iter()
        .map(|name| quote! { #mod_ident::#name() })
        .collect();

    let type_name = &impl_args.type_name;
    let trait_name_expr = match impl_args.trait_name {
        Some(ref t) => quote! { Some(#t.to_string()) },
        None => quote! { None },
    };

    let submod_code = quote! {
        mod #mod_ident {
            #(#generated_fns)*
        }
    };

    let construction = quote! {
        ::structured_agent_il::NativeImplDecl {
            type_name: #type_name.to_string(),
            trait_name: #trait_name_expr,
            functions: vec![#(#def_constructions),*],
        }
    };

    Ok((submod_code, construction))
}
