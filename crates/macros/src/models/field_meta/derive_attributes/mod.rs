mod db_field_types;
mod generics;
mod ident;

use darling::FromField;
use proc_macro2::Ident;
use syn::Type;

use crate::models::*;

use super::{AttributeAssert, AttributeDefine, AttributeItemAssert, AttributeValue, Permissions};

create_ident_wrapper!(FieldIdentNormalized);
create_ident_wrapper!(FieldWrapperIdent);
create_ident_wrapper!(FieldIdentOriginal);
create_ident_wrapper!(OldFieldName);

#[derive(Clone, Debug, FromField)]
#[darling(attributes(orm), forward_attrs(surreal))]
pub struct MyFieldReceiver {
    /// Get the ident of the field. For fields in tuple or newtype structs or
    /// enum bodies, this can be `None`.
    pub ident: Option<Ident>,
    /// This magic field name pulls the type from the input.
    pub ty: Type,
    /// The field's `#[surreal(…)]` attributes, read for its stored name.
    pub attrs: Vec<syn::Attribute>,

    /// Explicity specify the array or set item/element rust type
    #[darling(default, rename = "item_rust_ty")]
    pub(crate) array_item_ty_specified: Option<ArrayItemType>,

    /// Old name of field when renaming
    #[darling(default)]
    pub(crate) old_name: Option<OldFieldName>,

    // reference singular: LinkOne<Account>
    #[darling(default)]
    pub relate: Option<Relate>,

    // reference singular: LinkOne<Account>
    #[darling(default)]
    pub(crate) link_one: Option<LinkOneAttrType>,

    // reference singular: LinkSelf<Account>
    #[darling(default)]
    pub(crate) link_self: Option<LinkSelfAttrType>,

    // reference plural: LinkMany<Account>
    #[darling(default)]
    pub(crate) link_many: Option<LinkManyAttrType>,

    #[darling(default)]
    pub(crate) nest_array: Option<NestArrayAttrType>,

    #[darling(default)]
    pub(crate) nest_object: Option<NestObjectAttrType>,

    /// `merge = "TheirMerge"`: the field merges field by field through the caller's own
    /// `MergeFields` type, for a value that is not a `#[model(object)]`.
    #[darling(default)]
    pub(crate) merge: Option<MergeAttrType>,

    #[darling(default, rename = "ty")]
    pub(crate) field_type_db: Option<FieldTypeDb>,

    #[darling(default)]
    pub(crate) assert: Option<AttributeAssert>,

    #[darling(default)]
    pub(crate) define: Option<AttributeDefine>,

    #[darling(default)]
    pub(crate) value: Option<AttributeValue>,

    #[darling(default)]
    pub(crate) permissions: Option<Permissions>,

    #[darling(default)]
    pub(crate) item_assert: Option<AttributeItemAssert>,
}
