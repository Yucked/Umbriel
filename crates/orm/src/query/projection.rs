use crate::{
    __private::ModelField, Alias, Field, Function, Param, Render, Renderer, ValueLike,
    kinds::KindOf,
};

mod sealed {
    /// Implemented under the same bounds as `FieldTuple`, so no other implementation of it can
    /// satisfy this supertrait.
    pub trait Sealed<M, V> {}
}

/// A tuple of 1 to 10 of `M`'s generated schema fields. `V` is the tuple of the fields' own types,
/// in the same order: `(User::schema().name, User::schema().age)` is a `FieldTuple<User, (String,
/// u8)>`. [`SurrealRead::select_fields`](crate::SurrealRead::select_fields) takes one.
///
/// Sealed: it is implemented for tuples only, and only when every element is a schema field of
/// `M`. A plain [`Field`], an expression or another model's field doesn't qualify, and another
/// crate can't implement it, even for its own model. Ten is the largest tuple SurrealDB's
/// `SurrealValue` decodes.
pub trait FieldTuple<M, V>: sealed::Sealed<M, V> {
    /// The fields as one array expression, `[a, b]`.
    #[doc(hidden)]
    fn into_array(self) -> ValueLike;
}

/// The array literal `[a, b]` over a list of fields.
struct FieldArray(Vec<Field>);

impl Render for FieldArray {
    fn render(&self, out: &mut Renderer) {
        out.push_str("[").push(&self.0).push_str("]");
    }
}

/// Implements [`FieldTuple`] for each listed tuple arity, as `(index Field Value, …)`.
macro_rules! field_tuples {
    ($(($($index:tt $field:ident $value:ident),+)),+ $(,)?) => {
        $(
            impl<M, $($field, $value),+> sealed::Sealed<M, ($($value,)+)> for ($($field,)+)
            where
                $($field: ModelField<M, $value> + Into<Field>,)+
            {
            }

            impl<M, $($field, $value),+> FieldTuple<M, ($($value,)+)> for ($($field,)+)
            where
                $($field: ModelField<M, $value> + Into<Field>,)+
            {
                fn into_array(self) -> ValueLike {
                    ValueLike::new(FieldArray(vec![$(self.$index.into()),+]))
                }
            }
        )+
    };
}

field_tuples! {
    (0 F0 V0),
    (0 F0 V0, 1 F1 V1),
    (0 F0 V0, 1 F1 V1, 2 F2 V2),
    (0 F0 V0, 1 F1 V1, 2 F2 V2, 3 F3 V3),
    (0 F0 V0, 1 F1 V1, 2 F2 V2, 3 F3 V3, 4 F4 V4),
    (0 F0 V0, 1 F1 V1, 2 F2 V2, 3 F3 V3, 4 F4 V4, 5 F5 V5),
    (0 F0 V0, 1 F1 V1, 2 F2 V2, 3 F3 V3, 4 F4 V4, 5 F5 V5, 6 F6 V6),
    (0 F0 V0, 1 F1 V1, 2 F2 V2, 3 F3 V3, 4 F4 V4, 5 F5 V5, 6 F6 V6, 7 F7 V7),
    (0 F0 V0, 1 F1 V1, 2 F2 V2, 3 F3 V3, 4 F4 V4, 5 F5 V5, 6 F6 V6, 7 F7 V7, 8 F8 V8),
    (0 F0 V0, 1 F1 V1, 2 F2 V2, 3 F3 V3, 4 F4 V4, 5 F5 V5, 6 F6 V6, 7 F7 V7, 8 F8 V8, 9 F9 V9),
}

/// Used to represent a list of projections to access fields of a table or
/// those of foreign tables and can even include filters.
#[derive(Debug, Clone, PartialEq)]
pub struct Projections(pub Vec<ValueLike>);

impl std::ops::Deref for Projections {
    type Target = Vec<ValueLike>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Render for Projections {
    fn render(&self, out: &mut Renderer) {
        self.0.render(out);
    }
}

impl From<Vec<ValueLike>> for Projections {
    fn from(value: Vec<ValueLike>) -> Self {
        Self(value)
    }
}

impl From<Vec<&ValueLike>> for Projections {
    fn from(value: Vec<&ValueLike>) -> Self {
        Self(value.into_iter().cloned().collect())
    }
}

impl From<&[ValueLike]> for Projections {
    fn from(value: &[ValueLike]) -> Self {
        Self(value.to_vec())
    }
}

/// Implements `From<$single>`, `From<Vec<$single>>` and `From<&[$single]>` for `Projections`.
macro_rules! projections_from {
    ($($single:ty),*) => {
        $(
            impl From<$single> for Projections {
                fn from(value: $single) -> Self {
                    Self(vec![value.into()])
                }
            }

            impl From<Vec<$single>> for Projections {
                fn from(value: Vec<$single>) -> Self {
                    Self(value.into_iter().map(Into::into).collect())
                }
            }

            impl From<&[$single]> for Projections {
                fn from(value: &[$single]) -> Self {
                    Self(value.iter().cloned().map(Into::into).collect())
                }
            }
        )*
    };
}

projections_from!(Field, Alias, Param);

impl<K: KindOf> From<Function<K>> for Projections {
    fn from(value: Function<K>) -> Self {
        Self(vec![value.into()])
    }
}

impl<K: KindOf> From<Vec<Function<K>>> for Projections {
    fn from(value: Vec<Function<K>>) -> Self {
        Self(value.into_iter().map(Into::into).collect())
    }
}

impl<K: KindOf> From<&[Function<K>]> for Projections {
    fn from(value: &[Function<K>]) -> Self {
        Self(value.iter().cloned().map(Into::into).collect())
    }
}

impl From<&Field> for Projections {
    fn from(value: &Field) -> Self {
        Self(vec![value.into()])
    }
}
