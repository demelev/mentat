# Mentat Improvement Proposals (MIPs)

This document contains detailed, actionable improvement proposals for the Mentat project.

---

## MIP-001: Build System Modernization

**Status:** Proposed  
**Priority:** P0 (Critical)  
**Estimated Effort:** 2-3 weeks  
**User Value:** Unblocks all usage

### Problem Statement
The current build system has critical issues:
- Tolstoy (sync) crate fails to compile with missing dependencies
- Multiple crates use Rust 2015 edition (should be 2018+)
- 42+ compiler warnings in mentat_db alone
- Inconsistent edition usage across workspace

### Proposed Solution

#### Phase 1: Fix Tolstoy Dependencies (Week 1)
```toml
# tolstoy/Cargo.toml additions
[dependencies]
tokio = { version = "1.0", features = ["full"] }
reqwest = { version = "0.11", features = ["json"] }
futures = "0.3"
```

#### Phase 2: Update Rust Editions (Week 1)
Update all Cargo.toml files to use `edition = "2021"`:
- [ ] core/Cargo.toml
- [ ] db/Cargo.toml
- [ ] edn/Cargo.toml
- [ ] query-algebrizer/Cargo.toml
- [ ] query-projector/Cargo.toml
- [ ] query-sql/Cargo.toml
- [ ] sql/Cargo.toml
- [ ] transaction/Cargo.toml
- [ ] tolstoy/Cargo.toml
- [ ] ffi/Cargo.toml
- [ ] All *-traits crates

#### Phase 3: Update Dependencies (Week 2)
```toml
# Priority dependency updates
uuid = "1.20"          # was 0.5
chrono = "0.4"         # already current
rusqlite = "0.38"      # already current but check for newer
time = "0.3"           # was 0.1
futures = "0.3"        # was 0.1
hyper = "1.0"          # was 0.11 (if still needed)
```

#### Phase 4: Resolve Warnings (Week 2-3)
- [ ] Fix unused imports
- [ ] Fix unused variables
- [ ] Fix deprecated API usage
- [ ] Run `cargo clippy --fix`
- [ ] Enable `deny(warnings)` in CI

#### Phase 5: CI/CD Setup (Week 3)
```yaml
# .github/workflows/ci.yml
name: CI
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --all-features
      - run: cargo test --all-features
      - run: cargo clippy -- -D warnings
```

### Success Criteria
- [ ] `cargo build --all-features` succeeds
- [ ] All tests pass
- [ ] Zero compiler warnings
- [ ] CI runs on all PRs

### Breaking Changes
None expected (internal improvements only)

---

## MIP-002: Quick Start Documentation

**Status:** Proposed  
**Priority:** P0 (Critical)  
**Estimated Effort:** 1 week  
**User Value:** Enables onboarding

### Problem Statement
New users cannot get started due to:
- No clear entry point in documentation
- Examples are scattered or outdated
- Concepts are assumed (Datalog, EDN, EAV)
- No "Hello World" equivalent

### Proposed Solution

#### Create docs/quick-start.md
```markdown
# Quick Start Guide

## Installation
\`\`\`toml
[dependencies]
mentat = "0.13"
\`\`\`

## Your First Database (5 minutes)

### 1. Create a Store
\`\`\`rust
use mentat::{Store, new_connection};

let mut store = Store::open("my_db.db")?;
\`\`\`

### 2. Define Schema
\`\`\`rust
let schema = r#"
[
  {:db/ident       :person/name
   :db/valueType   :db.type/string
   :db/cardinality :db.cardinality/one}
  
  {:db/ident       :person/age
   :db/valueType   :db.type/long
   :db/cardinality :db.cardinality/one}
]
"#;

let mut in_progress = store.begin_transaction()?;
in_progress.transact(schema)?;
in_progress.commit()?;
\`\`\`

### 3. Insert Data
\`\`\`rust
let mut in_progress = store.begin_transaction()?;
in_progress.transact(r#"
[
  {:person/name "Alice"
   :person/age 30}
  
  {:person/name "Bob"
   :person/age 25}
]
"#)?;
in_progress.commit()?;
\`\`\`

### 4. Query Data
\`\`\`rust
let results = store.q_once(r#"
[:find ?name ?age
 :where
 [?person :person/name ?name]
 [?person :person/age ?age]
 [(> ?age 25)]]
"#, None)?;

for result in results.into_rel()? {
    let name: String = result[0].clone().into();
    let age: i64 = result[1].clone().into();
    println!("{} is {} years old", name, age);
}
\`\`\`

## Next Steps
- [Tutorial: CRUD Operations](tutorial-crud.md)
- [SQL to Mentat Guide](sql-comparison.md)
- [Datalog Query Reference](datalog-reference.md)
```

