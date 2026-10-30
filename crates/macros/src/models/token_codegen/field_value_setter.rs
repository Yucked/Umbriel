use crate::helpers::get_crate_name;
use proc_macro2::TokenStream;
use quote::quote;

use crate::models::*;

create_tokenstream_wrapper!(=>FieldSetterNumericImpl);
create_tokenstream_wrapper!(=>ArrayElementFieldSetterToken);
create_tokenstream_wrapper!(=>FieldTypeSetterPatcherImpls);

create_tokenstream_wrapper!(
    /// The generated wrapper of one schema field and its implementations. The wrapper is generic
    /// over the model's parameters, so each specialization of a generic model has its own field
    /// types, e.g. for `email: Duration` on `User<T>`:
    /// ```rust,ignore
    /// pub struct __4User5emailField<T>(pub Field, pub PhantomData<fn() -> (T,)>);
    ///
    /// impl<T: Bound> umbriel::SetterAssignable<Duration> for __4User5emailField<T> {}
    /// impl<T: Bound> umbriel::__private::ModelField<User<T>, Duration> for __4User5emailField<T> {}
    /// ```
=>
FieldSetterImplTokens);

use super::Codegen;

/// The model's generics as the wrapper's own definition takes them: without bounds or defaults,
/// since the schema struct names the wrapper without the model's bounds. The bounds sit on the
/// wrapper's implementations instead.
fn unbounded_definition_generics(model: &syn::Generics) -> syn::Generics {
    let mut generics = model.clone();
    generics.where_clause = None;
    for param in &mut generics.params {
        match param {
            syn::GenericParam::Type(type_param) => {
                type_param.bounds.clear();
                type_param.colon_token = None;
                type_param.default = None;
            }
            syn::GenericParam::Lifetime(lifetime) => {
                lifetime.bounds.clear();
                lifetime.colon_token = None;
            }
            syn::GenericParam::Const(const_param) => {
                const_param.default = None;
            }
        }
    }
    generics
}

