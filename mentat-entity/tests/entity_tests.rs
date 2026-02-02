// Copyright 2024 Mentat Contributors
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use
// this file except in compliance with the License. You may obtain a copy of the
// License at http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software distributed
// under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
// CONDITIONS OF ANY KIND, either express or implied. See the License for the
// specific language governing permissions and limitations under the License.

//! Integration tests for mentat-entity crate

extern crate mentat_entity;
extern crate mentat_core;
extern crate core_traits;
extern crate public_traits;
extern crate failure;

use mentat_entity::{Entity, EntitySchema, FieldDefinition, FieldType, Cardinality, Unique};
use mentat_core::Keyword;
use core_traits::TypedValue;
use std::collections::HashMap;

/// Manual implementation of Entity trait for testing without derive macro
struct TestPerson {
    email: String,
    name: String,
    age: Option<i64>,
}

impl Entity for TestPerson {
    fn schema() -> EntitySchema {
        EntitySchema {
            namespace: "person".to_string(),
            fields: vec![
                FieldDefinition {
                    name: "email".to_string(),
                    ident: Keyword::namespaced("person", "email"),
                    field_type: FieldType::String,
                    cardinality: Cardinality::One,
                    unique: Unique::Identity,
                    indexed: true,
                    optional: false,
                },
                FieldDefinition {
                    name: "name".to_string(),
                    ident: Keyword::namespaced("person", "name"),
                    field_type: FieldType::String,
                    cardinality: Cardinality::One,
                    unique: Unique::None,
                    indexed: false,
                    optional: false,
                },
                FieldDefinition {
                    name: "age".to_string(),
                    ident: Keyword::namespaced("person", "age"),
                    field_type: FieldType::Long,
                    cardinality: Cardinality::One,
                    unique: Unique::None,
                    indexed: false,
                    optional: true,
                },
            ],
        }
    }
    
    fn namespace() -> &'static str {
        "person"
    }
    
    fn to_values(&self) -> HashMap<Keyword, TypedValue> {
        let mut values = HashMap::new();
        values.insert(
            Keyword::namespaced("person", "email"),
            TypedValue::String(self.email.clone().into()),
        );
        values.insert(
            Keyword::namespaced("person", "name"),
            TypedValue::String(self.name.clone().into()),
        );
        if let Some(age) = self.age {
            values.insert(
                Keyword::namespaced("person", "age"),
                TypedValue::Long(age),
            );
        }
        values
    }
    
    fn from_values(mut values: HashMap<Keyword, TypedValue>) -> public_traits::errors::Result<Self> {
        let email_key = Keyword::namespaced("person", "email");
        let name_key = Keyword::namespaced("person", "name");
        let age_key = Keyword::namespaced("person", "age");
        
        let email = values.remove(&email_key)
            .and_then(|v| match v {
                TypedValue::String(s) => Some(s.to_string()),
                _ => None,
            })
            .ok_or_else(|| failure::err_msg("Missing or invalid email"))?;
        
        let name = values.remove(&name_key)
            .and_then(|v| match v {
                TypedValue::String(s) => Some(s.to_string()),
                _ => None,
            })
            .ok_or_else(|| failure::err_msg("Missing or invalid name"))?;
        
        let age = values.remove(&age_key)
            .and_then(|v| match v {
                TypedValue::Long(n) => Some(n),
                _ => None,
            });
        
        Ok(TestPerson { email, name, age })
    }
}

#[test]
fn test_schema_generation() {
    let schema = TestPerson::schema();
    assert_eq!(schema.namespace, "person");
    assert_eq!(schema.fields.len(), 3);
    
    // Check email field
    let email_field = schema.fields.iter().find(|f| f.name == "email").unwrap();
    assert_eq!(email_field.field_type, FieldType::String);
    assert_eq!(email_field.unique, Unique::Identity);
    assert!(email_field.indexed);
    
    // Check age field
    let age_field = schema.fields.iter().find(|f| f.name == "age").unwrap();
    assert_eq!(age_field.field_type, FieldType::Long);
    assert!(age_field.optional);
}

#[test]
fn test_schema_edn_generation() {
    let schema = TestPerson::schema();
    let edn = schema.to_edn();
    
    // Check that the EDN contains expected elements
    assert!(edn.contains(":person/email"));
    assert!(edn.contains(":person/name"));
    assert!(edn.contains(":person/age"));
    assert!(edn.contains(":db.type/string"));
    assert!(edn.contains(":db.type/long"));
    assert!(edn.contains(":db.cardinality/one"));
    assert!(edn.contains(":db.unique/identity"));
}

#[test]
fn test_to_values() {
    let person = TestPerson {
        email: "test@example.com".to_string(),
        name: "Test User".to_string(),
        age: Some(25),
    };
    
    let values = person.to_values();
    assert_eq!(values.len(), 3);
    
    // Check email
    let email = values.get(&Keyword::namespaced("person", "email")).unwrap();
    match email {
        TypedValue::String(s) => assert_eq!(s.as_ref(), "test@example.com"),
        _ => panic!("Expected string value"),
    }
    
    // Check age
    let age = values.get(&Keyword::namespaced("person", "age")).unwrap();
    match age {
        TypedValue::Long(n) => assert_eq!(*n, 25),
        _ => panic!("Expected long value"),
    }
}

#[test]
fn test_to_values_optional_none() {
    let person = TestPerson {
        email: "test@example.com".to_string(),
        name: "Test User".to_string(),
        age: None,
    };
    
    let values = person.to_values();
    // Should only have 2 values (email and name), not age
    assert_eq!(values.len(), 2);
    assert!(!values.contains_key(&Keyword::namespaced("person", "age")));
}

#[test]
fn test_from_values() {
    let mut values = HashMap::new();
    values.insert(
        Keyword::namespaced("person", "email"),
        TypedValue::String("test@example.com".into()),
    );
    values.insert(
        Keyword::namespaced("person", "name"),
        TypedValue::String("Test User".into()),
    );
    values.insert(
        Keyword::namespaced("person", "age"),
        TypedValue::Long(25),
    );
    
    let person = TestPerson::from_values(values).unwrap();
    assert_eq!(person.email, "test@example.com");
    assert_eq!(person.name, "Test User");
    assert_eq!(person.age, Some(25));
}

#[test]
fn test_from_values_optional_missing() {
    let mut values = HashMap::new();
    values.insert(
        Keyword::namespaced("person", "email"),
        TypedValue::String("test@example.com".into()),
    );
    values.insert(
        Keyword::namespaced("person", "name"),
        TypedValue::String("Test User".into()),
    );
    // age is missing
    
    let person = TestPerson::from_values(values).unwrap();
    assert_eq!(person.email, "test@example.com");
    assert_eq!(person.name, "Test User");
    assert_eq!(person.age, None);
}
