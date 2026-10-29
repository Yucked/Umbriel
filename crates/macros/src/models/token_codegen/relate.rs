use std::{
    collections::{HashMap, hash_map::Entry},
    fmt::Display,
    ops::{Deref, DerefMut},
};

use crate::helpers::get_crate_name;
use heck::ToUpperCamelCase;
use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};

use crate::models::*;

use super::Codegen;

impl Codegen<'_> {
    pub fn create_relation_connection_tokenstream(&mut self) -> ExtractorResult<()> {
        let crate_name = get_crate_name(false);
        let table_derive_attributes = self.table_derive_attributes();
        let field_receiver = self.field_receiver();
        let db_field_name = field_receiver.db_field_name(&table_derive_attributes.casing()?)?;

        if let RelationType::Relate(ref relate) =
            field_receiver.to_relation_type(table_derive_attributes)
        {
            self.static_assertions
                .push(self.create_static_assertions(relate)?);
            self.relate(relate)?;
            let connection = &relate.connection;
            self.fields_relations_aliased.push(
                quote!(#crate_name::Aliasable::__as__(
                    &#crate_name::Field::new(#connection),
                    #crate_name::AliasName::new(#db_field_name),
                ))
                .into(),
            );
        }
        Ok(())
    }

    fn relate(&mut self, relate: &Relate) -> ExtractorResult<()> {
        let crate_name = get_crate_name(false);
        let table_derive_attributes = &self.table_derive_attributes();
        let VariablesModelMacro {
            __________connect, ..
        } = VariablesModelMacro::new();
        let edge_type = &relate.edge_type;
        let RelateAttribute {
            edge_direction,
            edge_table,
            foreign_node_table,
        } = &RelateAttribute::try_from(relate)?;
        let arrow = &ArrowTokenStream::from(edge_direction);

        let edge_name_with_direction_as_method_ident =
            &(|| Self::add_direction_indication_to_ident(edge_table, edge_direction));

        // The edge's `In` or `Out` node, whichever is at the far end of this relation.
        let foreign_node_in_or_out = ForeignNodeAssociatedTypeInOrOut(match edge_direction {
            EdgeDirection::Out => format_ident!("Out"),
            EdgeDirection::In => format_ident!("In"),
        });
        // Named through the edge where the model is declared: no alias or module is needed.
        let foreign_node_schema = quote!(
            <<#edge_type as #crate_name::Edge>::#foreign_node_in_or_out as #crate_name::SchemaGetter>::Schema
        );

        // i.e Edge to foreign Node
        let edge_to_foreign_node_connection_method = EdgeToForeignNodeMethod(quote!(
            pub fn #foreign_node_table(
                self,
                clause: impl #crate_name::IntoNodeClause<<#edge_type as #crate_name::Edge>::#foreign_node_in_or_out>,
            ) -> #foreign_node_schema {
                let clause: #crate_name::NodeClause = #crate_name::IntoNodeClause::<
                    <#edge_type as #crate_name::Edge>::#foreign_node_in_or_out,
                >::into_node_clause(clause);
                let clause = clause.with_arrow(#arrow).with_family(#crate_name::TableSpec::of::<
                    <#edge_type as #crate_name::Edge>::#foreign_node_in_or_out,
                >());

                <#foreign_node_schema>::#__________connect(
                    &self.0,
                    clause.into_inner(),
                )
            }
        ));

        let node_edge_meta = NodeEdgeMetadata {
            edge_table: edge_table.to_owned(),
            edge_type: edge_type.to_owned(),
            // We want to take all edge concrete types even for each edge type(with direction)
            // because we want to allow the use of generics in edge.
            edge_types: vec![edge_type.to_owned()],
            direction: *edge_direction,
            edge_to_foreign_node_connection_method: vec![edge_to_foreign_node_connection_method],
            table_derive_attributes,
            edge_name_as_method_ident: format_ident!(
                "{}",
                edge_name_with_direction_as_method_ident().to_string()
            )
            .into(),
            edge_relation_model_selected_ident: edge_type.type_name()?.into(),
        };

        match self
            .node_edge_metadata
            .deref_mut()
            .entry(edge_name_with_direction_as_method_ident())
        {
            Entry::Occupied(o) => {
                let existing = o.into_mut();
                // Get all the Edge types(with direction) aliased types, so
                // we can find a common denomator types between them all.
                existing.edge_types.extend(node_edge_meta.edge_types);
                existing
                    .edge_to_foreign_node_connection_method
                    .extend(node_edge_meta.edge_to_foreign_node_connection_method);
            }
            Entry::Vacant(v) => {
                v.insert(node_edge_meta);
            }
        };
        Ok(())
    }

    /// add direction indication to the edge name. This is non-functional
    /// semantics, it's just a way to differentiate same edge but with different
    /// direction
    /// e.g for ->writes->book, gives writes__. <-writes<-book, gives __writes
    pub fn add_direction_indication_to_ident(
        edge_table: &EdgeTableName,
        edge_direction: &EdgeDirection,
    ) -> EdgeWithDunderDirectionIndicator {
        let edge_table = edge_table.to_string();
        let edge = match edge_direction {
            EdgeDirection::Out => format_ident!("{edge_table}__"),
            EdgeDirection::In => format_ident!("__{edge_table}"),
        };
        edge.into()
    }

    fn create_static_assertions(&self, relate: &Relate) -> ExtractorResult<StaticAssertionToken> {
        let crate_name = get_crate_name(false);
        let model_attributes = &self.table_derive_attributes();
        let current_struct_ident = &model_attributes.ident();
        let current_struct_generics = model_attributes.generics();
        let (_struct_impl_generics, struct_ty_generics, _struct_where_clause) =
            current_struct_generics.split_for_impl();
        let field_receiver = self.field_receiver();
        let field_type = &field_receiver.ty();
        let edge_type = &relate.edge_type;
        let RelateAttribute {
            edge_table,
            foreign_node_table: destination_node_table,
            edge_direction,
        } = RelateAttribute::try_from(relate)?;
        let (home_node_associated_type_ident, foreign_node_associated_type_ident) =
            match &edge_direction {
                EdgeDirection::Out => (format_ident!("In"), format_ident!("Out")),
                EdgeDirection::In => (format_ident!("Out"), format_ident!("In")),
            };

        // e.g for struct Student {
        //                   #[orm(relate(mode="StudentWritesBook", connection="->writes->book"))]
        //                   fav_books: Relate<Book>
        //              }
        let home_node_type =
            quote!(<#edge_type as #crate_name::Edge>::#home_node_associated_type_ident);

        let foreign_node_type =
            quote!(<#edge_type as #crate_name::Edge>::#foreign_node_associated_type_ident);

        let static_assertions = &[
            // type HomeIdent = <StudentWritesBook  as surreal_macros::Edge>::In;
            // type HomeNodeTableChecker = <HomeIdent as
            // surreal_macros::Node>::TableNameChecker;
            // #crate_name::validators::assert_type_eq_all!(HomeIdent, Student);
            // #crate_name::validators::assert_impl_one!(HomeIdent, surreal_macros::Node);
            quote!(
                {
                     // #crate_name::validators::assert_fields!(<#home_node_type as #crate_name::Node>::TableNameChecker: #origin_node_table);
                     #crate_name::validators::assert_type_eq_all!(#home_node_type, #current_struct_ident #struct_ty_generics);
                     #crate_name::validators::assert_impl_one!(<#edge_type as #crate_name::Edge>::#home_node_associated_type_ident: #crate_name::Node);
                }
            ),
            quote!(
                 #crate_name::validators::assert_fields!(<#foreign_node_type as #crate_name::Node>::TableNameChecker: #destination_node_table);
                 #crate_name::validators::assert_impl_one!(#foreign_node_type: #crate_name::Node);
            ),
            quote!(
                 #crate_name::validators::assert_fields!(<#edge_type as #crate_name::Edge>::TableNameChecker: #edge_table);
            ),
            // assert field type and attribute reference match
            // e.g Relate<Book> should match from attribute link = "->Writes->Book"
            quote!(
                 #crate_name::validators::assert_impl_one!(#edge_type: #crate_name::Edge);
                 #crate_name::validators::assert_type_eq_all!(#field_type,  #crate_name::Relate<#foreign_node_type>);
            ),
        ];
        Ok(StaticAssertionToken(quote!(
                #( #static_assertions) *
        )))
    }
}

impl EdgeTableName {
    pub fn to_str_typed(&self) -> EdgeTableNameStr {
        EdgeTableNameStr(self.to_string())
    }
}

#[derive(Clone, Debug)]
pub struct EdgeTableNameStr(String);

impl EdgeTableNameStr {
    pub fn to_pascal_case_ident(&self) -> EdgeNameAsStructOriginalIdent {
        EdgeNameAsStructOriginalIdent(format_ident!("{}", self.as_pascal_case().to_string()))
    }

    fn as_pascal_case(&self) -> EdgeTableNameStr {
        EdgeTableNameStr(self.to_string().to_upper_camel_case())
    }
}

impl Display for EdgeTableNameStr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl ToTokens for EdgeTableNameStr {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let value = &self.0;
        tokens.extend(quote!(#value));
    }
}

create_ident_wrapper!(EdgeNameAsMethodIdent);
create_ident_wrapper!(EdgeRelationModelSelectedIdent);

create_tokenstream_wrapper!(=>EdgeToForeignNodeMethod);
create_tokenstream_wrapper!(=>DestinationNodeSchemaOne);
create_ident_wrapper!(ForeignNodeAssociatedTypeInOrOut);
create_ident_wrapper!(EdgeNameAsStructOriginalIdent);

#[derive(Clone, Debug)]
pub struct NodeEdgeMetadata<'a> {
    /// Example value: writes
    edge_table: EdgeTableName,
    /// The first edge model declared for this table and direction. Every model of one edge table
    /// is the same struct, so it gives the table family the edge step accepts.
    edge_type: EdgeType,
    pub edge_types: Vec<EdgeType>,
    // /// The current struct name ident.
    // /// e.g given: struct Student {  }, value = Student
    // current_struct_ident: StructIdent,
    table_derive_attributes: &'a ModelAttributes<'a>,
    /// The database table name of the edge. Used for generating other tokens
    /// e.g "writes"
    direction: EdgeDirection,
    /// Example of value: `StudentWritesBook`
    ///
    /// For each edge table e.g writes, we usually can have many aliases reusing thesame edge
    /// e.g for Writes<In, Out>, we could have  StudentWritesBook, StudentWritesBlog, for each direction(e.g ->),
    /// we want to select one of these to use its schema which is aliased as the Cased table name
    /// in the calling location e.g
    /// for a model field annotation e.g relate(edge="StudentWritesBook", link="->writes->book")
    /// So we can do
    /// type Writes = <StudentWritesBook as Edge>::Schema;
    edge_relation_model_selected_ident: EdgeRelationModelSelectedIdent,
    /// Example Generated:
    ///
    /// ```rust, ignore
    /// impl Writes__ {
    ///     fn book(&self, filter: Filter) -> Book {
    ///         Book::__________connect_to_graph_traversal_string(
    ///             &self.___________graph_traversal_string,
    ///             filter,
    ///         )
    ///     }
    ///
    ///     fn blog(&self, filter: Filter) -> Blog {
    ///         Blog::__________connect_to_graph_traversal_string(
    ///             &self.___________graph_traversal_string,
    ///             filter,
    ///         )
    ///     }
    /// }
    /// ```
    ///
    /// Example Value:
    /// ```rust, ignore
    /// vec![
    ///     quote!(
    ///        fn book(&self, filter: Filter) -> Book {
    ///            Book::__________connect_to_graph_traversal_string(
    ///                &self.___________graph_traversal_string,
    ///                filter,
    ///            )
    ///        }
    ///     ),
    ///     quote!(
    ///        fn blog(&self, filter: Filter) -> Blog {
    ///            Blog::__________connect_to_graph_traversal_string(
    ///                &self.___________graph_traversal_string,
    ///                filter,
    ///            )
    ///        }
    ///     ),
    ///    ]
    /// ```
    edge_to_foreign_node_connection_method: Vec<EdgeToForeignNodeMethod>,
    edge_name_as_method_ident: EdgeNameAsMethodIdent,
}

// #[derive(Clone, Debug, PartialEq, Eq, Hash)]
// struct EdgeNameWithDirectionIndicator(String);
create_ident_wrapper!(EdgeWithDunderDirectionIndicator);

#[derive(Default, Clone)]
pub struct NodeEdgeMetadataLookupTable<'a>(
    HashMap<EdgeWithDunderDirectionIndicator, NodeEdgeMetadata<'a>>,
);
impl<'a> Deref for NodeEdgeMetadataLookupTable<'a> {
    type Target = HashMap<EdgeWithDunderDirectionIndicator, NodeEdgeMetadata<'a>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for NodeEdgeMetadataLookupTable<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl NodeEdgeMetadataLookupTable<'_> {}

create_tokenstream_wrapper!(=>ArrowTokenStream);

impl From<&EdgeDirection> for ArrowTokenStream {
    fn from(value: &EdgeDirection) -> Self {
        let crate_name = get_crate_name(false);

        let arrow = match value {
            EdgeDirection::Out => quote!(#crate_name::Arrow::Right),
            EdgeDirection::In => quote!(#crate_name::Arrow::Left),
        };
        Self(arrow)
    }
}

impl ToTokens for NodeEdgeMetadata<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let crate_name = get_crate_name(false);
        let VariablesModelMacro {
            __________connect, ..
        } = VariablesModelMacro::new();
        let value = self;
        let Self {
            direction,
            edge_relation_model_selected_ident,
            edge_to_foreign_node_connection_method,
            edge_name_as_method_ident,
            edge_table,
            edge_type,
            edge_types,
            table_derive_attributes,
            ..
        }: &NodeEdgeMetadata = value;
        let current_struct_ident = value.table_derive_attributes.ident();
        let current_schema = current_struct_ident.schema_ident();
        let (struct_impl_generics, struct_ty_generics, struct_where_clause) =
            value.table_derive_attributes.generics().split_for_impl();
        let arrow = ArrowTokenStream::from(direction);
        let edge_table_str = edge_table.to_str_typed();
        let edge_name_as_struct_original_ident = edge_table_str.to_pascal_case_ident();
        let aggregated_edge_type =
            EdgeType::aggregate_lifetime_and_generics_from_variations_of_a_type(
                edge_name_as_struct_original_ident.deref(),
                edge_types,
            );

        let binding =
            match aggregated_edge_type.get_generics_from_current_struct(table_derive_attributes) {
                Ok(binding) => binding,
                Err(err) => return tokens.extend(err.write_errors()),
            };

        let (edge_type_impl_generics, edge_type_ty_generics, edge_type_where_clause) =
            binding.split_for_impl();

        // Hidden items next to the model, named after it and the edge table verbatim with its
        // length and direction (e.g. `__7Student6writesOutEdge` for `->writes`), so no two
        // models or edges share one; case conversion would merge `foo_bar` and `fooBar`.
        let edge_table_name = edge_table.to_string();
        let direction_name = match direction {
            EdgeDirection::Out => "Out",
            EdgeDirection::In => "In",
        };
        let edge_name = format!("{}{edge_table_name}{direction_name}", edge_table_name.len());
        let edge_wrapper = current_struct_ident.hidden_ident(format_args!("{edge_name}Edge"));
        let edge_schema = current_struct_ident.hidden_ident(format_args!("{edge_name}EdgeSchema"));

        tokens.extend(quote!(
            // Node to Edge
            impl #struct_impl_generics #current_schema #struct_ty_generics #struct_where_clause {
                pub fn #edge_name_as_method_ident(
                    &self,
                    clause: impl ::std::convert::Into<#crate_name::EdgeClause>,
                ) -> #edge_wrapper {
                    let clause: #crate_name::EdgeClause = clause.into();
                    let clause = clause.with_arrow(#arrow).with_family(#crate_name::TableSpec::of::<#edge_type>());

                    #edge_schema::#__________connect(
                        self,
                        clause.into_inner(),
                    ).into()
                }
            }

