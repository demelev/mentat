# Implementation Summary: ORM-like Functionality for Mentat

## Overview

This implementation adds ORM-like functionality to the Mentat knowledge base, allowing Rust structs to be directly serialized to and deserialized from the database without JSON round-trips.

## What Was Implemented

### 1. New Crates

#### `mentat-entity` (Main Crate)
Located at `/mentat-entity/`, this crate provides:

- **Core Traits**:
  - `Entity`: Core trait for entities that can be persisted
  - `EntityWrite`: Trait for writing entities to the database
  - `EntityRead`: Trait for reading entities from the database

- **Schema Types**:
  - `EntitySchema`: Represents the complete schema for an entity type
  - `FieldDefinition`: Metadata for individual fields
  - `FieldType`: Maps Rust types to Mentat types
  - `Cardinality`: One or Many cardinality
  - `Unique`: Uniqueness constraints (None, Value, Identity)

- **Helper Functions**:
  - `transact_schema<T>()`: Helper to transact an entity's schema
  - `read_entity_attributes()`: Query helper for reading entity data
  - `find_entity_by_unique()`: Query helper for finding entities by unique attributes

#### `mentat-entity-derive` (Procedural Macro Crate)
Located at `/mentat-entity-derive/`, this crate provides:

- **`#[derive(Entity)]` Macro**: Automatically generates implementations of `Entity`, `EntityWrite`, and `EntityRead` traits

### 2. Key Features

#### Direct Type Conversion (No JSON Round-trips)
- All conversions happen directly between Rust types and Mentat's `TypedValue` enum
- No intermediate serialization formats
- Type-safe at compile time

#### Supported Types
| Rust Type | Mentat Type |
|-----------|-------------|
| `String` | `:db.type/string` |
| `i64` | `:db.type/long` |
| `f64` | `:db.type/double` |
| `bool` | `:db.type/boolean` |
| `DateTime<Utc>` | `:db.type/instant` |
| `Uuid` | `:db.type/uuid` |
| `Keyword` | `:db.type/keyword` |
| `Entid` | `:db.type/ref` |
| `Option<T>` | Any of above (nullable) |

#### Schema Definition
Schemas are automatically generated from struct definitions:

```rust
#[derive(Entity)]
#[entity(namespace = "person")]
struct Person {
    #[entity(unique = "identity")]
    email: String,
    name: String,
    age: Option<i64>,
}
```

This generates:
- Attribute definitions (`:person/email`, `:person/name`, `:person/age`)
- Type mappings (String → `:db.type/string`, i64 → `:db.type/long`)
- Uniqueness constraints (email as unique identity)
- Optional field handling (age can be `None`)

#### Field Attributes

**Container attributes** (on struct):
- `#[entity(namespace = "...")]` - Required, sets the namespace for attributes

**Field attributes**:
- `#[entity(unique = "identity")]` - Unique identity constraint (for upserts)
- `#[entity(unique = "value")]` - Unique value constraint
- `#[entity(indexed)]` - Mark field for indexing
- `#[entity(many)]` - Many cardinality (default is one)

#### Write Operations

Two methods provided:

```rust
// Write new entity (auto-generates entid)
let entid = person.write(&mut in_progress)?;

// Update existing entity
let entid = person.write_with_entid(&mut in_progress, existing_entid)?;
```

Implementation uses Mentat's `TermBuilder` to construct transactions programmatically.

#### Read Operations

Two methods provided:

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

Implementation uses Mentat's query system to fetch entity data.

#### Optional Field Handling

Fields wrapped in `Option<T>` are properly handled:

- **Write**: If `None`, attribute is not written to database
- **Read**: If attribute doesn't exist, field is set to `None`
- **Validation**: Required fields return error if missing; optional fields default to `None`

### 3. Testing

Comprehensive tests in `/mentat-entity/tests/entity_tests.rs`:

- ✅ Schema generation from struct definition
- ✅ EDN schema string generation
- ✅ `to_values()` conversion (Rust struct → TypedValue map)
- ✅ `from_values()` conversion (TypedValue map → Rust struct)
- ✅ Optional field handling (both `Some` and `None` cases)
- ✅ Required field validation

### 4. Documentation

- **README.md**: Complete usage guide with examples
- **Example**: Demonstration in `/mentat-entity/examples/basic_usage.rs`
- **Inline docs**: Comprehensive documentation in source files

## How It Works

### Schema Transaction Flow

1. Define struct with `#[derive(Entity)]`
2. Macro generates `schema()` method that returns `EntitySchema`
3. Call `transact_schema::<Person>(&mut in_progress)`
4. Schema is converted to EDN and transacted into database

### Write Flow

