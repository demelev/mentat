# Mentat Entity - ORM-like Functionality for Mentat

This crate provides ORM-like functionality for the Mentat knowledge base, allowing Rust structs to be directly serialized to and deserialized from the database without JSON round-trips.

## Features

- **Derive Macros**: 
  - `#[derive(Entity)]` - Automatically implement Entity traits for your structs
  - `#[derive(EntityView)]` - Generate metadata for entity views with refs/backrefs
  - `#[derive(EntityPatch)]` - Generate transaction operations from entity patches
- **Schema Generation**: Automatically generate Mentat schema from struct definitions
- **Type-Safe Operations**: All database operations are type-checked at compile time
- **Optional Fields**: Support for `Option<T>` fields that map to optional attributes
- **Refs and Backrefs**: Support for forward and reverse references between entities
- **Cardinality**: Automatic detection of cardinality (one vs many) from field types
- **Unique Constraints**: Support for unique identity and unique value constraints
- **Indexing**: Mark fields for indexing with attributes
- **No JSON Round-trips**: Direct conversion between Rust types and Mentat's TypedValue

## Quick Start

### EntityView and EntityPatch (Recommended for New Code)

```rust
use mentat_entity::{EntityView, EntityPatch, EntityId, Patch, ManyPatch};

// Define a view of an entity
#[derive(EntityView)]
#[entity(ns = "person")]
struct PersonView {
    #[attr(":db/id")]
    id: i64,
    name: String,
    #[backref(attr = ":car/owner")]
    cars: Vec<CarView>,
}

// Define a patch for updating entities
#[derive(EntityPatch)]
#[entity(ns = "person")]
struct PersonPatch {
    #[entity_id]
    id: EntityId,
    name: Patch<String>,
    tags: ManyPatch<String>,
}

// Use the patch
let patch = PersonPatch {
    id: EntityId::Entid(100),
    name: Patch::Set("Alice".to_string()),
    tags: ManyPatch {
        add: vec!["vip".to_string()],
        remove: vec![],
        clear: false,
    },
};

let ops = patch.to_tx(); // Generate transaction operations
```

See [ENTITY_CODEGEN.md](./ENTITY_CODEGEN.md) for complete documentation of EntityView and EntityPatch.

## Usage

### Basic Example (Entity macro - traditional ORM)

```rust
use mentat_entity::{Entity, EntityWrite, EntityRead, transact_schema};
use mentat::{Store, TypedValue};

#[derive(Entity)]
struct Car {
    number: String,
    registration: EntityId,
}

#[derive(Entity)]
#[entity(namespace = "person")]
struct Person {
    #[entity(unique = "identity")]
    email: String,
    name: String,
    age: Option<i64>,
    #[attr(":car/_owner") ]
    car: Car,
}

fn main() {
    let mut store = Store::open("").unwrap();
    
    // 1. Transact the schema
    {
        let mut in_progress = store.begin_transaction().unwrap();
        transact_schema::<Person, _, _>(&mut in_progress).unwrap();
        in_progress.commit().unwrap();
    }
    
    // 2. Create and write an entity
    let person = Person {
        email: "alice@example.com".to_string(),
        name: "Alice".to_string(),
        age: Some(30),
    };
    
    let person_id = {
        let mut in_progress = store.begin_transaction().unwrap();
        let id = person.write(&mut in_progress).unwrap();
        in_progress.commit().unwrap();
        id
    };
    
    // 3. Read the entity back
    {
        let in_progress = store.begin_read().unwrap();
        let person = Person::read(&in_progress, person_id).unwrap();
        println!("Read person: {} ({})", person.name, person.email);
    }
}
```

### Container Attributes

Container attributes are applied to the struct itself:

- `#[entity(namespace = "my_namespace")]` - **Required**. Specifies the namespace for all attributes of this entity.

### Field Attributes

Field attributes are applied to individual fields:

