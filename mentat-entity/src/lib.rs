// Copyright 2024 Mentat Contributors
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use
// this file except in compliance with the License. You may obtain a copy of the
// License at http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software distributed
// under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
// CONDITIONS OF ANY KIND, either express or implied. See the License for the
// specific language governing permissions and limitations under the License.

//! # Mentat Entity
//!
//! This crate provides ORM-like functionality for Mentat, allowing Rust structs
//! to be directly serialized to and deserialized from the Mentat database without
//! JSON round-trips.
//!
//! ## Usage
//!
//! ```ignore
//! use mentat_entity::{Entity, EntityRead, EntityWrite};
//! use mentat::{TypedValue, Keyword};
//!
//! #[derive(Entity)]
//! #[entity(namespace = "person")]
//! struct Person {
//!     #[entity(unique = "identity")]
//!     email: String,
//!     name: String,
//!     age: Option<i64>,
//! }
//!
//! // The derive macro generates:
//! // - Schema definition (Person::schema())
//! // - Write method (person.write(&mut in_progress))
//! // - Read method (Person::read(&in_progress, entid))
//! // - Query methods (Person::find_by_email(&in_progress, "email@example.com"))
//! ```

extern crate core_traits;
extern crate mentat_core;
extern crate mentat_db;
extern crate db_traits;
extern crate mentat_transaction;
extern crate public_traits;
extern crate edn;
extern crate failure;
extern crate chrono;
extern crate uuid;

pub use mentat_entity_derive::Entity;

use std::collections::HashMap;

use core_traits::{
    Entid,
    TypedValue,
    ValueType,
};

use mentat_core::{
    Keyword,
    TxReport,
    Schema,
};

use mentat_transaction::{
    InProgress,
};

use public_traits::errors::{
    Result,
};

/// Represents the type of an entity field in the schema
#[derive(Debug, Clone, PartialEq)]
pub enum FieldType {
    Ref,
    Boolean,
    Instant,
    Long,
    Double,
    String,
    Keyword,
    Uuid,
}

impl FieldType {
    pub fn to_value_type(&self) -> ValueType {
        match self {
            FieldType::Ref => ValueType::Ref,
            FieldType::Boolean => ValueType::Boolean,
            FieldType::Instant => ValueType::Instant,
            FieldType::Long => ValueType::Long,
            FieldType::Double => ValueType::Double,
            FieldType::String => ValueType::String,
            FieldType::Keyword => ValueType::Keyword,
            FieldType::Uuid => ValueType::Uuid,
        }
    }
}

/// Cardinality of a field
#[derive(Debug, Clone, PartialEq)]
pub enum Cardinality {
    One,
    Many,
}

/// Uniqueness constraint for a field
#[derive(Debug, Clone, PartialEq)]
pub enum Unique {
    None,
    Value,
    Identity,
}

/// Metadata for a single field in an entity
#[derive(Debug, Clone)]
pub struct FieldDefinition {
    pub name: String,
    pub ident: Keyword,
    pub field_type: FieldType,
    pub cardinality: Cardinality,
    pub unique: Unique,
    pub indexed: bool,
    pub optional: bool,
}

/// Schema definition for an entity type
#[derive(Debug, Clone)]
pub struct EntitySchema {
    pub namespace: String,
    pub fields: Vec<FieldDefinition>,
}

