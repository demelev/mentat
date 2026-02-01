# Example: Deserializing Query Results to Custom Structs

This example demonstrates how to deserialize Mentat query results directly into custom domain types.

## Basic Example

```rust
extern crate mentat;
extern crate core_traits;
extern crate mentat_query_projector;
extern crate serde;
#[macro_use]
extern crate serde_derive;

use mentat::{Store, Queryable};
use mentat_query_projector::deserialize::rel_result_to_structs;
use core_traits::{Binding, TypedValue};

// Define your domain struct
#[derive(Debug, Deserialize, PartialEq)]
struct Person {
    entity_id: i64,
    name: String,
    age: i64,
}

fn main() {
    // Create a store and schema
    let mut store = Store::open("").expect("Failed to open store");
    
    // Define schema (simplified for example)
    store.transact(r#"
        [
            {:db/ident :person/name
             :db/valueType :db.type/string
             :db/cardinality :db.cardinality/one}
            {:db/ident :person/age
             :db/valueType :db.type/long
             :db/cardinality :db.cardinality/one}
        ]
    "#).expect("Failed to define schema");
    
    // Add some data
    store.transact(r#"
        [
            {:person/name "Alice" :person/age 30}
            {:person/name "Bob" :person/age 25}
            {:person/name "Charlie" :person/age 35}
        ]
    "#).expect("Failed to add data");
    
    // Query for people
    let query = r#"
        [:find ?e ?name ?age
         :where
         [?e :person/name ?name]
         [?e :person/age ?age]]
    "#;
    
    let results = store.q_once(query, None)
        .expect("Query failed");
    
    // Convert QueryResults to RelResult
    if let mentat_query_projector::QueryResults::Rel(rel_result) = results {
        // Deserialize into Vec<Person>
        let people: Vec<Person> = rel_result_to_structs(rel_result)
            .expect("Failed to deserialize");
        
        println!("Found {} people:", people.len());
        for person in people {
            println!("  {} (age: {}, entity: {})", 
                     person.name, person.age, person.entity_id);
        }
    }
}
```

## Handling Different Query Result Types

```rust
use mentat_query_projector::{QueryResults, deserialize::rel_result_to_structs};
use core_traits::{Binding, TypedValue};

#[derive(Debug, Deserialize)]
struct SimpleResult {
    value: i64,
}

fn handle_query_results(results: QueryResults) {
    match results {
        QueryResults::Scalar(Some(binding)) => {
            println!("Got a single value: {:?}", binding);
        },
        QueryResults::Coll(bindings) => {
            println!("Got a collection of {} values", bindings.len());
        },
        QueryResults::Rel(rel_result) => {
            // This is where we can use our deserialization helper
            match rel_result_to_structs::<SimpleResult>(rel_result) {
                Ok(results) => {
                    println!("Deserialized {} rows", results.len());
                    for r in results {
                        println!("  Value: {}", r.value);
                    }
                },
                Err(e) => {
                    eprintln!("Deserialization failed: {}", e);
                }
            }
        },
        _ => println!("Other result type"),
    }
}
```

## Complex Struct Example

```rust
use serde::Deserialize;
use mentat_query_projector::deserialize::row_to_struct;
use core_traits::{Binding, TypedValue};

#[derive(Debug, Deserialize)]
struct Order {
    order_id: i64,
    customer_name: String,
    total_amount: f64,
    is_paid: bool,
    item_count: i64,
}

fn process_order_row(row: Vec<Binding>) {
    match row_to_struct::<Order>(row) {
        Ok(order) => {
            println!("Order #{}: {} items, ${:.2} (paid: {})",
                     order.order_id,
                     order.item_count,
                     order.total_amount,
                     order.is_paid);
            
            if !order.is_paid && order.total_amount > 1000.0 {
                println!("  ⚠️  High-value unpaid order!");
            }
        },
        Err(e) => {
            eprintln!("Failed to parse order: {}", e);
        }
    }
}
```

## Nested Data Structures

For queries that return nested data (using pull expressions), you can use nested structs:

```rust
#[derive(Debug, Deserialize)]
struct Address {
    street: String,
    city: String,
    zip: String,
}

#[derive(Debug, Deserialize)]
struct PersonWithAddress {
    id: i64,
    name: String,
    // This would work if the query returns a structured map
    // address: Address,
}
```

Note: Nested structures work best when your query returns StructuredMap bindings from pull expressions.

## Best Practices

1. **Match Field Order**: Ensure your struct fields match the order of columns in your query's `:find` clause
2. **Use Appropriate Types**: Map Mentat types to Rust types correctly:
   - `:db.type/long` → `i64`
   - `:db.type/string` → `String`
   - `:db.type/boolean` → `bool`
   - `:db.type/double` → `f64`
   - `:db.type/instant` → Can be deserialized to chrono types
   - `:db.type/ref` → `i64` (entity ID)
   
3. **Handle Errors**: Always handle deserialization errors gracefully
4. **Test with Sample Data**: Verify your struct definition works with sample data before running on production
5. **Use Option<T>**: For nullable fields, use `Option<T>` in your struct

## Error Handling Patterns

```rust
use mentat_query_projector::deserialize::{rel_result_to_structs, RelResultDeserializeError};

fn safe_deserialize<T: serde::de::DeserializeOwned>(
    result: RelResult<Binding>
) -> Vec<T> {
    match rel_result_to_structs(result) {
        Ok(items) => items,
        Err(RelResultDeserializeError::SerializationError(msg)) => {
            log::error!("Binding serialization failed: {}", msg);
            Vec::new()
        },
        Err(RelResultDeserializeError::DeserializationError(msg)) => {
            log::error!("Struct deserialization failed: {}", msg);
            log::error!("Check that struct fields match query column order and types");
            Vec::new()
        },
    }
}
```
