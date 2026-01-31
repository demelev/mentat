# Entity Spanned Architecture Analysis

## Problem Statement

Explore whether Entity struct needs a generic parameter, or if TypedValue can be used as an associated type. Additionally, consider wrapping Entity in a Spanned wrapper: `Spanned<Entity>`.

## Current Architecture

### Entity Generic Parameter

Currently, `Entity<V>` is defined as a generic enum:

```rust
pub enum Entity<V> {
    AddOrRetract {
        op: OpType,
        e: EntityPlace<V>,
        a: AttributePlace,
        v: ValuePlace<V>,
    },
    MapNotation(MapNotation<V>),
}
```

The generic parameter `V` allows Entity to work with different value types:

1. **`Entity<ValueAndSpan>`** - Used by EDN parser
   - Produced when parsing EDN strings
   - Each value carries span information (source location)
   - Defined in `edn` crate

2. **`Entity<TypedValue>`** - Used by transaction builder
   - Used for programmatic transaction building
   - Values are typed and validated
   - Defined in `core-traits` crate

### Why Generic Parameter Exists

The generic parameter serves a crucial purpose:

- **Separation of Concerns**: The `edn` crate handles parsing without knowing about `TypedValue`
- **Reusability**: Same Entity definition works for both parsing and programmatic building
- **Type Safety**: Different value types can't be mixed accidentally

### Related Generic Types

- `EntityPlace<V>`: Can contain Entid, TempId, LookupRef<V>, or TxFunction
- `ValuePlace<V>`: Can contain Entid, TempId, LookupRef<V>, TxFunction, Vector, Atom(V), or MapNotation
- `LookupRef<V>`: Lookup reference with attribute and value
- `MapNotation<V>`: Type alias for `BTreeMap<EntidOrIdent, ValuePlace<V>>`

## Proposal: Spanned<Entity> Architecture

### Motivation

Current observation: **Span information is never used in transaction processing**

- Verified in `db/src/` - zero accesses to `.span` field
- `ValueAndSpan` carries spans through parsing but they're discarded
- Spans at individual value level provide no benefit

### Proposed Change

Instead of embedding spans in every value, attach ONE span per entity:

**Before:**
```rust
Parser:  Entity<ValueAndSpan>  // Spans embedded in every value
Builder: Entity<TypedValue>     // No spans
```

**After:**
```rust
Parser:  Spanned<Entity<Value>>  // One span wraps entire entity
Builder: Entity<TypedValue>       // No spans (unchanged)
```

### Spanned<T> Wrapper

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialOrd, PartialEq)]
pub struct Spanned<T> {
    pub inner: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    pub fn new(inner: T, span: Span) -> Self {
        Spanned { inner, span }
    }
}
```

### Benefits

1. **Simpler Structure**
   - No need to wrap every value in ValueAndSpan
   - Flatter type hierarchy
   - Less verbose pattern matching

2. **Same Functionality**
   - Since spans aren't used in transaction processing anyway
   - Entity-level span sufficient for error reporting
   - Can still point to "this entity" in error messages

3. **Easier Conversion**
   - Convert `Spanned<Entity<Value>>` → `Entity<TypedValue>` by:
     - Unwrapping the Spanned wrapper
     - Converting Value → TypedValue
   - No need to recursively strip spans from nested structures

4. **More Appropriate Granularity**
   - Span indicates where entire entity expression appeared
   - Makes sense: `[:db/add e a v]` is one expression, one span
   - Fine-grained value spans unused → unnecessary complexity

## Implementation Changes Required

### 1. EDN Parser (edn/src/edn.rustpeg)

Change parser rule from:
```
pub entity -> Entity<ValueAndSpan>
```

To:
```
pub entity -> Spanned<Entity<Value>>
```

This requires:
- Using plain `Value` instead of `ValueAndSpan` in entity construction
- Wrapping result in `Spanned` with entity's overall span

### 2. TransactableValue Trait (db/src/types.rs)

Add implementation for plain `Value`:
```rust
impl TransactableValue for Value {
    fn into_typed_value(self, schema: &Schema, value_type: ValueType) -> Result<TypedValue> {
        // Convert Value to TypedValue based on value_type
        // Similar to existing ValueAndSpan impl but without span handling
    }
    
