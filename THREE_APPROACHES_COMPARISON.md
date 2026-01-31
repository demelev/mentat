# Entity Architecture: Three Approaches Compared

## Overview

This document compares three architectural approaches for the Entity type in Mentat:

1. **Current: Generic Entity<V>** (Status quo)
2. **Proposed A: Spanned<Entity<Value>>** (Wrapper approach)
3. **Proposed B: Parser::Entity → Entity** (Split types approach)

## Approach 1: Generic Entity<V> (Current)

### Architecture

```rust
// Single generic definition
pub enum Entity<V> {
    AddOrRetract {
        op: OpType,
        e: EntityPlace<V>,
        a: AttributePlace,
        v: ValuePlace<V>,
    },
    MapNotation(MapNotation<V>),
}

// Used with different V:
// Parser: Entity<ValueAndSpan>
// Builder: Entity<TypedValue>
```

### Metrics

| Metric | Value |
|--------|-------|
| Lines of code | 50 (base types) |
| Type definitions | 1 set |
| Conversion mechanism | Trait-based |
| Code duplication | None |
| Maintenance burden | Low |

### Pros

✅ **Single source of truth** - one definition for all uses  
✅ **No code duplication** - changes apply everywhere  
✅ **Elegant trait-based conversion** - TransactableValue trait  
✅ **Clear generic abstraction** - well-understood Rust pattern  
✅ **Proven and working** - used in production  
✅ **Easy to maintain** - change once, works everywhere  

### Cons

⚠️ **Generic complexity** - requires understanding of generics  
⚠️ **Type signatures** - can be verbose with V parameter  

### Verdict

✅ **RECOMMENDED** - Well-designed, proven, maintainable

---

## Approach 2: Spanned<Entity<Value>> (Wrapper)

### Architecture

```rust
// Add wrapper for source location
pub struct Spanned<T> {
    pub inner: T,
    pub span: Span,
}

// Parser produces:
Spanned<Entity<Value>>  // Plain Value, wrapped in Spanned

// Builder uses:
Entity<TypedValue>  // No wrapper needed
```

### Metrics

| Metric | Value |
|--------|-------|
| Lines of code | 50 (base) + 10 (wrapper) |
| Type definitions | 1 set + wrapper |
| Conversion mechanism | Trait-based + unwrap |
| Code duplication | None |
| Maintenance burden | Low |

### Pros

✅ **Single source of truth** - still one Entity definition  
✅ **No code duplication** - generic still used  
✅ **Simpler than ValueAndSpan** - one span per entity instead of per value  
✅ **Same functionality** - spans available but not nested  
✅ **90% reduction in span storage** - measured benefit  
✅ **Backward compatible** - can be added gradually  

### Cons

⚠️ **Still uses generics** - Entity<Value> vs Entity<TypedValue>  
⚠️ **Additional wrapper type** - one more concept to understand  

### Verdict

✅ **RECOMMENDED** - Pure improvement over current ValueAndSpan nesting

**Analysis documents:**
- SPANNED_WRAPPER_SUMMARY.md
- ENTITY_SPANNED_ARCHITECTURE.md
- ARCHITECTURE_VISUAL.md

---

## Approach 3: Parser::Entity → Entity (Split Types)

### Architecture

```rust
// Parser types (in edn crate)
pub mod parser {
    pub enum Entity { ... }          // Uses ValueAndSpan
    pub enum EntityPlace { ... }     // Uses ValueAndSpan
    pub enum ValuePlace { ... }      // Uses ValueAndSpan
    pub struct LookupRef { ... }     // Uses ValueAndSpan
}

// Core types (in core-traits)
pub enum Entity { ... }              // Uses TypedValue
pub enum EntityPlace { ... }         // Uses TypedValue
pub enum ValuePlace { ... }          // Uses TypedValue
pub struct LookupRef { ... }         // Uses TypedValue

// Explicit conversion
impl TryFrom<parser::Entity> for Entity { ... }
impl TryFrom<parser::EntityPlace> for EntityPlace { ... }
impl TryFrom<parser::ValuePlace> for ValuePlace { ... }
impl TryFrom<parser::LookupRef> for LookupRef { ... }
```

### Metrics

| Metric | Value |
|--------|-------|
| Lines of code | 300+ (100 parser + 100 core + 100 conversion) |
| Type definitions | 2 sets (duplicated) |
| Conversion mechanism | Explicit TryFrom impls |
| Code duplication | Massive (2x everything) |
| Maintenance burden | Very High |

### Pros

