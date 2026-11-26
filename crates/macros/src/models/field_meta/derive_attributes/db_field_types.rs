use surrealdb_types::Kind;

use crate::models::*;

use super::MyFieldReceiver;

impl MyFieldReceiver {
    pub fn field_type_db_token(
        &self,
        model_attributes: &ModelAttributes,
        // A relate field is an example of where we should not
        // have a db field as that is a readonly derived field.
    ) -> ExtractorResult<Option<FieldTypeDbToken>> {
        Ok(self
            .field_type_db_with_static_assertions(model_attributes)?
            .map(|x| x.field_type_db_token))
    }

    pub fn field_type_db_original(
        &self,
        model_attributes: &ModelAttributes,
    ) -> ExtractorResult<Option<FieldTypeDb>> {
        Ok(self
            .field_type_db_with_static_assertions(model_attributes)?
            .map(|x| FieldTypeDb(x.field_type_db_original)))
    }

    pub fn field_type_db_with_static_assertions(
        &self,
        model_attributes: &ModelAttributes,
    ) -> ExtractorResult<Option<DbFieldTypeAstMeta>> {
        if self.to_relation_type(model_attributes).is_relate_graph() {
            return Ok(None);
        }

        let crate_name = get_crate_name();
        let field_ty = self
            .ty()
            // .remove_non_static_lifetime_and_reference()
            .replace_self_with_current_struct_concrete_type(model_attributes)?;

        let db_type = match self.field_type_db {
            // On `id`, the type is the record key's; the field itself is always a record id.
            Some(ref db_type) if self.db_field_name(&model_attributes.casing()?)?.is_id() => {
                Some(DbFieldTypeAstMeta::new(
                    db_type.inner().clone(),
                    quote!(#crate_name::validators::assert_type_is_record_id::<#field_ty>();),
                ))
            }
            Some(ref db_type) => {
                let relation = self.to_relation_type(model_attributes);
                let static_assertion_token = match &relation {
                    RelationType::LinkMany(ref_node)
                    | RelationType::LinkManyInAndOutEdgeNodesInert(ref_node) => {
                        quote! {
                            #crate_name::validators::assert_type_is_link_many::<#field_ty>();
                            #crate_name::validators::assert_type_is_node::<#ref_node>();
                        }
                    }
                    RelationType::LinkOne(ref_node) => {
                        quote! {
                            #crate_name::validators::assert_type_is_link_one::<#field_ty>();
                            #crate_name::validators::assert_type_is_node::<#ref_node>();
                        }
                    }
                    RelationType::LinkSelf(ref_node) => {
                        quote! {
                            #crate_name::validators::assert_type_is_link_self::<#field_ty>();
                            #crate_name::validators::assert_type_is_node::<#ref_node>();
                        }
                    }
                    RelationType::NestArray(nested_array_object) => {
                        quote! {
                            #crate_name::validators::assert_type_is_array::<#field_ty>();
                            #crate_name::validators::assert_type_is_object::<#nested_array_object>();
                        }
                    }
                    RelationType::NestObject(nested_object) => {
                        quote! {
                            #crate_name::validators::assert_type_is_object::<#field_ty>();
                            #crate_name::validators::assert_type_is_object::<#nested_object>();
                        }
                    }
                    RelationType::Relate(_) | RelationType::None | RelationType::List(_) => {
                        // db_type.generate_static_assertion(&field_ty.into_inner())
                        quote! {}
                    }
                };

                let assertions_from_db_type = if !relation.is_relate_graph() {
                    db_type.generate_static_assertion(&field_ty.into_inner())
                } else {
                    quote! {}
                };

                Some(DbFieldTypeAstMeta::new(
                    db_type.inner().clone(),
                    quote! {
                        #static_assertion_token
                        #assertions_from_db_type
                    },
                ))
            }
            None => {
                let casing = model_attributes.casing()?;
                let field_name = &self.db_field_name(&casing)?;

                FieldTypeInference {
                    db_field_name: field_name,
                    relation_type: &self.to_relation_type(model_attributes),
                    field_ty: &field_ty.into_inner(),
                    model_attrs: model_attributes,
                }
                .infer_type()?
            }
        };

        Ok(db_type)
    }

    pub fn is_in_or_out_edge_node_field(&self, model_attributes: &ModelAttributes) -> bool {
        let is_in_or_out_edge_node = self
            .db_field_name(
                &model_attributes
                    .casing()
                    .expect("Failed to get the casing. Please, provide one."),
            )
            .expect("Failed to get the database field name")
            .is_in_or_out_edge_node(&model_attributes.to_data_type());

        model_attributes.to_data_type().is_edge() && is_in_or_out_edge_node
    }

    /// Whether this is the record id, whose `ty` is its key's type rather than the field's.
    fn is_record_id_field(&self, model_attributes: &ModelAttributes) -> bool {
        model_attributes
            .casing()
            .and_then(|casing| self.db_field_name(&casing))
            .is_ok_and(|name| name.is_id())
    }

    pub fn to_relation_type(&self, model_attributes: &ModelAttributes) -> RelationType {
        let field_receiver = self;

        match field_receiver {
            MyFieldReceiver {
                relate: Some(relation),
                ..
            } => RelationType::Relate(relation.to_owned()),
            MyFieldReceiver {
                link_one: Some(link_one),
                ..
            } => RelationType::LinkOne(link_one.to_owned()),
            MyFieldReceiver {
                link_self: Some(link_self),
                ..
            } => RelationType::LinkSelf(link_self.to_owned()),
            // This is a special case where we have a link_many field
            // that is used to link many in and out edge nodes.
            // In this case, we may not want to do any codegen with the field
            // directly due to the complexities and alternate approaches
            // already used.
            MyFieldReceiver {
                link_many: Some(link_many),
                ..
            } if self.is_in_or_out_edge_node_field(model_attributes) => {
                RelationType::LinkManyInAndOutEdgeNodesInert(link_many.to_owned())
            }
            MyFieldReceiver {
                link_many: Some(link_many),
                ..
            } => RelationType::LinkMany(link_many.to_owned()),
            MyFieldReceiver {
                nest_object: Some(nest_object),
                ..
            } => RelationType::NestObject(nest_object.to_owned()),
            MyFieldReceiver {
                nest_array: Some(nest_array),
                ..
            } => RelationType::NestArray(nest_array.to_owned()),
            // An array key type does not make the record id a list.
            _ if self.is_record_id_field(model_attributes) => RelationType::None,
            _ if field_receiver.is_array() || field_receiver.is_set() => {
                RelationType::List(ListSimple)
            }
            _ => RelationType::None,
        }
    }

    fn explicit_kind(&self) -> Option<&Kind> {
        self.field_type_db.as_ref().map(FieldTypeDb::inner)
    }

    pub fn is_numeric(&self) -> bool {
        matches!(
            self.explicit_kind(),
            Some(Kind::Int | Kind::Float | Kind::Decimal | Kind::Number)
        ) || self.ty().is_numeric()
    }

    pub fn is_array(&self) -> bool {
        matches!(self.explicit_kind(), Some(Kind::Array(..)))
            || self.ty().is_array()
            || self.link_many.is_some()
    }

    pub fn is_set(&self) -> bool {
        matches!(self.explicit_kind(), Some(Kind::Set(..))) || self.ty().is_set()
    }

    pub fn ty(&self) -> CustomType {
        CustomType::new(self.ty.clone())
    }
}