    fn into_entity_place(self) -> Result<EntityPlace<Self>> {
        // Convert Value to EntityPlace<Value>
        // Similar to existing ValueAndSpan impl but simpler
    }
}
```

### 3. Transaction Processing (transaction/src/lib.rs)

Update signature:
```rust
pub fn transact_entities<I, V: TransactableValue>(&mut self, entities: I) -> Result<TxReport> 
where I: IntoIterator<Item=Spanned<Entity<V>>>
{
    // Unwrap Spanned, process inner Entity
}
```

Or keep current signature and handle Spanned at call site:
```rust
pub fn transact<B>(&mut self, transaction: B) -> Result<TxReport> 
where B: Borrow<str> 
{
    let spanned_entities = edn::parse::entities(transaction.borrow())?;
    let entities = spanned_entities.into_iter().map(|s| s.inner);
    self.transact_entities(entities)
}
```

### 4. Bootstrap (db/src/bootstrap.rs)

Update to use new types:
```rust
pub(crate) fn bootstrap_entities() -> Vec<Spanned<Entity<Value>>> {
    let bootstrap_assertions = include_str!("bootstrap/assertions.edn");
    edn::parse::entities(&bootstrap_assertions.to_string())
        .expect("bootstrap assertions")
}
```

## Alternative: Keep Generic, Add Associated Type

An alternative would be to remove the generic and use an associated type:

```rust
pub trait HasValue {
    type Value;
}

pub enum Entity<H: HasValue> {
    AddOrRetract {
        op: OpType,
        e: EntityPlace<H::Value>,
        a: AttributePlace,
        v: ValuePlace<H::Value>,
    },
    MapNotation(MapNotation<H::Value>),
}
```

**Rejected because:**
- More complex than current generic parameter
- Requires phantom type parameter
- No clear benefit over simple generic
- Breaks existing API more significantly

## Decision: Keep Generic Parameter

The generic parameter `V` should be **kept** because:

1. **Clean Separation**: `edn` crate doesn't depend on `core-traits`
2. **Simple & Effective**: Generic parameter is straightforward
3. **Multiple Use Cases**: Parser uses `Value`, builder uses `TypedValue`
4. **Minimal Change**: Spanned wrapper works with existing generic

## Recommendation

Implement `Spanned<Entity<V>>` architecture:

1. Add `Spanned<T>` wrapper type ✅ (Done)
2. Mark `Value` as `TransactableValueMarker` ✅ (Done)
3. Update parser to produce `Spanned<Entity<Value>>`
4. Implement `TransactableValue` for `Value`
5. Update transaction processing to handle `Spanned<Entity<V>>`
6. Update bootstrap and tests
7. Deprecate `ValueAndSpan` usage in entities (keep for backward compatibility)

## Benefits Summary

| Aspect | Entity<ValueAndSpan> | Spanned<Entity<Value>> |
|--------|---------------------|------------------------|
| **Complexity** | High (nested spans) | Low (single span) |
| **Verbosity** | High | Low |
| **Span Granularity** | Per-value (unused) | Per-entity (sufficient) |
| **Conversion Complexity** | Recursive stripping | Simple unwrap |
| **Error Reporting** | Can point to values | Can point to entity |
| **Actual Usage** | Spans discarded | Spans available |

## Conclusion

The `Spanned<Entity<Value>>` architecture is **strictly better** than `Entity<ValueAndSpan>` for this codebase because:

- Span information at value level is not utilized
- Simpler structure with same functionality
- Entity-level span sufficient for error reporting
- Easier to implement and maintain

The generic parameter `V` should be **retained** as it serves a different purpose (Value vs TypedValue separation) and works well with the Spanned wrapper.
