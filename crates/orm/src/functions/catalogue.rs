//! `functions!`: one table per namespace generates every wrapper of a built-in function.
//!
//! Each entry generates:
//! - `<name>_fn(args) -> Function<K>`, the call;
//! - the macro `<prefix>_<name>!` (exported at the crate root) and its alias `<name>!` in the
//!   namespace module, which forward to the function;
//! - for each `[Receiver.method]` pair, the method form `receiver.method(args)` as an inherent
//!   method on that typed expression. A pair is listed only where SurrealDB 3.3.4 dispatches the
//!   method on that receiver (`fnc/mod.rs` `idiom()`); nothing is derived from the path.
//!
//! The table starts with `$` (the generated macros need it), the namespace path and its separator
//! (`"string" "::"`, or `"" ""` for a function at the root such as `rand`), the macro prefix and
//! the module path the generated macros call through. An entry is
//!
//! ```text
//! /// Docs.
//! name [= sql_name] (ARGS) -> Kind [Receiver.method, Receiver.rust_name = sql_method];
//! ```
//!
//! `sql_name` is given when the SurrealQL name is not a usable Rust name (`enum_ = enum`). ARGS is
//! one of:
//! - `a: T, b: U`: fixed arguments (or none);
//! - `a: T; b: U` and `a: T; b: U, c: V`: then one or two optional trailing arguments;
//! - `; b: U` and `; b: U, c: V`: only optional arguments;
//! - `; [b: U, c: V]`: two arguments given together or not at all;
//! - `.. values: T`: any number of arguments of one type.
//!
//! Every argument type is a typed expression re-exported at the crate root (`StrandLike`,
//! `ValueLike`, …). Optional arguments are `Option<impl Into<T>>` in the function and the
//! method, and may be left out of the macro.