➖ **No generic parameter** - types are concrete  
➖ **Clear separation** - parser vs core types explicit  

### Cons

❌ **Massive code duplication** - 6x more code than current  
❌ **High maintenance burden** - update 3-5 places for any change  
❌ **Conversion complexity** - 100+ lines of error-prone conversion code  
❌ **Import confusion** - which Entity to import?  
❌ **Consistency risk** - easy to make parser and core types diverge  
❌ **No tangible benefits** - doesn't enable anything new  
❌ **Breaking change** - requires massive refactoring  

### Verdict

❌ **NOT RECOMMENDED** - Adds complexity without benefits

**Analysis documents:**
- PARSER_ENTITY_ANALYSIS.md
- CODE_DUPLICATION_ANALYSIS.md

---

## Comparison Table

| Aspect | Generic Entity<V> | Spanned Wrapper | Split Types |
|--------|-------------------|-----------------|-------------|
| **Lines of code** | 50 | 60 | 300+ |
| **Code duplication** | None | None | Massive |
| **Maintenance** | Easy | Easy | Hard |
| **Type complexity** | Medium | Medium | High |
| **Conversion logic** | Trait | Trait + unwrap | Explicit |
| **Breaking changes** | None | Minor | Major |
| **Import clarity** | Good | Good | Poor |
| **Benefits** | Proven | Simpler spans | None |
| **Risks** | None | Low | High |

## Code Example Comparison

### Adding a new field to Entity

**Generic approach:**
```rust
// Change in ONE place ✅
pub enum Entity<V> {
    AddOrRetract {
        // ... existing fields
        tx: Option<Entid>,  // Add this
    }
}
```

**Spanned wrapper:**
```rust
// Change in ONE place ✅
pub enum Entity<V> {
    AddOrRetract {
        // ... existing fields
        tx: Option<Entid>,  // Add this
    }
}
// Spanned wrapper doesn't need changes
```

**Split types:**
```rust
// Change in THREE places ❌

// 1. Parser type
pub mod parser {
    pub enum Entity {
        AddOrRetract {
            // ... existing fields
            tx: Option<Entid>,  // Add here
        }
    }
}

// 2. Core type
pub enum Entity {
    AddOrRetract {
        // ... existing fields
        tx: Option<Entid>,  // Add here
    }
}

// 3. Conversion logic
impl TryFrom<parser::Entity> for Entity {
    fn try_from(e: parser::Entity) -> Result<Self> {
        match e {
            parser::Entity::AddOrRetract { ..., tx } => {
                Ok(Entity::AddOrRetract {
                    // ... existing fields
                    tx,  // Handle here
                })
            }
        }
    }
}
```

## Recommendations

### Primary Recommendation: Keep Current Generic Approach ✅

The current `Entity<V>` is well-designed and should remain as-is because:
- Single source of truth
- No code duplication
- Proven in production
- Easy to maintain

### Secondary Recommendation: Consider Spanned Wrapper ✅

The `Spanned<Entity<Value>>` approach is a pure improvement:
- Simplifies span handling
- Reduces code verbosity
- Same functionality
- Backward compatible addition

See implementation plan in: ENTITY_SPANNED_ARCHITECTURE.md

### Strong Anti-Recommendation: Do NOT Split Types ❌

The `Parser::Entity → Entity` approach should NOT be implemented:
- 6x more code
- High maintenance burden
- No benefits
- Breaking changes required

See detailed analysis in: PARSER_ENTITY_ANALYSIS.md

## Timeline of Exploration

1. **Initial Question** (SUMMARY.md)
   - "Do we need generic parameter?"
   - Answer: YES, it's essential

2. **Spanned Wrapper** (SPANNED_WRAPPER_SUMMARY.md)
   - "What if wrap into Spanned(Entity)?"
   - Answer: YES, good idea - implement it

3. **Split Types** (PARSER_ENTITY_ANALYSIS.md)
   - "What if Parser::Entity → Entity?"
   - Answer: NO, creates problems without benefits

## Conclusion

After thorough exploration of three approaches:

✅ **Current generic approach is best** - keep it  
✅ **Spanned wrapper is improvement** - consider implementing  
❌ **Split types approach is harmful** - do not implement  

The generic `Entity<V>` is a well-designed abstraction that should not be replaced. The `Spanned<Entity<Value>>` wrapper can be added as an enhancement, but the core generic design should remain.

---

**Exploration Complete**: 2026-01-31  
**Status**: All approaches analyzed and documented  
**Recommendation**: Status quo (+ optional Spanned wrapper)
