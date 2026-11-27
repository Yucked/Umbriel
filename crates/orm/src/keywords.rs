//! `keywords!`: enums whose variants each stand for one SurrealQL word.

/// Declares an enum whose variants are SurrealQL words, from one table of `Variant => "word"`
/// rows. It generates `as_str`, the word for a value, and `Display`, which writes it. After
/// `: render`, it also implements [`Render`](crate::Render), which writes it inline.
///
/// Enums whose variants carry values stay hand-written.
macro_rules! keywords {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident $(: $render:ident)? {
            $($(#[$vmeta:meta])* $variant:ident => $word:literal),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        $vis enum $name {
            $($(#[$vmeta])* $variant,)+
        }

        impl $name {
            /// The word SurrealQL writes for this value.
            pub const fn as_str(&self) -> &'static str {
                match self {
                    $(Self::$variant => $word,)+
                }
            }
        }

        impl ::core::fmt::Display for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        $(keywords!(@ $render $name);)?
    };

    (@ render $name:ident) => {
        impl $crate::Render for $name {
            fn render(&self, out: &mut $crate::Renderer) {
                out.push_str(self.as_str());
            }
        }
    };
}

pub(crate) use keywords;