impl EntitySchema {
    /// Generate EDN schema string for transacting into the database
    pub fn to_edn(&self) -> String {
        let mut edn_parts = Vec::new();
        
        for field in &self.fields {
            let ident = &field.ident;
            let value_type = match field.field_type {
                FieldType::Ref => ":db.type/ref",
                FieldType::Boolean => ":db.type/boolean",
                FieldType::Instant => ":db.type/instant",
                FieldType::Long => ":db.type/long",
                FieldType::Double => ":db.type/double",
                FieldType::String => ":db.type/string",
                FieldType::Keyword => ":db.type/keyword",
                FieldType::Uuid => ":db.type/uuid",
            };
            
            let cardinality = match field.cardinality {
                Cardinality::One => ":db.cardinality/one",
                Cardinality::Many => ":db.cardinality/many",
            };
            
            // Basic attribute definition
            let mut attr_def = format!(
                "[[:db/add \"{}\" :db/ident :{}]\n \
                 [:db/add \"{}\" :db/valueType {}]\n \
                 [:db/add \"{}\" :db/cardinality {}]",
                field.name, ident,
                field.name, value_type,
                field.name, cardinality
            );
            
            // Add uniqueness constraint if specified
            if field.unique != Unique::None {
                let unique_val = match field.unique {
                    Unique::Value => ":db.unique/value",
                    Unique::Identity => ":db.unique/identity",
                    Unique::None => unreachable!(),
                };
                attr_def.push_str(&format!("\n [:db/add \"{}\" :db/unique {}]", field.name, unique_val));
            }
            
            // Add index if specified
            if field.indexed {
                attr_def.push_str(&format!("\n [:db/add \"{}\" :db/index true]", field.name));
            }
            
            attr_def.push(']');
            edn_parts.push(attr_def);
        }
        
        format!("[{}]", edn_parts.join("\n "))
    }
}

/// Core trait for entities that can be persisted to Mentat
pub trait Entity: Sized {
    /// Get the schema definition for this entity type
    fn schema() -> EntitySchema;
    
    /// Get the namespace used for this entity's attributes
    fn namespace() -> &'static str;
    
    /// Convert this entity to a map of attribute keywords to typed values
    fn to_values(&self) -> HashMap<Keyword, TypedValue>;
    
    /// Create an entity instance from a map of typed values
    fn from_values(values: HashMap<Keyword, TypedValue>) -> Result<Self>;
}

/// Trait for writing entities to the database
pub trait EntityWrite: Entity {
    /// Write this entity to the database using the provided InProgress transaction
    /// Returns the Entid of the created/updated entity
    fn write<'a, 'c>(&self, in_progress: &mut InProgress<'a, 'c>) -> Result<Entid>;
    
    /// Write this entity with a specific entid (for updates)
    fn write_with_entid<'a, 'c>(&self, in_progress: &mut InProgress<'a, 'c>, entid: Entid) -> Result<Entid>;
}

/// Trait for reading entities from the database
pub trait EntityRead: Entity {
    /// Read an entity by its entid
    fn read<'a, 'c>(in_progress: &InProgress<'a, 'c>, entid: Entid) -> Result<Self>;
    
    /// Read an entity by a unique attribute value
    fn read_by_unique<'a, 'c>(
        in_progress: &InProgress<'a, 'c>,
        attribute: &Keyword,
        value: TypedValue,
    ) -> Result<Self>;
}

/// Helper function to transact schema for an entity type
pub fn transact_schema<T: Entity, 'a, 'c>(
    in_progress: &mut InProgress<'a, 'c>,
) -> Result<TxReport> {
    let schema = T::schema();
    let edn = schema.to_edn();
    in_progress.transact(&edn)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_field_type_conversion() {
        assert_eq!(FieldType::String.to_value_type(), ValueType::String);
        assert_eq!(FieldType::Long.to_value_type(), ValueType::Long);
        assert_eq!(FieldType::Boolean.to_value_type(), ValueType::Boolean);
    }
    
    #[test]
    fn test_schema_edn_generation() {
        let schema = EntitySchema {
            namespace: "test".to_string(),
            fields: vec![
                FieldDefinition {
                    name: "name_attr".to_string(),
                    ident: Keyword::namespaced("test", "name"),
                    field_type: FieldType::String,
                    cardinality: Cardinality::One,
                    unique: Unique::None,
                    indexed: false,
                    optional: false,
                },
            ],
        };
        
        let edn = schema.to_edn();
        assert!(edn.contains(":test/name"));
        assert!(edn.contains(":db.type/string"));
        assert!(edn.contains(":db.cardinality/one"));
    }
}
