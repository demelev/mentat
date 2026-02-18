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
use mentat::StructuredMap;
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
// struct TestPerson {
//     email: String,
//     name: String,
//     age: Option<i64>,
// }

#[derive(Debug, EntityDerive)]
#[entity(ns = "person")]
struct TestPersonDerive {
    email: String,
    name: String,
    age: Option<i64>,
}

#[test]
fn test_derived_entity() {
    use mentat::Queryable;
    use mentat_entity::{EntityRead, EntityWrite};

    let testperson = TestPersonDerive {
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
        TestPersonDerive::read(&mut store.begin_read().expect("Begin transaction"), id).unwrap();
    assert_eq!(person.email, "email@gmail.com");
}
