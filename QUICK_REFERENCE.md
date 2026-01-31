# Entity Architecture - Quick Reference

## Three Questions, Three Answers

### Question 1: Generic Parameter? ✅ YES

```rust
// CURRENT & RECOMMENDED
pub enum Entity<V> {
    AddOrRetract { e, a, v },
    MapNotation(...),
}

// Used as:
Entity<ValueAndSpan>  // Parser
Entity<TypedValue>    // Builder
```

**Why:** Single source of truth, no duplication, proven design

**Details:** SUMMARY.md, ANALYSIS.md

---

### Question 2: Spanned Wrapper? ✅ YES (Optional Enhancement)

```rust
// CURRENT
Entity<ValueAndSpan> {
    v: ValuePlace::Atom(ValueAndSpan {
        inner: Text("x"),
        span: Span(15, 18),  // 10-20 spans per entity
    })
}

// PROPOSED (Better)
Spanned<Entity<Value>> {
    inner: Entity {
        v: ValuePlace::Atom(Value::Text("x"))
    },
    span: Span(0, 25),  // 1 span per entity
}
```

**Why:** 90% reduction in span storage, simpler, same functionality

**Details:** SPANNED_WRAPPER_SUMMARY.md, ARCHITECTURE_VISUAL.md

---

### Question 3: Split Types? ❌ NO

```rust
// PROPOSED (Bad)
// In edn crate:
pub mod parser {
    pub enum Entity { ... }  // Uses ValueAndSpan
}

// In core-traits:
pub enum Entity { ... }      // Uses TypedValue

// Conversion:
impl TryFrom<parser::Entity> for Entity { ... }
```

**Why NOT:**
- 300+ lines of duplicated code (6x current)
- Update 3-5 places for any change
- Import confusion
- Zero benefits

**Details:** PARSER_ENTITY_ANALYSIS.md, CODE_DUPLICATION_ANALYSIS.md

---

## Quick Comparison

| Metric | Generic | + Spanned | Split Types |
|--------|---------|-----------|-------------|
| **LOC** | 50 | 60 | 300+ |
| **Duplication** | None | None | Massive |
| **Maintenance** | Easy | Easy | Hard |
| **Verdict** | ✅ Keep | ✅ Add | ❌ Reject |

## Code Impact

### Adding a field to Entity

**Generic / Spanned (1 change):**
```rust
pub enum Entity<V> {
    AddOrRetract {
        op, e, a, v,
        tx: Option<Entid>,  // <- Add here
    }
}
```

**Split Types (3 changes):**
```rust
// 1. Parser type
parser::Entity::AddOrRetract { ..., tx }

// 2. Core type  
core::Entity::AddOrRetract { ..., tx }

// 3. Conversion
impl TryFrom<parser::Entity> for core::Entity {
    // Handle tx field in conversion
}
```

## Recommendations

1. ✅ **Keep `Entity<V>`** - don't change it
2. ✅ **Consider `Spanned<Entity<Value>>`** - optional improvement  
3. ❌ **Do NOT split types** - creates problems

## Learn More

- **Executive Summary:** FINAL_SUMMARY.md
- **Full Comparison:** THREE_APPROACHES_COMPARISON.md
- **Visual Diagrams:** ARCHITECTURE_VISUAL.md
- **Navigation:** INDEX.md

---

**TL;DR:** Current generic design is excellent. Optional Spanned wrapper can improve span handling. Split types would be harmful.
