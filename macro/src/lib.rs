use proc_macro::TokenStream;
use syn::parse_macro_input;
use justcxx_build::{ast, wrapper,parser}; 

#[proc_macro]
pub fn bind(input: TokenStream) -> TokenStream {
    let ast_input = parse_macro_input!(input as ast::BindInput);
    let bind_context = parser::preprocess(&ast_input);
    let tokens = wrapper::generate_rust(&bind_context);
    tokens.into()
}