/// `PhantomData<fn() -> (T, &'a (), …)>`: marks every type and lifetime parameter as used,
/// without bounds and without affecting `Send` or `Sync`. Const parameters need no marker.
fn parameter_marker(model: &syn::Generics) -> TokenStream {
    let used = model.params.iter().filter_map(|param| match param {
        syn::GenericParam::Type(type_param) => {
            let ident = &type_param.ident;
            Some(quote!(#ident))
        }
        syn::GenericParam::Lifetime(lifetime) => {
            let lifetime = &lifetime.lifetime;
            Some(quote!(&#lifetime ()))
        }
        syn::GenericParam::Const(_) => None,
    });
    quote!(::std::marker::PhantomData<fn() -> ( #( #used, )* )>)
}

/// The model's impl generics with one more type parameter, for implementations generic over an
/// operand, and that parameter's name. The name is `__UmbrielOperand`, suffixed with a number
/// when the model already declares a parameter of that name, so it never clashes.
fn generics_with_operand(model: &syn::Generics, bound: TokenStream) -> (syn::Generics, syn::Ident) {
    let taken = model
        .params
        .iter()
        .filter_map(|param| match param {
            syn::GenericParam::Type(type_param) => Some(type_param.ident.to_string()),
            syn::GenericParam::Const(const_param) => Some(const_param.ident.to_string()),
            syn::GenericParam::Lifetime(_) => None,
        })
        .collect::<std::collections::HashSet<_>>();
    let mut name = "__UmbrielOperand".to_owned();
    let mut suffix = 0_usize;
    while taken.contains(&name) {
        name = format!("__UmbrielOperand{suffix}");
        suffix += 1;
    }
    let operand = syn::Ident::new(&name, proc_macro2::Span::mixed_site());
    let mut generics = model.clone();
    generics.params.push(syn::parse_quote!(#operand: #bound));
    (generics, operand)
}

impl Codegen<'_> {
    pub fn create_field_setter_impl(&mut self) -> ExtractorResult<()> {
        let field_receiver = self.field_receiver();

        match field_receiver.to_relation_type(self.table_derive_attributes()) {
            // Relate fields are readonly and mainly for aliasing connections in select statements
            // To create a relation, we typically use a separate relation statement
            RelationType::Relate(_) => {}
            _ => {
                self.field_wrapper_type_custom_implementations
                    .push(self.get_field_value_setter_impl()?);
            }
        };
        Ok(())
    }

    fn get_field_value_setter_impl(&self) -> ExtractorResult<FieldSetterImplTokens> {
        let crate_name = get_crate_name(false);
        let field_receiver = self.field_receiver();
        let table_attributes = self.table_derive_attributes();
        let casing = &table_attributes.casing()?;
        let field_wrapper = field_receiver.field_wrapper_ident(table_attributes)?;
        let field_type = field_receiver
            .ty()
            .replace_self_with_current_struct_concrete_type(table_attributes)?;

        let model_generics = table_attributes.generics().to_basic_generics_ref();
        let (impl_generics, ty_generics, where_clause) = model_generics.split_for_impl();
        let definition_generics = unbounded_definition_generics(model_generics);
        let marker = parameter_marker(model_generics);
        // Named from the model's identifier and its type generics (`Eats<In, Out>`), without the
        // bounds its declaration carries.
        let model_ident = table_attributes.ident();
        let model_type = quote!(#model_ident #ty_generics);
        let wrapper = quote!(#field_wrapper #ty_generics);

        let setter_assignable_impl = quote!(
            impl #impl_generics #crate_name::SetterAssignable<#field_type> for #wrapper #where_clause {}
        );

        let is_id = field_receiver.db_field_name(casing)?.is_id();
        let field_type_setter_patcher_impls: FieldTypeSetterPatcherImpls = if is_id
            && table_attributes.to_data_type().is_node_or_edge()
        {
            quote!(#setter_assignable_impl).into()
        } else {
            quote!(
                #setter_assignable_impl

                impl #impl_generics #crate_name::Patchable<#field_type> for #wrapper #where_clause {}
            ).into()
        };

        // An `id`'s explicit `ty` is its key type; the field itself is a record id, never a
        // number or a list.
        let numeric_trait = if !is_id && field_receiver.is_numeric() {
            Self::numeric_setter_impl(field_receiver, table_attributes)?
        } else {
            quote!().into()
        };

        let array_trait = if !is_id && (field_receiver.is_array() || field_receiver.is_set()) {
            Self::array_trait_impl(field_receiver, table_attributes)?
        } else {
            quote!().into()
        };

        // A computed field, and any field of a view, is never written: without these, `.set(…)`,
        // patches and `object!` refuse it at compile time.
        let assignments = (!field_receiver.is_computed() && !table_attributes.receiver().view)
            .then(|| {
                quote!(
                    #field_type_setter_patcher_impls

                    #numeric_trait

                    #array_trait
                )
            });

        let field_setter_impls = quote!(
            // Keeps the schema field's own case, e.g. `__4User7foo_barField`.
            #[doc(hidden)]
            #[allow(non_camel_case_types)]
            pub struct #field_wrapper #definition_generics (
                pub #crate_name::Field,
                #[doc(hidden)] pub #marker,
            );

            // Hand-written: derives would require the model's parameters to be `Debug`/`Clone`.
            impl #impl_generics ::std::fmt::Debug for #wrapper #where_clause {
                fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                    f.debug_tuple(::std::stringify!(#field_wrapper)).field(&self.0).finish()
                }
            }

            impl #impl_generics ::std::clone::Clone for #wrapper #where_clause {
                fn clone(&self) -> Self {
                    Self(::std::clone::Clone::clone(&self.0), ::std::marker::PhantomData)
                }
            }

            impl #impl_generics ::std::convert::From<#crate_name::Field> for #wrapper #where_clause {
                fn from(field: #crate_name::Field) -> Self {
                    Self(field, ::std::marker::PhantomData)
                }
            }

            impl #impl_generics ::std::convert::From<&#wrapper> for #crate_name::ValueLike #where_clause {
                fn from(value: &#wrapper) -> Self {
                    #crate_name::ValueLike::from(&value.0)
                }
            }

            impl #impl_generics ::std::convert::From<#wrapper> for #crate_name::ValueLike #where_clause {
                fn from(value: #wrapper) -> Self {
                    #crate_name::ValueLike::from(value.0)
                }
            }

            impl #impl_generics ::std::convert::From<&#wrapper> for #crate_name::Field #where_clause {
                fn from(field: &#wrapper) -> Self {
                    field.0.clone()
                }
            }

            impl #impl_generics ::std::convert::From<#wrapper> for #crate_name::Field #where_clause {
                fn from(field: #wrapper) -> Self {
                    field.0
                }
            }

            impl #impl_generics ::std::ops::Deref for #wrapper #where_clause {
                type Target = #crate_name::Field;

                fn deref(&self) -> &Self::Target {
                    &self.0
                }
            }

            impl #impl_generics ::std::ops::DerefMut for #wrapper #where_clause {
                fn deref_mut(&mut self) -> &mut Self::Target {
                    &mut self.0
                }
            }

            // A field stands for a value of its own type only; an explicit `Field` remains the
            // untyped form.
            impl #impl_generics ::std::convert::From<#wrapper> for #crate_name::SetterArg<#field_type> #where_clause {
                fn from(value: #wrapper) -> Self {
                    Self::from(value.0)
                }
            }

            impl #impl_generics ::std::convert::From<&#wrapper> for #crate_name::SetterArg<#field_type> #where_clause {
                fn from(value: &#wrapper) -> Self {
                    Self::from(&value.0)
                }
            }

            impl #impl_generics #crate_name::__private::ModelField<#model_type, #field_type> for #wrapper #where_clause {}

            #assignments
        );
        Ok(FieldSetterImplTokens(field_setter_impls))
    }

    // For Setters trait implementations for setting, and appending to array/set fields.
    // So, this should probably not exist for other non list fields
    fn array_trait_impl(
        field_receiver: &MyFieldReceiver,
        model_attributes: &ModelAttributes,
    ) -> ExtractorResult<ArrayElementFieldSetterToken> {
        let crate_name = get_crate_name(false);
        let field_wrapper = field_receiver.field_wrapper_ident(model_attributes)?;
        let (impl_generics, ty_generics, where_clause) = model_attributes
            .generics()
            .to_basic_generics_ref()
            .split_for_impl();

        let array_item_type = match field_receiver.to_relation_type(model_attributes) {
            RelationType::LinkMany(foreign_node) => {
                Some(quote!(#crate_name::RecordId<#foreign_node>))
            }
            RelationType::LinkManyInAndOutEdgeNodesInert(_foreign_node_generics) => {
                Some(quote!(#crate_name::types::RecordId))
            }
            RelationType::NestArray(foreign_object) => Some(quote!(#foreign_object)),
            RelationType::Relate(_)
            | RelationType::LinkOne(_)
            | RelationType::LinkSelf(_)
            | RelationType::NestObject(_)
            | RelationType::List(_)
            | RelationType::None => {
                let field_type = field_receiver.ty();
                let guessed_arrayish_item_type = field_type
                    .get_array_inner_type()
                    .or_else(|| field_type.get_set_inner_type());

                field_receiver
                    .array_item_ty_specified
                    .as_deref()
                    .or(guessed_arrayish_item_type.as_ref())
                    .map(|ty| quote!(#ty))
            }
        };

        let array_setter_impl = array_item_type.map_or(quote!(), |item_type| {
            quote!(
                impl #impl_generics #crate_name::SetterArray<#item_type> for
                #field_wrapper #ty_generics #where_clause {}
            )
        });

        Ok(ArrayElementFieldSetterToken(array_setter_impl))
    }

    fn numeric_setter_impl(
        field_receiver: &MyFieldReceiver,
        model_attributes: &ModelAttributes,
    ) -> ExtractorResult<FieldSetterNumericImpl> {
        let crate_name = get_crate_name(false);
        let field_wrapper = field_receiver.field_wrapper_ident(model_attributes)?;
        let field_type = field_receiver
            .ty()
            .replace_self_with_current_struct_concrete_type(model_attributes)?;
        let model_generics = model_attributes.generics().to_basic_generics_ref();
        let (impl_generics, ty_generics, where_clause) = model_generics.split_for_impl();
        let wrapper = quote!(#field_wrapper #ty_generics);
        let (with_operand, operand) = generics_with_operand(
            model_generics,
            quote!(::std::convert::Into<#crate_name::NumberLike>),
        );
        let (operand_impl_generics, _, _) = with_operand.split_for_impl();

        // Arithmetic is the field's own, written once in `umbriel`.
        let arithmetic = [
            (quote!(Add), quote!(add)),
            (quote!(Sub), quote!(sub)),
            (quote!(Mul), quote!(mul)),
            (quote!(Div), quote!(div)),
        ]
        .into_iter()
        .map(|(op, method)| {
            quote!(
                impl #operand_impl_generics ::std::ops::#op<#operand> for #wrapper #where_clause {
                    type Output = #crate_name::Operation;

                    fn #method(self, rhs: #operand) -> Self::Output {
                        ::std::ops::#op::#method(self.0, rhs)
                    }
                }

                impl #operand_impl_generics ::std::ops::#op<#operand> for &#wrapper #where_clause {
                    type Output = #crate_name::Operation;

                    fn #method(self, rhs: #operand) -> Self::Output {
                        ::std::ops::#op::#method(self.0.clone(), rhs)
                    }
                }
            )
        });

        let numeric_trait = quote!(
            impl #impl_generics #crate_name::SetterNumeric<#field_type> for #wrapper #where_clause {}

            impl #impl_generics ::std::convert::From<#wrapper> for #crate_name::NumberLike #where_clause {
                fn from(val: #wrapper) -> Self {
                    val.0.into()
                }
            }

            impl #impl_generics ::std::convert::From<&#wrapper> for #crate_name::NumberLike #where_clause {
                fn from(val: &#wrapper) -> Self {
                    (&val.0).into()
                }
            }

            #( #arithmetic )*
        );
        Ok(FieldSetterNumericImpl(numeric_trait))
    }
}
