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

extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, DeriveInput, Data, Fields, Meta, NestedMeta, Lit,
    MetaNameValue, Attribute, Type, PathArguments, GenericArgument,
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
        let field_name_str = field_name.to_string();
        let field_attrs = parse_field_attributes(&field.attrs);
        
        let (field_type, is_optional) = extract_field_type(&field.ty);
        let field_type_enum = rust_type_to_field_type(&field_type);
        
        let ident_name = format!("{}:{}", namespace, field_name_str);
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
                ident: mentat_core::Keyword::namespaced(#namespace, #field_name_str),
                field_type: #field_type_enum,
                cardinality: #cardinality,
                unique: #unique_variant,
                indexed: #indexed,
                optional: #is_optional,
            }
        });
        
        // to_values implementation
        let keyword = quote! { mentat_core::Keyword::namespaced(#namespace, #field_name_str) };
        
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
                #field_name: values.remove(&#keyword).map(|value| #from_typed_value).transpose()?
            });
        } else {
            from_values_fields.push(quote! {
                #field_name: {
                    let value = values.remove(&#keyword)
                        .ok_or_else(|| failure::err_msg(format!("Missing required field: {}", #field_name_str)))?;
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
            
            fn to_values(&self) -> std::collections::HashMap<mentat_core::Keyword, core_traits::TypedValue> {
                let mut values = std::collections::HashMap::new();
                #(#to_values_fields)*
                values
            }
            
            fn from_values(mut values: std::collections::HashMap<mentat_core::Keyword, core_traits::TypedValue>) 
                -> public_traits::errors::Result<Self> 
            {
                Ok(#name {
                    #(#from_values_fields),*
                })
            }
        }
        
        impl mentat_entity::EntityWrite for #name {
            fn write<'a, 'c>(&self, in_progress: &mut mentat_transaction::InProgress<'a, 'c>) 
                -> public_traits::errors::Result<core_traits::Entid> 
            {
                use mentat_transaction::entity_builder::{BuildTerms, TermBuilder};
                
                let mut builder = TermBuilder::new();
                let entity = builder.named_tempid("e");
                
                #(#write_fields)*
                
                let (terms, tempids) = builder.build()?;
                let report = in_progress.transact_entities(terms, tempids)?;
                
                // Get the entid for our temp entity
                let entid = report.tempids.get("e")
                    .ok_or_else(|| failure::err_msg("Failed to get entity ID from transaction"))?;
                
                Ok(*entid)
            }
            
            fn write_with_entid<'a, 'c>(&self, in_progress: &mut mentat_transaction::InProgress<'a, 'c>, entid: core_traits::Entid) 
                -> public_traits::errors::Result<core_traits::Entid> 
            {
                use mentat_transaction::entity_builder::{BuildTerms, TermBuilder};
                
                let mut builder = TermBuilder::new();
                let entity = entid;
                
                #(#write_fields)*
                
                let (terms, tempids) = builder.build()?;
                in_progress.transact_entities(terms, tempids)?;
                
                Ok(entid)
            }
        }
        
        impl mentat_entity::EntityRead for #name {
            fn read<'a, 'c>(in_progress: &mentat_transaction::InProgress<'a, 'c>, entid: core_traits::Entid) 
                -> public_traits::errors::Result<Self> 
            {
                // Get the schema and read all attributes
                let schema = Self::schema();
                let values = mentat_entity::read_entity_attributes(in_progress, entid, &schema)?;
                
                // Convert to entity using from_values
                Self::from_values(values)
            }
            
            fn read_by_unique<'a, 'c>(
                in_progress: &mentat_transaction::InProgress<'a, 'c>,
                attribute: &mentat_core::Keyword,
                value: core_traits::TypedValue,
            ) -> public_traits::errors::Result<Self> {
                // Find entity by unique attribute
                let entid = mentat_entity::find_entity_by_unique(in_progress, attribute, value)?
                    .ok_or_else(|| failure::err_msg("Entity not found"))?;
                
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
                    if let NestedMeta::Meta(Meta::NameValue(MetaNameValue { path, lit: Lit::Str(s), .. })) = nested {
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
                        NestedMeta::Meta(Meta::NameValue(MetaNameValue { path, lit: Lit::Str(s), .. })) => {
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

fn generate_to_typed_value(field_type: &str, value_expr: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    match field_type {
        "String" => quote! { core_traits::TypedValue::String(#value_expr.clone().into()) },
        "i64" => quote! { core_traits::TypedValue::Long(#value_expr) },
        "f64" => quote! { core_traits::TypedValue::Double((#value_expr).into()) },
        "bool" => quote! { core_traits::TypedValue::Boolean(#value_expr) },
        "DateTime" => quote! { core_traits::TypedValue::Instant(#value_expr) },
        "Uuid" => quote! { core_traits::TypedValue::Uuid(#value_expr) },
        "Keyword" => quote! { core_traits::TypedValue::Keyword(#value_expr.into()) },
        "Entid" => quote! { core_traits::TypedValue::Ref(#value_expr) },
        _ => panic!("Unsupported type conversion: {}", field_type),
    }
}

fn generate_from_typed_value(field_type: &str, value_expr: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    match field_type {
        "String" => quote! {
            if let core_traits::TypedValue::String(s) = #value_expr {
                Ok(s.to_string())
            } else {
                Err(failure::err_msg("Expected String value"))
            }
        },
        "i64" => quote! {
            if let core_traits::TypedValue::Long(n) = #value_expr {
                Ok(n)
            } else {
                Err(failure::err_msg("Expected Long value"))
            }
        },
        "f64" => quote! {
            if let core_traits::TypedValue::Double(d) = #value_expr {
                Ok(d.into_inner())
            } else {
                Err(failure::err_msg("Expected Double value"))
            }
        },
        "bool" => quote! {
            if let core_traits::TypedValue::Boolean(b) = #value_expr {
                Ok(b)
            } else {
                Err(failure::err_msg("Expected Boolean value"))
            }
        },
        "DateTime" => quote! {
            if let core_traits::TypedValue::Instant(dt) = #value_expr {
                Ok(dt)
            } else {
                Err(failure::err_msg("Expected Instant value"))
            }
        },
        "Uuid" => quote! {
            if let core_traits::TypedValue::Uuid(u) = #value_expr {
                Ok(u)
            } else {
                Err(failure::err_msg("Expected Uuid value"))
            }
        },
        "Keyword" => quote! {
            if let core_traits::TypedValue::Keyword(k) = #value_expr {
                Ok(k.into())
            } else {
                Err(failure::err_msg("Expected Keyword value"))
            }
        },
        "Entid" => quote! {
            if let core_traits::TypedValue::Ref(e) = #value_expr {
                Ok(e)
            } else {
                Err(failure::err_msg("Expected Ref value"))
            }
        },
        _ => panic!("Unsupported type conversion: {}", field_type),
    }
}
