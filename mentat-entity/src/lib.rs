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
//! use mentat_entity::{Entity, EntityRead, EntityWrite, EnsureEntitySchema};
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
//!
//! // Ensure schema is installed (useful for tests with in-memory databases):
//! // Implement EnsureEntitySchema for your Store type, then:
//! // store.ensure_entity_schema::<Person>()?;
//! ```

mod read;

pub extern crate mentat_entity_derive;
pub use core_traits;
use core_traits::{Entid, StructuredMap, TypedValue, ValueType};
pub use mentat_core;
use mentat_core::{Keyword, TxReport};
pub use mentat_entity_derive::{Entity, EntityPatch, EntityView};
pub use mentat_transaction;
use mentat_transaction::{InProgress, InProgressRead};
pub use public_traits;
pub use public_traits::errors::MentatError;
use public_traits::errors::Result;
pub use read::{find_entity_by_unique, read_entity_attributes};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum MentatEntityError {
    #[error("Missing required field: {0}")]
    MissingRequiredField(String),
    #[error("Backref cardinality violation: expected one, found multiple")]
    BackrefCardinalityViolation,
    #[error("Missing entity: {0}")]
    MissingEntity(String),
    #[error("Type mismatch: {0}")]
    TypeMismatch(String),
    #[error("Missing attribute: {0}")]
    MissingAttributeInDatabase(&'static Keyword),
}

// ============================================================================
// EntityView/EntityPatch Types (from tech spec)
// ============================================================================

/// Universal entity identifier
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum EntityId {
    /// Direct entity ID
    Entid(i64),
    /// Lookup reference: find entity by unique attribute
    LookupRef {
        attr: &'static str,
        value: TypedValue,
    },
    /// Temporary ID for transaction (will be resolved to actual Entid)
    Temp(i64),
}

impl From<i64> for EntityId {
    fn from(id: i64) -> Self {
        EntityId::Entid(id)
    }
}

/// Transaction operation
#[derive(Clone, Debug, PartialEq)]
pub enum TxOp {
    /// Assert a fact (add or update)
    Assert {
        e: EntityId,
        a: &'static str,
        v: TypedValue,
    },
    /// Retract a specific fact
    Retract {
        e: EntityId,
        a: &'static str,
        v: TypedValue,
    },
    /// Retract all values for an attribute
    RetractAttr { e: EntityId, a: &'static str },
    /// Ensure predicate (optimistic concurrency check)
    Ensure {
        e: EntityId,
        a: &'static str,
        v: TypedValue,
    },
}

/// Patch for a single-valued (cardinality-one) field
#[derive(Clone, Debug, PartialEq)]
pub enum Patch<T> {
    /// No change to this field
    NoChange,
    /// Set the field to a new value
    Set(T),
    /// Unset (retract) the field
    Unset,
    /// Set with ensure predicate (optimistic concurrency)
    /// Ensures current value matches expected before setting new value
    SetWithEnsure {
        /// Expected current value
        expected: T,
        /// New value to set
        new: T,
    },
}

impl<T> Default for Patch<T> {
    fn default() -> Self {
        Patch::NoChange
    }
}

/// Patch for a multi-valued (cardinality-many) field
#[derive(Clone, Debug, PartialEq)]
pub struct ManyPatch<T> {
    /// Values to add
    pub add: Vec<T>,
    /// Values to remove
    pub remove: Vec<T>,
    /// Clear all existing values before adding
    pub clear: bool,
}

impl<T> Default for ManyPatch<T> {
    fn default() -> Self {
        Self {
            add: Vec::new(),
            remove: Vec::new(),
            clear: false,
        }
    }
}

impl<T> ManyPatch<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_add(mut self, values: Vec<T>) -> Self {
        self.add = values;
        self
    }

    pub fn with_remove(mut self, values: Vec<T>) -> Self {
        self.remove = values;
        self
    }

    pub fn with_clear(mut self, clear: bool) -> Self {
        self.clear = clear;
        self
    }
}

/// Kind of field in an entity view
#[derive(Clone, Debug, PartialEq)]
pub enum FieldKind {
    /// Scalar value (string, long, etc.)
    Scalar,
    /// Reference to another entity
    Ref {
        /// Type name of the nested view
        nested: &'static str,
    },
    /// Reverse reference (backref)
    Backref {
        /// Type name of the nested view
        nested: &'static str,
    },
}

/// Specification for a field in an entity view
#[derive(Clone, Debug)]
pub struct FieldSpec {
    /// Rust field name
    pub rust_name: &'static str,
    /// Attribute identifier (e.g., ":person/name" or forward ident for backref)
    pub attr: &'static str,
    /// Kind of field
    pub kind: FieldKind,
    /// Whether this field has many cardinality
    pub cardinality_many: bool,
    /// Optional profiles this field belongs to (None = all profiles)
    pub profiles: Option<&'static [&'static str]>,
    /// Whether this field is a component (for cascade operations)
    pub is_component: bool,
}

