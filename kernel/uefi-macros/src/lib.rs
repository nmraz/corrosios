extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, LitStr};

#[proc_macro]
pub fn u16cstr(input: TokenStream) -> TokenStream {
    let lit = parse_macro_input!(input as LitStr).value();
    let mut encoded = Vec::with_capacity(lit.len());

    ucs2::encode_with(&lit, |c| {
        if c == 0 {
            panic!("embedded nul in `U16CStr` literal");
        }

        encoded.push(c);

        Ok(())
    })
    .expect("invalid UCS-2 in `U16CStr` literal");

    let expanded = quote! {
        unsafe {
            ::uefi::U16CStr::from_u16s_with_nul_unchecked(&[
                #(#encoded),*, 0
            ])
        }
    };
    expanded.into()
}
