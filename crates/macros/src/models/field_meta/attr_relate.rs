use crate::models::*;
use darling::FromMeta;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    Constraint, GenericArgument, Ident, Lifetime, Path, PathArguments, PathSegment, Type,
    TypeParamBound,
    visit::{self, Visit},
};

#[derive(Debug, Clone)]
pub struct Relate {
    /// e.g ->writes->book
    pub connection: String,
    // #[darling(default)]
    /// e.g StudentWritesBook,
    /// derived from: type StudentWritesBook = Writes<Student, Book>;
    /// e.g2
    /// StudentWritesBook<'a, 'b: 'a, T, U>,
    /// derived from: type StudentWritesBook<'a, 'b: 'a, T, U> = Writes<'a, 'b: 'a, T, U><Student<'a, 'b, T, Book<U>>;
    pub edge_type: EdgeType,
}

// edge model rust type with generics
create_custom_type_wrapper!(EdgeType);

create_custom_type_wrapper!(EdgeTypeWithAggregatedGenerics);

impl EdgeType {
    pub fn aggregate_lifetime_and_generics_from_variations_of_a_type(
        edge_type_ident: &Ident,
        edge_types: &[Self],
    ) -> EdgeTypeWithAggregatedGenerics {
        // In the order the edge types first name them: the order of a type's generic arguments
        // is part of the type, and the expansion must not depend on hash order.
        let mut common_lifetimes = Vec::new();
        let mut common_generics = Vec::new();

        for edge_type in edge_types {
            let mut visitor = UniqueTypeVisitor::default();
            visitor.visit_type(edge_type.inner());

            for lt in visitor.lifetimes {
                insert_once(&mut common_lifetimes, lt);
            }
            for generic in visitor.generics {
                insert_once(&mut common_generics, generic);
            }
        }

        let lifetimes: Vec<Lifetime> = common_lifetimes
            .iter()
            .map(|lt| Lifetime::new(&format!("'{}", lt), proc_macro2::Span::call_site()))
            .collect();

        let generics: Vec<Ident> = common_generics
            .iter()
            .map(|generic| Ident::new(generic, proc_macro2::Span::call_site()))
            .collect();

        let quoted_type: TokenStream = if !lifetimes.is_empty() || !generics.is_empty() {
            let generics_list = quote! { <#(#lifetimes,)* #(#generics,)*> };
            quote! { #edge_type_ident #generics_list }
        } else {
            quote! { #edge_type_ident }
        };

        let edge_type = syn::parse2::<Type>(quoted_type).expect("Failed to parse edge type");

        edge_type.into()
    }
}

/// Adds `name` to `names` unless it's already there, keeping first-seen order.
fn insert_once(names: &mut Vec<String>, name: String) {
    if !names.contains(&name) {
        names.push(name);
    }
}

/// The lifetimes and type parameters a type names, each once, in the order it names them.
#[derive(Default)]
struct UniqueTypeVisitor {
    lifetimes: Vec<String>,
    generics: Vec<String>,
}

impl<'ast> Visit<'ast> for UniqueTypeVisitor {
    fn visit_lifetime(&mut self, i: &'ast syn::Lifetime) {
        insert_once(&mut self.lifetimes, i.ident.to_string());
        visit::visit_lifetime(self, i);
    }

    fn visit_type_param(&mut self, i: &'ast syn::TypeParam) {
        insert_once(&mut self.generics, i.ident.to_string());
        visit::visit_type_param(self, i);
    }

    // Visit path arguments to handle generics in paths, e.g., `Vec<T>`
    fn visit_path_arguments(&mut self, path_arguments: &'ast PathArguments) {
        if let PathArguments::AngleBracketed(args) = path_arguments {
            for arg in &args.args {
                match arg {
                    GenericArgument::Type(Type::Path(type_path)) => {
                        self.visit_type_path(type_path);
                    }
                    GenericArgument::Lifetime(lt) => {
                        self.visit_lifetime(lt);
                    }
                    _ => {}
                }
            }
        }

        visit::visit_path_arguments(self, path_arguments);
    }

    // Visit paths, useful for extracting generics from qualified types
    fn visit_path(&mut self, i: &'ast Path) {
        for segment in &i.segments {
            self.visit_path_segment(segment);
        }
        visit::visit_path(self, i);
    }

    fn visit_path_segment(&mut self, segment: &'ast PathSegment) {
        if let PathArguments::AngleBracketed(ref args) = segment.arguments {
            for arg in &args.args {
                match arg {
                    GenericArgument::Type(ty) => {
                        self.visit_type(ty);
                    }
                    GenericArgument::Lifetime(lt) => {
                        self.visit_lifetime(lt);
                    }
                    // e.g., `T: Display` in `Foo<T: Display>`
                    GenericArgument::Constraint(constraint) => {
                        self.visit_constraint(constraint);
                    }
                    GenericArgument::Const(_) => {}
                    _ => {}
                }
            }
        }

        visit::visit_path_segment(self, segment);
    }

    fn visit_constraint(&mut self, i: &'ast Constraint) {
        insert_once(&mut self.generics, i.ident.to_string());
        for bound in i.bounds.iter() {
            self.visit_type_param_bound(bound)
        }
    }

    // e.g: `T: Display` in generics
    fn visit_type_param_bound(&mut self, bound: &'ast TypeParamBound) {
        match &bound {
            TypeParamBound::Lifetime(lt) => {
                self.visit_lifetime(lt);
            }
            TypeParamBound::Trait(trait_bound) => {
                self.visit_path(&trait_bound.path);
            }
            // e.g., `?Sized` in `T: ?Sized`
            TypeParamBound::Verbatim(token) if token.to_string() == "?Sized" => {
                insert_once(&mut self.generics, token.to_string());
            }
            _ => {}
        }

        visit::visit_type_param_bound(self, bound);
    }
}

impl FromMeta for Relate {
    fn from_list(items: &[darling::ast::NestedMeta]) -> darling::Result<Self> {
        #[derive(FromMeta)]
        struct FullRelate {
            model: EdgeType,
            connection: String,
        }

        impl From<FullRelate> for Relate {
            fn from(v: FullRelate) -> Self {
                let FullRelate {
                    connection, model, ..
                } = v;
                Self {
                    connection,
                    edge_type: model,
                }
            }
        }
        FullRelate::from_list(items).map(Relate::from)
    }
}