#### Create docs/sql-comparison.md
Side-by-side comparison of SQL and Mentat for common operations

#### Create docs/tutorial-crud.md
Step-by-step tutorial for Create, Read, Update, Delete operations

### Success Criteria
- [ ] User can go from zero to first query in < 15 minutes
- [ ] All code examples are tested and work
- [ ] Links to deeper documentation exist

---

## MIP-003: Enhanced ORM Layer

**Status:** Proposed  
**Priority:** P1 (High)  
**Estimated Effort:** 4-6 weeks  
**User Value:** Dramatically reduces boilerplate

### Problem Statement
Current entity API requires:
- Manual EDN string construction
- No relationship support
- No query helpers
- Limited convenience methods

### Proposed Solution

#### Feature 1: Automatic Finder Methods
```rust
#[derive(Entity)]
#[entity(namespace = "person")]
struct Person {
    #[entity(unique = "identity")]
    email: String,
    name: String,
    age: Option<i64>,
}

// Auto-generated:
impl Person {
    fn find_by_email(conn: &InProgress, email: &str) -> Result<Option<Person>>;
    fn all(conn: &InProgress) -> Result<Vec<Person>>;
    fn count(conn: &InProgress) -> Result<usize>;
}
```

#### Feature 2: Relationship Support
```rust
#[derive(Entity)]
struct Order {
    #[entity(relation = "Person")]
    customer_id: Entid,
    
    #[entity(many, relation = "Product")]
    item_ids: Vec<Entid>,
}

// Usage
let order = Order::read(&in_progress, order_id)?;
let customer: Person = order.customer(&in_progress)?;  // Load related
let items: Vec<Product> = order.items(&in_progress)?;  // Load related
```

#### Feature 3: Batch Operations
```rust
Person::insert_batch(&mut in_progress, vec![
    Person { name: "Alice".into(), ... },
    Person { name: "Bob".into(), ... },
])?;
```

#### Feature 4: Query Builder Integration
```rust
let adults = Person::query()
    .filter(|p| p.age.ge(18))
    .order_by(|p| p.name, Ascending)
    .limit(10)
    .execute(&in_progress)?;
```

#### Feature 5: Validation
```rust
#[derive(Entity)]
struct User {
    #[entity(validate = "email_format")]
    email: String,
    
    #[entity(min = 0, max = 120)]
    age: i64,
}

fn email_format(s: &str) -> Result<()> {
    if !s.contains('@') {
        Err("Invalid email format".into())
    } else {
        Ok(())
    }
}
```

### Implementation Plan

#### Week 1-2: Finder Methods
- Modify derive macro to generate finder methods
- Add `all()`, `count()` methods
- Generate `find_by_X` for unique fields

#### Week 3-4: Relationships
- Add `#[entity(relation = "Type")]` attribute
- Generate lazy-loading methods
- Support one-to-many and many-to-many

#### Week 5: Batch Operations
- Implement `insert_batch`, `update_batch`
- Optimize transaction handling

#### Week 6: Validation + Query Builder
- Add validation attribute support
- Basic query builder integration

### Success Criteria
- [ ] 80% reduction in boilerplate code
- [ ] Type-safe relationship traversal
- [ ] Comprehensive test coverage
- [ ] Documentation with examples

### Breaking Changes
- None (additive features only)

---

## MIP-004: Query Builder API

**Status:** Proposed  
**Priority:** P1 (High)  
**Estimated Effort:** 4-6 weeks  
**User Value:** Familiar, type-safe query construction

### Problem Statement
Current query API requires:
- Writing EDN strings manually
- No compile-time validation
- No IDE autocomplete
- Error-prone for complex queries

### Proposed Solution

#### API Design
```rust
// Basic query
let query = Query::new()
    .find(vec![var!(?name), var!(?age)])
    .r#where(vec![
        pattern!(?person, :person/name, ?name),
        pattern!(?person, :person/age, ?age),
    ]);

// With filters
let query = Query::new()
    .find(vec![var!(?name)])
    .r#where(vec![
        pattern!(?person, :person/name, ?name),
        pattern!(?person, :person/age, ?age),
    ])
    .filter(|b| b.gt(var!(?age), 18))
    .order_by(var!(?name), Ascending)
    .limit(10);

// Execute
let results = query.execute(&conn)?;

// Reusable with parameters
let query = Query::new()
    .find(vec![var!(?name)])
    .r#where(vec![
        pattern!(?person, :person/age, var!(?min_age)),
    ])
    .param("min_age", 18);
```

