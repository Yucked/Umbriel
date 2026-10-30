use crate::helpers::get_crate_name;
use quote::{ToTokens, quote};

use super::Codegen;
use crate::models::*;

impl Codegen<'_> {
    /// Checks a field's definition settings. The definitions themselves are part of the field's
    /// metadata, which `define_fields` reads.
    pub fn create_field_definition_checks(&mut self) -> ExtractorResult<()> {
        self.validate_field_attributes()?;

        self.static_assertions
            .push(self.static_assertion_field_value(self.table_derive_attributes())?);
        Ok(())
    }

    /// Relations are read through graph traversals and never stored, so they have no definition.
    pub fn skip_field(&self) -> bool {
        self.field_receiver()
            .to_relation_type(self.table_derive_attributes())
            .is_relate_graph()
    }

    /// The record id without `ty = "…"`: its key may have any type, so it has no stored type.
    fn is_untyped_id(&self) -> ExtractorResult<bool> {
        let field_receiver = self.field_receiver();
        let casing = self.table_derive_attributes().casing()?;
        Ok(
            field_receiver.db_field_name(&casing)?.is_id()
                && field_receiver.field_type_db.is_none(),
        )
    }

    pub fn field_defintion_db(&self) -> ExtractorResult<Vec<DefineFieldStatementToken>> {
        let field_receiver = self.field_receiver();
        let model_attributes = self.table_derive_attributes();
        // A view's records are computed by its `AS SELECT` query, so it defines no fields.
        if self.skip_field() || model_attributes.receiver().view {
            return Ok(vec![]);
        }

        let crate_name = get_crate_name(false);
        let VariablesModelMacro { _____table, .. } = VariablesModelMacro::new();
        let casing = model_attributes.casing()?;
        let db_field_name = field_receiver.db_field_name(&casing)?;
        // Definitions write their values inline; `inline` keeps the builder's errors.
        let raw = |definition: TokenStream| -> DefineFieldStatementToken {
            quote!(#crate_name::Raw::inline(&#definition)).into()
        };

        // `DEFINE FIELD id … TYPE` constrains the record key, which `RecordId<T>` leaves open, so
        // `id` has a type only when given a key type (`ty = "int"`). Every other stored field has
        // one: inference fails rather than leave it out.
        let untyped_id = self.is_untyped_id()?;
        let type_clause = if untyped_id {
            None
        } else {
            let field_type_in_db_token = field_receiver
                .field_type_db_token(model_attributes)?
                .ok_or_else(|| {
                    syn::Error::new_spanned(
                        &field_receiver.ty,
                        "the field's database type is unknown; specify `#[orm(ty = \"…\")]`",
                    )
                })?;
            Some(quote!(.type_(#field_type_in_db_token)))
        };

        let mut define_field_methods = vec![];
        let mut define_array_field_item_methods = vec![];
        let mut all_field_defintions: Vec<DefineFieldStatementToken> = vec![];

        if let Some(assert) = field_receiver.assert.as_ref() {
            define_field_methods.push(quote!(.assert(#assert)));
        }

        if let Some(item_assert) = field_receiver.item_assert.as_ref() {
            define_array_field_item_methods.push(quote!(.assert(#item_assert)));
        }

        // `#[orm(value = …)]` is documented as the field's default, so it emits `DEFAULT`;
        // SurrealQL's `VALUE` is `value_expr`.
        if let Some(value) = field_receiver.value.as_ref() {
            define_field_methods.push(quote!(.default(#value)));
        }

        if let Some(default_always) = field_receiver.default_always.as_ref() {
            define_field_methods.push(quote!(.default_always(#default_always)));
        }

        if let Some(value_expr) = field_receiver.value_expr.as_ref() {
            define_field_methods.push(quote!(.value(#value_expr)));
        }

        if let Some(computed) = field_receiver.computed.as_ref() {
            define_field_methods.push(quote!(.computed(#computed)));
        }

        if field_receiver.readonly {
            define_field_methods.push(quote!(.readonly()));
        }

        if field_receiver.flexible {
            define_field_methods.push(quote!(.flexible()));
        }

        if let Some(comment) = field_receiver.comment.as_ref() {
            define_field_methods.push(quote!(.comment(#comment)));
        }

        if let Some(permissions) = field_receiver.permissions.as_ref() {
            define_field_methods.push(permissions.into_token_stream());
        }

        // An untyped `id` with no clauses needs no definition; its `ASSERT` and `DEFAULT` are
        // kept, as SurrealDB applies both to the record key.
        if !untyped_id || !define_field_methods.is_empty() {
            all_field_defintions.push(raw(quote!(
                #crate_name::statements::define_field(#crate_name::Field::new(#db_field_name))
                .on_table(#_____table.clone())
                #type_clause
                #( # define_field_methods) *
            )));
        }

        if !define_array_field_item_methods.is_empty() {
            // `field.*` defines every element of an array field.
            all_field_defintions.push(raw(quote!(
                #crate_name::statements::define_field(#crate_name::Field::new(#db_field_name).all())
                    .on_table(#_____table.clone())
                    #( # define_array_field_item_methods) *
            )));
        };

        if let Some(define) = field_receiver.define.as_ref() {
            all_field_defintions.push(raw(quote!(#define)));
        }

        Ok(all_field_defintions)
    }

    /// Checks that `value` and `default_always`, the field's defaults, can hold a value of its
    /// database type.
    fn static_assertion_field_value(
        &self,
        model_attrs: &ModelAttributes,
    ) -> ExtractorResult<StaticAssertionToken> {
        let field_receiver = self.field_receiver();
        // An untyped `id`'s default is a key of any type, not the record id it is inferred as.
        if self.is_untyped_id()? {
            return Ok(StaticAssertionToken::default());
        }
        let Some(field_type) = field_receiver.field_type_db_original(model_attrs)? else {
            return Ok(StaticAssertionToken::default());
        };

        let assertions = [&field_receiver.value, &field_receiver.default_always]
            .into_iter()
            .flatten()
            .map(|default| default.get_default_value_static_assertion(field_type.inner()));
        Ok(quote!(#( #assertions )*).into())
    }

    fn validate_field_attributes(&self) -> ExtractorResult<()> {
        let field_receiver = self.field_receiver();
        let MyFieldReceiver {
            define,
            assert: assert_,
            value,
            default_always,
            value_expr,
            computed,
            readonly,
            flexible,
            comment,
            permissions,
            item_assert,
            relate,
            ..
        } = field_receiver;
        let ident = field_receiver.ident()?;

        let clauses = assert_.is_some()
            || value.is_some()
            || default_always.is_some()
            || value_expr.is_some()
            || computed.is_some()
            || *readonly
            || *flexible
            || comment.is_some()
            || permissions.is_some()
            || item_assert.is_some();

        if define.is_some() && clauses {
            return Err(syn::Error::new_spanned(
                &ident,
                "`define` gives the whole definition, so it cannot be combined with `assert`, \
                 `value`, `default_always`, `value_expr`, `computed`, `readonly`, `flexible`, \
                 `comment`, `permissions` or `item_assert`",
            )
            .into());
        }

        if value.is_some() && default_always.is_some() {
            return Err(syn::Error::new_spanned(
                &ident,
                "a field has one default: `value` or `default_always`, not both",
            )
            .into());
        }

        if computed.is_some()
            && (assert_.is_some()
                || value.is_some()
                || default_always.is_some()
                || value_expr.is_some()
                || *readonly
                || *flexible
                || item_assert.is_some())
        {
            return Err(syn::Error::new_spanned(
                &ident,
                "a `computed` field is evaluated on read, so it cannot also have `assert`, \
                 `value`, `default_always`, `value_expr`, `readonly`, `flexible` or `item_assert`",
            )
            .into());
        }

        if relate.is_some() && (define.is_some() || clauses) {
            return Err(syn::Error::new_spanned(
                ident,
                r#"This is a read-only relation field and does not allow the following attributes:
    define,
    assert,
    value,
    default_always,
    value_expr,
    computed,
    readonly,
    flexible,
    comment,
    permissions,
    item_assert"#,
            )
            .into());
        }

        Ok(())
    }
}
