use proc_macro::TokenStream;
use syn::parse_macro_input;
use justcxx_build::{ast, rust, preprocess}; 

#[proc_macro]
pub fn bind(input: TokenStream) -> TokenStream {
    let ast_input = parse_macro_input!(input as ast::BindInput);
    let bind_context = preprocess(&ast_input);
    let tokens = rust::generate_rust(&bind_context);
    tokens.into()
}