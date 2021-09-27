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

#[proc_macro]
pub fn guid(input: TokenStream) -> TokenStream {
    let lit = parse_macro_input!(input as LitStr).value();
    let parts: [_; 5] = lit
        .split('-')
        .collect::<Vec<_>>()
        .try_into()
        .expect("invalid GUID");

    let time_low = parse_hex(parts[0], 8) as u32;
    let time_mid = parse_hex(parts[1], 4) as u16;
    let time_high_ver = parse_hex(parts[2], 4) as u16;
    let clock = (parse_hex(parts[3], 4) as u16).to_be_bytes();
    let node = &parse_hex(parts[4], 12).to_be_bytes()[2..];

    let expanded = quote! {
        ::uefi::Guid(#time_low, #time_mid, #time_high_ver, [#(#clock),*, #(#node),*])
    };
    expanded.into()
}

fn parse_hex(input: &str, digits: usize) -> u64 {
    if input.len() != digits {
        panic!("invalid GUID");
    }

    u64::from_str_radix(input, 16).expect("invalid hex in GUID")
}
