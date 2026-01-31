# Code Duplication Analysis: Parser::Entity vs Generic Entity<V>

## Side-by-Side Comparison

### Current Approach (Generic Entity<V>)

**Single definition in edn/src/entities.rs:**

```rust
// ============================================================
// SINGLE DEFINITION - Used by both parser and transaction
// ============================================================

#[derive(Clone, Debug, Eq, Hash, Ord, PartialOrd, PartialEq)]
pub enum Entity<V> {
    AddOrRetract {
        op: OpType,
        e: EntityPlace<V>,
        a: AttributePlace,
        v: ValuePlace<V>,
    },
    MapNotation(MapNotation<V>),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialOrd, PartialEq)]
pub enum EntityPlace<V> {
    Entid(EntidOrIdent),
    TempId(ValueRc<TempId>),
    LookupRef(LookupRef<V>),
    TxFunction(TxFunction),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialOrd, PartialEq)]
pub enum ValuePlace<V> {
    Entid(EntidOrIdent),
    TempId(ValueRc<TempId>),
    LookupRef(LookupRef<V>),
    TxFunction(TxFunction),
    Vector(Vec<ValuePlace<V>>),
    Atom(V),                    // <- Generic!
    MapNotation(MapNotation<V>),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialOrd, PartialEq)]
pub struct LookupRef<V> {
    pub a: AttributePlace,
    pub v: V,                   // <- Generic!
}

pub type MapNotation<V> = BTreeMap<EntidOrIdent, ValuePlace<V>>;

// ============================================================
// Usage:
// Parser: Entity<ValueAndSpan>
// Builder: Entity<TypedValue>
// ============================================================
```

**Lines of code:** ~50 lines

**Maintainability:** Change once, works everywhere

---

### Proposed Approach (Two Separate Types)

**Definition 1 in edn/src/entities.rs (parser module):**

```rust
// ============================================================
// PARSER TYPES - Must be maintained separately
// ============================================================

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
        Atom(ValueAndSpan),        // <- Concrete: ValueAndSpan
        MapNotation(MapNotation),
    }

    #[derive(Clone, Debug, Eq, Hash, Ord, PartialOrd, PartialEq)]
    pub struct LookupRef {
        pub a: AttributePlace,
        pub v: ValueAndSpan,       // <- Concrete: ValueAndSpan
    }

    pub type MapNotation = BTreeMap<EntidOrIdent, ValuePlace>;
    
    // Need From impls for each variant
    impl From<EntidOrIdent> for EntityPlace { ... }
    impl From<TempId> for EntityPlace { ... }
    impl From<ValueRc<TempId>> for EntityPlace { ... }
    impl From<LookupRef> for EntityPlace { ... }
    impl From<TxFunction> for EntityPlace { ... }
    
    impl From<EntidOrIdent> for ValuePlace { ... }
    impl From<TempId> for ValuePlace { ... }
    impl From<ValueRc<TempId>> for ValuePlace { ... }
    impl From<LookupRef> for ValuePlace { ... }
    impl From<TxFunction> for ValuePlace { ... }
    impl From<Vec<ValuePlace>> for ValuePlace { ... }
    impl From<ValueAndSpan> for ValuePlace { ... }
    impl From<MapNotation> for ValuePlace { ... }
}
```

**Definition 2 in core-traits/lib.rs or separate module:**

```rust
// ============================================================
// CORE TYPES - DUPLICATE of parser types!
// ============================================================

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
    Atom(TypedValue),             // <- Concrete: TypedValue
    MapNotation(MapNotation),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialOrd, PartialEq)]
pub struct LookupRef {
    pub a: AttributePlace,
    pub v: TypedValue,            // <- Concrete: TypedValue
}

pub type MapNotation = BTreeMap<EntidOrIdent, ValuePlace>;

// Need From impls for each variant (DUPLICATED!)
impl From<EntidOrIdent> for EntityPlace { ... }
impl From<TempId> for EntityPlace { ... }
impl From<ValueRc<TempId>> for EntityPlace { ... }
impl From<LookupRef> for EntityPlace { ... }
impl From<TxFunction> for EntityPlace { ... }

impl From<EntidOrIdent> for ValuePlace { ... }
impl From<TempId> for ValuePlace { ... }
impl From<ValueRc<TempId>> for ValuePlace { ... }
impl From<LookupRef> for ValuePlace { ... }
impl From<TxFunction> for ValuePlace { ... }
impl From<Vec<ValuePlace>> for ValuePlace { ... }
impl From<TypedValue> for ValuePlace { ... }
impl From<MapNotation> for ValuePlace { ... }
```

