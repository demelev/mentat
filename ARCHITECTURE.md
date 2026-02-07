# Mentat Architecture Deep Dive

**Last Updated:** February 2026

---

## Table of Contents
1. [System Architecture](#system-architecture)
2. [Component Breakdown](#component-breakdown)
3. [Data Flow](#data-flow)
4. [Query Pipeline](#query-pipeline)
5. [Transaction Pipeline](#transaction-pipeline)
6. [Storage Schema](#storage-schema)
7. [Performance Characteristics](#performance-characteristics)
8. [Extension Points](#extension-points)

---

## System Architecture

### High-Level View

```
┌─────────────────────────────────────────────────────────────┐
│                    Application Layer                         │
│          (User Code - Rust, Swift, Kotlin, C)               │
└────────────────────────┬────────────────────────────────────┘
                         │
            ┌────────────┼────────────┐
            ▼            ▼            ▼
      ┌─────────┐  ┌─────────┐  ┌──────────┐
      │   Rust  │  │  Swift  │  │  Kotlin  │
      │   API   │  │   SDK   │  │   SDK    │
      └────┬────┘  └────┬────┘  └────┬─────┘
           │            │             │
           │     ┌──────┴─────────────┘
           │     │
           ▼     ▼
      ┌─────────────────┐
      │    FFI Layer    │  (C-compatible interface)
      └────────┬────────┘
               │
               ▼
┌──────────────────────────────────────────────────────────────┐
│                   Mentat Core (Rust)                          │
│  ┌──────────────────────────────────────────────────────┐   │
│  │                  Public API Layer                     │   │
│  │   Store, Conn, QueryBuilder, InProgress, etc.        │   │
│  └──────────────┬───────────────────────────────────────┘   │
│                 │                                             │
│    ┌────────────┼───────────────┬─────────────────┐         │
│    ▼            ▼               ▼                 ▼         │
│ ┌──────┐  ┌──────────┐  ┌────────────┐  ┌─────────────┐   │
│ │Entity│  │Transaction│  │   Query    │  │  Database   │   │
│ │System│  │  Manager  │  │   Engine   │  │   Layer     │   │
│ └──┬───┘  └─────┬─────┘  └──────┬─────┘  └──────┬──────┘   │
│    │            │                │                │          │
│    └────────────┴────────────────┴────────────────┘          │
│                        │                                      │
│                        ▼                                      │
│             ┌──────────────────────┐                         │
│             │  Type System & Core  │                         │
│             │  (TypedValue, etc.)  │                         │
│             └──────────────────────┘                         │
└─────────────────────────┬─────────────────────────────────────┘
                          │
                          ▼
               ┌──────────────────────┐
               │   SQLite Backend     │
               │   (Persistent Store) │
               └──────────────────────┘
```

---

## Component Breakdown

### 1. Core Types (`mentat_core`)

**Purpose:** Fundamental data structures and type system

**Key Types:**
```rust
// Value types
pub enum ValueType {
    Ref,        // Entity reference
    Boolean,    // true/false
    Instant,    // Date/time
    Long,       // 64-bit integer
    Double,     // Floating point
    String,     // UTF-8 string
    Keyword,    // :namespace/name
    Uuid,       // UUID
}

// Typed values
pub enum TypedValue {
    Ref(Entid),
    Boolean(bool),
    Long(i64),
    Double(f64),
    String(Rc<String>),
    Keyword(Rc<Keyword>),
    Instant(DateTime<Utc>),
    Uuid(Uuid),
}

// Entity ID
pub type Entid = i64;

// Keywords (attributes)
pub struct Keyword {
    namespace: String,
    name: String,
}
```

**Responsibilities:**
- Type definitions
- Value conversions
- Validation
- Interning (string deduplication)

---

### 2. EDN Parser (`edn`)

**Purpose:** Parse Extensible Data Notation format

**Flow:**
```
Text → Tokenizer → Parser → EDN Values → Transaction Data
```

**Example:**
```edn
[{:db/id "tempid-1"
  :person/name "Alice"
  :person/age 30}]
```
↓
```rust
vec![
    HashMap {
        kw!(:db/id) => "tempid-1",
        kw!(:person/name) => "Alice",
        kw!(:person/age) => 30,
    }
]
```

---

### 3. Database Layer (`mentat_db`)

**Purpose:** Storage, schema, and indexing

#### Schema Structure
```rust
pub struct Schema {
    // Attribute definitions
    attributes: BTreeMap<Entid, Attribute>,
    // Keyword → Entid mapping
    ident_map: IdentMap,
    // Component attributes
    component_attributes: Vec<Entid>,
}

pub struct Attribute {
    value_type: ValueType,
    cardinality: Cardinality,  // One or Many
    unique: Option<Unique>,    // None, Value, or Identity
    index: bool,
    fulltext: bool,
    component: bool,
}
```

#### Storage Schema (SQLite)
```sql
-- Main datoms table (Entity-Attribute-Value-Transaction)
CREATE TABLE datoms (
    e INTEGER NOT NULL,  -- Entity ID
    a INTEGER NOT NULL,  -- Attribute ID
    v BLOB NOT NULL,     -- Value (typed)
    tx INTEGER NOT NULL, -- Transaction ID
    added BOOLEAN NOT NULL DEFAULT 1  -- 1=asserted, 0=retracted
);

-- Indexes
CREATE INDEX idx_avet ON datoms(a, v, e, tx);  -- Lookup by attribute+value
CREATE INDEX idx_vaet ON datoms(v, a, e, tx);  -- Lookup by value (refs)
CREATE INDEX idx_eavt ON datoms(e, a, v, tx);  -- Lookup by entity

-- Transaction metadata
CREATE TABLE transactions (
    tx INTEGER PRIMARY KEY,
    instant INTEGER NOT NULL  -- Timestamp
);

-- Partition map (ID allocation)
CREATE TABLE parts (
    part TEXT NOT NULL PRIMARY KEY,
    start INTEGER NOT NULL,
    end INTEGER NOT NULL
);

-- Schema cache
CREATE TABLE schema (
    ident TEXT NOT NULL PRIMARY KEY,
    attr INTEGER NOT NULL,
    value_type INTEGER NOT NULL,
    cardinality INTEGER NOT NULL,
    unique_value INTEGER,
    index_av INTEGER,
    index_vaet INTEGER,
    index_fulltext INTEGER,
    component INTEGER
);
```

#### Attribute Cache
```rust
pub struct AttributeCache {
    // Forward cache: Entid → Attribute
    forward: HashMap<Entid, Attribute>,
    // Reverse cache: Keyword → Entid
    reverse: HashMap<Keyword, Entid>,
    // Cached attribute values for fast lookup
    cached: HashMap<(Entid, TypedValue), Entid>,
}
```

---

### 4. Transaction System (`mentat_transaction`)

**Purpose:** ACID transaction management

#### Transaction Flow
```
1. begin_transaction()
   ↓
2. InProgress created (holds mutable state)
   ↓
3. transact(edn_string)
   ↓
4. Parse EDN → Entities
   ↓
5. Resolve temporary IDs
   ↓
6. Validate against schema
   ↓
7. Generate datoms (assertions/retractions)
   ↓
8. Insert into SQLite
   ↓
9. Update caches
   ↓
10. commit() or rollback()
    ↓
11. Notify observers (if committed)
```

#### InProgress Structure
```rust
pub struct InProgress<'a> {
    transaction: SQLiteTransaction<'a>,
    partition_map: PartitionMap,
    schema: Schema,
    cache: AttributeCache,
    use_caching: bool,
}
```

#### Entity Builder
```rust
// Fluent API for constructing entities
let mut builder = in_progress.builder();
builder
    .add_kw(&tempid, kw!(:person/name), "Alice")?
    .add_kw(&tempid, kw!(:person/age), 30)?
    .commit()?;
```

---

### 5. Query Engine

#### 5.1 Query Parser (`mentat_query`)

**Purpose:** Parse Datalog queries into AST

**Example Query:**
```clojure
[:find ?name ?age
 :in $
 :where
 [?person :person/name ?name]
 [?person :person/age ?age]
 [(> ?age 18)]]
```

**Parsed Structure:**
```rust
FindQuery {
    find_spec: FindRel(vec![?name, ?age]),
    in_vars: vec![$],
    where_clauses: vec![
        Pattern { e: ?person, a: :person/name, v: ?name },
        Pattern { e: ?person, a: :person/age, v: ?age },
        Predicate { operator: >, args: [?age, 18] },
    ],
}
```

#### 5.2 Algebrizer (`mentat_query_algebrizer`)

**Purpose:** Convert parsed query + schema → algebraic query plan

**Process:**
1. **Bind variables:** Assign types based on schema
2. **Resolve keywords:** Convert `:person/name` → Entid
3. **Apply constraints:** Filter based on where clauses
4. **Plan joins:** Determine optimal join order
5. **Generate Column Constraints:** SQL-level filters

**Output:** `AlgebraicQuery` (intermediate representation)

#### 5.3 SQL Generator (`mentat_query_sql`, `mentat_sql`)

**Purpose:** Convert algebraic query → SQL

**Example:**
```sql
-- Generated SQL for above query
SELECT datoms0.e AS ?person,
       datoms0.v AS ?name,
       datoms1.v AS ?age
FROM datoms AS datoms0,
     datoms AS datoms1
WHERE datoms0.a = 65536  -- :person/name attribute ID
  AND datoms1.a = 65537  -- :person/age attribute ID
  AND datoms0.e = datoms1.e  -- Same entity
  AND datoms1.v > 18  -- Age filter
```

#### 5.4 Projector (`mentat_query_projector`)

**Purpose:** Map SQL results → Datalog results

**Result Types:**
```rust
pub enum QueryResults {
    Scalar(Option<TypedValue>),           // Single value
    Tuple(Option<Vec<TypedValue>>),       // Single row
    Coll(Vec<TypedValue>),                // Column of values
    Rel(Vec<Vec<TypedValue>>),            // Multiple rows
}
```

**Projection Flow:**
```
SQL ResultSet → Raw Rows → Type Conversion → QueryResults
```

#### 5.5 Pull API (`mentat_query_pull`)

**Purpose:** Fetch entity with specified attributes

**Example:**
```rust
// Pull pattern
let pattern = "[*]";  // All attributes

// Or specific attributes
let pattern = "[:person/name :person/age]";

// With nested entities
let pattern = "[:order/id {:order/customer [:person/name]}]";

// Result
let entity_map: ValueMap = conn.pull_attributes_for_entity(
    entid,
    pattern,
)?;
```

---

### 6. Entity System (`mentat-entity`)

**Purpose:** ORM-like abstraction over Mentat

**Components:**
1. **Core Traits:** `Entity`, `EntityWrite`, `EntityRead`
2. **Schema Types:** `EntitySchema`, `FieldDefinition`
3. **Derive Macro:** `#[derive(Entity)]`

**Example:**
```rust
#[derive(Entity)]
#[entity(namespace = "person")]
struct Person {
    #[entity(unique = "identity")]
    email: String,
    name: String,
    age: Option<i64>,
}

// Generated schema
impl Entity for Person {
    fn schema() -> EntitySchema {
        EntitySchema {
            namespace: "person",
            fields: vec![
                FieldDefinition {
                    name: "email",
                    field_type: FieldType::String,
                    unique: Unique::Identity,
                    ...
                },
                ...
            ],
        }
    }
}

// Usage
let person = Person { ... };
let entid = person.write(&mut in_progress)?;
let loaded = Person::read(&in_progress, entid)?;
```

---

### 7. FFI Layer (`ffi`)

**Purpose:** C-compatible interface for other languages

**Key Patterns:**
```rust
// Opaque pointer pattern
#[repr(C)]
pub struct Store {
    // Hidden from C
}

// C-compatible functions
#[no_mangle]
pub extern "C" fn store_open(
    path: *const c_char,
    error: *mut ExternError,
) -> *mut Store {
    // Implementation
}

// Memory management
#[no_mangle]
pub extern "C" fn store_destroy(store: *mut Store) {
    // Safe deallocation
}
```

**Error Handling:**
```rust
#[repr(C)]
pub struct ExternError {
    pub message: *mut c_char,
    pub code: i32,
}
```

---

### 8. SDKs

#### Swift SDK
```swift
public class MentatStore {
    private let rawPointer: OpaquePointer
    
    public func transact(_ edn: String) throws -> TxReport {
        // Call FFI
    }
    
    public func query(_ datalog: String) throws -> QueryResults {
        // Call FFI
    }
}
```

#### Android SDK
```kotlin
class MentatStore(path: String) {
    private val pointer: Long
    
    fun transact(edn: String): TxReport {
        // JNI bridge to FFI
    }
    
    fun query(datalog: String): QueryResults {
        // JNI bridge to FFI
    }
}
```

---

## Data Flow

### Write Path
```
Application
    ↓
  Store.begin_transaction()
    ↓
  InProgress created
    ↓
  transact(edn_string)
    ↓
  EDN Parser
    ↓
  Transaction Processor
    ├→ Resolve temp IDs
    ├→ Validate schema
    └→ Generate datoms
    ↓
  SQLite Write
    ├→ Insert datoms
    ├→ Update indexes
    └→ Record transaction
    ↓
  Update Caches
    ↓
  commit()
    ↓
  Notify Observers
    ↓
  TxReport returned
```

### Read Path (Query)
```
Application
    ↓
  q_once(query_string)
    ↓
  Query Parser (Datalog → AST)
    ↓
  Algebrizer (AST + Schema → AlgebraicQuery)
    ↓
  SQL Generator (AlgebraicQuery → SQL)
    ↓
  SQLite Execution
    ↓
  Projector (SQL Results → Typed Results)
    ↓
  QueryResults returned
```

### Read Path (Pull)
```
Application
    ↓
  pull_attributes_for_entity(entid, pattern)
    ↓
  Parse pull pattern
    ↓
  Generate attribute queries
    ↓
  SQLite Lookup (indexed by entity)
    ↓
  Assemble ValueMap
    ↓
  ValueMap returned
```

---

## Performance Characteristics

### Indexing Strategy

**Three indexes for different access patterns:**

1. **AVET (Attribute-Value-Entity-Transaction)**
   - Fast lookup by attribute + value
   - Use case: Find entities with specific value
   - Example: Find all people named "Alice"

2. **VAET (Value-Attribute-Entity-Transaction)**
   - Fast reverse lookup for references
   - Use case: Find entities referencing this entity
   - Example: Find all orders for customer X

3. **EAVT (Entity-Attribute-Value-Transaction)**
   - Fast entity scan
   - Use case: Load all attributes of an entity
   - Example: Pull pattern `[*]`

### Query Optimization

**Strategies:**
- **Join Ordering:** Small result sets first
- **Index Selection:** Choose AVET vs VAET vs EAVT
- **Predicate Pushdown:** Apply filters early
- **Attribute Caching:** Frequently accessed attributes cached

### Transaction Performance

**Optimizations:**
- **Batch Inserts:** Multiple datoms in one transaction
- **Lazy ID Allocation:** Partition-based, no contention
- **Write-ahead Log:** SQLite WAL mode
- **Checkpoint Control:** Manual checkpoint for large writes

### Memory Usage

**Footprint:**
- **Schema Cache:** ~KB (loaded once)
- **Attribute Cache:** Configurable (LRU eviction)
- **Query Results:** Streaming where possible
- **Transaction Buffer:** Held in memory until commit

---

## Extension Points

### 1. Custom Attributes

Add new value types:
```rust
// Define custom type
enum CustomValueType {
    GeoPoint(f64, f64),
}

// Register with Mentat
// (requires core modification)
```

### 2. Transaction Functions

Custom logic during transactions:
```clojure
[:db/add tempid :db/fn custom-function]
```

### 3. Observers

React to data changes:
```rust
conn.register_observer(
    "my-observer",
    Arc::new(MyObserver),
);

impl TxObserver for MyObserver {
    fn on_tx(&mut self, tx_report: &TxReport) {
        // React to changes
    }
}
```

### 4. Custom Validation

Schema-level constraints:
```rust
// Via entity system
#[entity(validate = "my_validator")]
field: String,
```

---

## Comparison with Other Systems

### vs. SQLite

| Feature | Mentat | SQLite |
|---------|--------|--------|
| Schema | Flexible, attribute-based | Rigid, table-based |
| Queries | Datalog (declarative) | SQL |
| History | First-class (time-travel) | Requires audit tables |
| Relationships | Native (refs) | Foreign keys |
| Migrations | Rarely needed | Frequent |
| Learning Curve | Steep | Moderate |
| Performance | Good | Excellent |

### vs. Datomic

| Feature | Mentat | Datomic |
|---------|--------|---------|
| Deployment | Embedded | Client-server |
| Scale | Single device | Distributed |
| Cost | Free | Enterprise pricing |
| Maturity | Early | Mature |
| Maintenance | Community | Active (Cognitect) |
| Mobile | Native | Not designed for |

### vs. DataScript

| Feature | Mentat | DataScript |
|---------|--------|------------|
| Language | Rust | ClojureScript |
| Persistence | SQLite (durable) | Memory (ephemeral) |
| Performance | Fast | Very fast (in-memory) |
| Use Case | Embedded DB | App state management |
| FFI | Yes | No |

---

## Security Considerations

### Input Validation
- EDN parsing is safe (no code execution)
- Query parameters are bound (no SQL injection)
- Type validation at transaction time

### Encryption
- Optional SQLCipher support
- At-rest encryption
- Requires recompilation with feature flag

### Memory Safety
- Rust guarantees prevent buffer overflows
- FFI layer carefully audited
- No unsafe code in public APIs

---

## Future Architecture Considerations

### Potential Improvements

1. **Async I/O:**
   - Use `tokio` for non-blocking operations
   - Requires major refactoring

2. **Streaming Results:**
   - Iterator-based query results
   - Reduce memory footprint

3. **Distributed Mentat:**
   - CRDTs for conflict resolution
   - Peer-to-peer sync

4. **Advanced Indexing:**
   - Full-text search improvements
   - Geospatial indexes
   - Graph-specific indexes

5. **Query Optimizer:**
   - Cost-based optimization
   - Statistics gathering
   - Adaptive query plans

---

## Conclusion

Mentat's architecture is **well-designed and modular**, with clear separation of concerns:
- ✅ Type-safe from top to bottom
- ✅ Extensible through traits and features
- ✅ Multi-language support via FFI
- ✅ Solid storage foundation (SQLite)

**Key Strengths:**
- Innovative EAV model with time-travel
- Strong type system prevents bugs
- Clean abstraction layers

**Areas for Improvement:**
- Async I/O support
- Performance optimization
- Better documentation of internals

The architecture supports the proposed improvements in the IMPROVEMENT_PROPOSALS document without major refactoring.
