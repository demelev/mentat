# Entity Generic Parameter Analysis

## Quick Start

**Question:** Does the Entity struct really need a generic parameter, or can TypedValue be used as an associated type?

**Answer:** The generic parameter is architecturally necessary. See documentation below.

## Documentation

- **[SUMMARY.md](SUMMARY.md)** - Executive summary with quick facts and recommendations (5 min read)
- **[ANALYSIS.md](ANALYSIS.md)** - Comprehensive architectural analysis (15 min read)
- **[EXAMPLE.md](EXAMPLE.md)** - Concrete code examples with side-by-side comparisons (10 min read)

## TL;DR

The generic parameter on `Entity<V>` enables:
- ✅ Parse-time source location preservation for error reporting
- ✅ Code reuse via generic processing functions
- ✅ Clean architectural separation (parser ↔ transaction processor)
- ✅ Type safety without sacrificing flexibility

**Recommendation: Do NOT refactor.** The current design is optimal.

## Key Findings

### Current Architecture
```rust
pub enum Entity<V> {
    AddOrRetract { e: EntityPlace<V>, v: ValuePlace<V>, ... }
}
```

### Two Instantiations (Both Necessary)
1. `Entity<ValueAndSpan>` - Parser output (preserves source locations)
2. `Entity<TypedValue>` - Transaction builder (validated typed values)

### Data Flow
```
EDN String → Entity<ValueAndSpan> → Entity<TypedValue> → Resolved Terms
           (parse, keep spans)   (validate, type check)
```

### Why Alternatives Fail

**Associated Types:** Less intuitive, requires marker types, no benefit

**Remove Generic:** Loses source locations, breaks architecture, poor error messages

## Code References

- `edn/src/entities.rs` - Entity type definitions
- `db/src/types.rs` - TransactableValue trait
- `db/src/tx.rs` - Generic transaction processing
- `transaction/src/entity_builder.rs` - Transaction builder

## Conclusion

The generic parameter is a well-designed architectural choice. No changes recommended.

---

*Analysis performed: 2026-01-31*