**Conversion logic (also needed):**

```rust
// ============================================================
// CONVERSION LAYER - Additional complexity
// ============================================================

impl TryFrom<edn::entities::parser::Entity> for core_traits::Entity {
    type Error = DbError;
    
    fn try_from(e: edn::entities::parser::Entity) -> Result<Self> {
        match e {
            edn::entities::parser::Entity::AddOrRetract { op, e, a, v } => {
                Ok(core_traits::Entity::AddOrRetract {
                    op,
                    e: e.try_into()?,
                    a,
                    v: v.try_into()?,
                })
            }
            edn::entities::parser::Entity::MapNotation(map) => {
                let mut new_map = BTreeMap::new();
                for (k, v) in map {
                    new_map.insert(k, v.try_into()?);
                }
                Ok(core_traits::Entity::MapNotation(new_map))
            }
        }
    }
}

impl TryFrom<edn::entities::parser::EntityPlace> for core_traits::EntityPlace {
    type Error = DbError;
    
    fn try_from(e: edn::entities::parser::EntityPlace) -> Result<Self> {
        match e {
            edn::entities::parser::EntityPlace::Entid(x) => 
                Ok(core_traits::EntityPlace::Entid(x)),
            edn::entities::parser::EntityPlace::TempId(x) => 
                Ok(core_traits::EntityPlace::TempId(x)),
            edn::entities::parser::EntityPlace::LookupRef(x) => 
                Ok(core_traits::EntityPlace::LookupRef(x.try_into()?)),
            edn::entities::parser::EntityPlace::TxFunction(x) => 
                Ok(core_traits::EntityPlace::TxFunction(x)),
        }
    }
}

impl TryFrom<edn::entities::parser::ValuePlace> for core_traits::ValuePlace {
    type Error = DbError;
    
    fn try_from(v: edn::entities::parser::ValuePlace) -> Result<Self> {
        match v {
            edn::entities::parser::ValuePlace::Entid(x) => 
                Ok(core_traits::ValuePlace::Entid(x)),
            edn::entities::parser::ValuePlace::TempId(x) => 
                Ok(core_traits::ValuePlace::TempId(x)),
            edn::entities::parser::ValuePlace::LookupRef(x) => 
                Ok(core_traits::ValuePlace::LookupRef(x.try_into()?)),
            edn::entities::parser::ValuePlace::TxFunction(x) => 
                Ok(core_traits::ValuePlace::TxFunction(x)),
            edn::entities::parser::ValuePlace::Vector(vec) => {
                let new_vec = vec.into_iter()
                    .map(|v| v.try_into())
                    .collect::<Result<Vec<_>>>()?;
                Ok(core_traits::ValuePlace::Vector(new_vec))
            }
            edn::entities::parser::ValuePlace::Atom(vs) => {
                // Need schema access here to convert ValueAndSpan -> TypedValue
                let typed = schema.to_typed_value(&vs, value_type)?;
                Ok(core_traits::ValuePlace::Atom(typed))
            }
            edn::entities::parser::ValuePlace::MapNotation(map) => {
                let mut new_map = BTreeMap::new();
                for (k, v) in map {
                    new_map.insert(k, v.try_into()?);
                }
                Ok(core_traits::ValuePlace::MapNotation(new_map))
            }
        }
    }
}

impl TryFrom<edn::entities::parser::LookupRef> for core_traits::LookupRef {
    type Error = DbError;
    
    fn try_from(lr: edn::entities::parser::LookupRef) -> Result<Self> {
        // Need schema access to convert ValueAndSpan -> TypedValue
        let typed = schema.to_typed_value(&lr.v, value_type)?;
        Ok(core_traits::LookupRef {
            a: lr.a,
            v: typed,
        })
    }
}
```

**Lines of code:** ~300+ lines (100 for parser types + 100 for core types + 100 for conversions)

