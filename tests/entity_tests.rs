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

use core_traits;
use mentat::store::StoreExt;
use mentat_core;
use mentat_entity;
use public_traits;

use core_traits::{TypedValue, ValueType};
use db_traits::errors::{DbError, DbErrorKind};
use mentat::Binding;
use mentat::EntityDerive;
use mentat::entity_builder::BuildTerms;
use mentat::{Binding::Scalar, QueryResults, Queryable, RelResult, kw};
use mentat_core::{HasSchema, Keyword};
use mentat_entity::{
    Cardinality, Entity, EntityRead, EntitySchema, EntityWrite, FieldDefinition, FieldType, Unique,
};
use mentat_transaction::TermBuilder;
use public_traits::errors::{MentatError, Result};
use std::collections::HashMap;
use std::sync::Arc;

/// Manual implementation of Entity trait for testing without derive macro
struct TestPerson {
    email: String,
    name: String,
    age: Option<i64>,
}

#[derive(EntityDerive)]
#[entity(namespace = "person")]
struct TestPersonDerive {
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
            TypedValue::String(Arc::new(self.email.clone())),
        );
        values.insert(
            Keyword::namespaced("person", "name"),
            TypedValue::String(Arc::new(self.name.clone())),
        );
        if let Some(age) = self.age {
            values.insert(Keyword::namespaced("person", "age"), TypedValue::Long(age));
        }
        values
    }

    fn from_values(
        mut values: HashMap<Keyword, TypedValue>,
    ) -> public_traits::errors::Result<Self> {
        let mut builder = TermBuilder::new();
        let entity = builder.named_tempid("e");

        // #(#write_fields)*

        builder.add(
            entity,
            Keyword::namespaced("person", "email"),
            values
                .get(&Keyword::namespaced("person", "email"))
                .cloned()
                .ok_or_else(|| {
                    MentatError::DbError(DbError::from(DbErrorKind::BadValuePair(
                        "Missing email".to_string(),
                        ValueType::String,
                    )))
                })?,
        );

        let (terms, tempids) = builder.build()?;
        let mut store = mentat::store::Store::open("").unwrap();
        let mut in_progress = store.begin_transaction().expect("");
        let report = in_progress.transact_entities(terms)?;

        let email_key = Keyword::namespaced("person", "email");
        let name_key = Keyword::namespaced("person", "name");
        let age_key = Keyword::namespaced("person", "age");

        let email = values
            .remove(&email_key)
            .and_then(|v| match v {
                TypedValue::String(s) => Some(s.to_string()),
                _ => None,
            })
            .ok_or_else(|| {
                MentatError::DbError(DbError::from(DbErrorKind::BadValuePair(
                    "Missing or invalid email".to_string(),
                    ValueType::String,
                )))
            })?;

        let name = values
            .remove(&name_key)
            .and_then(|v| match v {
                TypedValue::String(s) => Some(s.to_string()),
                _ => None,
            })
            .ok_or_else(|| {
                MentatError::DbError(DbError::from(DbErrorKind::BadValuePair(
                    "Missing or invalid name".to_string(),
                    ValueType::String,
                )))
            })?;

        let age = values.remove(&age_key).and_then(|v| match v {
            TypedValue::Long(n) => Some(n),
            _ => None,
        });

        Ok(TestPerson { email, name, age })
    }
}
impl EntityWrite for TestPerson {
    fn write<'a, 'c>(&self, in_progress: &mut mentat::InProgress<'a, 'c>) -> Result<mentat::Entid> {
        let mut builder = TermBuilder::new();
        let entity = builder.named_tempid("e");
        // Add fields
        builder
            .add(
                entity.clone(),
                Keyword::namespaced("person", "email"),
                TypedValue::String(Arc::new(self.email.clone())),
            )
            .expect("Add email");
        builder
            .add(
                entity.clone(),
                Keyword::namespaced("person", "name"),
                TypedValue::String(Arc::new(self.name.clone())),
            )
            .expect("Add name");
        if let Some(age) = self.age {
            builder
                .add(
                    entity,
                    Keyword::namespaced("person", "age"),
                    TypedValue::Long(age),
                )
                .expect("Add age");
        }

        let (terms, tempids) = builder.build()?;
        let report = dbg!(in_progress.transact_entities(terms)?);

        // Resolve the entid for the tempid
        report.tempids.get("e").cloned().ok_or_else(|| {
            MentatError::DbError(DbError::from(DbErrorKind::NotYetImplemented(
                "Failed to resolve entid".to_string(),
            )))
        })
    }

