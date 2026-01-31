# Entity Architecture Exploration - Results

This PR explores two architectural questions about the `Entity` type in Mentat:

## 📋 Questions Explored

1. **Do we really need Entity struct to have generic parameter, or can TypedValue be used as associated type?**
2. **What if wrap into Spanned(Entity)?**

## ✅ Answers

### Question 1: Generic Parameter

**Answer: YES, keep the generic parameter `V`**

The generic parameter serves a critical purpose:
- `Entity<Value>` - Used by EDN parser (no TypedValue dependency)
- `Entity<TypedValue>` - Used by transaction builder (typed values)
- Clean separation between parsing and transaction logic

**Alternative considered:** Associated types  
**Result:** More complex, no benefits → Rejected

**See:** `SUMMARY.md` for full analysis from previous exploration

### Question 2: Spanned Wrapper

**Answer: YES, implement `Spanned<Entity>` - it's much better!**

Current architecture uses `Entity<ValueAndSpan>` with spans embedded at every value level.

**Key finding:** Span information is **never used** in transaction processing (verified in `db/src/`)

Proposed architecture: `Spanned<Entity<Value>>` with one span per entity.

**Benefits:**
- ✅ Simpler structure (flat vs nested)
- ✅ Less code verbosity
- ✅ Easier maintenance
- ✅ Same functionality (spans unused anyway)
- ✅ Cleaner conversion logic

**See:** 
- `SPANNED_WRAPPER_SUMMARY.md` - Quick summary
- `ENTITY_SPANNED_ARCHITECTURE.md` - Complete analysis

## 📊 Comparison

### Current Architecture
```rust
// Parser produces deeply nested spans
Entity<ValueAndSpan> {
    v: ValuePlace::Atom(ValueAndSpan {
        inner: SpannedValue::Text("value"),
        span: Span(10, 15),  // Individual value span
    }),
}
```

### Proposed Architecture
```rust
// Parser produces single wrapper span
Spanned<Entity<Value>> {
    inner: Entity {
        v: ValuePlace::Atom(Value::Text("value")),  // Plain value!
    },
    span: Span(0, 25),  // Entity-level span
}
```

## 🔨 Implementation Status

### Completed ✅
- [x] Architectural analysis
- [x] Added `Spanned<T>` wrapper type
- [x] Marked `Value` as `TransactableValueMarker`
- [x] Verified spans unused in transaction processing
- [x] Created comprehensive documentation

### Remaining 🔲
- [ ] Update EDN parser (edn.rustpeg) to produce `Spanned<Entity<Value>>`
- [ ] Implement `TransactableValue` for `Value` type
- [ ] Update transaction processing to handle `Spanned<Entity<V>>`
- [ ] Update bootstrap code
- [ ] Add tests
- [ ] Deprecate `ValueAndSpan` usage in entities

## 📁 Documentation Files

| File | Description |
|------|-------------|
| `SUMMARY.md` | Previous analysis: why generic parameter is needed |
| `SPANNED_WRAPPER_SUMMARY.md` | **NEW** - Quick summary of Spanned wrapper benefits |
| `ENTITY_SPANNED_ARCHITECTURE.md` | **NEW** - Comprehensive architectural analysis |
| `edn/src/entities.rs` | **MODIFIED** - Added Spanned<T> type and Value support |

## 🎯 Recommendations

### Primary Recommendation
✅ **Keep generic parameter V** - Essential for architecture

### Secondary Recommendation  
✅ **Implement Spanned<Entity<Value>>** - Pure improvement, no downsides

### Together
Both recommendations work perfectly together:
```rust
Parser:  Spanned<Entity<Value>>      // Generic V + Spanned wrapper
Builder: Entity<TypedValue>           // Just generic V
```

## 💡 Key Insights

1. **Separation of Concerns**: Generic parameter enables edn crate independence
2. **Unused Complexity**: ValueAndSpan embeds spans that are never accessed
3. **Simpler is Better**: One span per entity sufficient for error reporting
4. **Zero Trade-offs**: Proposed architecture is strictly better

## 🚀 Next Steps

If approved:
1. Review proposed architecture
2. Implement parser changes
3. Update transaction processing
4. Test thoroughly
5. Deploy

## 📝 Notes

- This is an **exploration and analysis** PR
- Actual implementation can be done in follow-up PR
- No breaking changes to existing API (yet)
- All documentation written and ready

---

**Exploration Date:** 2026-01-31  
**Status:** Analysis Complete ✅  
**Recommendation:** Proceed with implementation
