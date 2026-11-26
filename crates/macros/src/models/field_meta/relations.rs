use proc_macro2::Span;

/// Direction of a relation edge as written in the `relate` attribute.
#[derive(Debug, Clone, Copy)]
pub(crate) enum EdgeDirection {
    Out,
    In,
}

use super::*;

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

impl TryFrom<&Relate> for RelateAttribute {
    type Error = darling::Error;

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
