# Concrete Example: Why Entity<V> Needs a Generic Parameter

## Example Transaction

Consider this simple EDN transaction:
```edn
[[:db/add "my-temp-id" :person/name "Alice"]]
```

## Two-Stage Processing

### Stage 1: Parsing (EDN string → Entity<ValueAndSpan>)

The parser produces:
```rust
Entity::AddOrRetract {
    op: OpType::Add,
    e: EntityPlace::TempId("my-temp-id"),
    a: AttributePlace::Entid(EntidOrIdent::Ident(Keyword::namespaced("person", "name"))),
    v: ValuePlace::Atom(ValueAndSpan {
        inner: SpannedValue::Text("Alice".into()),
        span: Span(35, 42),  // ← Source location preserved!
    })
}
```

**Key point:** The value `"Alice"` is stored as `ValueAndSpan`, which includes:
- The actual value: `SpannedValue::Text("Alice")`
- Source location: `Span(35, 42)` - character positions in the original EDN string

**Why this matters:**
If there's an error (e.g., wrong type), we can report:
```
Error at position 35-42: Expected :db.type/long but got string "Alice"
```

### Stage 2: Transaction Processing (Entity<ValueAndSpan> → TypedValue)

The transaction processor calls:
```rust
v.into_typed_value(&schema, ValueType::String)?
```

This converts:
```rust
// From ValueAndSpan:
ValueAndSpan {
    inner: SpannedValue::Text("Alice".into()),
    span: Span(35, 42),
}

// To TypedValue:
TypedValue::String(ValueRc::new("Alice"))
```

**Key point:** The conversion is **schema-aware**:
- Checks that the value matches the expected type
- Coerces values where appropriate (e.g., integers to refs)
- Reports errors with source locations

## Why Generic Parameter Is Necessary

### The Generic Function

Look at this function signature from `db/src/tx.rs:271`:
```rust
fn entities_into_terms_with_temp_ids_and_lookup_refs<I, V: TransactableValue>(
    &self,
    entities: I
) -> Result<(Vec<TermWithTempIdsAndLookupRefs>, InternSet<TempId>, InternSet<AVPair>)>
where
    I: IntoIterator<Item=Entity<V>>
{
    // ... processing code ...
}
```

This **single function** handles both:
1. `Entity<ValueAndSpan>` - from the parser
2. `Entity<TypedValue>` - from programmatic transaction builders

### Generic Processing Example

From `db/src/tx.rs:386`:
```rust
entmod::ValuePlace::Atom(v) => {
    match v.as_tempid() {
        Some(tempid) => /* ... */,
        None => {
            // This works for BOTH ValueAndSpan AND TypedValue!
            if let TypedValue::Ref(entid) = v.into_typed_value(&self.schema, ValueType::Ref)? {
                Ok(Either::Left(KnownEntid(entid)))
            } else {
                bail!(/* ... */)
            }
        }
    }
}
```

**Key point:** The generic parameter `V` is constrained by `TransactableValue`, which provides:
- `into_typed_value()` - convert to TypedValue
- `into_entity_place()` - convert to EntityPlace
- `as_tempid()` - check if it's a temp ID

## What Would Happen Without Generics?

### Option 1: Remove Generic, Always Use TypedValue

```rust
pub enum Entity {
    AddOrRetract {
        e: EntityPlace<TypedValue>,  // No generic
        v: ValuePlace<TypedValue>,   // No generic
        // ...
    }
}
```

**Problem:** Parser would produce:
```rust
Entity::AddOrRetract {
    v: ValuePlace::Atom(TypedValue::String(ValueRc::new("Alice")))
    // ❌ Lost source location! Can't report "Error at position 35-42"
}
```

**Error messages would become:**
```
Error: Expected :db.type/long but got string "Alice"
                                           ↑
                        Where in the EDN file? Unknown!
```

### Option 2: Use Associated Types

```rust
trait HasValue {
    type Value: TransactableValue;
}

struct ParsedEntity;
impl HasValue for ParsedEntity {
    type Value = ValueAndSpan;
}

struct TypedEntity;
impl HasValue for TypedEntity {
    type Value = TypedValue;
}

pub enum Entity<T: HasValue> {
    AddOrRetract {
        e: EntityPlace<T::Value>,
        v: ValuePlace<T::Value>,
        // ...
    }
}
```

**Problems:**

1. **Less intuitive:**
   - Before: `Entity<ValueAndSpan>` - clear what the value type is
   - After: `Entity<ParsedEntity>` - what's a ParsedEntity? Have to check trait impl

2. **Generic functions become harder:**
   ```rust
   // Before: Simple
   fn process<V: TransactableValue>(entities: Vec<Entity<V>>) { /* ... */ }

   // After: Complex
   fn process<T: HasValue>(entities: Vec<Entity<T>>)
   where
       T::Value: TransactableValue
   { /* ... */ }
   ```

3. **Need marker types:**
   - `ParsedEntity` and `TypedEntity` are just markers
   - Don't provide any additional value
   - Just add noise to the type system

## Real-World Usage Example

### From Parser (edn/src/edn.rustpeg:281)
```rust
pub entity -> Entity<ValueAndSpan>
```

### From Transaction Builder (transaction/src/entity_builder.rs:85)
```rust
pub type Terms = (Vec<Entity<TypedValue>>, InternSet<TempId>);
```

### From Transaction Processor (db/src/tx.rs:271)
```rust
fn entities_into_terms_with_temp_ids_and_lookup_refs<I, V: TransactableValue>(
    &self,
    entities: I
) -> Result<...>
where
    I: IntoIterator<Item=Entity<V>>
```

**All three work together seamlessly** because:
1. Parser produces `Entity<ValueAndSpan>`
2. Transaction builder produces `Entity<TypedValue>`
3. Transaction processor accepts both via `Entity<V: TransactableValue>`

## Conclusion

The generic parameter `V` on `Entity<V>` is **essential** because:

1. **Preserves Information:**
   - Parse stage: keeps source locations for error reporting
   - Transaction stage: uses validated typed values

2. **Enables Code Reuse:**
   - Single generic function processes both Entity types
   - No duplication between parse and transaction paths

3. **Maintains Separation:**
   - Parser doesn't need schema
   - Schema validation happens later
   - Clear architectural boundaries

4. **Type Safety:**
   - Compile-time verification via `TransactableValue` trait
   - Can't mix incompatible value representations

**The current design is optimal and should not be changed.**
