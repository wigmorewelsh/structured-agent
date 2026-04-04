mod convert;
mod extract;
mod fn_gen;
mod module_gen;
mod types;

use fn_gen::generate_native_function;
use module_gen::generate_module;
use proc_macro::TokenStream;
use syn::{ItemFn, ItemMod, parse_macro_input};

#[proc_macro_attribute]
pub fn sa_fn(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    generate_native_function(attr.into(), input)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

#[proc_macro_attribute]
pub fn sa_module(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemMod);
    generate_module(input)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}
