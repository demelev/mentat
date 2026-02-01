# Serde Support for Mentat Query Results

This document describes how to use serde to serialize and deserialize Mentat query results, including direct deserialization into custom structs.

## Overview

As of this update, the following types in Mentat now support serde serialization and deserialization:

- `Binding` - Represents a bound value in a query result
- `StructuredMap` - Represents a map of keyword-binding pairs
- `QueryResults` - The main query results type with variants:
  - `Scalar(Option<Binding>)` - Single value results
  - `Tuple(Option<Vec<Binding>>)` - Fixed-length tuple results
  - `Coll(Vec<Binding>)` - Collection of values
  - `Rel(RelResult<Binding>)` - Relational/tabular results
- `RelResult<T>` - Generic relation result structure

## Usage Examples

### Deserializing RelResult into Custom Structs (NEW!)

The most ergonomic way to work with query results is to deserialize them directly into your domain types:

```rust
use mentat_query_projector::{RelResult, deserialize::rel_result_to_structs};
use core_traits::{Binding, TypedValue};
use serde::Deserialize;

// Define your domain struct
#[derive(Deserialize, Debug, PartialEq)]
struct Person {
    id: i64,
    name: String,
}

// Get a RelResult from a query
let mut result = RelResult::empty(2);
result.values.push(Binding::Scalar(TypedValue::Long(1)));
result.values.push(Binding::Scalar(TypedValue::from("Alice")));
result.values.push(Binding::Scalar(TypedValue::Long(2)));
result.values.push(Binding::Scalar(TypedValue::from("Bob")));

// Deserialize directly into your struct
let people: Vec<Person> = rel_result_to_structs(result).unwrap();

assert_eq!(people.len(), 2);
assert_eq!(people[0], Person { id: 1, name: "Alice".to_string() });
assert_eq!(people[1], Person { id: 2, name: "Bob".to_string() });
```

### Deserializing a Single Row

You can also deserialize individual rows:

```rust
use mentat_query_projector::deserialize::row_to_struct;
use core_traits::{Binding, TypedValue};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct Product {
    id: i64,
    name: String,
    price: f64,
    in_stock: bool,
}

let row = vec![
    Binding::Scalar(TypedValue::Long(1)),
    Binding::Scalar(TypedValue::from("Laptop")),
    Binding::Scalar(TypedValue::Double(999.99.into())),
    Binding::Scalar(TypedValue::Boolean(true)),
];

let product: Product = row_to_struct(row).unwrap();
assert_eq!(product.id, 1);
assert_eq!(product.name, "Laptop");
```

### Basic Serialization with JSON

```rust
use mentat_query_projector::QueryResults;
use core_traits::{Binding, TypedValue};
use serde_json;

// Scalar result
let result = QueryResults::Scalar(Some(Binding::Scalar(TypedValue::Long(42))));
let json = serde_json::to_string(&result).unwrap();
println!("JSON: {}", json);

// Deserialize back
let deserialized: QueryResults = serde_json::from_str(&json).unwrap();
assert_eq!(result, deserialized);
```

### Collection Results

```rust
use mentat_query_projector::QueryResults;
use core_traits::{Binding, TypedValue};
use serde_json;

let results = QueryResults::Coll(vec![
    Binding::Scalar(TypedValue::Long(1)),
    Binding::Scalar(TypedValue::Long(2)),
    Binding::Scalar(TypedValue::Long(3)),
]);

let json = serde_json::to_string(&results).unwrap();
let deserialized: QueryResults = serde_json::from_str(&json).unwrap();
```

### Relational Results

```rust
use mentat_query_projector::{QueryResults, RelResult};
use core_traits::{Binding, TypedValue};
use serde_json;

let mut rel_result = RelResult::empty(2);
rel_result.values.push(Binding::Scalar(TypedValue::Long(1)));
rel_result.values.push(Binding::Scalar(TypedValue::from("Alice")));
rel_result.values.push(Binding::Scalar(TypedValue::Long(2)));
rel_result.values.push(Binding::Scalar(TypedValue::from("Bob")));

let results = QueryResults::Rel(rel_result);
let json = serde_json::to_string(&results).unwrap();
```