#### Type-Safe Results
```rust
// Strongly typed extraction
for row in results.into_rel()? {
    let name: String = row.get(0)?;
    let age: i64 = row.get(1)?;
}

// Or with tuple destructuring
let (name, age): (String, i64) = row.into()?;
```

### Implementation Plan

#### Week 1-2: Core Query Builder
- Implement `Query` struct
- Add `find()`, `where()` methods
- Basic pattern construction

#### Week 3-4: Filters and Predicates
- Implement filter DSL
- Add comparison operators
- Support logical operators (and, or, not)

#### Week 5: Order, Limit, Offset
- Add ordering support
- Implement pagination

#### Week 6: Advanced Features
- Aggregations (count, sum, avg)
- Grouping
- Subqueries

### Success Criteria
- [ ] Type-safe query construction
- [ ] Compile-time validation
- [ ] IDE autocomplete support
- [ ] Zero EDN strings in user code

---

## MIP-005: Comprehensive Documentation Overhaul

**Status:** Proposed  
**Priority:** P1 (High)  
**Estimated Effort:** 6-8 weeks  
**User Value:** Reduces learning curve dramatically

### Problem Statement
Documentation is:
- Incomplete (many APIs undocumented)
- Outdated (doesn't match current code)
- Scattered (no central guide)
- Lacks examples (theory without practice)

### Proposed Solution

#### Documentation Structure
```
docs/
├── getting-started/
│   ├── installation.md
│   ├── quick-start.md
│   └── first-app.md
├── tutorials/
│   ├── 01-crud-operations.md
│   ├── 02-querying-data.md
│   ├── 03-relationships.md
│   ├── 04-schema-design.md
│   ├── 05-transactions.md
│   └── 06-observers.md
├── guides/
│   ├── sql-to-mentat.md
│   ├── datalog-reference.md
│   ├── schema-evolution.md
│   ├── performance-tuning.md
│   └── mobile-deployment.md
├── cookbook/
│   ├── authentication.md
│   ├── one-to-many.md
│   ├── many-to-many.md
│   ├── full-text-search.md
│   ├── audit-logging.md
│   └── data-validation.md
├── api-reference/
│   └── (rustdoc generated)
└── examples/
    ├── todo-app/
    ├── blog-engine/
    └── mobile-app/
```

#### Cookbook Pattern Template
```markdown
# Pattern: User Authentication

## Problem
You need to store user credentials and authenticate logins.

## Solution
\`\`\`rust
// Schema definition
// Implementation
// Usage example
\`\`\`

## Explanation
(step-by-step breakdown)

## Variations
- Password hashing
- OAuth integration
- Session management

## See Also
- [Security Best Practices](../guides/security.md)
- [Data Validation](./data-validation.md)
```

### Implementation Plan

#### Week 1-2: Getting Started + Tutorials
- Write Quick Start guide
- Create 6 core tutorials
- Test all code examples

#### Week 3-4: Guides
- SQL comparison guide
- Datalog reference
- Schema evolution guide

#### Week 5-6: Cookbook
- 10+ common patterns
- Real-world examples
- Copy-paste ready code

#### Week 7-8: Polish + Examples
- Complete example applications
- Review all rustdoc comments
- Create documentation site

### Success Criteria
- [ ] Complete documentation for all public APIs
- [ ] 10+ cookbook recipes
- [ ] 3+ complete example apps
- [ ] User can find answer to any question in < 5 minutes

---

## MIP-006: Async/Await Support

**Status:** Proposed  
**Priority:** P2 (Medium)  
**Estimated Effort:** 6-8 weeks  
**User Value:** Modern Rust patterns, better concurrency

### Problem Statement
Current API is:
- Fully synchronous (blocking I/O)
- Doesn't integrate with async ecosystem
- Suboptimal for server applications

### Proposed Solution

#### Async API
```rust
// Async versions of core operations
pub struct AsyncStore { ... }

impl AsyncStore {
    async fn open(path: &str) -> Result<Self>;
    async fn begin_transaction(&self) -> Result<AsyncInProgress>;
    async fn q_once(&self, query: &str) -> Result<QueryOutput>;
}

impl AsyncInProgress {
    async fn transact(&mut self, data: &str) -> Result<TxReport>;
    async fn commit(self) -> Result<()>;
}
```

#### Backward Compatibility
```rust
// Keep sync API
pub struct Store { ... }  // Existing
pub struct AsyncStore { ... }  // New

// User chooses
#[tokio::main]
async fn main() {
    let store = AsyncStore::open("db.db").await?;
}

// Or sync
fn main() {
    let store = Store::open("db.db")?;
}
```

### Implementation Plan

#### Week 1-2: Design & Architecture
- Choose async runtime (tokio recommended)
- Design async trait structure
- Plan migration strategy

#### Week 3-4: Core Async Primitives
- Async Store
- Async InProgress
- Async query execution

#### Week 5-6: Testing & Optimization
- Comprehensive async tests
- Performance benchmarks
- Concurrency testing

#### Week 7-8: Documentation & Examples
- Async usage guide
- Migration from sync guide
- Example async applications

### Success Criteria
- [ ] Full async API available
- [ ] Backward compatible (sync API remains)
- [ ] Performance equal or better
- [ ] Comprehensive async examples

---

## MIP-007: Developer Tools Suite

**Status:** Proposed  
**Priority:** P2 (Medium)  
**Estimated Effort:** 8-12 weeks  
**User Value:** Better development experience

### Problem Statement
Developers lack tools for:
- Visual data exploration
- Query debugging
- Schema visualization
- Performance profiling

### Proposed Solution

#### Tool 1: Mentat Studio (GUI)
Desktop application for:
- Browsing data
- Running queries interactively
- Visualizing schemas
- Monitoring transactions

**Tech Stack:** Tauri (Rust + web frontend)

#### Tool 2: CLI Improvements
Enhanced command-line interface:
- Syntax highlighting (EDN, Datalog)
- Query history
- Autocomplete for attributes
- Pretty-printed results

#### Tool 3: VS Code Extension
- Syntax highlighting for .edn files
- Datalog query snippets
- Inline documentation
- Query validation

#### Tool 4: Query Debugger
Step-through query execution:
- Show query plan
- Display intermediate results
- Highlight performance bottlenecks

### Implementation Plan

#### Weeks 1-4: CLI Improvements
- Add syntax highlighting (syntect)
- Implement query history
- Add autocomplete

#### Weeks 5-8: VS Code Extension
- Create extension scaffolding
- Add syntax highlighting
- Implement snippets

#### Weeks 9-12: Mentat Studio
- Design UI mockups
- Implement basic functionality
- Polish and release

### Success Criteria
- [ ] Enhanced CLI with highlighting
- [ ] VS Code extension published
- [ ] Working Mentat Studio beta
- [ ] Positive user feedback

---

## Implementation Priority Matrix

```
┌─────────────────────────────────────────────┐
│  Critical Path (Do First)                   │
├─────────────────────────────────────────────┤
│  1. MIP-001: Build System Modernization     │
│  2. MIP-002: Quick Start Documentation      │
└─────────────────────────────────────────────┘

┌─────────────────────────────────────────────┐
│  High Value (Do Soon)                       │
├─────────────────────────────────────────────┤
│  3. MIP-003: Enhanced ORM Layer             │
│  4. MIP-004: Query Builder API              │
│  5. MIP-005: Documentation Overhaul         │
└─────────────────────────────────────────────┘

┌─────────────────────────────────────────────┐
│  Nice to Have (Do Later)                    │
├─────────────────────────────────────────────┤
│  6. MIP-006: Async/Await Support            │
│  7. MIP-007: Developer Tools Suite          │
└─────────────────────────────────────────────┘
```

---

## Timeline

### Month 1
- Week 1-2: MIP-001 (Build System)
- Week 3-4: MIP-002 (Quick Start)

### Month 2
- Week 1-4: MIP-003 (ORM Layer)

### Month 3
- Week 1-4: MIP-004 (Query Builder)

### Month 4
- Week 1-4: MIP-005 Part 1 (Core Documentation)

### Month 5
- Week 1-4: MIP-005 Part 2 (Cookbook + Examples)

### Month 6
- Week 1-4: MIP-006 (Async Support)

### Month 7+
- MIP-007 (Developer Tools)
- Additional improvements based on feedback

---

## Success Metrics

Track progress with these metrics:

### Build Health
- [ ] Build success rate: 100%
- [ ] Compiler warnings: 0
- [ ] Test coverage: > 80%

### Documentation
- [ ] API coverage: 100%
- [ ] Tutorial completion: 100%
- [ ] Cookbook recipes: > 10

### User Experience
- [ ] Time to first query: < 15 min
- [ ] GitHub issues (new): trend down
- [ ] Community engagement: trend up

### Adoption
- [ ] crates.io downloads: track growth
- [ ] GitHub stars: track growth
- [ ] Real-world projects: identify 5+

---

## Contributing

To contribute to these MIPs:
1. Read the full proposal
2. Discuss in GitHub issues
3. Submit PRs referencing MIP number
4. Update MIP status as work progresses

**MIP Status Values:**
- **Proposed:** Under discussion
- **Accepted:** Approved for implementation
- **In Progress:** Being worked on
- **Completed:** Fully implemented
- **Rejected:** Not moving forward
