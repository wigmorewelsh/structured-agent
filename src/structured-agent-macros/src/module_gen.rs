use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{Item, ItemImpl, ItemMod, ItemTrait, Token, TraitItem};

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
    let outer_use_items: Vec<TokenStream2> = items
        .iter()
        .filter_map(|item| match item {
            Item::Use(u) => Some(quote! { #u }),
            _ => None,
        })
        .collect();

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
            Item::Impl(mut item_impl) => {
                let sa_impl_pos = item_impl
                    .attrs
                    .iter()
                    .position(|a| a.path().is_ident("sa_impl"));
                if let Some(pos) = sa_impl_pos {
                    let attr_tokens = match &item_impl.attrs[pos].meta {
                        syn::Meta::Path(_) => proc_macro2::TokenStream::new(),
                        syn::Meta::List(ml) => ml.tokens.clone(),
                        _ => {
                            return Err(syn::Error::new_spanned(
                                &*item_impl.self_ty,
                                "#[sa_impl] should be bare or #[sa_impl(Type for Trait)]",
                            ));
                        }
                    };
                    item_impl.attrs.remove(pos);
                    let (submod_code, construction) =
                        impl_block_to_decl(attr_tokens, item_impl, &outer_use_items)?;
                    mod_items.push(submod_code);
                    impl_constructions.push(construction);
                    continue;
                }
                mod_items.push(quote! { #item_impl });
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
    let mut use_items = Vec::new();

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
                    fn_def_names.push(format_ident!("{}_native_def", fn_name));
                    generated_fns.push(generate_native_function(attr_tokens, item_fn)?);
                }
            }
            Item::Use(use_item) => {
                use_items.push(quote! { #use_item });
            }
            _ => {}
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
            #(#use_items)*
            #(#generated_fns)*
        }
    };

    let construction = quote! {
        ::structured_agent_il::NativeImplDecl {
            type_name: #type_name.to_string(),
            type_params: vec![],
            trait_name: #trait_name_expr,
            functions: vec![#(#def_constructions),*],
        }
    };

    Ok((submod_code, construction))
}

fn impl_self_ident(item_impl: &ItemImpl) -> syn::Result<syn::Ident> {
    if let syn::Type::Path(tp) = item_impl.self_ty.as_ref()
        && let Some(seg) = tp.path.segments.last()
    {
        return Ok(seg.ident.clone());
    }
    Err(syn::Error::new_spanned(
        &*item_impl.self_ty,
        "#[sa_impl] on an impl block requires a simple type name",
    ))
}

fn rust_type_to_sa_name(ident: &syn::Ident) -> String {
    let name = ident.to_string();
    name.strip_suffix("Value").unwrap_or(&name).to_string()
}

fn rust_trait_to_sa_name(ident: &syn::Ident) -> String {
    ident.to_string()
}

fn replace_self_tokens(stream: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    use proc_macro2::TokenTree;
    let replacement = format_ident!("__sa_self");
    stream
        .into_iter()
        .map(|tt| match tt {
            TokenTree::Ident(ref ident) if ident == "self" => TokenTree::Ident(replacement.clone()),
            TokenTree::Group(group) => {
                let new_stream = replace_self_tokens(group.stream());
                TokenTree::Group(proc_macro2::Group::new(group.delimiter(), new_stream))
            }
            other => other,
        })
        .collect()
}

fn impl_method_to_item_fn(
    method: syn::ImplItemFn,
    self_ty: &syn::Type,
) -> syn::Result<syn::ItemFn> {
    let sa_self = format_ident!("__sa_self");
    let has_receiver = matches!(method.sig.inputs.first(), Some(syn::FnArg::Receiver(_)));

    let mut new_inputs = syn::punctuated::Punctuated::new();
    for arg in method.sig.inputs.iter() {
        match arg {
            syn::FnArg::Receiver(_) => {
                let typed: syn::FnArg = syn::parse_quote! { #sa_self: #self_ty };
                new_inputs.push(typed);
            }
            other => new_inputs.push(other.clone()),
        }
    }

    let block = if has_receiver {
        let orig_block = method.block;
        let block_tokens = replace_self_tokens(quote! { #orig_block });
        syn::parse2::<syn::Block>(block_tokens)?
    } else {
        method.block
    };

    let mut sig = method.sig;
    sig.inputs = new_inputs;
    Ok(syn::ItemFn {
        attrs: method.attrs,
        vis: method.vis,
        sig,
        block: Box::new(block),
    })
}

fn impl_block_to_decl(
    attr_tokens: TokenStream2,
    item_impl: ItemImpl,
    outer_use_items: &[TokenStream2],
) -> syn::Result<(TokenStream2, TokenStream2)> {
    let self_ident = impl_self_ident(&item_impl)?;
    let trait_ident = item_impl
        .trait_
        .as_ref()
        .and_then(|(_, path, _)| path.segments.last())
        .map(|s| s.ident.clone());

    let (type_name, trait_name) = if attr_tokens.is_empty() {
        (
            rust_type_to_sa_name(&self_ident),
            trait_ident.as_ref().map(rust_trait_to_sa_name),
        )
    } else {
        let impl_args: SaImplArgs = syn::parse2(attr_tokens)?;
        (impl_args.type_name, impl_args.trait_name)
    };

    let mut generated_fns = Vec::new();
    let mut fn_def_names = Vec::new();

    for item in item_impl.items {
        if let syn::ImplItem::Fn(method) = item {
            let fn_name = method.sig.ident.to_string();
            fn_def_names.push(format_ident!("{}_native_def", fn_name));
            let item_fn = impl_method_to_item_fn(method, &item_impl.self_ty)?;
            generated_fns.push(generate_native_function(
                proc_macro2::TokenStream::new(),
                item_fn,
            )?);
        }
    }

    let type_part = type_name.to_lowercase();
    let trait_part = trait_name.as_deref().unwrap_or("inherent").to_lowercase();
    let submod_name = format_ident!("{}_{}_impl", type_part, trait_part);

    let def_constructions: Vec<TokenStream2> = fn_def_names
        .iter()
        .map(|name| quote! { #submod_name::#name() })
        .collect();

    let trait_name_expr = match trait_name {
        Some(ref t) => quote! { Some(#t.to_string()) },
        None => quote! { None },
    };

    let submod_code = quote! {
        mod #submod_name {
            #(#outer_use_items)*
            #(#generated_fns)*
        }
    };

    let construction = quote! {
        ::structured_agent_il::NativeImplDecl {
            type_name: #type_name.to_string(),
            type_params: vec![],
            trait_name: #trait_name_expr,
            functions: vec![#(#def_constructions),*],
        }
    };

    Ok((submod_code, construction))
}
