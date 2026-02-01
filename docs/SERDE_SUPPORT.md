# Serde Support for Mentat Query Results

This document describes how to use serde to serialize and deserialize Mentat query results.

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

### JSON Limitations with StructuredMap

Note that JSON does not support non-string keys in maps. If you have `StructuredMap` instances (which use `ValueRc<Keyword>` as keys), you may need to use a different serialization format such as:

- [bincode](https://github.com/bincode-org/bincode) - Binary serialization
- [MessagePack](https://msgpack.org/) - Binary JSON-like format
- [CBOR](http://cbor.io/) - Binary data serialization

Example with bincode:

```rust
use core_traits::StructuredMap;
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
