use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput};

pub fn generate_from_di_function_for_struct(ast: &DeriveInput) -> TokenStream {
    let name = &ast.ident;

    let s = match ast.data {
        Data::Struct(ref s) => s,
        _ => panic!("Enums or Unions can not be mapped"),
    };

    let fields = s.fields.iter().map(|field| {
        let ident = field.ident.as_ref().unwrap();
        quote::quote! {
            #ident: sp.try_get()?
        }
    });

    let tokens = quote! {
        impl FromDi for #name {
            fn from_di(sp: &ServiceProvider) -> Option<Self> {
                Some(Self {
                     #(#fields),*
                 })
            }
        }
    };
    tokens
}

#[cfg(test)]
mod tests {
    use super::generate_from_di_function_for_struct;

    #[test]
    fn basic_test() {
        let ts = quote::quote!(
            pub struct SomeEntity {
                id: i32,
                name: String,
            }
        );

        let ast: syn::DeriveInput = syn::parse2(ts).expect("Couldn't parse item");
        let ret = generate_from_di_function_for_struct(&ast);

        std::fs::write("/tmp/test-struct-derive.rs", format!("{}", ret)).unwrap();
    }
}
