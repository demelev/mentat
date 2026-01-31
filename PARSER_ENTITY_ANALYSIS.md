# Parser::Entity to Non-Generic Entity Architecture Analysis

## Problem Statement

> "what if use in edn parser not Entity but Parser::Entity and then convert it to enum Entity which uses inside TypedValue type, without generic types."

## Current Architecture

### Crate Dependencies

```
edn (no dependencies on mentat types)
  ↓
core-traits (defines TypedValue)
  ↓
db, transaction (use Entity<TypedValue>)
```

### Current Entity Types

```rust
// In edn/src/entities.rs
pub enum Entity<V> {
    AddOrRetract { e, a, v },
    MapNotation(...),
}
pub enum EntityPlace<V> { ... }
pub enum ValuePlace<V> { ... }
```

### Current Usage

1. **Parser**: Produces `Entity<ValueAndSpan>`
2. **Transaction builder**: Uses `Entity<TypedValue>`
3. **Generic parameter V**: Allows both without duplication

## Proposed Architecture

### Option 1: Two Separate Entity Types

```rust
// In edn/src/entities.rs (parser module)
pub mod parser {
    pub enum Entity {
        AddOrRetract {
            e: EntityPlace,
            a: AttributePlace,
            v: ValuePlace,
        },
        MapNotation(...),
    }
    // EntityPlace, ValuePlace use ValueAndSpan
}

// In core-traits/lib.rs or new module
pub enum Entity {
    AddOrRetract {
        e: EntityPlace,
        a: AttributePlace,
        v: ValuePlace,
    },
    MapNotation(...),
}
// EntityPlace, ValuePlace use TypedValue
```

### Conversion Layer

```rust
// In db/src/internal_types.rs
impl TryFrom<edn::entities::parser::Entity> for core_traits::Entity {
    fn try_from(parser_entity: edn::entities::parser::Entity) -> Result<Self> {
        // Convert ValueAndSpan -> TypedValue
        // Handle schema validation
    }
}
```

## Analysis

### Benefits

1. ✅ **No Generic Parameters**
   - Simpler type signatures
   - No generic constraint propagation
   - Clearer separation of concerns

2. ✅ **Clear Type Separation**
   - Parser types explicitly parser-specific
   - Core types explicitly use TypedValue
   - No confusion about which V to use

3. ✅ **Potential Performance**
   - No monomorphization overhead (though likely minimal)
   - Direct type usage

### Drawbacks

1. ❌ **Code Duplication**
   - Two nearly identical Entity definitions
   - Two sets of EntityPlace, ValuePlace, etc.
   - Maintenance burden: changes need to be applied twice

2. ❌ **Conversion Complexity**
   - Need explicit conversion logic
   - More opportunities for bugs
   - Conversion can fail (error handling)

3. ❌ **Import Confusion**
   - `edn::entities::Entity` vs `core_traits::Entity`
   - Need to be careful about which to import
   - More verbose imports

4. ❌ **Breaking Change**
   - All existing code uses `Entity<V>`
   - Large refactoring needed
   - High risk

### Comparison with Generic Approach

| Aspect | Generic Entity<V> | Two Separate Types |
|--------|-------------------|-------------------|
| **Code duplication** | None | High |
| **Type complexity** | Medium (generics) | Low (concrete) |
| **Conversion logic** | Trait-based | Explicit |
| **Maintainability** | High | Medium |
| **Import clarity** | Medium | Low |
| **Breaking change** | No | Yes |

## Detailed Design

### If We Proceed with This Approach

#### Step 1: Create Parser Entity Types

```rust
// edn/src/entities.rs
pub mod parser {
    use super::*;
    
    #[derive(Clone, Debug, Eq, Hash, Ord, PartialOrd, PartialEq)]
    pub enum Entity {
        AddOrRetract {
            op: OpType,
            e: EntityPlace,
            a: AttributePlace,
            v: ValuePlace,
        },
        MapNotation(MapNotation),
    }
    
    #[derive(Clone, Debug, Eq, Hash, Ord, PartialOrd, PartialEq)]
    pub enum EntityPlace {
        Entid(EntidOrIdent),
        TempId(ValueRc<TempId>),
        LookupRef(LookupRef),
        TxFunction(TxFunction),
    }
    
    #[derive(Clone, Debug, Eq, Hash, Ord, PartialOrd, PartialEq)]
    pub enum ValuePlace {
        Entid(EntidOrIdent),
        TempId(ValueRc<TempId>),
        LookupRef(LookupRef),
        TxFunction(TxFunction),
        Vector(Vec<ValuePlace>),
        Atom(ValueAndSpan),  // Parser uses ValueAndSpan
        MapNotation(MapNotation),
    }
    
    #[derive(Clone, Debug, Eq, Hash, Ord, PartialOrd, PartialEq)]
    pub struct LookupRef {
        pub a: AttributePlace,
        pub v: ValueAndSpan,  // Parser uses ValueAndSpan
    }
    
    pub type MapNotation = BTreeMap<EntidOrIdent, ValuePlace>;
}
```

