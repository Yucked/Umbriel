use syn::{
    AngleBracketedGenericArguments, ExprPath, GenericArgument, Ident, Path, PathArguments,
    PathSegment, TypePath, parse_quote,
    visit_mut::{self, VisitMut},
};

use crate::models::{CustomType, CustomTypeNoSelf};

pub struct ReplaceSelfVisitor {
    pub struct_ident: Ident,
    pub generics: AngleBracketedGenericArguments,
}

impl ReplaceSelfVisitor {
    pub fn replace_self(&mut self, ty: &CustomType) -> CustomTypeNoSelf {
        let mut ty = ty.inner().clone();
        self.visit_type_mut(&mut ty);
        CustomTypeNoSelf::new(ty)
    }
}

impl VisitMut for ReplaceSelfVisitor {
    fn visit_path_segment_mut(&mut self, segment: &mut PathSegment) {
        if segment.ident == "Self" {
            segment.ident = self.struct_ident.clone();
            if !self.generics.args.is_empty() {
                segment.arguments = PathArguments::AngleBracketed(self.generics.clone());
            }
        } else {
            // Recursively handle the arguments of the segment.
            match &mut segment.arguments {
                PathArguments::AngleBracketed(angle_bracketed) => {
                    for arg in &mut angle_bracketed.args {
                        if let GenericArgument::Type(ty) = arg {
                            self.visit_type_mut(ty)
                        }
                    }
                }
                PathArguments::Parenthesized(parenthesized) => {
                    for input in &mut parenthesized.inputs {
                        self.visit_type_mut(&mut input.ty);
                    }
                    self.visit_return_type_mut(&mut parenthesized.output);
                }
                PathArguments::None => {}
            }
        }
    }

    fn visit_angle_bracketed_generic_arguments_mut(
        &mut self,
        i: &mut AngleBracketedGenericArguments,
    ) {
        for arg in &mut i.args {
            self.visit_generic_argument_mut(arg);
        }
    }

    // Replace `Self` in paths
    fn visit_path_mut(&mut self, i: &mut Path) {
        // Example: Replace `Self` in `Self::method()` or just `Self`
        if i.segments.len() == 1 && i.segments[0].ident == "Self" {
            i.segments[0].ident = self.struct_ident.clone();
            if !self.generics.args.is_empty() {
                i.segments[0].arguments = PathArguments::AngleBracketed(self.generics.clone());
            }
        } else {
            visit_mut::visit_path_mut(self, i);
        }
    }

    // Replace `Self` in type paths
    fn visit_type_path_mut(&mut self, i: &mut TypePath) {
        // Example: Replace `Self` in type annotations, e.g., `let x: Self;`
        self.visit_path_mut(&mut i.path);
        visit_mut::visit_type_path_mut(self, i);
    }

    // Replace `Self` in expressions
    fn visit_expr_path_mut(&mut self, i: &mut ExprPath) {
        // Example: Replace `Self` in expressions, e.g., `Self::function()`
        if i.path.is_ident("Self") {
            let struct_ident = &self.struct_ident;
            i.path = parse_quote! { #struct_ident };
            if !self.generics.args.is_empty() {
                i.path
                    .segments
                    .last_mut()
                    // invariant: the path was just set to the struct's identifier.
                    .expect("Empty paths segments.")
                    .arguments = PathArguments::AngleBracketed(self.generics.clone());
            }
        }
        visit_mut::visit_expr_path_mut(self, i);
    }

    // Replace `Self` in reference types (&Self, &mut Self)
    fn visit_type_reference_mut(&mut self, i: &mut syn::TypeReference) {
        // Example: Replace `Self` in `&Self` and `&mut Self`
        syn::visit_mut::visit_type_reference_mut(self, i);
    }

    // Replace `Self` in tuple types (Self, Self)
    fn visit_type_tuple_mut(&mut self, i: &mut syn::TypeTuple) {
        // Example: Replace `Self` in `(Self, Self)`
        syn::visit_mut::visit_type_tuple_mut(self, i);
    }

    // Replace `Self` in slice types ([Self])
    fn visit_type_slice_mut(&mut self, i: &mut syn::TypeSlice) {
        // Example: Replace `Self` in `[Self]`
        syn::visit_mut::visit_type_slice_mut(self, i);
    }

    // Replace `Self` in array types ([Self; N])
    fn visit_type_array_mut(&mut self, i: &mut syn::TypeArray) {
        // Example: Replace `Self` in `[Self; 5]`
        syn::visit_mut::visit_type_array_mut(self, i);
    }

    // Replace `Self` in pointer types (*const Self, *mut Self)
    fn visit_type_ptr_mut(&mut self, i: &mut syn::TypePtr) {
        // Example: Replace `Self` in `*const Self` and `*mut Self`
        syn::visit_mut::visit_type_ptr_mut(self, i);
    }

    // Replace `Self` in group types (used in complex type manipulation)
    fn visit_type_group_mut(&mut self, i: &mut syn::TypeGroup) {
        // Example: Rarely used directly but can wrap other type constructs
        syn::visit_mut::visit_type_group_mut(self, i);
    }

    // Replace `Self` in "never" types (!Self)
    fn visit_type_never_mut(&mut self, i: &mut syn::TypeNever) {
        // Example: Replace `Self` in `!Self`, though `!Self` is a rare and somewhat theoretical use case
        syn::visit_mut::visit_type_never_mut(self, i);
    }

    // Replace `Self` in generic arguments (e.g., Vec<Self>)
    fn visit_generic_argument_mut(&mut self, i: &mut syn::GenericArgument) {
        if let syn::GenericArgument::Type(ty) = i {
            self.visit_type_mut(ty);
        }
        syn::visit_mut::visit_generic_argument_mut(self, i);
    }

    // Replace `Self` in trait objects (e.g., Box<dyn Trait<Self>>)
    fn visit_type_trait_object_mut(&mut self, i: &mut syn::TypeTraitObject) {
        // Example: Replace `Self` in trait objects
        for bound in &mut i.bounds {
            if let syn::TypeParamBound::Trait(trait_bound) = bound {
                for segment in &mut trait_bound.path.segments {
                    self.visit_path_segment_mut(segment);
                }
            }
        }
        syn::visit_mut::visit_type_trait_object_mut(self, i);
    }

    // Replace `Self` in impl Trait (e.g., impl Trait<Self>)
    fn visit_type_impl_trait_mut(&mut self, i: &mut syn::TypeImplTrait) {
        // Example: Replace `Self` in `impl Trait for Self`
        for bound in &mut i.bounds {
            if let syn::TypeParamBound::Trait(trait_bound) = bound {
                for segment in &mut trait_bound.path.segments {
                    self.visit_path_segment_mut(segment);
                }
            }
        }
        syn::visit_mut::visit_type_impl_trait_mut(self, i);
    }
}
