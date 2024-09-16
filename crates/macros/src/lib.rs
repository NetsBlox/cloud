use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Witness)]
pub fn derive_witness(input: TokenStream) -> TokenStream {
    let DeriveInput { ident, attrs, .. } = parse_macro_input!(input);
    // TODO: add _private field to struct
    let output = quote! {
        impl #ident {
            // "new" shouldn't be pub(crate) so it can only be instantiated in the defining file/module
            // as a bonus, this would disallow anyone from trying to define a new "new" function for a Witness (already defined!)
            fn new() -> Self {
                todo!();
            }

            #[cfg(test)]
            pub(crate) fn test(
                // Figure out the attributes and make equivalent arguments here
            ) -> Self {
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
