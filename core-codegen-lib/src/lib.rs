use proc_macro2::TokenStream;
use quote::format_ident;
use quote::quote;
use syn::{FnArg, GenericArgument, ItemFn, Pat, PathArguments, Type, TypePath};

#[allow(dead_code)]
#[derive(Debug)]
struct Argument {
    name: String,
    is_ref: bool,
    is_mut: bool,
    ty: String
}

fn get_args_from_function(sig: &syn::Signature) -> Vec<Argument> {
    let mut args = Vec::<Argument>::new();
    for a in &sig.inputs {
        let mut name = String::new();
        let mut ty = String::new();
        let mut is_ref = false;
        let mut is_mut = false;
        if let FnArg::Typed(x) = a {
            if let Pat::Ident(x) = x.pat.as_ref() {
                name = x.ident.to_string();
                is_mut = x.mutability.is_some();
            }
            if let Type::Path(x) = x.ty.as_ref()
            {
                process_segments(&mut ty, &x)
            } else if let Type::Reference(x) = x.ty.as_ref() {
                is_ref = true;
                if let Type::Path(x) = x.elem.as_ref()
                {
                    process_segments(&mut ty, &x)
                }
            }
        }
        args.push(Argument{
            name,
            is_ref,
            is_mut,
            ty,
        });
    }
    args
}

fn process_segments(ty: &mut String, x: &&TypePath) {
    for x in &x.path.segments {
        if !ty.is_empty() {
            ty.push_str("::")
        }
        ty.push_str(&x.ident.to_string());
        if let PathArguments::AngleBracketed(x) = &x.arguments {
            let mut genargs = Vec::<String>::new();
            for a in &x.args {
                if let GenericArgument::Type(Type::Path(x)) = a {
                    genargs.push(x.path.segments.first().unwrap().ident.to_string());
                }
            }
            if !genargs.is_empty() {
                ty.push_str(&format!("<{}>", genargs.join(",")));
            }
        }
    }
}

pub fn consumer_with_di(ast: ItemFn) -> TokenStream {
    let is_async = ast.sig.asyncness;
    let args = get_args_from_function(&ast.sig);
    let mut var_names = Vec::<syn::Ident>::new();

    let idents = args.iter().enumerate().map(|(i, x)| {
        let ty : syn::Type = syn::parse_str(&x.ty).unwrap();
        let varident = format_ident!("var{i}");

        let ts = if x.is_ref {
            quote!(let #varident = service_provider.try_get_ref::<#ty>().unwrap();)
        } else if x.ty.contains("MutexGuard") {
            quote!(let #varident = service_provider.try_get_mut::<#ty>().unwrap();)
        } else {
            quote!(let #varident = service_provider.try_get::<#ty>().unwrap();)
        };

        var_names.push(varident);

        ts
    });

    let ret = &ast.sig.output;


    let modast = ast.clone();
    let ident = modast.sig.ident.clone();
    let f_name = format_ident!("{}", ident.to_string());
    let f_name_new = format_ident!("{}_di", ident.to_string());
    let fn_await = is_async.map(|_| quote!(.await));

    let fin = quote!{
        #ast

        #is_async fn #f_name_new(service_provider: std::sync::Arc<lean_dev_rs::dependencyinjection::ServiceProvider>) #ret {
            #(#idents)*
            #f_name(#(#var_names,)*)#fn_await
        }

    };

    fin.into()
}

#[cfg(test)]
mod tests {

    #[test]
    fn basic_test() {
        let ts = quote::quote!(async fn consume_some_message(mut ctx: ConsumerContext<SomeMessage, TestSegment>, cnt : &std::tst::AtomicU64<i32>, client: &tokio_postgres::Client) -> Result<(), Report> {
            println!("this is cool");
            Ok(())
        });

        let ast = syn::parse2(ts.into()).unwrap();
        let ret = crate::consumer_with_di(ast);
        std::fs::write("/tmp/test.rs", format!("{}", ret)).unwrap();
    }
}