// Copyright 2024 Mentat Contributors
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use
// this file except in compliance with the License. You may obtain a copy of the
// License at http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software distributed
// under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
// CONDITIONS OF ANY KIND, either express or implied. See the License for the
// specific language governing permissions and limitations under the License.

//! Procedural macro for deriving the Entity trait

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Attribute, Data, DeriveInput, Fields, GenericArgument, Lit, Meta, MetaNameValue, NestedMeta,
    PathArguments, Type, parse_macro_input,
};

/// Derive macro for the Entity trait
///
/// # Attributes
///
/// ## Container attributes (on the struct):
/// - `#[entity(namespace = "my_namespace")]` - Required namespace for entity attributes
///
/// ## Field attributes:
/// - `#[entity(unique = "identity")]` - Mark field as unique identity (for upserts)
/// - `#[entity(unique = "value")]` - Mark field as unique value
/// - `#[entity(indexed)]` - Mark field as indexed
/// - `#[entity(many)]` - Mark field as having many cardinality (default is one)
///
/// # Example
///
/// ```ignore
/// #[derive(Entity)]
/// #[entity(namespace = "person")]
/// struct Person {
///     #[entity(unique = "identity")]
///     email: String,
///     name: String,
///     age: Option<i64>,
/// }
/// ```
#[proc_macro_derive(Entity, attributes(entity))]
pub fn derive_entity(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = &input.ident;
    let namespace = extract_namespace(&input.attrs);

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => panic!("Entity can only be derived for structs with named fields"),
        },
        _ => panic!("Entity can only be derived for structs"),
    };

    // Generate field definitions
    let mut field_defs = Vec::new();
    let mut to_values_fields = Vec::new();
    let mut from_values_fields = Vec::new();
    let mut write_fields = Vec::new();

    for field in fields {
        let field_name = field.ident.as_ref().unwrap();
        let field_name_str = field_name.to_string().replace("r#", "");
        let field_attrs = parse_field_attributes(&field.attrs);

        let (field_type, is_optional) = extract_field_type(&field.ty);
        let field_type_enum = rust_type_to_field_type(&field_type);

        let unique_variant = match field_attrs.unique.as_deref() {
            Some("identity") => quote! { mentat_entity::Unique::Identity },
            Some("value") => quote! { mentat_entity::Unique::Value },
            _ => quote! { mentat_entity::Unique::None },
        };

        let cardinality = if field_attrs.many {
            quote! { mentat_entity::Cardinality::Many }
        } else {
            quote! { mentat_entity::Cardinality::One }
        };

        let indexed = field_attrs.indexed;

        // Field definition
        field_defs.push(quote! {
            mentat_entity::FieldDefinition {
                name: #field_name_str.to_string(),
                ident: Keyword::namespaced(#namespace, #field_name_str),
                field_type: #field_type_enum,
                cardinality: #cardinality,
                unique: #unique_variant,
                indexed: #indexed,
                optional: #is_optional,
            }
        });

        // to_values implementation
        let keyword = quote! { Keyword::namespaced(#namespace, #field_name_str) };

        if is_optional {
            let to_typed_value_inner = generate_to_typed_value(&field_type, quote! { val });
            to_values_fields.push(quote! {
                if let Some(ref val) = self.#field_name {
                    values.insert(#keyword, #to_typed_value_inner);
                }
            });
        } else {
            let to_typed_value = generate_to_typed_value(&field_type, quote! { self.#field_name });
            to_values_fields.push(quote! {
                values.insert(#keyword, #to_typed_value);
            });
        }

        // from_values implementation
        let from_typed_value = generate_from_typed_value(&field_type, quote! { value });

        if is_optional {
            from_values_fields.push(quote! {
                #field_name: values.remove(&#keyword).map(|value| #from_typed_value).transpose()
                    .map_err(|e| mentat::MentatError::EntityError(e.to_string()))?
            });
        } else {
            from_values_fields.push(quote! {
                #field_name: {
                    let value = values.remove(&#keyword)
                        .ok_or_else(|| mentat::MentatError::EntityError(format!("Missing required field: {}", #field_name_str)))?;
                    #from_typed_value?
                }
            });
        }

        // write fields - similar to to_values but for EntityBuilder
        if is_optional {
            let to_typed_value_inner = generate_to_typed_value(&field_type, quote! { val });
            write_fields.push(quote! {
                if let Some(ref val) = self.#field_name {
                    builder.add(entity.clone(), #keyword, #to_typed_value_inner)?;
                }
            });
        } else {
            let to_typed_value = generate_to_typed_value(&field_type, quote! { self.#field_name });
            write_fields.push(quote! {
                builder.add(entity.clone(), #keyword, #to_typed_value)?;
            });
        }
    }

    // Generate the implementation
    let expanded = quote! {
        impl mentat_entity::Entity for #name {
            fn schema() -> mentat_entity::EntitySchema {
                mentat_entity::EntitySchema {
                    namespace: #namespace.to_string(),
                    fields: vec![
                        #(#field_defs),*
                    ],
                }
            }

            fn namespace() -> &'static str {
                #namespace
            }

            fn to_values(&self) -> std::collections::HashMap<Keyword, mentat_entity::core_traits::TypedValue> {
                let mut values = std::collections::HashMap::new();
                // #(#to_values_fields)*
                values
            }

            fn from_values(mut values: std::collections::HashMap<Keyword, mentat_entity::core_traits::TypedValue>)
                -> mentat_entity::public_traits::errors::Result<Self>
            {
                Ok(#name {
                    #(#from_values_fields),*
                })
            }
        }

        impl mentat_entity::EntityWrite for #name {
            fn write<'a, 'c>(&self, in_progress: &mut mentat_entity::mentat_transaction::InProgress<'a, 'c>)
                -> mentat_entity::public_traits::errors::Result<mentat_entity::core_traits::Entid>
            {
                use mentat_entity::mentat_transaction::entity_builder::{BuildTerms, TermBuilder};

                let mut builder = TermBuilder::new();
                let entity = builder.named_tempid("e");

                #(#write_fields)*

                let (terms, tempids) = builder.build()?;
                let report = in_progress.transact_entities(terms)?;

                // Get the entid for our temp entity
                let entid = report.tempids.get("e")
                    .ok_or_else(|| mentat::MentatError::EntityError("Failed to get entity ID from transaction".to_string()))?;

                Ok(*entid)
            }

            fn write_with_entid<'a, 'c>(&self, in_progress: &mut mentat_entity::mentat_transaction::InProgress<'a, 'c>, entid: mentat_entity::core_traits::Entid)
                -> mentat_entity::public_traits::errors::Result<mentat_entity::core_traits::Entid>
            {
                use mentat_entity::mentat_transaction::entity_builder::{BuildTerms, TermBuilder};

                let mut builder = TermBuilder::new();
                let entity = entid;

                #(#write_fields)*

                let (terms, tempids) = builder.build()?;
                in_progress.transact_entities(terms)?;

                Ok(entid)
             }
         }

         impl mentat_entity::EntityRead for #name {
             fn read<'a, 'c>(in_progress: &mentat_entity::mentat_transaction::InProgress<'a, 'c>, entid: mentat_entity::core_traits::Entid)
                 -> mentat_entity::public_traits::errors::Result<Self>
             {
                 // Get the schema and read all attributes
                 let schema = Self::schema();
                 let values = mentat_entity::read_entity_attributes(in_progress, entid, &schema)?;

                 // Convert to entity using from_values
                 Self::from_values(values)
             }

             fn read_by_unique<'a, 'c>(
                 in_progress: &mentat_entity::mentat_transaction::InProgress<'a, 'c>,
                 attribute: &Keyword,
                 value: mentat_entity::core_traits::TypedValue,
             ) -> mentat_entity::public_traits::errors::Result<Self> {
                 // Find entity by unique attribute
                 let entid = mentat_entity::find_entity_by_unique(in_progress, attribute, value)?
                     .ok_or_else(|| mentat::MentatError::EntityError("Entity not found".to_string()))?;

                 // Read the entity
                 Self::read(in_progress, entid)
             }
         }
    };

    TokenStream::from(expanded)
}

fn extract_namespace(attrs: &[Attribute]) -> String {
    for attr in attrs {
        if attr.path.is_ident("entity") {
            if let Ok(Meta::List(meta_list)) = attr.parse_meta() {
                for nested in &meta_list.nested {
                    if let NestedMeta::Meta(Meta::NameValue(MetaNameValue {
                        path,
                        lit: Lit::Str(s),
                        ..
                    })) = nested
                    {
                        if path.is_ident("namespace") {
                            return s.value();
                        }
                    }
                }
            }
        }
    }
    panic!("Entity derive requires #[entity(namespace = \"...\")] attribute");
}

struct FieldAttributes {
    unique: Option<String>,
    indexed: bool,
    many: bool,
}

fn parse_field_attributes(attrs: &[Attribute]) -> FieldAttributes {
    let mut result = FieldAttributes {
        unique: None,
        indexed: false,
        many: false,
    };

    for attr in attrs {
        if attr.path.is_ident("entity") {
            if let Ok(Meta::List(meta_list)) = attr.parse_meta() {
                for nested in &meta_list.nested {
                    match nested {
                        NestedMeta::Meta(Meta::NameValue(MetaNameValue {
                            path,
                            lit: Lit::Str(s),
                            ..
                        })) => {
                            if path.is_ident("unique") {
                                result.unique = Some(s.value());
                            }
                        }
                        NestedMeta::Meta(Meta::Path(path)) => {
                            if path.is_ident("indexed") {
                                result.indexed = true;
                            } else if path.is_ident("many") {
                                result.many = true;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    result
}

fn extract_field_type(ty: &Type) -> (String, bool) {
    match ty {
        Type::Path(type_path) => {
            let segment = &type_path.path.segments.last().unwrap();
            let type_name = segment.ident.to_string();

            // Check if it's Option<T>
            if type_name == "Option" {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(inner_ty)) = args.args.first() {
                        let (inner_type, _) = extract_field_type(inner_ty);
                        return (inner_type, true);
                    }
                }
            }

            (type_name, false)
        }
        _ => panic!("Unsupported field type"),
    }
}

fn rust_type_to_field_type(rust_type: &str) -> proc_macro2::TokenStream {
    match rust_type {
        "String" => quote! { mentat_entity::FieldType::String },
        "i64" => quote! { mentat_entity::FieldType::Long },
        "f64" => quote! { mentat_entity::FieldType::Double },
        "bool" => quote! { mentat_entity::FieldType::Boolean },
        "DateTime" => quote! { mentat_entity::FieldType::Instant },
        "Uuid" => quote! { mentat_entity::FieldType::Uuid },
        "Keyword" => quote! { mentat_entity::FieldType::Keyword },
        "Entid" => quote! { mentat_entity::FieldType::Ref },
        _ => panic!("Unsupported Rust type for Entity field: {}", rust_type),
    }
}

fn generate_to_typed_value(
    field_type: &str,
    value_expr: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    match field_type {
        "String" => {
            quote! { mentat_entity::core_traits::TypedValue::String(#value_expr.clone().into()) }
        }
        "i64" => quote! { mentat_entity::core_traits::TypedValue::Long(*#value_expr) },
        "f64" => quote! { mentat_entity::core_traits::TypedValue::Double((*#value_expr).into()) },
        "bool" => quote! { mentat_entity::core_traits::TypedValue::Boolean(#value_expr) },
        "DateTime" => quote! { mentat_entity::core_traits::TypedValue::Instant(#value_expr) },
        "Uuid" => quote! { mentat_entity::core_traits::TypedValue::Uuid(#value_expr) },
        "Keyword" => quote! { mentat_entity::core_traits::TypedValue::Keyword(#value_expr.into()) },
        "Entid" => quote! { mentat_entity::core_traits::TypedValue::Ref(#value_expr) },
        _ => panic!("Unsupported type conversion: {}", field_type),
    }
}

fn generate_from_typed_value(
    field_type: &str,
    value_expr: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    match field_type {
        "String" => quote! {
            if let mentat_entity::core_traits::TypedValue::String(s) = #value_expr {
                Ok(s.to_string())
            } else {
                Err(mentat::MentatError::EntityError("Expected String value".to_string()))
            }
        },
        "i64" => quote! {
            if let mentat_entity::core_traits::TypedValue::Long(n) = #value_expr {
                Ok(n)
            } else {
                Err(mentat::MentatError::EntityError("Expected Long value".to_string()))
            }
        },
        "f64" => quote! {
            if let mentat_entity::core_traits::TypedValue::Double(d) = #value_expr {
                Ok(d.into_inner())
            } else {
                Err(mentat::MentatError::EntityError("Expected Double value".to_string()))
            }
        },
        "bool" => quote! {
            if let mentat_entity::core_traits::TypedValue::Boolean(b) = #value_expr {
                Ok(b)
            } else {
                Err(mentat::MentatError::EntityError("Expected Boolean value".to_string()))
            }
        },
        "DateTime" => quote! {
            if let mentat_entity::core_traits::TypedValue::Instant(dt) = #value_expr {
                Ok(dt)
            } else {
                Err(mentat::MentatError::EntityError("Expected Instant value".to_string()))
            }
        },
        "Uuid" => quote! {
            if let mentat_entity::core_traits::TypedValue::Uuid(u) = #value_expr {
                Ok(u)
            } else {
                Err(mentat::MentatError::EntityError("Expected Uuid value".to_string()))
            }
        },
        "Keyword" => quote! {
            if let mentat_entity::core_traits::TypedValue::Keyword(k) = #value_expr {
                Ok(k.into())
            } else {
                Err(mentat::MentatError::EntityError("Expected Keyword value".to_string()))
            }
        },
        "Entid" => quote! {
            if let mentat_entity::core_traits::TypedValue::Ref(e) = #value_expr {
                Ok(e)
            } else {
                Err(mentat::MentatError::EntityError("Expected Ref value".to_string()))
            }
        },
        _ => panic!("Unsupported type conversion: {}", field_type),
    }
}

// ============================================================================
// EntityView and EntityPatch derive macros (from tech spec)
// ============================================================================

/// Helper function to convert a string to snake_case
fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    let mut prev_was_uppercase = false;
    
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() {
            if i > 0 && !prev_was_uppercase {
                result.push('_');
            }
            result.push(ch.to_lowercase().next().unwrap());
            prev_was_uppercase = true;
        } else {
            result.push(ch);
            prev_was_uppercase = false;
        }
    }
    
    result
}

/// Extract namespace from container attributes, with default to snake_case of struct name
fn extract_namespace_or_default(attrs: &[Attribute], struct_name: &str) -> String {
    for attr in attrs {
        if attr.path.is_ident("entity") {
            if let Ok(Meta::List(meta_list)) = attr.parse_meta() {
                for nested in &meta_list.nested {
                    if let NestedMeta::Meta(Meta::NameValue(MetaNameValue {
                        path,
                        lit: Lit::Str(s),
                        ..
                    })) = nested
                    {
                        if path.is_ident("ns") || path.is_ident("namespace") {
                            return s.value();
                        }
                    }
                }
            }
        }
    }
    // Default: snake_case of struct name
    to_snake_case(struct_name)
}

fn parse_view_field_attributes(attrs: &[Attribute]) -> (Option<String>, Option<String>, bool, bool) {
    let mut attr_override = None;
    let mut ref_attr = None;
    let mut is_ref = false;
    let mut is_backref = false;

    for attr in attrs {
        // Get the attribute name as a string to handle both "ref" and "r#ref"
        let attr_name = attr.path.get_ident().map(|i| i.to_string());
        
        if attr.path.is_ident("attr") {
            if let Ok(Meta::List(meta_list)) = attr.parse_meta() {
                for nested in &meta_list.nested {
                    if let NestedMeta::Lit(Lit::Str(s)) = nested {
                        attr_override = Some(s.value());
                    }
                }
            }
        } else if attr_name.as_deref() == Some("ref") || attr.path.is_ident("ref") {
            is_ref = true;
            if let Ok(Meta::List(meta_list)) = attr.parse_meta() {
                for nested in &meta_list.nested {
                    if let NestedMeta::Meta(Meta::NameValue(MetaNameValue {
                        path,
                        lit: Lit::Str(s),
                        ..
                    })) = nested
                    {
                        if path.is_ident("attr") {
                            ref_attr = Some(s.value());
                        }
                    }
                }
            }
        } else if attr.path.is_ident("backref") {
            is_backref = true;
            if let Ok(Meta::List(meta_list)) = attr.parse_meta() {
                for nested in &meta_list.nested {
                    if let NestedMeta::Meta(Meta::NameValue(MetaNameValue {
                        path,
                        lit: Lit::Str(s),
                        ..
                    })) = nested
                    {
                        if path.is_ident("attr") {
                            ref_attr = Some(s.value());
                        }
                    }
                }
            }
        }
    }

    (attr_override, ref_attr, is_ref, is_backref)
}

fn extract_type_info(ty: &Type) -> (String, bool, Option<String>) {
    match ty {
        Type::Path(type_path) => {
            let segment = type_path.path.segments.last().unwrap();
            let type_name = segment.ident.to_string();

            // Check for Option<T>
            if type_name == "Option" {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(inner_ty)) = args.args.first() {
                        let (inner_type, is_vec, nested) = extract_type_info(inner_ty);
                        return (inner_type, is_vec, nested);
                    }
                }
            }

            // Check for Vec<T>
            if type_name == "Vec" {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(inner_ty)) = args.args.first() {
                        if let Type::Path(inner_path) = inner_ty {
                            let inner_segment = inner_path.path.segments.last().unwrap();
                            let inner_name = inner_segment.ident.to_string();
                            return (inner_name.clone(), true, Some(inner_name));
                        }
                    }
                }
            }

            (type_name, false, None)
        }
        _ => panic!("Unsupported field type"),
    }
}

/// Derive macro for EntityView
///
/// # Attributes
///
/// ## Container attributes:
/// - `#[entity(ns="namespace")]` - Optional namespace (defaults to snake_case of struct name)
///
/// ## Field attributes:
/// - `#[attr(":custom/ident")]` - Override attribute identifier
/// - `#[entity_id]` - Mark field as entity ID
/// - `#[ref(attr=":x/y")]` - Forward reference
/// - `#[backref(attr=":x/y")]` - Reverse reference
///
/// # Example
///
/// ```ignore
/// #[derive(EntityView)]
/// #[entity(ns="person")]
/// struct PersonView {
///     #[attr(":db/id")]
///     id: i64,
///     name: String,
///     #[backref(attr=":car/owner")]
///     car: Option<CarView>,
/// }
/// ```
#[proc_macro_derive(EntityView, attributes(entity, attr, r#ref, backref, entity_id))]
pub fn derive_entity_view(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let name_str = name.to_string();
    let namespace = extract_namespace_or_default(&input.attrs, &name_str);

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => panic!("EntityView can only be derived for structs with named fields"),
        },
        _ => panic!("EntityView can only be derived for structs"),
    };

    let mut field_specs = Vec::new();

    for field in fields {
        let field_name = field.ident.as_ref().unwrap();
        let field_name_str = field_name.to_string();
        
        let (attr_override, ref_attr, is_ref, is_backref) = parse_view_field_attributes(&field.attrs);
        let (base_type, is_vec, nested_type) = extract_type_info(&field.ty);

        // Determine attribute identifier
        let attr_ident = if let Some(override_attr) = attr_override {
            override_attr
        } else if let Some(ref_attr_val) = ref_attr {
            ref_attr_val
        } else {
            format!(":{}/{}", namespace, to_snake_case(&field_name_str))
        };

        // Determine field kind
        let kind = if is_backref {
            let nested = nested_type.as_deref().unwrap_or(&base_type);
            quote! { mentat_entity::FieldKind::Backref { nested: #nested } }
        } else if is_ref {
            let nested = nested_type.as_deref().unwrap_or(&base_type);
            quote! { mentat_entity::FieldKind::Ref { nested: #nested } }
        } else {
            quote! { mentat_entity::FieldKind::Scalar }
        };

        field_specs.push(quote! {
            mentat_entity::FieldSpec {
                rust_name: #field_name_str,
                attr: #attr_ident,
                kind: #kind,
                cardinality_many: #is_vec,
            }
        });
    }

    let expanded = quote! {
        impl mentat_entity::EntityViewSpec for #name {
            const NS: &'static str = #namespace;
            const FIELDS: &'static [mentat_entity::FieldSpec] = &[
                #(#field_specs),*
            ];
        }
    };

    TokenStream::from(expanded)
}

fn parse_patch_field_attributes(attrs: &[Attribute]) -> (bool, Option<String>) {
    let mut is_entity_id = false;
    let mut attr_override = None;

    for attr in attrs {
        if attr.path.is_ident("entity_id") {
            is_entity_id = true;
        } else if attr.path.is_ident("attr") {
            if let Ok(Meta::List(meta_list)) = attr.parse_meta() {
                for nested in &meta_list.nested {
                    if let NestedMeta::Lit(Lit::Str(s)) = nested {
                        attr_override = Some(s.value());
                    }
                }
            }
        }
    }

    (is_entity_id, attr_override)
}

fn extract_patch_type_info(ty: &Type) -> (bool, bool, String) {
    match ty {
        Type::Path(type_path) => {
            let segment = type_path.path.segments.last().unwrap();
            let type_name = segment.ident.to_string();

            // Check for Patch<T>
            if type_name == "Patch" {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(inner_ty)) = args.args.first() {
                        if let Type::Path(inner_path) = inner_ty {
                            let inner_segment = inner_path.path.segments.last().unwrap();
                            let inner_name = inner_segment.ident.to_string();
                            return (true, false, inner_name);
                        }
                    }
                }
            }

            // Check for ManyPatch<T>
            if type_name == "ManyPatch" {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(inner_ty)) = args.args.first() {
                        if let Type::Path(inner_path) = inner_ty {
                            let inner_segment = inner_path.path.segments.last().unwrap();
                            let inner_name = inner_segment.ident.to_string();
                            return (false, true, inner_name);
                        }
                    }
                }
            }

            (false, false, type_name)
        }
        _ => panic!("Unsupported field type"),
    }
}

/// Derive macro for EntityPatch
///
/// # Attributes
///
/// ## Container attributes:
/// - `#[entity(ns="namespace")]` - Optional namespace (defaults to snake_case of struct name)
///
/// ## Field attributes:
/// - `#[entity_id]` - Required: mark field as entity ID (must be of type EntityId)
/// - `#[attr(":custom/ident")]` - Override attribute identifier
///
/// # Example
///
/// ```ignore
/// #[derive(EntityPatch)]
/// #[entity(ns="order")]
/// struct OrderPatch {
///     #[entity_id]
///     id: EntityId,
///     status: Patch<OrderStatus>,
///     tags: ManyPatch<String>,
/// }
/// ```
#[proc_macro_derive(EntityPatch, attributes(entity, attr, entity_id))]
pub fn derive_entity_patch(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let name_str = name.to_string();
    let namespace = extract_namespace_or_default(&input.attrs, &name_str);

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => panic!("EntityPatch can only be derived for structs with named fields"),
        },
        _ => panic!("EntityPatch can only be derived for structs"),
    };

    // Find entity_id field
    let mut entity_id_field = None;
    let mut patch_fields = Vec::new();

    for field in fields {
        let field_name = field.ident.as_ref().unwrap();
        let (is_entity_id, attr_override) = parse_patch_field_attributes(&field.attrs);

        if is_entity_id {
            entity_id_field = Some(field_name.clone());
        } else {
            let field_name_str = field_name.to_string();
            let (is_patch, is_many_patch, inner_type) = extract_patch_type_info(&field.ty);

            let attr_ident = if let Some(override_attr) = attr_override {
                override_attr
            } else {
                format!(":{}/{}", namespace, to_snake_case(&field_name_str))
            };

            patch_fields.push((field_name.clone(), is_patch, is_many_patch, attr_ident, inner_type));
        }
    }

    let entity_id_field = entity_id_field.expect("EntityPatch requires a field with #[entity_id] attribute");

    // Generate to_tx() implementation
    let mut tx_op_arms = Vec::new();

    for (field_name, is_patch, is_many_patch, attr_ident, inner_type) in patch_fields {
        if is_patch {
            // Handle Patch<T>
            let to_value = generate_value_conversion(&inner_type, quote! { v });
            tx_op_arms.push(quote! {
                match &self.#field_name {
                    mentat_entity::Patch::NoChange => {},
                    mentat_entity::Patch::Set(v) => {
                        let value = #to_value;
                        ops.push(mentat_entity::TxOp::Assert {
                            e: self.#entity_id_field.clone(),
                            a: #attr_ident,
                            v: value,
                        });
                    },
                    mentat_entity::Patch::Unset => {
                        ops.push(mentat_entity::TxOp::RetractAttr {
                            e: self.#entity_id_field.clone(),
                            a: #attr_ident,
                        });
                    },
                }
            });
        } else if is_many_patch {
            // Handle ManyPatch<T>
            let to_value = generate_value_conversion(&inner_type, quote! { v });
            tx_op_arms.push(quote! {
                if self.#field_name.clear {
                    ops.push(mentat_entity::TxOp::RetractAttr {
                        e: self.#entity_id_field.clone(),
                        a: #attr_ident,
                    });
                }
                for v in &self.#field_name.add {
                    let value = #to_value;
                    ops.push(mentat_entity::TxOp::Assert {
                        e: self.#entity_id_field.clone(),
                        a: #attr_ident,
                        v: value,
                    });
                }
                for v in &self.#field_name.remove {
                    let value = #to_value;
                    ops.push(mentat_entity::TxOp::Retract {
                        e: self.#entity_id_field.clone(),
                        a: #attr_ident,
                        v: value,
                    });
                }
            });
        }
    }

    let expanded = quote! {
        impl #name {
            /// Convert this patch to a list of transaction operations
            pub fn to_tx(&self) -> Vec<mentat_entity::TxOp> {
                let mut ops = Vec::new();
                #(#tx_op_arms)*
                ops
            }
        }
    };

    TokenStream::from(expanded)
}

/// Generate code to convert a value to TypedValue
fn generate_value_conversion(
    inner_type: &str,
    value_expr: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    match inner_type {
        "String" => quote! { mentat_entity::core_traits::TypedValue::String(#value_expr.clone().into()) },
        "i64" => quote! { mentat_entity::core_traits::TypedValue::Long(*#value_expr) },
        "f64" => quote! { mentat_entity::core_traits::TypedValue::Double((*#value_expr).into()) },
        "bool" => quote! { mentat_entity::core_traits::TypedValue::Boolean(*#value_expr) },
        "Uuid" => quote! { mentat_entity::core_traits::TypedValue::Uuid(*#value_expr) },
        "EntityId" => quote! {
            match #value_expr {
                mentat_entity::EntityId::Entid(id) => mentat_entity::core_traits::TypedValue::Ref(*id),
                _ => panic!("Only Entid is supported in patches for now"),
            }
        },
        _ => {
            // For custom types, assume they implement Into<TypedValue> or have a to_keyword method
            quote! { (#value_expr).clone().into() }
        }
    }
}
