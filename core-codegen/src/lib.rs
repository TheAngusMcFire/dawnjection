use proc_macro::TokenStream;
use syn::ItemFn;

#[proc_macro_attribute]
pub fn with_di(_args: TokenStream, input: TokenStream) -> TokenStream {
    let ast: ItemFn = syn::parse(input.clone()).unwrap();
    dawnjection_codegen_lib::consumer_with_di(ast).into()
}