**Maintainability:** Any structural change requires updating 3 places

---

## Code Duplication Metrics

| Aspect | Generic Entity<V> | Two Separate Types |
|--------|-------------------|-------------------|
| **Type definitions** | 1 set (50 LOC) | 2 sets (200 LOC) |
| **From implementations** | 1 set (50 LOC) | 2 sets (100 LOC) |
| **Conversion logic** | Trait-based (exists) | Explicit (100 LOC) |
| **Total new code** | 0 LOC | ~300 LOC |
| **Places to update** | 1 | 3 |

## Maintenance Scenarios

### Scenario 1: Add new field to Entity

**Generic approach:**
```rust
// Change in ONE place
pub enum Entity<V> {
    AddOrRetract {
        op: OpType,
        e: EntityPlace<V>,
        a: AttributePlace,
        v: ValuePlace<V>,
        tx: Option<Entid>,  // <- Add this
    },
    MapNotation(MapNotation<V>),
}
```

**Two-type approach:**
```rust
// Change in THREE places:
// 1. edn/src/entities.rs parser module
pub enum Entity {
    AddOrRetract {
        op: OpType,
        e: EntityPlace,
        a: AttributePlace,
        v: ValuePlace,
        tx: Option<Entid>,  // <- Add here
    },
    MapNotation(MapNotation),
}

// 2. core-traits/lib.rs
pub enum Entity {
    AddOrRetract {
        op: OpType,
        e: EntityPlace,
        a: AttributePlace,
        v: ValuePlace,
        tx: Option<Entid>,  // <- And here
    },
    MapNotation(MapNotation),
}

// 3. Conversion logic
impl TryFrom<edn::entities::parser::Entity> for core_traits::Entity {
    fn try_from(e: edn::entities::parser::Entity) -> Result<Self> {
        match e {
            edn::entities::parser::Entity::AddOrRetract { op, e, a, v, tx } => {
                Ok(core_traits::Entity::AddOrRetract {
                    op,
                    e: e.try_into()?,
                    a,
                    v: v.try_into()?,
                    tx,  // <- And handle here
                })
            }
            // ...
        }
    }
}
```

### Scenario 2: Add new ValuePlace variant

**Generic approach:**
```rust
// Change in ONE place
pub enum ValuePlace<V> {
    // ... existing variants
    Range(V, V),  // <- Add new variant
}

// Add From impl
impl<V: TransactableValueMarker> From<(V, V)> for ValuePlace<V> {
    fn from((start, end): (V, V)) -> Self {
        ValuePlace::Range(start, end)
    }
}
```

**Two-type approach:**
```rust
// Change in FIVE places:
// 1. Parser ValuePlace
pub enum ValuePlace {
    // ... existing variants
    Range(ValueAndSpan, ValueAndSpan),
}

// 2. Core ValuePlace
pub enum ValuePlace {
    // ... existing variants
    Range(TypedValue, TypedValue),
}

// 3. Parser From impl
impl From<(ValueAndSpan, ValueAndSpan)> for parser::ValuePlace {
    fn from((start, end): (ValueAndSpan, ValueAndSpan)) -> Self {
        parser::ValuePlace::Range(start, end)
    }
}

// 4. Core From impl
impl From<(TypedValue, TypedValue)> for core_traits::ValuePlace {
    fn from((start, end): (TypedValue, TypedValue)) -> Self {
        core_traits::ValuePlace::Range(start, end)
    }
}

// 5. Conversion logic
impl TryFrom<parser::ValuePlace> for core_traits::ValuePlace {
    fn try_from(v: parser::ValuePlace) -> Result<Self> {
        match v {
            // ... existing variants
            parser::ValuePlace::Range(start, end) => {
                Ok(core_traits::ValuePlace::Range(
                    schema.to_typed_value(&start, vtype)?,
                    schema.to_typed_value(&end, vtype)?,
                ))
            }
        }
    }
}
```

## Conclusion

The proposed approach would:
- **Add ~300 lines of duplicated code**
- **Require changes in 3-5 places** for any structural modification
- **Introduce conversion bugs** (easy to forget a field)
- **Create import confusion** (which Entity to use?)
- **Provide no tangible benefits**

The current generic approach is **vastly superior** for maintainability and simplicity.
