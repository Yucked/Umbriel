use proc_macro2::Span;

use crate::models::*;

/// Direction of a relation edge as written in the `relate` attribute.
#[derive(Debug, Clone, Copy)]
pub(crate) enum EdgeDirection {
    Out,
    In,
}


use super::*;
// Tuple, Array (T, T), [T; N]

// FieldTypeMetadata ->
// 1. RawFieldType -> Ident, LifeTime, Generics.
// 1. RawFieldType -> Ident, LifeTime, Generics.

// #[derive(Node, Serialize, Deserialize, Debug, Clone, Default)]
// #[serde(rename_all = "camelCase")]
// #[orm(table = "company")]
// pub struct Company<'a, 'b: 'a, T, U: Clone + Default> {
//     pub id: SurrealSimpleId<Self>,
//     pub name: &'b T, // &'b T RawField
//     pub moniker: &'a Somestuff<T, U>, // &'a Somestuff<T, U> RawFieldType
//     #[orm(link_self = Self<...>)]
//     pub branch: LinkSelf<Self<...>>, // user<'a, t>> rawfieldtype, linkrustfieldtype
//     #[orm(link_many = user<'a, t>)]
//     pub users: LinkMany<user<'a, t>>, // user<'a, t>> rawfieldtype, linkrustfieldtype
//     // impl Company<'a, 'b, T, U> {
//     //    pub fn users(&self) -> User<'a, T> { }
//
//     #[orm(relate(model = "CompanyLikeUser<'a, 'b, T, U>", connection = "->like->user"))]
//     pub devs: RelatedUser<'a, T, U>>, // RawFieldType, EdgeFieldType, DestinationFieldType
//     // pub devs: Relate<User<T>>,
// }

// - Company<'a, 'b, T, U>
// - Like<'a, 'b, T, U>  // Bridge or edge or link between origin and destination nodes
// - User<'a, T, U >
// type RelatedUser<'a, T> = Relate<User<'a, T>>;

#[derive(Debug, Clone)]
pub enum RelationType {
    // ->studies->Course
    Relate(Relate),
    LinkOne(LinkOneAttrType),
    LinkSelf(LinkSelfAttrType),
    LinkMany(LinkManyAttrType),
    // These are in and out nodes in edge tables and
    // we usually dont want to do much code gen for it beyond compile-time check codegen but
    // have it for deserialization and serialization, to enjoy
    // the benefits that `LinkMany` helper wrapper struct provides.
    LinkManyInAndOutEdgeNodesInert(LinkManyAttrType),
    NestObject(NestObjectAttrType),
    NestArray(NestArrayAttrType),
    List(ListSimple),
    None,
}

impl RelationType {
    pub fn is_relate_graph(&self) -> bool {
        matches!(self, RelationType::Relate(_))
    }
}

// macro_rules! wrapper_struct_to_ident {
//     ($simple_wrapper_struct:ty) => {
//         impl ::std::convert::From<&$simple_wrapper_struct> for ::proc_macro2::TokenStream {
//             fn from(simple_wrapper_struct: &$simple_wrapper_struct) -> Self {
//                 let ident = ::quote::format_ident!("{}", simple_wrapper_struct.0);
//                 ::quote::quote!(#ident)
//             }
//         }
//
//         impl ::std::convert::From<$simple_wrapper_struct> for ::proc_macro2::TokenStream {
//             fn from(simple_wrapper_struct: $simple_wrapper_struct) -> Self {
//                 let ident = ::quote::format_ident!("{}", simple_wrapper_struct.0);
//                 ::quote::quote!(#ident)
//             }
//         }
//
//         impl ::std::fmt::Display for $simple_wrapper_struct {
//             fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//                 f.write_fmt(format_args!("{}", self.0))
//             }
//         }
//
//         impl ::std::convert::From<&String> for $simple_wrapper_struct {
//             fn from(value: &String) -> Self {
//                 Self(value.into())
//             }
//         }
//
//         impl ::std::convert::From<$simple_wrapper_struct> for String {
//             fn from(value: $simple_wrapper_struct) -> Self {
//                 value.0
//             }
//         }
//
//
//         impl ::quote::ToTokens for $simple_wrapper_struct {
//             fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
//                 let ident = ::quote::format_ident!("{self}");
//                 tokens.extend(ident.into_token_stream());
//             }
//         }
//     };
// }

// #[derive(Debug, Clone)]
// pub struct EdgeTableName(String);
// wrapper_struct_to_ident!(EdgeTableName);
//
// #[derive(Debug, Clone)]
// pub struct NodeTableName(String);
// wrapper_struct_to_ident!(NodeTableName);

create_ident_wrapper!(EdgeTableName);
create_ident_wrapper!(NodeTableName);

#[derive(Debug, Clone)]
pub struct RelateAttribute {
    pub(crate) edge_direction: EdgeDirection,
    pub(crate) edge_table: EdgeTableName,
    /// user->writes->book // here, user is current struct, book is the foreign node
    /// book<-writes<-user // here, book is current struct, user is the foreign node
    pub(crate) foreign_node_table: NodeTableName,
}

// TODO: Remove
// impl From<RelateAttribute> for ::proc_macro2::TokenStream {
//     fn from(relate_attrs: RelateAttribute) -> Self {
//         let edge_direction = ::proc_macro2::TokenStream::from(relate_attrs.edge_direction);
//         let edge_name = ::proc_macro2::TokenStream::from(relate_attrs.edge_table);
//         let node_name = ::proc_macro2::TokenStream::from(relate_attrs.node_table);
//         // ->action->NodeObject
//         // <-action<-NodeObject
//         // e.g ->manages->Project
//         ::quote::quote!(#edge_direction #edge_name #edge_direction #node_name)
//     }
// }

impl TryFrom<&Relate> for RelateAttribute {
    type Error = ExtractorError;

    fn try_from(relation: &Relate) -> Result<Self, Self::Error> {
        let (edge_direction, edge, node) =
            crate::surql::graph_connection(&relation.connection, Span::call_site())?;

        // The tables also name generated methods, so each must be a Rust identifier too.
        let table = |name: &str| {
            syn::parse_str::<syn::Ident>(name).map_err(|_| {
                syn::Error::new(
                    Span::call_site(),
                    format!(
                        "`{name}` in the connection `{}` must also be a Rust identifier",
                        relation.connection
                    ),
                )
            })
        };

        Ok(Self {
            foreign_node_table: NodeTableName(table(node)?),
            edge_table: EdgeTableName(table(edge)?),
            edge_direction,
        })
    }
}

