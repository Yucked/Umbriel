use proc_macro2::TokenStream;
use quote::quote;

use crate::models::*;

create_tokenstream_wrapper!(
/// Genera
=>
ListSimpleTraversalMethod
);

use super::{Codegen, RelationType};

/// The schema type of a model, named through `SchemaGetter` where the model is declared, so no
/// alias or module is needed to reach it.
fn schema_of(model: &impl quote::ToTokens) -> TokenStream {
    let crate_name = get_crate_name();
    quote!(<#model as #crate_name::SchemaGetter>::Schema)
}

impl Codegen<'_> {
    pub fn is_in_or_out_edge_node(&self) -> ExtractorResult<bool> {
        Ok(self
            .field_receiver()
            .db_field_name(&self.table_derive_attributes().casing()?)?
            .is_in_or_out_edge_node(&self.data_type()))
    }

    pub fn create_link_methods(&mut self) -> ExtractorResult<()> {
        let table_derive_attrs = self.table_derive_attributes();
        let field_receiver = self.field_receiver();

        let mut metas = vec![];
        match field_receiver.to_relation_type(table_derive_attrs) {
            RelationType::LinkSelf(link_self) => {
                let link_one = link_self.to_linkone_attr_type(table_derive_attrs);
                let meta = self.link_one(link_one?)?;
                metas.push(meta);
            }
            RelationType::LinkOne(link_one) => {
                let meta = self.link_one(link_one)?;
                metas.push(meta);
            }
            RelationType::LinkMany(link_many) => {
                let meta = self.link_many(&link_many)?;
                metas.push(meta);
            }
            RelationType::LinkManyInAndOutEdgeNodesInert(_link_many) => {
                // Do nothing as we dont want to generate code for in and out nodes
                // fields in the edge tables. That is handled other ways using the aliased
                // type to the edge table. e.g Like<In: Node + Default, Out: Node + Default>
                // where In and Out are the nodes in the edge table
                // we could use type StudentLikeBook = Like<Student, Book>;
                // StudentLikeBook already handles most of what we need but we can
                // revisit this later to see if we can generate more code for this
            }
            RelationType::NestArray(nest_array) => {
                let meta = self.nest_array(&nest_array)?;
                metas.push(meta);
            }
            RelationType::NestObject(nest_object) => {
                let meta = self.nest_object(&nest_object)?;
                metas.push(meta);
            }
            RelationType::List(_list) => {
                let meta = self.list_simple()?;
                metas.push(meta);
            }
            RelationType::Relate(_relate) => {}
            RelationType::None => {}
        }

        for meta in metas {
            // The `in` and `out` fields of an edge link its generic node types, which get no
            // traversal methods of their own.
            if self.is_in_or_out_edge_node()? {
                continue;
            }

            self.record_link_fields_methods.push(meta.link_field_method);

            self.static_assertions
                .push(meta.foreign_node_type_validator.0);
        }

        Ok(())
    }

    pub fn list_simple(&self) -> ExtractorResult<LinkMethodMeta> {
        let crate_name = get_crate_name();
        let table_derive_attrs = self.table_derive_attributes();
        let field_receiver = self.field_receiver();
        let struct_casing = table_derive_attrs.casing()?;
        let field_ident_normalized = field_receiver.field_ident_normalized(&struct_casing)?;
        let db_field_name = field_receiver.db_field_name(&struct_casing)?;

        let record_link_default_alias_as_method = quote!(
            pub fn #field_ident_normalized(&self, clause: impl ::std::convert::Into<#crate_name::NodeAliasClause>) -> #crate_name::Field {
                let clause: #crate_name::NodeAliasClause = clause.into();
                let mut field = #crate_name::Field::from(self).with_field(#db_field_name);
                field.push_clause(clause.into_inner());
                field
            }
        );

        Ok(LinkMethodMeta {
            foreign_node_type_validator: quote!().into(),
            link_field_method: record_link_default_alias_as_method.into(),
        })
    }

    fn link_one(&self, link_one: LinkOneAttrType) -> ExtractorResult<LinkMethodMeta> {
        let crate_name = get_crate_name();
        let table_derive_attrs = self.table_derive_attributes();
        let struct_casing = table_derive_attrs.casing()?;
        let field_receiver = self.field_receiver();
        let db_field_name = field_receiver.db_field_name(&struct_casing)?;
        let field_ident_normalized = field_receiver.field_ident_normalized(&struct_casing)?;
        let VariablesModelMacro {
            __________connect, ..
        } = VariablesModelMacro::new();
        let schema = schema_of(&link_one);

        let record_link_default_alias_as_method = quote!(
            pub fn #field_ident_normalized(&self) -> #schema {
                <#schema>::#__________connect(
                    self,
                    #crate_name::Clause::from(#crate_name::Empty).with_field(#db_field_name),
                )
            }
        );

        Ok(LinkMethodMeta {
            foreign_node_type_validator: quote!(
                #crate_name::validators::assert_impl_one!(#link_one: #crate_name::Node);
            )
            .into(),
            link_field_method: record_link_default_alias_as_method.into(),
        })
    }

    fn link_many(&self, link_many_node_type: &LinkManyAttrType) -> ExtractorResult<LinkMethodMeta> {
        let crate_name = get_crate_name();
        let table_derive_attrs = self.table_derive_attributes();
        let field_attr = self.field_receiver();
        let struct_casing = table_derive_attrs.casing()?;
        let field_ident_normalized = field_attr.field_ident_normalized(&struct_casing)?;
        let db_field_name = field_attr.db_field_name(&struct_casing)?;
        let VariablesModelMacro {
            __________connect, ..
        } = VariablesModelMacro::new();
        let schema = schema_of(link_many_node_type);

        let link_field_method = quote!(
            pub fn #field_ident_normalized(&self, clause: impl ::std::convert::Into<#crate_name::NodeAliasClause>) -> #schema {
                let clause: #crate_name::NodeAliasClause = clause.into();
                <#schema>::#__________connect(
                    self,
                    clause.into_inner().with_field(#db_field_name),
                )
            }
        );

        Ok(LinkMethodMeta {
            link_field_method: link_field_method.into(),
            foreign_node_type_validator: quote!(
                #crate_name::validators::assert_impl_one!(#link_many_node_type: #crate_name::Node);
            )
            .into(),
        })
    }

    pub fn nest_object(
        &self,
        embedded_object: &NestObjectAttrType,
    ) -> ExtractorResult<LinkMethodMeta> {
        let crate_name = get_crate_name();
        let table_derive_attrs = self.table_derive_attributes();
        let struct_casing = table_derive_attrs.casing()?;
        let field_receiver = self.field_receiver();
        let field_ident_normalized = field_receiver.field_ident_normalized(&struct_casing)?;
        let db_field_name = field_receiver.db_field_name(&struct_casing)?;
        let VariablesModelMacro {
            __________connect, ..
        } = VariablesModelMacro::new();
        let schema = schema_of(embedded_object);

        let connect_object_field_method = quote!(
            pub fn #field_ident_normalized(&self) -> #schema {
                <#schema>::#__________connect(
                    self,
                    #crate_name::Clause::from(#crate_name::Empty).with_field(#db_field_name),
                )
            }
        );

        Ok(LinkMethodMeta {
            foreign_node_type_validator: quote!(
                #crate_name::validators::assert_impl_one!(#embedded_object: #crate_name::Object);
            )
            .into(),
            link_field_method: connect_object_field_method.into(),
        })
    }

    pub fn nest_array(&self, nested_array: &NestArrayAttrType) -> ExtractorResult<LinkMethodMeta> {
        let crate_name = get_crate_name();
        let table_derive_attrs = self.table_derive_attributes();
        let struct_casing = table_derive_attrs.casing()?;
        let field_receiver = self.field_receiver();
        let field_ident_normalized = field_receiver.field_ident_normalized(&struct_casing)?;
        let db_field_name = field_receiver.db_field_name(&struct_casing)?;
        let VariablesModelMacro {
            __________connect, ..
        } = VariablesModelMacro::new();
        let schema = schema_of(nested_array);

        let record_link_default_alias_as_method = quote!(
            pub fn #field_ident_normalized(
                &self,
                clause: impl ::std::convert::Into<#crate_name::ObjectClause>
            ) -> #schema {
                let clause: #crate_name::ObjectClause = clause.into();
                <#schema>::#__________connect(
                    self,
                    clause.into_inner().with_field(#db_field_name),
                )
            }
        );

        Ok(LinkMethodMeta {
            foreign_node_type_validator: quote!(
                #crate_name::validators::assert_impl_one!(#nested_array: #crate_name::Object);
            )
            .into(),
            link_field_method: record_link_default_alias_as_method.into(),
        })
    }
}

create_tokenstream_wrapper!(
/// Contains static assertions for foreign node type
=>
ForeignNodeTypeValidator
);

create_tokenstream_wrapper!(
/// The method that continues a path through a link field to the foreign node's schema.
=>
LinkFieldTraversalMethodToken
);

pub struct LinkMethodMeta {
    foreign_node_type_validator: ForeignNodeTypeValidator,
    link_field_method: LinkFieldTraversalMethodToken,
}

impl LinkSelfAttrType {
    pub(crate) fn to_linkone_attr_type(
        &self,
        table_derive_attrs: &ModelAttributes,
    ) -> ExtractorResult<LinkOneAttrType> {
        Ok(LinkOneAttrType(
            self.replace_self_with_current_struct_concrete_type(table_derive_attrs)?
                .into_inner(),
        ))
    }
}
