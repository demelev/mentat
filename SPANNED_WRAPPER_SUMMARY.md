# Spanned Entity Wrapper - Analysis Summary

## Question: "what if wrap into Spanned(Entity)?"

## Answer: YES - Excellent Idea! ✅

The `Spanned<Entity<Value>>` architecture is **significantly better** than the current `Entity<ValueAndSpan>` approach.

## Why Spanned Wrapper is Better

### Current Problem
```rust
Entity<ValueAndSpan>  // Every value carries span info
- ValuePlace::Atom(ValueAndSpan { inner: Value, span: Span })
- EntityPlace::LookupRef(LookupRef { v: ValueAndSpan })
- Deeply nested spans throughout the structure
```

### Proposed Solution
```rust
Spanned<Entity<Value>>  // One span wraps entire entity
- Spanned { inner: Entity<Value>, span: Span }
- Values are plain Value types (no span wrapper)
- Clean, flat structure
```

## Key Finding

**Spans are NEVER used in transaction processing!**

Verified in codebase:
- Zero accesses to `.span` field in `db/src/`
- Spans discarded when converting ValueAndSpan → TypedValue
- All that complexity for nothing!

## Benefits Comparison

| Aspect | Entity<ValueAndSpan> | Spanned<Entity<Value>> |
|--------|---------------------|------------------------|
| **Complexity** | High (nested) | Low (flat) |
| **Code Verbosity** | Very verbose | Concise |
| **Span Granularity** | Per-value | Per-entity |
| **Actual Span Usage** | 0% | 0% |
| **Conversion Logic** | Recursive strip | Simple unwrap |
| **Type Complexity** | High | Low |
| **Maintainability** | Lower | Higher |

## Architecture Change

```rust
// BEFORE
edn::parse::entities() -> Vec<Entity<ValueAndSpan>>
                              ↓
                         Extract spans recursively
                              ↓
                      Entity<TypedValue>

// AFTER  
edn::parse::entities() -> Vec<Spanned<Entity<Value>>>
                              ↓
                         Unwrap Spanned wrapper
                              ↓
                      Entity<TypedValue>
```

## Concrete Example

### Before (Current)
```rust
Entity::AddOrRetract {
    op: OpType::Add,
    e: EntityPlace::Entid(EntidOrIdent::Entid(1)),
    a: AttributePlace::Entid(EntidOrIdent::Ident(kw)),
    v: ValuePlace::Atom(ValueAndSpan {
        inner: SpannedValue::Text("hello".into()),
        span: Span(10, 15),  // span for just this value
    }),
}
```

### After (Proposed)
```rust
Spanned {
    inner: Entity::AddOrRetract {
        op: OpType::Add,
        e: EntityPlace::Entid(EntidOrIdent::Entid(1)),
        a: AttributePlace::Entid(EntidOrIdent::Ident(kw)),
        v: ValuePlace::Atom(Value::Text("hello".into())),  // Plain value!
    },
    span: Span(0, 25),  // span for entire entity expression
}
```

## Implementation Status

### ✅ Completed
1. Added `Spanned<T>` wrapper type to `edn/src/entities.rs`
2. Marked `Value` as `TransactableValueMarker`
3. Created comprehensive analysis documentation
4. Verified spans are unused in transaction processing

### 🔲 Remaining Work
1. Update EDN parser (edn.rustpeg) to produce `Spanned<Entity<Value>>`
2. Implement `TransactableValue` trait for plain `Value` type
3. Update transaction processing to handle `Spanned<Entity<V>>`
4. Update bootstrap code to use new types
5. Add tests for new architecture
6. Deprecate `ValueAndSpan` in entity context (keep for backward compat)

## Relationship with Generic Parameter

**Important**: The Spanned wrapper and generic parameter serve **different purposes**:

- **Generic parameter `V`**: Allows Entity to work with different value types (Value vs TypedValue)
- **Spanned wrapper**: Attaches source location to entities

They work **together**:
```rust
Parser:  Spanned<Entity<Value>>      // Generic + Wrapper
Builder: Entity<TypedValue>           // Just Generic (no wrapper needed)
```

## Error Reporting

Both approaches support error reporting:

**Before**: "Error at position 10-15 in entity"  
**After**: "Error at position 0-25 in entity"

The difference:
- Before: Could point to individual value (but never did!)
- After: Points to entire entity (which is what we actually need!)

## Code Simplification

Estimated lines of code reduction:
- Remove ValueAndSpan wrapping in parser: ~50 lines simpler
- Simpler conversion logic: ~100 lines simpler
- Less verbose types throughout: Easier to read and maintain

## Recommendation

✅ **IMPLEMENT IT!**

The `Spanned<Entity<Value>>` architecture is:
1. Simpler
2. Cleaner
3. More maintainable
4. Functionally equivalent
5. Zero downsides

This is a **pure improvement** with no trade-offs.

## Next Steps

1. Review this analysis
2. Approve architecture change
3. Implement parser changes
4. Update transaction processing
5. Run tests
6. Deploy

---

**Analysis Date:** 2026-01-31  
**Status:** Architecture Design Complete ✅  
**Full Details:** See `ENTITY_SPANNED_ARCHITECTURE.md`
