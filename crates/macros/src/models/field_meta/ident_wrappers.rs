// use syn::Expr::
macro_rules! create_ident_wrapper {
    ($ident:ident) => {
        #[derive(Clone, Debug, PartialEq, Eq, Hash)]
        pub struct $ident(::syn::Ident);

        impl $ident {
            pub fn into_inner(self) -> ::syn::Ident {
                self.0
            }
        }

        impl ::darling::FromMeta for $ident {
            fn from_expr(expr: &::syn::Expr) -> ::darling::Result<Self> {
                match expr {
                    ::syn::Expr::Lit(expr) => {
                        let ::syn::Lit::Str(lit_str) = &expr.lit else {
                            return Err(darling::Error::custom("Expected a string literal."));
                        };
                        // `Ident::new` panics on text that isn't an identifier, so it is parsed
                        // first (UMB-031); a keyword, such as the table `type`, is taken as a raw
                        // identifier.
                        let value = lit_str.value();
                        let ident = ::syn::parse_str::<::syn::Ident>(&value)
                            .or_else(|_| ::syn::parse_str::<::syn::Ident>(&format!("r#{value}")))
                            .map_err(|_| {
                                ::darling::Error::custom(format!(
                                    "`{value}` is not a name an identifier can hold"
                                ))
                                .with_span(lit_str)
                            })?;
                        let mut ident = ident;
                        ident.set_span(lit_str.span());
                        Ok($ident(ident))
                    }
                    ::syn::Expr::Path(expr_path) => {
                        let ident = expr_path
                            .path
                            .get_ident()
                            .ok_or_else(|| ::darling::Error::custom("Expected an identifier."))?;
                        Ok($ident(ident.clone()))
                    }
                    _ => Err(darling::Error::custom("Expected a string literal.")),
                }
            }
        }

        impl ::std::ops::Deref for $ident {
            type Target = ::syn::Ident;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl ::std::convert::From<::syn::Ident> for $ident {
            fn from(ident: ::syn::Ident) -> Self {
                Self(ident)
            }
        }

        impl ::std::convert::From<$ident> for ::syn::Ident {
            fn from(ident: $ident) -> Self {
                ident.0
            }
        }

        impl ::quote::ToTokens for $ident {
            fn to_tokens(&self, tokens: &mut ::proc_macro2::TokenStream) {
                self.0.to_tokens(tokens)
            }
        }
    };
}
pub(crate) use create_ident_wrapper;
