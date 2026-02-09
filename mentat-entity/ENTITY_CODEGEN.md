# EntityView and EntityPatch Codegen

This feature provides derive macros for generating code to work with Mentat/Datomic-style entity views and patches.

## Overview

The `EntityView` and `EntityPatch` derive macros allow you to:
1. Define entity views with pull-pattern metadata (EntityView)
2. Define entity patches for transactional updates (EntityPatch)
3. Automatically handle refs, backrefs, and cardinality
4. Generate transaction operations from patches

## EntityView

The `EntityView` macro generates metadata for reading entities from the database.

### Basic Example

```rust
use mentat_entity::{EntityView, EntityViewSpec};

#[derive(EntityView)]
#[entity(ns = "person")]
struct PersonView {
    #[attr(":db/id")]
    id: i64,
    name: String,
    age: Option<i64>,
}
```

### Attributes

#### Container attributes (on the struct)
- `#[entity(ns = "namespace")]` - Specify namespace (optional, defaults to snake_case of struct name)

#### Field attributes
- `#[attr(":custom/ident")]` - Override the attribute identifier
- `#[fref(attr = ":x/y")]` - Mark as forward reference (use `fref` to avoid `ref` keyword)
- `#[backref(attr = ":x/y")]` - Mark as reverse reference (backref)

### Default Behavior

- **Namespace**: If not specified, defaults to snake_case of the struct name
  - `PersonView` → `"person_view"`
  - `OrderItem` → `"order_item"`

- **Attribute names**: Default to `:{namespace}/{snake_case(field_name)}`
  - `name` → `:person/name`
  - `order_date` → `:order/order_date`

### Cardinality

Cardinality is automatically detected from field types:
- `T` or `Option<T>` → cardinality-one
- `Vec<T>` → cardinality-many

### References

#### Forward References (fref)

```rust
#[derive(EntityView)]
#[entity(ns = "car")]
struct CarView {
    #[fref(attr = ":car/owner")]
    owner: EntityId,
}
```

#### Reverse References (backref)

```rust
#[derive(EntityView)]
#[entity(ns = "person")]
struct PersonView {
    #[backref(attr = ":car/owner")]
    cars: Vec<CarView>,
}
```

Note: For backref, the `attr` specifies the **forward** attribute (`:car/owner`), not the reverse form.

### Generated Code

The macro implements the `EntityViewSpec` trait:

```rust
impl EntityViewSpec for PersonView {
    const NS: &'static str = "person";
    const FIELDS: &'static [FieldSpec] = &[...];
}
```

## EntityPatch

The `EntityPatch` macro generates code for creating transaction operations from patches.

### Basic Example

```rust
use mentat_entity::{EntityPatch, EntityId, Patch, ManyPatch};

#[derive(EntityPatch)]
#[entity(ns = "order")]
struct OrderPatch {
    #[entity_id]
    id: EntityId,
    
    status: Patch<OrderStatus>,
    tags: ManyPatch<String>,
}
```

### Required Field

Every `EntityPatch` struct **must** have exactly one field with `#[entity_id]` attribute.

### Patch Types

#### Patch<T> (cardinality-one)

```rust
pub enum Patch<T> {
    NoChange,   // Don't modify this field
    Set(T),     // Set to new value → generates Assert
    Unset,      // Remove value → generates RetractAttr
}
```

#### ManyPatch<T> (cardinality-many)

```rust
pub struct ManyPatch<T> {
    pub add: Vec<T>,      // Values to add → generates Assert for each
    pub remove: Vec<T>,   // Values to remove → generates Retract for each
    pub clear: bool,      // Clear all first → generates RetractAttr
}
```

### Generated Method

The macro generates a `to_tx()` method:

```rust
impl OrderPatch {
    pub fn to_tx(&self) -> Vec<TxOp> {
        // ... generated code
    }
}
```

### Example Usage

```rust
let patch = OrderPatch {
    id: EntityId::Entid(100),
    status: Patch::Set(OrderStatus::Paid),
    tags: ManyPatch {
        add: vec!["premium".to_string()],
        remove: vec!["basic".to_string()],
        clear: false,
    },
};

let ops = patch.to_tx();
// Returns: [Assert, Assert, Retract]
```

## Core Types

### EntityId

```rust
pub enum EntityId {
    Entid(i64),
    LookupRef { attr: &'static str, value: TypedValue },
    Temp(i64),
}
```

### TxOp

```rust
pub enum TxOp {
    Assert { e: EntityId, a: &'static str, v: TypedValue },
    Retract { e: EntityId, a: &'static str, v: TypedValue },
    RetractAttr { e: EntityId, a: &'static str },
}
```

### FieldKind

```rust
pub enum FieldKind {
    Scalar,
    Ref { nested: &'static str },
    Backref { nested: &'static str },
}
```

## Complete Example

See `examples/entity_view_patch_example.rs` for a complete working example including:
- Person and Car views with backrefs
- Order patches with status and tags
- Default namespace behavior
- Multiple cardinality types

Run it with:
```bash
cargo run --package mentat-entity --example entity_view_patch_example
```

## Running Tests

```bash
cargo test --package mentat-entity --test entity_view_patch_tests
```

## Limitations (MVP)

1. **Value Conversion**: Custom types in patches must implement `Into<TypedValue>` or use one of the built-in conversions (String, i64, f64, bool, Uuid, EntityId).

2. **EntityId in Patches**: Currently only `EntityId::Entid` is supported in patches. LookupRef and Temp will panic.

3. **Keyword Conflict**: Use `#[fref(...)]` instead of `#[ref(...)]` to avoid Rust keyword conflict.

4. **Static Metadata**: Field metadata uses `&'static str`, suitable for metadata but not for runtime-generated patterns.

## Technical Details

### Attribute Mapping Rules

1. Namespace is determined by:
   - `#[entity(ns = "...")]` if specified
   - Otherwise: `snake_case(struct_name)`

2. Attribute identifier is determined by:
   - `#[attr(":custom/ident")]` if specified
   - `#[fref(attr = ":x/y")]` or `#[backref(attr = ":x/y")]` if specified
   - Otherwise: `:{namespace}/{snake_case(field_name)}`

### Transaction Operation Generation

For `Patch<T>`:
- `NoChange` → no operation
- `Set(v)` → `Assert { e, a, v }`
- `Unset` → `RetractAttr { e, a }`

For `ManyPatch<T>`:
1. If `clear == true` → `RetractAttr { e, a }`
2. For each in `add` → `Assert { e, a, v }`
3. For each in `remove` → `Retract { e, a, v }`

## Future Enhancements (Not in MVP)

- View profiles (different views of same entity)
- Ensure/CAS predicates for optimistic concurrency
- Component cascades
- Full LookupRef/TempId support in patches
- EDN pull pattern generation
- Pull depth control
