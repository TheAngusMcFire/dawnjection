use proc_macro::TokenStream;
use syn::{DeriveInput, ItemFn};

#[proc_macro_attribute]
pub fn with_di(_args: TokenStream, input: TokenStream) -> TokenStream {
    let ast: ItemFn = syn::parse(input.clone()).unwrap();
    dawnjection_codegen_lib::with_di(ast, false).into()
}

#[proc_macro_attribute]
pub fn handler_with_di(_args: TokenStream, input: TokenStream) -> TokenStream {
    let ast: ItemFn = syn::parse(input.clone()).unwrap();
    dawnjection_codegen_lib::with_di(ast, true).into()
}

#[proc_macro_derive(FromDi, attributes())]
pub fn postgres_entity(input: TokenStream) -> TokenStream {
    let ast: DeriveInput = syn::parse(input).expect("Couldn't parse item");
    dawnjection_codegen_lib::struct_from_di::generate_from_di_function_for_struct(&ast).into()
}