            // The edge's schema, through the edge model selected for this direction.
            #[doc(hidden)]
            #[allow(non_camel_case_types)]
            pub type #edge_schema #edge_type_ty_generics = <#edge_relation_model_selected_ident #edge_type_ty_generics as #crate_name::SchemaGetter>::Schema;

            #[doc(hidden)]
            #[allow(non_camel_case_types)]
            pub struct #edge_wrapper #edge_type_ty_generics(#edge_schema #edge_type_ty_generics);

            impl #edge_type_impl_generics ::std::convert::From<#edge_schema #edge_type_ty_generics> for #edge_wrapper #edge_type_ty_generics #edge_type_where_clause {
                fn from(value: #edge_schema #edge_type_ty_generics) -> Self {
                    Self(value)
                }
            }

            impl #edge_type_impl_generics #crate_name::Render for #edge_wrapper #edge_type_ty_generics #edge_type_where_clause {
                fn render(&self, out: &mut #crate_name::Renderer) {
                    #crate_name::Render::render(&self.0, out);
                }
            }

            impl #edge_type_impl_generics ::std::convert::From<&#edge_wrapper #edge_type_ty_generics> for #crate_name::Field #edge_type_where_clause {
                fn from(edge: &#edge_wrapper #edge_type_ty_generics) -> Self {
                    #crate_name::Field::from(&edge.0)
                }
            }

            impl #edge_type_impl_generics ::std::ops::Deref for #edge_wrapper #edge_type_ty_generics #edge_type_where_clause {
                type Target = #edge_schema #edge_type_ty_generics;

                fn deref(&self) -> &Self::Target {
                    &self.0
                }
            }

            impl #edge_type_impl_generics #edge_wrapper #edge_type_ty_generics #edge_type_where_clause {
                #( #edge_to_foreign_node_connection_method) *

                // Recursive edge traversal, e.g. `->knows->(? AS f1)->knows->(? AS f2)`.
                pub fn #edge_name_as_method_ident(
                    &self,
                    clause: impl ::std::convert::Into<#crate_name::EdgeClause>,
                ) -> #edge_wrapper #edge_type_ty_generics {
                    let clause: #crate_name::EdgeClause = clause.into();
                    let clause = clause.with_arrow(#arrow).with_family(#crate_name::TableSpec::of::<#edge_type>());

                    // i.e Edge to Edge
                    #edge_schema::#__________connect(
                        &self.0,
                        clause.into_inner(),
                    ).into()
                }
            }
        ))
    }
}

impl ToTokens for NodeEdgeMetadataLookupTable<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let node_edge_token_streams = self
            .deref()
            .values()
            .map(ToTokens::to_token_stream)
            .collect::<Vec<_>>();

        let token = quote!(#( #node_edge_token_streams) *);

        tokens.extend(token);
    }
}