/// Trait for entity view metadata
pub trait EntityViewSpec {
    /// Namespace for this entity type
    const NS: &'static str;
    /// Field specifications
    const FIELDS: &'static [FieldSpec];
    /// Available view profiles
    const PROFILES: &'static [&'static str] = &[];

    /// Get fields for a specific profile
    fn fields_for_profile(profile: &str) -> Vec<&'static FieldSpec> {
        Self::FIELDS
            .iter()
            .filter(|f| {
                f.profiles
                    .map_or(true, |profiles| profiles.contains(&profile))
            })
            .collect()
    }

    /// Generate EDN pull pattern for this view
    ///
    /// # Arguments
    /// * `depth` - Maximum depth for nested views (0 = scalars only, 1+ = include refs/backrefs)
    /// * `profile` - Optional profile name to filter fields
    fn pull_pattern(depth: usize, profile: Option<&str>) -> String {
        Self::pull_pattern_impl(depth, profile, &mut std::collections::HashSet::new())
    }

    /// Internal implementation with cycle detection
    fn pull_pattern_impl(
        depth: usize,
        profile: Option<&str>,
        visited: &mut std::collections::HashSet<&'static str>,
    ) -> String {
        // Prevent infinite recursion
        let type_name = std::any::type_name::<Self>();
        if visited.contains(&type_name) {
            return "...".to_string();
        }
        visited.insert(type_name);

        let fields = if let Some(prof) = profile {
            Self::fields_for_profile(prof)
        } else {
            Self::FIELDS.iter().collect()
        };

        let mut attrs = vec!["*".to_string()]; // Start with :db/id

        for field in fields {
            match &field.kind {
                FieldKind::Scalar => {
                    attrs.push(field.attr.replace("#", "").to_string());
                }
                FieldKind::Ref { nested: _ } | FieldKind::Backref { nested: _ } => {
                    if depth > 0 {
                        // For refs/backrefs, we'd need to recursively call pull_pattern
                        // on the nested type, but we can't do that statically without
                        // trait bounds. For now, just include the attribute.
                        attrs.push(field.attr.to_string());
                    }
                }
            }
        }

        visited.remove(&type_name);
        format!("[{}]", attrs.join(" "))
    }
}

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

            // Build attribute map
            let mut attr_map = format!(
                "{{:db/ident {} :db/valueType {} :db/cardinality {}",
                ident, value_type, cardinality
            );

            // Add uniqueness constraint if specified
            match field.unique {
                Unique::Value => {
                    attr_map.push_str(" :db/unique :db.unique/value");
                }
                Unique::Identity => {
                    attr_map.push_str(" :db/unique :db.unique/identity");
                }
                Unique::None => {
                    // No uniqueness constraint
                }
            }

            // Add index if specified
            if field.indexed {
                attr_map.push_str(" :db/index true");
            }

            attr_map.push('}');
            edn_parts.push(attr_map);
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
    fn to_values(&self) -> StructuredMap;

    /// Create an entity instance from a map of typed values
    fn from_values(values: StructuredMap) -> Result<Self>;
}

/// Trait for writing entities to the database
pub trait EntityWrite: Entity {
    /// Write this entity to the database using the provided InProgress transaction
    /// Returns the Entid of the created/updated entity
    fn write<'a, 'c>(&self, in_progress: &mut InProgress<'a, 'c>) -> Result<Entid>;

    /// Write this entity with a specific entid (for updates)
    fn write_with_entid<'a, 'c>(
        &self,
        in_progress: &mut InProgress<'a, 'c>,
        entid: Entid,
    ) -> Result<Entid>;
}

/// Trait for reading entities from the database
pub trait EntityRead: Entity {
    /// Read an entity by its entid
    fn read<'a, 'c>(in_progress: &mut InProgressRead<'a, 'c>, entid: Entid) -> Result<Self>;

    /// Read an entity by a unique attribute value
    fn read_by_unique<'a, 'c>(
        in_progress: &mut InProgressRead<'a, 'c>,
        attribute: &Keyword,
        value: TypedValue,
    ) -> Result<Self>;
}

/// Helper function to transact schema for an entity type
pub fn transact_schema<'a, 'c, T: Entity>(
    in_progress: &mut InProgress<'a, 'c>,
) -> Result<TxReport> {
    let schema = T::schema();
    let edn = schema.to_edn();
    in_progress.transact(edn.as_str())
}

/// Extension trait to add `ensure_entity_schema` to Store-like types
///
/// This trait provides a convenient way to ensure that an entity's schema
/// is installed in the database. It checks if the attributes exist and
/// transacts them if needed.
///
/// # Example
///
/// Implement this trait for your Store type in tests or application code:
///
/// ```ignore
/// use mentat_entity::{Entity, EnsureEntitySchema};
/// use mentat::store::Store;
/// use mentat_core::HasSchema;
/// use public_traits::errors::Result;
///
/// impl EnsureEntitySchema for Store {
///     fn ensure_entity_schema<T: Entity>(&mut self) -> Result<()> {
///         let entity_schema = T::schema();
///         let current_schema = self.conn().current_schema();
///         
///         let all_exist = entity_schema.fields.iter().all(|field| {
///             current_schema.identifies_attribute(&field.ident)
///         });
///         
///         if !all_exist {
///             let mut in_progress = self.begin_transaction()?;
///             mentat_entity::transact_schema::<T>(&mut in_progress)?;
///             in_progress.commit()?;
///         }
///         
///         Ok(())
///     }
/// }
///
/// // Then use it:
/// let mut store = Store::open("").unwrap();
/// store.ensure_entity_schema::<Person>().unwrap();
/// ```
pub trait EnsureEntitySchema {
    fn ensure_entity_schema<T: Entity>(&mut self) -> Result<()>;
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
            fields: vec![FieldDefinition {
                name: "name_attr".to_string(),
                ident: Keyword::namespaced("test", "name"),
                field_type: FieldType::String,
                cardinality: Cardinality::One,
                unique: Unique::None,
                indexed: false,
                optional: false,
            }],
        };

        let edn = schema.to_edn();
        assert!(edn.contains(":test/name"));
        assert!(edn.contains(":db.type/string"));
        assert!(edn.contains(":db.cardinality/one"));
    }
}
