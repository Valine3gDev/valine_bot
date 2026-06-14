use proc_macro::TokenStream;
use quote::quote;
use syn::{FnArg, ItemFn, ReturnType, parse_macro_input};

#[proc_macro_attribute]
pub fn event_handler(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let function = parse_macro_input!(item as ItemFn);

    if function.sig.asyncness.is_none() {
        return syn::Error::new_spanned(
            &function.sig.fn_token,
            "`#[event_handler]` can only be used on async functions",
        )
        .into_compile_error()
        .into();
    }

    if matches!(function.sig.output, ReturnType::Default) {
        return syn::Error::new_spanned(
            &function.sig.output,
            "`#[event_handler]` functions must return `Result<(), AppError>`",
        )
        .into_compile_error()
        .into();
    }

    if !function.sig.generics.params.is_empty() || function.sig.generics.where_clause.is_some() {
        return syn::Error::new_spanned(
            &function.sig.generics,
            "`#[event_handler]` functions cannot have generics",
        )
        .into_compile_error()
        .into();
    }

    if function.sig.inputs.len() != 2
        || function
            .sig
            .inputs
            .iter()
            .any(|input| !matches!(input, FnArg::Typed(_)))
    {
        return syn::Error::new_spanned(
            &function.sig.inputs,
            "`#[event_handler]` functions must take `&Context` and `&FullEvent`",
        )
        .into_compile_error()
        .into();
    }

    let attributes = function.attrs;
    let visibility = function.vis;
    let handler_name = function.sig.ident;
    let inputs = function.sig.inputs;
    let output = function.sig.output;
    let body = function.block;

    quote! {
        #(#attributes)*
        #[allow(non_camel_case_types)]
        #visibility struct #handler_name;

        #[::serenity::async_trait]
        impl crate::core::BotEventHandler for #handler_name {
            async fn dispatch(&self, #inputs) #output #body
        }
    }
    .into()
}

#[proc_macro_attribute]
pub fn event_error_handler(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let function = parse_macro_input!(item as ItemFn);

    if function.sig.asyncness.is_none() {
        return syn::Error::new_spanned(
            &function.sig.fn_token,
            "`#[event_error_handler]` can only be used on async functions",
        )
        .into_compile_error()
        .into();
    }

    if !matches!(function.sig.output, ReturnType::Default) {
        return syn::Error::new_spanned(
            &function.sig.output,
            "`#[event_error_handler]` functions must return `()`",
        )
        .into_compile_error()
        .into();
    }

    if !function.sig.generics.params.is_empty() || function.sig.generics.where_clause.is_some() {
        return syn::Error::new_spanned(
            &function.sig.generics,
            "`#[event_error_handler]` functions cannot have generics",
        )
        .into_compile_error()
        .into();
    }

    if function.sig.inputs.len() != 3
        || function
            .sig
            .inputs
            .iter()
            .any(|input| !matches!(input, FnArg::Typed(_)))
    {
        return syn::Error::new_spanned(
            &function.sig.inputs,
            "`#[event_error_handler]` functions must take `&Context`, `&FullEvent`, and `&AppError`",
        )
        .into_compile_error()
        .into();
    }

    let attributes = function.attrs;
    let visibility = function.vis;
    let handler_name = function.sig.ident;
    let inputs = function.sig.inputs;
    let body = function.block;

    quote! {
        #(#attributes)*
        #[allow(non_camel_case_types)]
        #visibility struct #handler_name;

        #[::serenity::async_trait]
        impl crate::core::BotEventErrorHandler for #handler_name {
            async fn handle(&self, #inputs) #body
        }
    }
    .into()
}
