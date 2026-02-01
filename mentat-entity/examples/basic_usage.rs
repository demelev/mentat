// Copyright 2024 Mentat Contributors
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use
// this file except in compliance with the License. You may obtain a copy of the
// License at http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software distributed
// under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
// CONDITIONS OF ANY KIND, either express or implied. See the License for the
// specific language governing permissions and limitations under the License.

//! Example demonstrating the use of the Entity trait and derive macro

// Note: This example is for demonstration purposes and won't compile
// until the full mentat dependency chain is properly set up

#[cfg(feature = "entity_examples")]
mod example {
    use mentat_entity::{Entity, EntityWrite, EntityRead, transact_schema};
    use mentat::{Store, TypedValue, Keyword};
    use chrono::{DateTime, Utc};
    use uuid::Uuid;

    #[derive(Entity)]
    #[entity(namespace = "person")]
    struct Person {
        #[entity(unique = "identity")]
        email: String,
        name: String,
        age: Option<i64>,
    }

    #[derive(Entity)]
    #[entity(namespace = "company")]
    struct Company {
        #[entity(unique = "identity")]
        name: String,
        #[entity(indexed)]
        industry: String,
        employees: Option<i64>,
        founded: Option<DateTime<Utc>>,
    }

    pub fn run_example() {
        // Create a store
        let mut store = Store::open("").expect("Failed to create store");
        
        // Transact the schema for Person
        {
            let mut in_progress = store.begin_transaction().expect("Failed to begin transaction");
            transact_schema::<Person, _, _>(&mut in_progress).expect("Failed to transact schema");
            in_progress.commit().expect("Failed to commit");
        }
        
        // Create and write a person
        let person = Person {
            email: "alice@example.com".to_string(),
            name: "Alice".to_string(),
            age: Some(30),
        };
        
        let person_id = {
            let mut in_progress = store.begin_transaction().expect("Failed to begin transaction");
            let id = person.write(&mut in_progress).expect("Failed to write person");
            in_progress.commit().expect("Failed to commit");
            id
        };
        
        println!("Created person with ID: {}", person_id);
        
        // Read the person back
        {
            let in_progress = store.begin_read().expect("Failed to begin read");
            let person_read = Person::read(&in_progress, person_id).expect("Failed to read person");
            println!("Read person: {} ({})", person_read.name, person_read.email);
        }
    }
}

fn main() {
    println!("This is a demonstration example.");
    println!("To run this, enable the 'entity_examples' feature.");
    println!("The Entity derive macro allows you to:");
    println!("  1. Define structs with #[derive(Entity)]");
    println!("  2. Automatically generate schema definitions");
    println!("  3. Write structs directly to the database");
    println!("  4. Read structs back from the database");
    println!("  5. Support optional fields with Option<T>");
}
