use proc_macro::TokenStream;
use syn::ItemFn;

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