    fn write_with_entid<'a, 'c>(
        &self,
        in_progress: &mut mentat::InProgress<'a, 'c>,
        entid: mentat::Entid,
    ) -> Result<mentat::Entid> {
        todo!()
    }
}

impl EntityRead for TestPerson {
    fn read<'a, 'c>(
        in_progress: &mentat::InProgress<'a, 'c>,
        entid: mentat::Entid,
    ) -> Result<Self> {
        let inputs = {
            let mut values = std::collections::BTreeMap::new();
            values.insert(
                mentat::query::Variable::from_valid_name("?e"),
                TypedValue::Ref(entid),
            );
            mentat::QueryInputs::with_values(values)
        };

        let res = in_progress
            .q_once("[:find (pull ?e [*]) :in ?e :where [?e _ _]]", inputs)
            .unwrap();

        match dbg!(res.results) {
            QueryResults::Rel(rel) => {
                for row in rel.values {
                    if let Binding::Map(map) = row {
                        let email = if let Some(Scalar(TypedValue::String(s))) =
                            map.get(&kw!(:person/email))
                        {
                            s.to_string()
                        } else {
                            String::new()
                        };
                        let name = if let Some(Scalar(TypedValue::String(s))) =
                            map.get(&kw!(:person/name))
                        {
                            s.to_string()
                        } else {
                            String::new()
                        };
                        let age = if let Some(Scalar(TypedValue::Long(n))) =
                            map.get(&Keyword::namespaced("person", "age"))
                        {
                            Some(*n)
                        } else {
                            None
                        };
                        return Ok(Self { email, name, age });
                    }
                }
            }
            // Ok(rel) => for row in rel.values {},
            _ => todo!(),
        }

        Ok(Self {
            email: String::new(),
            name: String::new(),
            age: None,
        })
    }

    fn read_by_unique<'a, 'c>(
        in_progress: &mentat::InProgress<'a, 'c>,
        attribute: &Keyword,
        value: TypedValue,
    ) -> Result<Self> {
        todo!()
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
fn test_derived_entity() {
    use mentat::Queryable;
    use mentat_entity::{EntityRead, EntityWrite};

    let testperson = TestPerson {
        email: String::from("email@gmail.com"),
        name: String::from("Derived User"),
        age: Some(30),
    };
    let person = TestPersonDerive {
        email: String::from("email@gmail.com"),
        name: String::from("Derived User"),
        age: Some(30),
    };
    let mut store = mentat::store::Store::open("").unwrap();
    store.ensure_entity_schema::<TestPersonDerive>().unwrap();

    // Verify schema was installed
    let schema = store.conn().current_schema();
    assert!(schema.identifies_attribute(&Keyword::namespaced("person", "email")));
    assert!(schema.identifies_attribute(&Keyword::namespaced("person", "name")));
    assert!(schema.identifies_attribute(&Keyword::namespaced("person", "age")));

    let mut builder = store.begin_transaction().expect("Begin transaction");
    // Write succeeds with the schema installed
    let id = person.write(&mut builder).unwrap();

    builder.commit().unwrap();

    // TODO: The read implementation has bugs that need to be fixed in the derive macro
    // For now, just verify that we can write successfully after ensuring schema
    assert!(id > 0);

    // dbg!(store.transact("[[:db/add \"s\" :person/name \"Dima\"]]"));
    dbg!(store.q_once("[:find ?v :where [_ :person/name ?v]]", None));
    let person =
        TestPerson::read(&store.begin_transaction().expect("Begin transaction"), id).unwrap();
    assert_eq!(person.email, "email@gmail.com");
}