## Important Notes

### Type Mapping for Custom Structs

When deserializing `RelResult<Binding>` into custom structs, the field order in your struct must match the column order in the query result. The deserialization uses positional mapping based on array indices.

For example, if your query is:
```sql
[:find ?id ?name :where [?e :person/id ?id] [?e :person/name ?name]]
```

Your struct should be:
```rust
#[derive(Deserialize)]
struct Person {
    id: i64,    // First column
    name: String,  // Second column
}
```

### Error Handling

The `rel_result_to_structs` and `row_to_struct` functions return `Result<T, RelResultDeserializeError>` which can fail in two ways:

1. **SerializationError** - If the `Binding` values cannot be converted to the intermediate format
2. **DeserializationError** - If the intermediate format doesn't match your struct's expected types or field count

Example:
```rust
use mentat_query_projector::deserialize::{rel_result_to_structs, RelResultDeserializeError};

match rel_result_to_structs::<Person>(result) {
    Ok(people) => println!("Got {} people", people.len()),
    Err(RelResultDeserializeError::SerializationError(msg)) => {
        eprintln!("Failed to serialize bindings: {}", msg);
    },
    Err(RelResultDeserializeError::DeserializationError(msg)) => {
        eprintln!("Failed to deserialize into Person: {}", msg);
    },
}
```

### How It Works

The deserialization process uses a two-step approach:

1. **Binding → JSON Value**: Each row (Vec<Binding>) is first serialized to `serde_json::Value` using the existing `Serialize` implementation
2. **JSON Value → Your Struct**: The JSON value is then deserialized into your target type using serde's standard deserialization

This approach leverages the existing serde infrastructure and ensures type safety while avoiding the complexity of implementing a custom deserializer.

### JSON Limitations with StructuredMap

Note that JSON does not support non-string keys in maps. If you have `StructuredMap` instances (which use `ValueRc<Keyword>` as keys), you may need to use a different serialization format such as:

- [bincode](https://github.com/bincode-org/bincode) - Binary serialization
- [MessagePack](https://msgpack.org/) - Binary JSON-like format
- [CBOR](http://cbor.io/) - Binary data serialization

Example with bincode:

```rust
use core_traits::{StructuredMap, TypedValue};
use edn::Keyword;

let mut map = StructuredMap::default();
map.insert(Keyword::namespaced("person", "name"), TypedValue::from("Alice"));

// Serialize with bincode
let encoded = bincode::serialize(&map).unwrap();

// Deserialize
let decoded: StructuredMap = bincode::deserialize(&encoded).unwrap();
assert_eq!(map, decoded);
```

## Implementation Details

All the necessary derives have been added to the following types:
- `Binding` enum in `core-traits/lib.rs`
- `StructuredMap` struct in `core-traits/lib.rs`
- `QueryResults` enum in `query-projector/src/lib.rs`
- `RelResult<T>` struct in `query-projector/src/relresult.rs`

The `indexmap` dependency has been configured with the `serde-1` feature to enable serialization of the internal `IndexMap` used by `StructuredMap`.

## Dependencies

To use serde with Mentat query results, ensure you have the following in your `Cargo.toml`:

```toml
[dependencies]
mentat = "0.11"
serde = "1.0"
serde_json = "1.0"  # or your preferred serde format

# For StructuredMap serialization
bincode = "1.3"  # optional, if you need to serialize maps
```

## Testing

Tests have been added in:
- `core-traits/tests/serde_test.rs` - Tests for Binding and StructuredMap
- `query-projector/tests/serde_test.rs` - Tests for QueryResults and RelResult

Run the tests with:
```bash
cargo test --package core_traits --test serde_test
cargo test --package mentat_query_projector --test serde_test
```