- `#[entity(unique = "identity")]` - Mark field as a unique identity attribute (enables upserts)
- `#[entity(unique = "value")]` - Mark field as a unique value attribute
- `#[entity(indexed)]` - Mark field for indexing (improves query performance)
- `#[entity(many)]` - Mark field as having many cardinality (default is one)

### Supported Types

The following Rust types are supported and map to Mentat types:

| Rust Type | Mentat Type | Notes |
|-----------|-------------|-------|
| `String` | `:db.type/string` | |
| `i64` | `:db.type/long` | |
| `f64` | `:db.type/double` | |
| `bool` | `:db.type/boolean` | |
| `DateTime<Utc>` | `:db.type/instant` | From `chrono` crate |
| `Uuid` | `:db.type/uuid` | From `uuid` crate |
| `Keyword` | `:db.type/keyword` | From `mentat_core` |
| `Entid` | `:db.type/ref` | References to other entities |
| `Option<T>` | Any of above | Makes field optional |

### Schema Generation

The `Entity` trait provides a `schema()` method that returns an `EntitySchema` object. This can be converted to EDN format and transacted into the database:

```rust
let schema = Person::schema();
let edn = schema.to_edn();
// edn contains the schema definition in EDN format
```

Alternatively, use the `transact_schema` helper:

```rust
let mut in_progress = store.begin_transaction().unwrap();
transact_schema::<Person, _, _>(&mut in_progress).unwrap();
in_progress.commit().unwrap();
```

### Writing Entities

The `EntityWrite` trait provides two methods:

```rust
// Write a new entity (assigns a new entid)
let entid = person.write(&mut in_progress)?;

// Update an existing entity
let entid = person.write_with_entid(&mut in_progress, existing_entid)?;
```

### Reading Entities

The `EntityRead` trait provides methods for reading entities:

```rust
// Read by entid
let person = Person::read(&in_progress, entid)?;

// Read by unique attribute
let person = Person::read_by_unique(
    &in_progress,
    &Keyword::namespaced("person", "email"),
    TypedValue::String("alice@example.com".into())
)?;
```

### Optional Fields

Fields wrapped in `Option<T>` are treated as optional:

```rust
#[derive(Entity)]
#[entity(namespace = "person")]
struct Person {
    name: String,          // Required field
    age: Option<i64>,      // Optional field
}

// If age is None, the attribute won't be written to the database
let person = Person {
    name: "Bob".to_string(),
    age: None,  // This is fine
};
```

When reading, if an optional field's attribute doesn't exist in the database, it will be read as `None`. If a required field is missing, an error will be returned.

## Implementation Details

### No JSON Round-trips

Unlike traditional ORMs that serialize to JSON and back, this implementation directly converts between Rust types and Mentat's `TypedValue` enum. This is more efficient and type-safe.

### Generated Code

The `#[derive(Entity)]` macro generates implementations for:

1. `Entity` trait - Core trait with schema definition and value conversion
2. `EntityWrite` trait - Methods for writing entities to the database
3. `EntityRead` trait - Methods for reading entities from the database

### Schema Validation

When you transact a schema using `transact_schema`, Mentat validates it and creates the necessary attributes. Subsequent writes are validated against this schema.

## Design Philosophy

This implementation follows these principles:

1. **Type Safety**: All operations are type-checked at compile time
2. **Zero-cost Abstractions**: Direct conversion without intermediate representations
3. **Extensibility**: Easy to add custom query methods and constraints
4. **Explicit**: Schema must be explicitly transacted before use
5. **Mentat-native**: Uses Mentat's existing APIs and types

## Limitations and Future Work

Current limitations:

- Query builder for complex queries not yet implemented
- No support for derived attributes
- No support for transaction functions

Future enhancements could include:

- Query builder API for complex queries with filters
- Support for relations and joins
- Automatic generation of lookup methods for unique fields
- Support for transaction functions
- Batch operations for bulk inserts/updates

## License

Licensed under the Apache License, Version 2.0. See LICENSE file for details.