#### Step 2: Create Core Entity Types

```rust
// core-traits/lib.rs or new entities.rs module
use edn::entities::{AttributePlace, EntidOrIdent, OpType, TempId, TxFunction};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialOrd, PartialEq)]
pub enum Entity {
    AddOrRetract {
        op: OpType,
        e: EntityPlace,
        a: AttributePlace,
        v: ValuePlace,
    },
    MapNotation(MapNotation),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialOrd, PartialEq)]
pub enum EntityPlace {
    Entid(EntidOrIdent),
    TempId(ValueRc<TempId>),
    LookupRef(LookupRef),
    TxFunction(TxFunction),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialOrd, PartialEq)]
pub enum ValuePlace {
    Entid(EntidOrIdent),
    TempId(ValueRc<TempId>),
    LookupRef(LookupRef),
    TxFunction(TxFunction),
    Vector(Vec<ValuePlace>),
    Atom(TypedValue),  // Core uses TypedValue
    MapNotation(MapNotation),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialOrd, PartialEq)]
pub struct LookupRef {
    pub a: AttributePlace,
    pub v: TypedValue,  // Core uses TypedValue
}

pub type MapNotation = BTreeMap<EntidOrIdent, ValuePlace>;
```

#### Step 3: Implement Conversion

```rust
// db/src/internal_types.rs
impl TryFrom<edn::entities::parser::Entity> for core_traits::Entity {
    type Error = DbError;
    
    fn try_from(parser_entity: edn::entities::parser::Entity) -> Result<Self> {
        match parser_entity {
            edn::entities::parser::Entity::AddOrRetract { op, e, a, v } => {
                Ok(core_traits::Entity::AddOrRetract {
                    op,
                    e: e.try_into()?,
                    a,
                    v: v.try_into()?,
                })
            }
            edn::entities::parser::Entity::MapNotation(map) => {
                // Convert each value in map
                let converted_map = map.into_iter()
                    .map(|(k, v)| Ok((k, v.try_into()?)))
                    .collect::<Result<_>>()?;
                Ok(core_traits::Entity::MapNotation(converted_map))
            }
        }
    }
}

// Similar implementations for EntityPlace, ValuePlace, LookupRef
```

## Key Issues

### Issue 1: Dependency Cycle Risk

Core-traits would need to import from edn for:
- `OpType`, `AttributePlace`, `EntidOrIdent`, `TempId`, `TxFunction`

But edn can't import from core-traits. This could work if:
- These shared types stay in edn
- Core-traits only adds the concrete Entity using TypedValue

### Issue 2: Massive Code Duplication

Almost identical code in two places:
- edn/src/entities.rs (parser module)
- core-traits/lib.rs (or new module)

Only difference: `ValueAndSpan` vs `TypedValue` in Atom and LookupRef.

### Issue 3: Conversion is Not Trivial

Converting ValueAndSpan → TypedValue requires:
- Schema lookup for type information
- Value validation
- Error handling with span information

This conversion already happens via the `TransactableValue` trait, which is well-designed for this purpose.

## Alternative: Enhanced Spanned Wrapper

Instead of two Entity types, use the Spanned wrapper from previous analysis:

```rust
// Parser produces
Spanned<Entity<Value>>

// Transaction builder uses
Entity<TypedValue>

// Still generic, but simpler than ValueAndSpan everywhere
```

This keeps the generic but makes it cleaner (as explored in previous analysis).

## Recommendation

**Do NOT implement the Parser::Entity approach** because:

1. ❌ Massive code duplication (2x Entity definitions)
2. ❌ Complex conversion logic
3. ❌ Breaking changes throughout codebase
4. ❌ Higher maintenance burden
5. ❌ Import confusion
6. ❌ No significant benefits over current generic approach

**Instead:**
- ✅ Keep generic `Entity<V>` (proven, working, clean)
- ✅ OR implement `Spanned<Entity<Value>>` (simpler than ValueAndSpan)
- ✅ Current architecture is actually well-designed

## Conclusion

The current generic approach is **better** than having two separate Entity types because:
- Single source of truth for Entity structure
- Trait-based conversion is clean and extensible
- No code duplication
- No import confusion
- Proven and working

The proposal to split Entity into parser and core versions would:
- Duplicate significant code
- Require complex conversion logic
- Create import confusion
- Provide no tangible benefits

**Verdict: Do not implement this proposal. Keep the current generic approach.**
