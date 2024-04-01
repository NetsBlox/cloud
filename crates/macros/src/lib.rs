use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Witness)]
pub fn derive_witness(input: TokenStream) -> TokenStream {
    let DeriveInput { ident, attrs, .. } = parse_macro_input!(input);
    // TODO: add _private field to struct
    let output = quote! {
        impl #ident {
            #[cfg(test)]
            pub(crate) fn test() -> Self {
                // TODO: get the inputs and pass them through
                Self {
                    _private: ()
                }
            }
        }
    };

    output.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
