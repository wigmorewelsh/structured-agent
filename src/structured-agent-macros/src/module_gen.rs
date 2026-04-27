use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{Item, ItemMod};

use crate::fn_gen::{generate_native_function, to_pascal_case};

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

    let (generated_fns, fn_def_fn_names, other_items) = partition_items(items)?;

    let def_constructions: Vec<TokenStream2> = fn_def_fn_names
        .iter()
        .map(|name| quote! { #name() })
        .collect();

    Ok(quote! {
        #vis mod #mod_name {
            #(#other_items)*
            #(#generated_fns)*

            pub struct #module_struct;

            impl ::structured_agent_il::Module for #module_struct {
                fn name(&self) -> &str {
                    #mod_name_str
                }

                fn native_functions(&self) -> Vec<::structured_agent_il::NativeFunctionDef> {
                    vec![#(#def_constructions),*]
                }
            }
        }
    })
}

fn partition_items(
    items: Vec<Item>,
) -> syn::Result<(
    Vec<TokenStream2>,
    Vec<proc_macro2::Ident>,
    Vec<TokenStream2>,
)> {
    let mut generated_fns = Vec::new();
    let mut fn_def_fn_names = Vec::new();
    let mut other_items = Vec::new();

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
                fn_def_fn_names.push(format_ident!("{}_native_def", fn_name));
                generated_fns.push(generate_native_function(attr_tokens, item_fn)?);
                continue;
            }
            other_items.push(quote! { #item_fn });
        } else {
            other_items.push(quote! { #item });
        }
    }

    Ok((generated_fns, fn_def_fn_names, other_items))
}