/// See the module documentation.
macro_rules! functions {
    ($d:tt $ns:literal $sep:literal, $prefix:ident, $($module:ident)::*; $($entries:tt)*) => {
        functions!(@munch [$d $ns $sep $prefix [$($module)::*]] $($entries)*);
    };

    (@munch $ctx:tt) => {};
    (@munch $ctx:tt
        $(#[$attr:meta])*
        $name:ident $(= $sql:ident)? ($($args:tt)*) -> $kind:ty $([$($methods:tt)*])?;
        $($rest:tt)*
    ) => {
        functions!(@entry $ctx [$(#[$attr])*] [$($sql)? $name] $name [$($args)*] [$kind]
            [$($($methods)*)?]);
        functions!(@munch $ctx $($rest)*);
    };

    // The first of the given names: the explicit SurrealQL name when there is one.
    (@str $first:ident $($rest:ident)?) => {
        ::core::stringify!($first)
    };

    // `name(.. values)`
    (@entry [$d:tt $ns:literal $sep:literal $prefix:ident [$($module:ident)::*]]
        [$($attr:tt)*] [$sql:ident $($_name:ident)?] $name:ident
        [.. $rest:ident : $rty:ident] [$kind:ty] [$($methods:tt)*]
    ) => {
        paste::paste! {
            $($attr)*
            #[doc = ::core::concat!("\n\nCalls `", $ns, $sep, ::core::stringify!($sql), "(…)` with each of `",
                ::core::stringify!($rest), "` as an argument.")]
            pub fn [<$sql _fn>]<T: Into<$crate::$rty>>($rest: impl IntoIterator<Item = T>) -> $crate::Function<$kind> {
                $crate::Function::new(::core::concat!($ns, $sep, ::core::stringify!($sql)))
                    .args($rest.into_iter().map(|value| Into::<$crate::$rty>::into(value)))
            }

            #[doc = ::core::concat!("`", $ns, $sep, ::core::stringify!($sql), "(…)`: one argument is the list of \
                arguments; several are the arguments themselves. Calls `", ::core::stringify!($sql),
                "_fn`.")]
            #[macro_export]
            macro_rules! [<$prefix _ $sql>] {
                ($d values:expr $d(,)?) => {
                    $crate::functions::$($module::)*[<$sql _fn>]($d values)
                };
                ($d($d value:expr),+ $d(,)?) => {
                    $crate::functions::$($module::)*[<$sql _fn>](
                        ::std::vec![$d($crate::$rty::from($d value)),+]
                    )
                };
            }
            pub use [<$prefix _ $sql>] as $name;
        }
        functions!(@rest_methods [$($methods)*] $ns $sep $sql $rest $rty [$kind]);
    };

    // `name()`
    (@entry [$d:tt $ns:literal $sep:literal $prefix:ident [$($module:ident)::*]]
        [$($attr:tt)*] [$sql:ident $($_name:ident)?] $name:ident [] [$kind:ty] []
    ) => {
        paste::paste! {
            $($attr)*
            #[doc = ::core::concat!("\n\nCalls `", $ns, $sep, ::core::stringify!($sql), "()`.")]
            pub fn [<$sql _fn>]() -> $crate::Function<$kind> {
                $crate::Function::new(::core::concat!($ns, $sep, ::core::stringify!($sql)))
            }

            #[doc = ::core::concat!("`", $ns, $sep, ::core::stringify!($sql), "()`; calls `", ::core::stringify!($sql),
                "_fn`.")]
            #[macro_export]
            macro_rules! [<$prefix _ $sql>] {
                () => {
                    $crate::functions::$($module::)*[<$sql _fn>]()
                };
            }
            pub use [<$prefix _ $sql>] as $name;
        }
    };

    // `name(; [a, b])`: both arguments or neither.
    (@entry [$d:tt $ns:literal $sep:literal $prefix:ident [$($module:ident)::*]]
        [$($attr:tt)*] [$sql:ident $($_name:ident)?] $name:ident
        [; [$o1:ident : $o1t:ident, $o2:ident : $o2t:ident]] [$kind:ty] []
    ) => {
        paste::paste! {
            $($attr)*
            #[doc = ::core::concat!("\n\nCalls `", $ns, $sep, ::core::stringify!($sql), "()`, or `", $ns, $sep,
                ::core::stringify!($sql), "(", ::core::stringify!($o1), ", ", ::core::stringify!($o2),
                ")` when both are given.")]
            pub fn [<$sql _fn>](
                $o1: Option<impl Into<$crate::$o1t>>,
                $o2: Option<impl Into<$crate::$o2t>>,
            ) -> $crate::Function<$kind> {
                let function = $crate::Function::new(::core::concat!($ns, $sep, ::core::stringify!($sql)));
                match ($o1, $o2) {
                    (Some($o1), Some($o2)) => function
                        .arg(Into::<$crate::$o1t>::into($o1))
                        .arg(Into::<$crate::$o2t>::into($o2)),
                    (Some($o1), None) => function.arg(Into::<$crate::$o1t>::into($o1)),
                    (None, Some($o2)) => function.arg(Into::<$crate::$o2t>::into($o2)),
                    (None, None) => function,
                }
            }

            #[doc = ::core::concat!("`", $ns, $sep, ::core::stringify!($sql), "()` or `", $ns, $sep,
                ::core::stringify!($sql), "(", ::core::stringify!($o1), ", ", ::core::stringify!($o2), ")`; calls `",
                ::core::stringify!($sql), "_fn`.")]
            #[macro_export]
            macro_rules! [<$prefix _ $sql>] {
                () => {
                    $crate::functions::$($module::)*[<$sql _fn>](
                        None::<$crate::$o1t>,
                        None::<$crate::$o2t>,
                    )
                };
                ($d $o1:expr, $d $o2:expr $d(,)?) => {
                    $crate::functions::$($module::)*[<$sql _fn>](Some($d $o1), Some($d $o2))
                };
            }
            pub use [<$prefix _ $sql>] as $name;
        }
    };

    // `name(; a)`; a method's receiver is `a`.
    (@entry [$d:tt $ns:literal $sep:literal $prefix:ident [$($module:ident)::*]]
        [$($attr:tt)*] [$sql:ident $($_name:ident)?] $name:ident
        [; $o1:ident : $o1t:ident] [$kind:ty] [$($methods:tt)*]
    ) => {
        paste::paste! {
            $($attr)*
            #[doc = ::core::concat!("\n\nCalls `", $ns, $sep, ::core::stringify!($sql), "(", ::core::stringify!($o1),
                ")`, or `", $ns, $sep, ::core::stringify!($sql), "()` without it.")]
            pub fn [<$sql _fn>]($o1: Option<impl Into<$crate::$o1t>>) -> $crate::Function<$kind> {
                let function = $crate::Function::new(::core::concat!($ns, $sep, ::core::stringify!($sql)));
                match $o1 {
                    Some($o1) => function.arg(Into::<$crate::$o1t>::into($o1)),
                    None => function,
                }
            }

            #[doc = ::core::concat!("`", $ns, $sep, ::core::stringify!($sql), "(", ::core::stringify!($o1),
                ")`, the argument optional; calls `", ::core::stringify!($sql), "_fn`.")]
            #[macro_export]
            macro_rules! [<$prefix _ $sql>] {
                () => {
                    $crate::functions::$($module::)*[<$sql _fn>](None::<$crate::$o1t>)
                };
                ($d $o1:expr $d(,)?) => {
                    $crate::functions::$($module::)*[<$sql _fn>](Some($d $o1))
                };
            }
            pub use [<$prefix _ $sql>] as $name;
        }
        functions!(@methods [$($methods)*] $ns $sep $sql [] [] [$kind]);
    };

    // `name(; a, b)`
    (@entry [$d:tt $ns:literal $sep:literal $prefix:ident [$($module:ident)::*]]
        [$($attr:tt)*] [$sql:ident $($_name:ident)?] $name:ident
        [; $o1:ident : $o1t:ident, $o2:ident : $o2t:ident] [$kind:ty] []
    ) => {
        paste::paste! {
            $($attr)*
            #[doc = ::core::concat!("\n\nCalls `", $ns, $sep, ::core::stringify!($sql), "(", ::core::stringify!($o1),
                ", ", ::core::stringify!($o2), ")`, leaving out the arguments not given.")]
            pub fn [<$sql _fn>](
                $o1: Option<impl Into<$crate::$o1t>>,
                $o2: Option<impl Into<$crate::$o2t>>,
            ) -> $crate::Function<$kind> {
                let function = $crate::Function::new(::core::concat!($ns, $sep, ::core::stringify!($sql)));
                let function = match $o1 {
                    Some($o1) => function.arg(Into::<$crate::$o1t>::into($o1)),
                    None => function,
                };
                match $o2 {
                    Some($o2) => function.arg(Into::<$crate::$o2t>::into($o2)),
                    None => function,
                }
            }

            #[doc = ::core::concat!("`", $ns, $sep, ::core::stringify!($sql), "(", ::core::stringify!($o1), ", ",
                ::core::stringify!($o2), ")`, both arguments optional; calls `", ::core::stringify!($sql),
                "_fn`.")]
            #[macro_export]
            macro_rules! [<$prefix _ $sql>] {
                () => {
                    $crate::functions::$($module::)*[<$sql _fn>](
                        None::<$crate::$o1t>,
                        None::<$crate::$o2t>,
                    )
                };
                ($d $o1:expr $d(,)?) => {
                    $crate::functions::$($module::)*[<$sql _fn>](Some($d $o1), None::<$crate::$o2t>)
                };
                ($d $o1:expr, $d $o2:expr $d(,)?) => {
                    $crate::functions::$($module::)*[<$sql _fn>](Some($d $o1), Some($d $o2))
                };
            }
            pub use [<$prefix _ $sql>] as $name;
        }
    };

    // `name(a, b; c)`
    (@entry [$d:tt $ns:literal $sep:literal $prefix:ident [$($module:ident)::*]]
        [$($attr:tt)*] [$sql:ident $($_name:ident)?] $name:ident
        [$a1:ident : $t1:ident $(, $arg:ident : $ty:ident)* ; $o1:ident : $o1t:ident]
        [$kind:ty] [$($methods:tt)*]
    ) => {
        paste::paste! {
            $($attr)*
            #[doc = ::core::concat!("\n\nCalls `", $ns, $sep, ::core::stringify!($sql), "(", ::core::stringify!($a1),
                $(", ", ::core::stringify!($arg),)* ", ", ::core::stringify!($o1), ")`, leaving out `",
                ::core::stringify!($o1), "` when it is not given.")]
            pub fn [<$sql _fn>](
                $a1: impl Into<$crate::$t1>,
                $($arg: impl Into<$crate::$ty>,)*
                $o1: Option<impl Into<$crate::$o1t>>,
            ) -> $crate::Function<$kind> {
                let function = $crate::Function::new(::core::concat!($ns, $sep, ::core::stringify!($sql)))
                    .arg(Into::<$crate::$t1>::into($a1))
                    $(.arg(Into::<$crate::$ty>::into($arg)))*;
                match $o1 {
                    Some($o1) => function.arg(Into::<$crate::$o1t>::into($o1)),
                    None => function,
                }
            }

            #[doc = ::core::concat!("`", $ns, $sep, ::core::stringify!($sql), "(", ::core::stringify!($a1),
                $(", ", ::core::stringify!($arg),)* ", ", ::core::stringify!($o1), ")`, the last argument \
                optional; calls `", ::core::stringify!($sql), "_fn`.")]
            #[macro_export]
            macro_rules! [<$prefix _ $sql>] {
                ($d $a1:expr $(, $d $arg:expr)* $d(,)?) => {
                    $crate::functions::$($module::)*[<$sql _fn>](
                        $d $a1, $($d $arg,)* None::<$crate::$o1t>
                    )
                };
                ($d $a1:expr $(, $d $arg:expr)*, $d $o1:expr $d(,)?) => {
                    $crate::functions::$($module::)*[<$sql _fn>](
                        $d $a1, $($d $arg,)* Some($d $o1)
                    )
                };
            }
            pub use [<$prefix _ $sql>] as $name;
        }
        functions!(@methods [$($methods)*] $ns $sep $sql [$($arg : $ty),*] [$o1 : $o1t]
            [$kind]);
    };

    // `name(a, b; c, d)`
    (@entry [$d:tt $ns:literal $sep:literal $prefix:ident [$($module:ident)::*]]
        [$($attr:tt)*] [$sql:ident $($_name:ident)?] $name:ident
        [$a1:ident : $t1:ident $(, $arg:ident : $ty:ident)* ;
            $o1:ident : $o1t:ident, $o2:ident : $o2t:ident]
        [$kind:ty] [$($methods:tt)*]
    ) => {
        paste::paste! {
            $($attr)*
            #[doc = ::core::concat!("\n\nCalls `", $ns, $sep, ::core::stringify!($sql), "(", ::core::stringify!($a1),
                $(", ", ::core::stringify!($arg),)* ", ", ::core::stringify!($o1), ", ", ::core::stringify!($o2),
                ")`, leaving out the optional arguments not given.")]
            pub fn [<$sql _fn>](
                $a1: impl Into<$crate::$t1>,
                $($arg: impl Into<$crate::$ty>,)*
                $o1: Option<impl Into<$crate::$o1t>>,
                $o2: Option<impl Into<$crate::$o2t>>,
            ) -> $crate::Function<$kind> {
                let function = $crate::Function::new(::core::concat!($ns, $sep, ::core::stringify!($sql)))
                    .arg(Into::<$crate::$t1>::into($a1))
                    $(.arg(Into::<$crate::$ty>::into($arg)))*;
                let function = match $o1 {
                    Some($o1) => function.arg(Into::<$crate::$o1t>::into($o1)),
                    None => function,
                };
                match $o2 {
                    Some($o2) => function.arg(Into::<$crate::$o2t>::into($o2)),
                    None => function,
                }
            }

            #[doc = ::core::concat!("`", $ns, $sep, ::core::stringify!($sql), "(", ::core::stringify!($a1),
                $(", ", ::core::stringify!($arg),)* ", ", ::core::stringify!($o1), ", ", ::core::stringify!($o2),
                ")`, the last two arguments optional; calls `", ::core::stringify!($sql), "_fn`.")]
            #[macro_export]
            macro_rules! [<$prefix _ $sql>] {
                ($d $a1:expr $(, $d $arg:expr)* $d(,)?) => {
                    $crate::functions::$($module::)*[<$sql _fn>](
                        $d $a1, $($d $arg,)* None::<$crate::$o1t>, None::<$crate::$o2t>
                    )
                };
                ($d $a1:expr $(, $d $arg:expr)*, $d $o1:expr $d(,)?) => {
                    $crate::functions::$($module::)*[<$sql _fn>](
                        $d $a1, $($d $arg,)* Some($d $o1), None::<$crate::$o2t>
                    )
                };
                ($d $a1:expr $(, $d $arg:expr)*, $d $o1:expr, $d $o2:expr $d(,)?) => {
                    $crate::functions::$($module::)*[<$sql _fn>](
                        $d $a1, $($d $arg,)* Some($d $o1), Some($d $o2)
                    )
                };
            }
            pub use [<$prefix _ $sql>] as $name;
        }
        functions!(@methods [$($methods)*] $ns $sep $sql [$($arg : $ty),*]
            [$o1 : $o1t, $o2 : $o2t] [$kind]);
    };

    // `name(a, b)`
    (@entry [$d:tt $ns:literal $sep:literal $prefix:ident [$($module:ident)::*]]
        [$($attr:tt)*] [$sql:ident $($_name:ident)?] $name:ident
        [$a1:ident : $t1:ident $(, $arg:ident : $ty:ident)*] [$kind:ty] [$($methods:tt)*]
    ) => {
        paste::paste! {
            $($attr)*
            #[doc = ::core::concat!("\n\nCalls `", $ns, $sep, ::core::stringify!($sql), "(", ::core::stringify!($a1),
                $(", ", ::core::stringify!($arg),)* ")`.")]
            pub fn [<$sql _fn>](
                $a1: impl Into<$crate::$t1>,
                $($arg: impl Into<$crate::$ty>,)*
            ) -> $crate::Function<$kind> {
                $crate::Function::new(::core::concat!($ns, $sep, ::core::stringify!($sql)))
                    .arg(Into::<$crate::$t1>::into($a1))
                    $(.arg(Into::<$crate::$ty>::into($arg)))*
            }

            #[doc = ::core::concat!("`", $ns, $sep, ::core::stringify!($sql), "(", ::core::stringify!($a1),
                $(", ", ::core::stringify!($arg),)* ")`; calls `", ::core::stringify!($sql), "_fn`.")]
            #[macro_export]
            macro_rules! [<$prefix _ $sql>] {
                ($d $a1:expr $(, $d $arg:expr)* $d(,)?) => {
                    $crate::functions::$($module::)*[<$sql _fn>]($d $a1 $(, $d $arg)*)
                };
            }
            pub use [<$prefix _ $sql>] as $name;
        }
        functions!(@methods [$($methods)*] $ns $sep $sql [$($arg : $ty),*] [] [$kind]);
    };

    // One method per `Receiver.method` pair: the receiver is the call's first argument (or its
    // only optional one), `args` the arguments after it and `optional` the optional ones.
    (@methods [] $ns:literal $sep:literal $sql:ident $args:tt $optional:tt $kind:tt) => {};
    (@methods
        [$recv:ident . $method:ident $(= $msql:ident)? $(, $($more:tt)*)?]
        $ns:literal $sep:literal $sql:ident
        [$($arg:ident : $ty:ident),*] [$($opt:ident : $optty:ident),*] [$kind:ty]
    ) => {
        impl $crate::$recv {
            #[doc = ::core::concat!("The method form of `", $ns, $sep, ::core::stringify!($sql),
                "`, with this value as its first argument.")]
            pub fn $method(
                &self,
                $($arg: impl Into<$crate::$ty>,)*
                $($opt: Option<impl Into<$crate::$optty>>,)*
            ) -> $crate::Function<$kind> {
                let function = $crate::Function::method(
                    $crate::ValueLike::from(self.clone()),
                    functions!(@str $($msql)? $method),
                )
                $(.arg(Into::<$crate::$ty>::into($arg)))*;
                $(
                    let function = match $opt {
                        Some($opt) => function.arg(Into::<$crate::$optty>::into($opt)),
                        None => function,
                    };
                )*
                function
            }
        }
        functions!(@methods [$($($more)*)?] $ns $sep $sql [$($arg : $ty),*]
            [$($opt : $optty),*] [$kind]);
    };

    // The method form of `name(.. values)`: the receiver is the first value.
    (@rest_methods [] $ns:literal $sep:literal $sql:ident $rest:ident $rty:ident $kind:tt) => {};
    (@rest_methods
        [$recv:ident . $method:ident $(= $msql:ident)? $(, $($more:tt)*)?]
        $ns:literal $sep:literal $sql:ident $rest:ident $rty:ident [$kind:ty]
    ) => {
        impl $crate::$recv {
            #[doc = ::core::concat!("The method form of `", $ns, $sep, ::core::stringify!($sql),
                "`, with this value as its first argument and `", ::core::stringify!($rest),
                "` as the others.")]
            pub fn $method<T: Into<$crate::$rty>>(
                &self,
                $rest: impl IntoIterator<Item = T>,
            ) -> $crate::Function<$kind> {
                $crate::Function::method(
                    $crate::ValueLike::from(self.clone()),
                    functions!(@str $($msql)? $method),
                )
                .args($rest.into_iter().map(|value| Into::<$crate::$rty>::into(value)))
            }
        }
        functions!(@rest_methods [$($($more)*)?] $ns $sep $sql $rest $rty [$kind]);
    };
}
