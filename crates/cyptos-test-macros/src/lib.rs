use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn};

/// Drop-in replacement for `#[test]`.
///
/// - **Host** (non-riscv64): expands to standard `#[test]`.
/// - **riscv64** (bare-metal): expands to `#[test_case]` for use with
///   `#![feature(custom_test_frameworks)]` and
///   `#![test_runner(cyptos_test::runner)]` in the consuming crate.
#[proc_macro_attribute]
pub fn cyptos_test(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);

    let expanded = quote! {
        #[cfg_attr(not(target_arch = "riscv64"), test)]
        #[cfg_attr(target_arch = "riscv64", test_case)]
        #input
    };

    expanded.into()
}