1. Create struct instance
2. Call `.write(&mut in_progress)`
3. Struct is converted to `HashMap<Keyword, TypedValue>` via `to_values()`
4. `TermBuilder` constructs transaction with temp IDs
5. Transaction is executed via `transact_entities()`
6. Final `Entid` is extracted from `TxReport`

### Read Flow

1. Call `Person::read(&in_progress, entid)`
2. Helper `read_entity_attributes()` queries each attribute individually
3. Results are collected into `HashMap<Keyword, TypedValue>`
4. `from_values()` constructs struct from map
5. Optional fields become `None` if missing; required fields error

## Design Decisions

### Why Direct TypedValue Conversion?

- **Performance**: No overhead of JSON parsing/generation
- **Type Safety**: Compile-time type checking
- **Simplicity**: Directly uses Mentat's native types
- **Extensibility**: Easy to add new type mappings

### Why Procedural Macro?

- **Developer Experience**: Write natural Rust structs, not boilerplate
- **Maintainability**: Schema and code stay in sync automatically
- **Type Safety**: All conversions are generated and type-checked

### Why Separate Read/Write Traits?

- **Flexibility**: Can implement write-only or read-only entities
- **Clarity**: Clear separation of concerns
- **Testing**: Easier to test each capability independently

### Why Query-Based Reading?

- **Correctness**: Uses Mentat's query system, not direct SQL
- **Schema Validation**: Queries are validated against schema
- **Flexibility**: Easy to extend with filters and constraints

## Limitations and Future Work

### Current Limitations

1. **No Batch Operations**: Each entity is written individually
2. **No Complex Queries**: No query builder for filters/joins yet
3. **No Derived Attributes**: Can't define computed attributes
4. **No Transaction Functions**: Can't use Mentat's transaction functions
5. **Simple Read Strategy**: Reads each attribute individually (could be optimized with pull API)

### Future Enhancements

1. **Query Builder**: Fluent API for complex queries
   ```rust
   Person::query()
       .filter("age", GreaterThan(25))
       .order_by("name")
       .limit(10)
       .execute(&in_progress)?
   ```

2. **Automatic Lookup Methods**: Generate methods for unique fields
   ```rust
   // Auto-generated from #[entity(unique = "identity")] on email field
   Person::find_by_email(&in_progress, "alice@example.com")?
   ```

3. **Batch Operations**: Efficient bulk inserts/updates
   ```rust
   Person::write_batch(&mut in_progress, vec![person1, person2, person3])?
   ```

4. **Relations**: Support for entity relationships
   ```rust
   struct Order {
       customer: Entid,  // Reference to Person
       items: Vec<Entid>,  // Many-cardinality reference
   }
   ```

5. **Pull API Integration**: Use Mentat's pull API for efficient reading
6. **Migration Support**: Schema evolution helpers
7. **Transaction Function Support**: Integrate with Mentat's transaction functions

## Integration with Mentat

This implementation integrates cleanly with existing Mentat APIs:

- **Uses** `InProgress` for transactions (no new transaction types)
- **Uses** existing query system (no custom SQL)
- **Uses** `TermBuilder` for programmatic transactions
- **Uses** `TypedValue` and `Keyword` (no new value types)
- **Uses** EDN format for schema definition (standard Mentat approach)

## Files Added

```
mentat-entity/
├── Cargo.toml                    # Main crate dependencies
├── README.md                     # User documentation
├── src/
│   ├── lib.rs                   # Core traits and types
│   └── read.rs                  # Query helpers
├── examples/
│   └── basic_usage.rs           # Usage demonstration
└── tests/
    └── entity_tests.rs          # Comprehensive tests

mentat-entity-derive/
├── Cargo.toml                    # Proc macro dependencies
└── src/
    └── lib.rs                   # #[derive(Entity)] implementation
```

## Compliance with Requirements

✅ **User defines struct with special trait** - `#[derive(Entity)]` macro

✅ **Generates serialization/deserialization code** - Automatic via macro

✅ **Tell database which types to use** - `transact_schema()` function

✅ **Write struct instances** - `EntityWrite::write()` method

✅ **Read struct instances** - `EntityRead::read()` method

✅ **Extensible lookups** - `read_by_unique()` method, extensible design

✅ **Support Option<T> fields** - Full support for optional fields

✅ **No JSON round-trips** - Direct `TypedValue` conversion

✅ **Type-safe queries** - All operations are type-checked

## Conclusion

This implementation provides a solid foundation for ORM-like functionality in Mentat. It meets all the specified requirements while maintaining type safety, performance, and integration with Mentat's existing architecture. The extensible design allows for future enhancements without breaking changes.
