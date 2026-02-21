# Design Document: Declarative SQL Schema Manager in Rust

## 1. Background and Motivation

### 1.1 sqldef Overview

sqldef is a declarative SQL schema management tool. Users declare the desired schema state in SQL files, and sqldef computes and applies the minimal DDL to migrate the current database to that state. It supports MySQL, PostgreSQL, SQL Server, and SQLite3.

### 1.2 Problems with the Current Architecture

#### P1: Single Grammar for All Dialects (Low Fidelity Parsing)

sqldef uses a single yacc grammar (`parser/parser.y`, 7,385 lines) to parse all SQL dialects. This leads to:

- **Ambiguity**: The grammar cannot distinguish dialect-specific syntax. For example, PostgreSQL's `::` cast operator, MSSQL's `[bracket]` identifiers, and MySQL's backtick quoting are all handled through tokenizer-level hacks rather than grammar rules.
- **Shift/reduce conflicts**: 11+ precedence directives exist solely to paper over ambiguity caused by cramming multiple dialects into one grammar.
- **Incomplete parsing**: PostgreSQL requires a fallback to `go-pgquery` (a native C-based parser via Wasm) because the generic grammar cannot handle exclusion constraints, complex expressions, deferrable constraints, and more. This fallback adds 1,670 lines of AST conversion code.
- **Fragile evolution**: Adding a new SQL feature for one dialect risks breaking others. The grammar is already at its practical maintainability limit.

#### P2: No Plugin / Extension Mechanism

Database-specific logic is hardcoded throughout the codebase:

- `GeneratorMode` (an enum with 4 values) is passed through the entire call chain. The core diff engine (`schema/generator.go`, 8,000+ lines) contains massive `switch g.mode` blocks at nearly every decision point.
- Adding a new database requires modifying: the parser grammar, the tokenizer, the schema AST, the generator, the normalizer, the database adapter, and the CLI entry point.
- There is no way to support a new database without forking the project.

#### P3: Monolithic Diff Engine

The diff engine (`generateDDLsForCreateTable`, 1,900+ lines) handles column diffs, index diffs, constraint diffs, partition diffs, and foreign key diffs in a single function with database-specific branches everywhere. This makes the code difficult to reason about, test, and extend.

#### P4: Normalization Complexity

The normalization layer (`schema/normalize.go`, 1,413 lines) performs complex AST transformations to make schemas comparable:
- Type alias resolution (`bool` → `boolean`, `int4` → `integer`)
- Expression normalization (removing redundant casts, rewriting `IN` to `= ANY`)
- View definition expansion
- Identifier case handling

This is necessary because the single grammar produces slightly different ASTs for semantically identical SQL across dialects. With per-dialect parsers, much of this normalization would be unnecessary.

### 1.3 Goals of the Rewrite

1. **Preserve the core value proposition**: Declarative schema management with idempotent DDL generation.
2. **Per-dialect parsing**: Each supported SQL dialect gets its own parser that produces a dialect-specific AST, eliminating the need for a lowest-common-denominator grammar.
3. **Source-level extensibility (within fixed IR)**: New databases can be supported by implementing a trait in a separate crate and recompiling, as long as they fit within the IR's fixed set of schema object types (Table, View, MaterializedView, Index, Trigger, etc.). Adding a new object type (e.g., a hypothetical database-specific object with no IR equivalent) requires a core change. No runtime plugin loading; Cargo features control which dialects are included in the binary.
4. **Modular diff engine**: The diff algorithm should be decomposed into composable, testable units.
5. **Safety by default**: Unknown or unsupported SQL constructs must cause an explicit error, never be silently ignored. This prevents accidental DROP of objects the tool doesn't understand.
6. **Reuse sqldef's test corpus**: The 1,051 YAML test cases represent years of edge-case discovery and should be reusable (with adaptation cost acknowledged).

### 1.4 Non-Goals

- GUI or web interface.
- Migration history tracking (like golang-migrate or Flyway). This tool is stateless by design.
- ORM integration.
- Supporting non-SQL databases.

---

## 2. Current Architecture Analysis

### 2.1 Component Map

```
┌─────────────────────────────────────────────────────────┐
│                     CLI Layer                            │
│  mysqldef / psqldef / mssqldef / sqlite3def             │
│  (cmd/*def/*.go)                                         │
└────────────────────────┬────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────┐
│                   Orchestrator                           │
│  sqldef.Run()                                            │
│  - ExportDDLs → ParseDDLs → GenerateIdempotentDDLs      │
│  - Apply or dry-run                                      │
└────────┬───────────────┬────────────────┬───────────────┘
         │               │                │
         ▼               ▼                ▼
┌────────────┐  ┌────────────────┐  ┌─────────────────┐
│   Parser   │  │  Schema Layer  │  │ Database Adapter │
│            │  │                │  │                  │
│ parser.y   │  │ ast.go         │  │ Database iface   │
│ node.go    │  │ parser.go      │  │ mysql/           │
│ token.go   │  │ generator.go   │  │ postgres/        │
│            │  │ normalize.go   │  │ mssql/           │
│ GenericParser│ │ ddl_ordering.go│  │ sqlite3/         │
│ PostgresParser│                │  │ file/            │
│ MssqlParser│  │                │  │ dry_run.go       │
└────────────┘  └────────────────┘  └─────────────────┘
```

### 2.2 Data Flow

```
User's SQL file (desired state)
        │
        ▼
   Parse SQL ──────────────────────┐
        │                          │
        ▼                          ▼
   DDL AST (desired)        Database.ExportDDLs()
        │                          │
        │                     Parse SQL
        │                          │
        │                          ▼
        │                   DDL AST (current)
        │                          │
        ▼                          ▼
   ┌───────────────────────────────────┐
   │     Schema Diff Engine            │
   │  (GenerateIdempotentDDLs)         │
   │                                   │
   │  1. Normalize both ASTs           │
   │  2. Compare table-by-table        │
   │  3. Generate minimal DDL          │
   │  4. Topological sort by deps      │
   └───────────────────┬───────────────┘
                       │
                       ▼
              DDL statements (strings)
                       │
                       ▼
              Database.Exec() or dry-run print
```

### 2.3 Key Design Decisions in sqldef

| Decision | Rationale | Trade-off |
|----------|-----------|-----------|
| Single yacc grammar | Maximize shared parsing logic | Low fidelity per dialect |
| GeneratorMode enum | Simple dispatch | Leaks throughout codebase |
| Normalization layer | Compensate for parser imprecision | Adds complexity |
| pgquery fallback | Handle PostgreSQL features the generic parser can't | Two parser paths to maintain |
| YAML test format | Declarative tests match declarative tool | Tied to Go test runner |
| Inline FK normalization | Unify column-level and table-level FKs | Lossy transformation |
| `@renamed` annotations | Enable rename detection via SQL comments | Non-standard SQL |

### 2.4 Quantitative Summary

| Component | Lines of Code | Files |
|-----------|---------------|-------|
| Parser (grammar + lexer + AST) | ~12,000 | 4 |
| PostgreSQL parser bridge | ~1,670 | 1 |
| Schema layer (AST + diff + normalize) | ~10,400 | 5 |
| Database adapters | ~3,500 | 9 |
| CLI entry points | ~1,200 | 4 |
| Test framework | ~900 | 2 |
| YAML test cases | ~1,051 cases | 51 files |
| **Total** | **~30,000** | **~76** |

---

## 3. Proposed Architecture

### 3.1 High-Level Design

```
┌────────────────────────────────────────────────────────────┐
│                       CLI Binary                            │
│  Single binary with subcommands: mysql, postgres, etc.     │
│  Or: separate binaries built from same crate with features │
└────────────────────────┬───────────────────────────────────┘
                         │
                         ▼
┌────────────────────────────────────────────────────────────┐
│                    Core Engine (core crate)                 │
│                                                            │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐ │
│  │  Diff Engine  │  │  DDL Planner │  │  Dependency Sort │ │
│  │              │  │  (DDL gen)   │  │  (topo sort)     │ │
│  └──────┬───────┘  └──────┬───────┘  └──────────────────┘ │
│         │                 │                                │
│         ▼                 ▼                                │
│  ┌─────────────────────────────────────────┐               │
│  │         Canonical Schema IR             │               │
│  │  (database-agnostic intermediate repr)  │               │
│  └─────────────────────────────────────────┘               │
└────────────────────────┬───────────────────────────────────┘
                         │
            ┌────────────┴────────────┐
            │    Dialect Trait         │
            │                         │
            │  fn parse() → IR        │
            │  fn generate_ddl(       │
            │     &[DiffOp]) → SQL    │
            │  fn normalize()         │
            │  fn connect()           │
            └────────────┬────────────┘
                         │
         ┌───────────────┼───────────────┐
         │               │               │
         ▼               ▼               ▼
   ┌──────────┐   ┌──────────┐   ┌──────────┐
   │  MySQL   │   │ Postgres │   │  SQLite  │  ...
   │ Dialect  │   │ Dialect  │   │ Dialect  │
   │          │   │          │   │          │
   │ Parser   │   │ Parser   │   │ Parser   │
   │ DDL Gen  │   │ DDL Gen  │   │ DDL Gen  │
   │ Adapter  │   │ Adapter  │   │ Adapter  │
   └──────────┘   └──────────┘   └──────────┘
```

### 3.2 Crate Structure

```
workspace/
├── Cargo.toml                    # Workspace root
├── crates/
│   ├── core/                     # Core engine (dialect-agnostic)
│   │   ├── src/
│   │   │   ├── ir.rs             # Canonical Schema IR types
│   │   │   ├── diff.rs           # Schema diff algorithm
│   │   │   ├── plan.rs           # DDL plan generation
│   │   │   ├── ordering.rs       # Topological sort
│   │   │   ├── dialect.rs        # Dialect trait definition
│   │   │   └── lib.rs
│   │   └── Cargo.toml
│   │
│   ├── dialect-mysql/            # MySQL dialect implementation
│   │   ├── src/
│   │   │   ├── parser.rs         # MySQL-specific SQL parser
│   │   │   ├── generator.rs      # MySQL DDL generation
│   │   │   ├── adapter.rs        # MySQL database adapter
│   │   │   ├── normalize.rs      # MySQL-specific normalization
│   │   │   └── lib.rs
│   │   └── Cargo.toml
│   │
│   ├── dialect-postgres/         # PostgreSQL dialect
│   │   ├── src/
│   │   │   ├── parser.rs
│   │   │   ├── generator.rs
│   │   │   ├── adapter.rs
│   │   │   ├── normalize.rs
│   │   │   └── lib.rs
│   │   └── Cargo.toml
│   │
│   ├── dialect-sqlite/           # SQLite dialect
│   │   └── ...
│   │
│   ├── dialect-mssql/            # MSSQL dialect
│   │   └── ...
│   │
│   ├── cli/                      # CLI binary
│   │   ├── src/
│   │   │   └── main.rs
│   │   └── Cargo.toml
│   │
│   └── testkit/                  # Shared test utilities
│       ├── src/
│       │   ├── yaml_runner.rs    # YAML test case loader & runner
│       │   └── lib.rs
│       └── Cargo.toml
│
└── tests/                        # YAML test cases (reused from sqldef)
    ├── mysql/
    ├── postgres/
    ├── sqlite/
    └── mssql/
```

Dialect extension is source-level through trait implementations compiled with Cargo features.
Runtime plugin loading is out of scope for v1.
Decision history is captured in [ADR-0003](docs/adr/0003-source-level-extensibility.md).

```toml
[features]
default = ["mysql", "postgres", "sqlite"]
mysql = ["dep:dialect-mysql"]
postgres = ["dep:dialect-postgres"]
sqlite = ["dep:dialect-sqlite"]
mssql = ["dep:dialect-mssql"]
```


### 3.3 Core Trait: `Dialect`

```rust
/// A SQL dialect implementation.
/// Each supported database implements this trait.
pub trait Dialect: Send + Sync {
    /// Unique name for this dialect (e.g., "mysql", "postgres").
    fn name(&self) -> &str;

    /// Parse SQL DDL statements into the canonical IR.
    ///
    /// Every statement in the input must be converted to a `SchemaObject`.
    /// If a statement is syntactically valid SQL but not a supported DDL type
    /// (e.g., DML, DCL, or a DDL variant the dialect doesn't handle yet),
    /// the parser must return `Err`, not silently skip it.
    /// This fail-fast behavior prevents the diff engine from interpreting
    /// "absent from desired" as "should be DROPped".
    fn parse(&self, sql: &str) -> Result<Vec<SchemaObject>>;

    /// Generate DDL SQL from a batch of diff operations.
    ///
    /// Receives the full set of operations for the migration plan.
    /// The dialect may merge, reorder, or batch operations as needed
    /// (e.g., MySQL CHANGE COLUMN combining type + nullability changes,
    /// SQLite table recreation, MSSQL GO batch separators).
    fn generate_ddl(&self, ops: &[DiffOp]) -> Result<Vec<Statement>>;

    /// Serialize a schema object back to SQL (for --export).
    ///
    /// This is the single canonical path for SQL rendering.
    /// The --export flow is:
    ///   DatabaseAdapter::export_schema() → raw SQL string
    ///   → Dialect::parse() → Vec<SchemaObject>
    ///   → Dialect::normalize() on each object
    ///   → Dialect::to_sql() on each object → output
    ///
    /// DatabaseAdapter::export_schema() returns raw SQL from the
    /// database's system catalog. Dialect::to_sql() renders a
    /// normalized SchemaObject back to SQL. These are NOT alternatives;
    /// they are sequential stages in the export pipeline.
    fn to_sql(&self, obj: &SchemaObject) -> Result<String>;

    /// Normalize a schema object for comparison.
    /// Called before diffing to eliminate superficial differences.
    fn normalize(&self, obj: &mut SchemaObject);

    /// Provide semantic equivalence rules used by the diff engine.
    ///
    /// This is an optional hook for cases where normalized values remain
    /// syntactically different but are semantically equivalent for this dialect
    /// (e.g., `'0'::integer` vs `0` in PostgreSQL defaults/checks).
    ///
    /// The returned policy must be pure and deterministic:
    /// no database access, no I/O, no time-dependent behavior.
    fn equivalence_policy(&self) -> &'static dyn EquivalencePolicy {
        &DEFAULT_EQUIVALENCE_POLICY
    }

    /// Quote an identifier according to dialect rules.
    fn quote_ident(&self, ident: &Ident) -> String;

    /// The string to render for `BatchBoundary` in dry-run output.
    /// Returns `"GO\n"` for MSSQL, `""` for other dialects.
    fn batch_separator(&self) -> &str { "" }

    /// Connect to a database and return an adapter.
    fn connect(&self, config: &ConnectionConfig) -> Result<Box<dyn DatabaseAdapter>>;
}

/// Semantic equivalence policy used by the core diff engine.
///
/// The default implementation is strict equality (`==`).
/// Dialects may override only the cases that need semantic relaxation.
pub trait EquivalencePolicy: Send + Sync {
    /// Expression-level equivalence (CHECK / DEFAULT / partial index predicates).
    fn is_equivalent_expr(&self, a: &Expr, b: &Expr) -> bool {
        a == b
    }

    /// Equivalence for `DataType::Custom(String)` payloads.
    fn is_equivalent_custom_type(&self, a: &str, b: &str) -> bool {
        a == b
    }
}

pub struct DefaultEquivalencePolicy;
impl EquivalencePolicy for DefaultEquivalencePolicy {}

pub static DEFAULT_EQUIVALENCE_POLICY: DefaultEquivalencePolicy = DefaultEquivalencePolicy;
```

### 3.4 Canonical Schema IR

The IR is the central data structure. Dialect parsers produce it; the diff engine consumes it.

#### Comment Preservation for `@renamed` Annotations

`@renamed` annotations are SQL comments (`-- @renamed from=old_name`). Most SQL parsers strip comments during lexing, so a naive `parse()` call loses this information. The design requires a **pre-parse annotation extraction pass**:

```
Raw SQL input
    │
    ▼
┌──────────────────────────────┐
│ 1. Annotation Extractor      │
│    Scan for `@renamed` and   │
│    `@rename` (deprecated)    │
│    in SQL comments.          │
│    Build: Vec<RenameAnnotation>│
│    (keyed by source line)    │
│    Strip annotations from    │
│    SQL (leave as plain       │
│    comments for parser).     │
└──────────────┬───────────────┘
               │
               ▼
┌──────────────────────────────┐
│ 2. SQL Parser (per-dialect)  │
│    Parse annotation-free SQL │
│    into raw AST.             │
└──────────────┬───────────────┘
               │
               ▼
┌──────────────────────────────┐
│ 3. IR Builder                │
│    Convert AST → SchemaObject│
│    Attach RenameAnnotation   │
│    from step 1 to matching   │
│    objects via `renamed_from`│
└──────────────┬───────────────┘
               │
               ▼
         Vec<SchemaObject>
```

The annotation extractor is implemented in **core** (not per-dialect), since the `@renamed` syntax is dialect-independent. Each dialect's `parse()` calls the core extractor first, then parses the SQL, then merges the annotations. If an annotation references an object that the parser did not produce, it is an error (prevents stale annotations from being silently ignored).

```rust
/// Core-provided annotation extraction.
pub struct AnnotationExtractor;

impl AnnotationExtractor {
    /// Extract `@renamed` annotations from SQL comments.
    /// Returns the cleaned SQL (annotations removed from comments)
    /// and a list of annotations, each recording its source line number.
    ///
    /// The cleaned SQL must preserve original line boundaries so parser-reported
    /// line numbers can be mapped back to the user's input file.
    pub fn extract(sql: &str) -> Result<(String, Vec<RenameAnnotation>)>;
}

pub struct RenameAnnotation {
    /// The line number in the original SQL where the annotation appeared.
    pub line: usize,
    /// The old name, with quoting preserved.
    pub from: Ident,
}
```

#### Index as Top-Level Schema Object

Indexes are top-level `SchemaObject::Index` values, not embedded in `Table.indexes`. Each `IndexDef` carries an `owner` field that records which object the index belongs to.

**Rationale**: Indexes can be created on tables, views (MSSQL indexed views), and materialized views (PostgreSQL). Embedding indexes in `Table` makes these cases unrepresentable. A top-level index with an explicit owner (`IndexOwner::Table`, `IndexOwner::View`, or `IndexOwner::MaterializedView`) avoids duplication and supports all ownership patterns.

**Diff engine behavior**:
- The diff engine matches indexes by name (within the same owner).
- When comparing schemas, the diff engine cross-references `IndexDef.owner` against existing `Table` / `View` / `MaterializedView` objects to validate that the owner exists.
- If a `CREATE INDEX` references an owner not present in the input, the parser returns an error (fail-fast, §1.3 goal 5).

```rust
/// A top-level schema object.
///
/// This enum is intentionally closed (no `Custom` / `#[non_exhaustive]`).
/// Adding a new variant is a semver-breaking change to the core crate.
///
/// Rationale: a closed enum ensures that the diff engine handles every
/// object type exhaustively. An open/extensible enum would allow dialect
/// crates to introduce objects that the diff engine silently ignores,
/// reintroducing the P0 "unknown DDL → accidental DROP" problem.
///
/// Dialect-specific *attributes* on existing objects use the `extra` map
/// (e.g., MySQL's AUTO_INCREMENT, PostgreSQL's TABLESPACE). New object
/// *kinds* require a core change and a new diff engine match arm.
pub enum SchemaObject {
    Table(Table),
    View(View),
    MaterializedView(MaterializedView),
    Index(IndexDef),
    Sequence(Sequence),   // PostgreSQL sequences (implicit via SERIAL, or explicit)
    Trigger(Trigger),
    Function(Function),
    Type(TypeDef),        // ENUMs, composite types
    Domain(Domain),       // PostgreSQL domains
    Extension(Extension), // PostgreSQL extensions
    Schema(SchemaDef),    // CREATE SCHEMA
    Comment(Comment),
    Privilege(Privilege),
    Policy(Policy),       // PostgreSQL RLS
}
```

#### SchemaObject Variant Addition Criteria

Adding a new `SchemaObject` variant is a semver-breaking change to the core crate. A new variant is warranted when **all** of the following hold:

1. **The object has an independent lifecycle**: it can be created, dropped, or altered independently of other objects (e.g., `CREATE EXTENSION`, `DROP DOMAIN`). Objects that are always subordinate to a parent (e.g., table partitions, column constraints) belong as fields on the parent struct, not as top-level variants.
2. **The diff engine must track its presence and absence**: the tool needs to detect "object exists in current but not in desired" (→ DROP) or vice versa (→ CREATE). If the object is purely decorative or informational and its absence should never trigger a DROP, it does not need a variant.
3. **At least one supported dialect uses it**: single-dialect objects are acceptable (e.g., `Extension` is PostgreSQL-only, `Policy` is PostgreSQL-only) because the fail-fast design ensures dialects that don't use the variant simply never produce it. The cost of a variant is one match arm in the diff engine, not per-dialect implementation.

Objects that do **not** meet these criteria but need dialect-specific representation should use `extra` maps on existing variants or be handled entirely within the dialect's parser/generator (e.g., MySQL `EVENT` can be modeled as a `Trigger`-like object if the diff semantics are identical, or deferred to a future core change if distinct diff behavior is needed).

```rust
pub struct Table {
    pub name: QualifiedName,
    pub columns: Vec<Column>,
    pub primary_key: Option<PrimaryKey>,
    pub foreign_keys: Vec<ForeignKey>,
    pub checks: Vec<CheckConstraint>,
    pub exclusions: Vec<ExclusionConstraint>,  // PostgreSQL
    pub options: TableOptions,
    pub partition: Option<Partition>,
    /// Explicit rename source. Populated only from `-- @renamed from=old_name`
    /// annotations in the desired schema SQL. The diff engine never infers
    /// renames; without this annotation, a name change produces DROP + CREATE.
    /// See [ADR-0006](docs/adr/0006-explicit-rename-annotations.md) for the rename detection specification.
    pub renamed_from: Option<Ident>,
}

pub struct MaterializedView {
    pub name: QualifiedName,
    pub columns: Vec<Column>,
    pub query: String,                // The SELECT query
    pub options: TableOptions,
    pub renamed_from: Option<Ident>,
}

/// An index definition. Top-level schema object with explicit owner.
pub struct IndexDef {
    pub name: Option<Ident>,
    pub owner: IndexOwner,
    pub columns: Vec<IndexColumn>,
    pub unique: bool,
    pub method: Option<String>,       // btree, hash, gin, gist, etc.
    pub where_clause: Option<Expr>,   // partial index
    pub concurrent: bool,             // CREATE INDEX CONCURRENTLY
    pub extra: BTreeMap<String, Value>,
}

/// The object that an index is defined on.
pub enum IndexOwner {
    Table(QualifiedName),
    View(QualifiedName),              // MSSQL indexed views
    MaterializedView(QualifiedName),
}

pub struct Column {
    pub name: Ident,
    pub data_type: DataType,
    pub not_null: bool,
    pub default: Option<Expr>,
    pub identity: Option<Identity>,
    pub generated: Option<GeneratedColumn>,
    pub comment: Option<String>,
    pub collation: Option<String>,
    /// See Table.renamed_from and [ADR-0006](docs/adr/0006-explicit-rename-annotations.md) for rename specification.
    pub renamed_from: Option<Ident>,
    /// Dialect-specific attributes that the core doesn't interpret
    /// but passes through to the dialect's DDL generator.
    /// See "Extra Map Convention" below for key naming rules and the Value type.
    pub extra: BTreeMap<String, Value>,
}

/// Typed values for dialect-specific `extra` maps.
///
/// Intentionally minimal: covers the value shapes needed by known
/// dialect attributes (flags, numeric settings, string options).
/// The diff engine compares `extra` maps using `BTreeMap::eq`,
/// so `Value` must implement `PartialEq`.
pub enum Value {
    String(String),
    Integer(i64),
    Float(f64),
    Bool(bool),
    Null,
}
```

#### Extra Map Convention

The `extra` field on `Column`, `IndexDef`, and other IR structs carries dialect-specific attributes that the core diff engine does not interpret. The following rules govern its usage:

**Key naming**: Keys use the format `<dialect>.<attribute>`, where `<dialect>` matches `Dialect::name()`. Examples:
- `mysql.auto_increment` (`Value::Bool`)
- `mysql.on_update` (`Value::String` — e.g., `"CURRENT_TIMESTAMP"`)
- `mssql.identity_seed` (`Value::Integer`)
- `mssql.identity_increment` (`Value::Integer`)
- `postgres.tablespace` (`Value::String`)

**Key constants (required)**: To prevent typo-driven bugs, each dialect crate defines `extra` keys as constants and uses those constants in both parser-to-IR conversion and DDL generation. Raw string literals for `extra` keys are prohibited outside the constants module.

```rust
pub mod extra_keys {
    // Use `pub(crate)` by default; expose `pub` only when cross-crate reuse is required.
    pub(crate) const AUTO_INCREMENT: &str = "mysql.auto_increment";
    pub(crate) const ON_UPDATE: &str = "mysql.on_update";
}

// Parser/normalizer side:
column.extra.insert(extra_keys::AUTO_INCREMENT.to_string(), Value::Bool(true));

// DDL generator side:
if let Some(Value::Bool(true)) = column.extra.get(extra_keys::AUTO_INCREMENT) {
    // emit AUTO_INCREMENT
}
```

This ensures that parser and generator stay in sync under refactoring, and the compiler catches renamed/removed keys.

**Core diff behavior**: The diff engine compares `extra` maps via `BTreeMap<String, Value>`'s `PartialEq`. Any difference in the `extra` map produces the corresponding `AlterColumn` / `AlterTableOptions` `DiffOp`. The core does not inspect individual keys — it treats the map as an opaque attribute bag.

**Dialect `generate_ddl` responsibility**: The dialect's `generate_ddl()` reads keys from `extra` by their well-known names and emits the appropriate SQL. Keys not recognized by a dialect are ignored (they belong to a different dialect's namespace).

**Promotion to IR fields**: An attribute graduates from `extra` to an explicit IR field when **all** of the following hold:
1. It is used by two or more dialects.
2. The diff engine must detect changes to the attribute and produce a specific `DiffOp` (beyond the generic "extra changed" signal).
3. String-keyed comparison is insufficient — the attribute has structural semantics (e.g., type parameters, nested values) that require a dedicated type.

Current examples of promoted attributes: `collation` (used by MySQL, PostgreSQL, MSSQL), `identity` (used by PostgreSQL, MSSQL), `generated` (used by MySQL, PostgreSQL).

```rust
/// Fully qualified name: schema.name
pub struct QualifiedName {
    pub schema: Option<Ident>,
    pub name: Ident,
}

/// An identifier with quoting information preserved.
///
/// `quoted` records whether the identifier appeared in quotes in the
/// source SQL. The specific quote character (double-quote, backtick,
/// or bracket) is NOT stored — `Dialect::quote_ident()` selects the
/// correct quoting style when rendering SQL output.
///
/// Comparison semantics:
/// - Two `Ident` values are equal when `value` matches and `quoted`
///   matches. Unquoted identifiers are case-insensitive for matching
///   purposes (the diff engine normalizes case before comparison via
///   `Dialect::normalize()`).
pub struct Ident {
    pub value: String,
    pub quoted: bool,
}

/// A database sequence (PostgreSQL CREATE SEQUENCE).
/// Only **explicitly** created sequences appear as `SchemaObject::Sequence`.
/// See "Sequence Representation Rules" below for the boundary with
/// `Column.identity` and `Column.default`.
pub struct Sequence {
    pub name: QualifiedName,
    pub data_type: Option<DataType>,     // AS smallint / integer / bigint
    pub increment: Option<i64>,
    pub min_value: Option<i64>,
    pub max_value: Option<i64>,
    pub start: Option<i64>,
    pub cache: Option<i64>,
    pub cycle: bool,
    pub owned_by: Option<(QualifiedName, Ident)>, // table, column
}

```

#### Sequence Representation Rules

Sequences can enter the system through two paths: explicit `CREATE SEQUENCE` statements and implicit creation via column definitions (`SERIAL`, `GENERATED AS IDENTITY`). To prevent double-counting in the diff engine, the following rules apply:

| Origin | IR Representation | Rationale |
|--------|-------------------|-----------|
| `CREATE SEQUENCE s; ... DEFAULT nextval('s')` | `SchemaObject::Sequence(s)` + `Column.default = nextval('s')` | Explicit sequence has an independent lifecycle; the column merely references it. |
| `SERIAL` / `BIGSERIAL` | `Column.default` with the expanded `nextval(...)` expression. **No** `SchemaObject::Sequence`. | The sequence is an implementation detail of the column type. The dialect normalizer rewrites `SERIAL` to the underlying type + default. |
| `GENERATED { ALWAYS | BY DEFAULT } AS IDENTITY` | `Column.identity = Some(Identity { ... })`. **No** `SchemaObject::Sequence`. | Identity columns own their sequence implicitly; `ALTER COLUMN ... SET/DROP IDENTITY` manages the lifecycle. |

**Dialect normalizer contract**: The PostgreSQL normalizer must ensure that `adapter.export_schema()` output, which may include both the implicit sequence and the column definition, is collapsed into the column-attribute form for `SERIAL` and `GENERATED AS IDENTITY` columns. If the exported SQL contains an explicit `CREATE SEQUENCE` that is solely owned by a single identity/serial column (detectable via `pg_depend`), the normalizer omits it from the `Vec<SchemaObject>` and folds the sequence parameters into `Column.identity` or `Column.default`.

**Diff engine invariant**: A sequence name must not appear both as a `SchemaObject::Sequence` and as the implicit sequence of a `Column.identity`. If the normalizer produces both, the diff engine treats it as a `DiffError` (prevents duplicate CREATE/DROP).

```rust
/// Data types normalized to a common representation.
/// Dialect-specific types are preserved as-is.
pub enum DataType {
    // Common types
    Boolean,
    SmallInt,
    Integer,
    BigInt,
    Real,
    DoublePrecision,
    Numeric { precision: Option<u32>, scale: Option<u32> },
    Text,
    Varchar { length: Option<u32> },
    Char { length: Option<u32> },
    Blob,
    Date,
    Time { with_timezone: bool },
    Timestamp { with_timezone: bool },
    Json,
    Jsonb,
    Uuid,
    // Array wrapper
    Array(Box<DataType>),
    // Dialect-specific (opaque to core, handled by dialect)
    Custom(String),
}
```

#### Remaining IR Struct Definitions

The following structs are referenced by `SchemaObject` but not yet defined above. Primary fields are listed; `extra: BTreeMap<String, Value>` and `renamed_from: Option<Ident>` are omitted where they follow the same pattern as `Table`.

```rust
pub struct View {
    pub name: QualifiedName,
    pub columns: Vec<Ident>,              // column name list (types inferred from query)
    pub query: String,                    // the SELECT body
    pub check_option: Option<CheckOption>, // LOCAL, CASCADED, or None
    pub security: Option<ViewSecurity>,   // DEFINER or INVOKER (MySQL, PostgreSQL)
    pub renamed_from: Option<Ident>,
}

pub struct Trigger {
    pub name: QualifiedName,
    pub table: QualifiedName,             // the table this trigger is on
    pub timing: TriggerTiming,            // BEFORE, AFTER, INSTEAD OF
    pub events: Vec<TriggerEvent>,        // INSERT, UPDATE, DELETE, TRUNCATE
    pub for_each: TriggerForEach,         // ROW or STATEMENT
    pub when_clause: Option<Expr>,
    pub body: String,                     // function call or inline body
}

pub struct Function {
    pub name: QualifiedName,
    pub params: Vec<FunctionParam>,       // (name, type, mode)
    pub return_type: Option<DataType>,
    pub language: String,                 // plpgsql, sql, etc.
    pub body: String,                     // function body (or external reference)
    pub volatility: Option<Volatility>,   // IMMUTABLE, STABLE, VOLATILE
    pub security: Option<FunctionSecurity>, // DEFINER or INVOKER
}

pub struct TypeDef {
    pub name: QualifiedName,
    pub kind: TypeKind,
}

pub enum TypeKind {
    Enum { labels: Vec<String> },
    Composite { fields: Vec<(Ident, DataType)> },
    Range { subtype: DataType },
}

pub struct Domain {
    pub name: QualifiedName,
    pub data_type: DataType,
    pub default: Option<Expr>,
    pub not_null: bool,
    pub checks: Vec<CheckConstraint>,     // domain-level CHECK constraints
}

pub struct Extension {
    pub name: Ident,
    pub schema: Option<Ident>,
    pub version: Option<String>,
}

pub struct SchemaDef {
    pub name: Ident,
}

pub struct Comment {
    pub target: CommentTarget,
    pub text: Option<String>,             // None means DROP COMMENT
}

pub enum CommentTarget {
    Table(QualifiedName),
    Column { table: QualifiedName, column: Ident },
    Index(QualifiedName),
    // ... other object types
}

pub struct Privilege {
    pub operations: Vec<PrivilegeOp>,     // SELECT, INSERT, ALL, etc.
    pub on: PrivilegeObject,              // TABLE t, SCHEMA s, etc.
    pub grantee: Ident,                   // role name
    pub with_grant_option: bool,
}
```

#### Privilege Diff Semantics

Privileges are matched by the composite key `(on, grantee)`. The diff engine compares the `operations` set and `with_grant_option` flag between current and desired.

| Current | Desired | DiffOps Emitted |
|---------|---------|-----------------|
| `GRANT SELECT ON t TO r` | `GRANT SELECT, INSERT ON t TO r` | `Grant(Privilege { ops: [INSERT], on: t, grantee: r })` |
| `GRANT SELECT, INSERT ON t TO r` | `GRANT SELECT ON t TO r` | `Revoke(Privilege { ops: [INSERT], on: t, grantee: r })` |
| `GRANT SELECT ON t TO r` | (absent) | `Revoke(Privilege { ops: [SELECT], on: t, grantee: r })` |
| (absent) | `GRANT SELECT ON t TO r` | `Grant(Privilege { ops: [SELECT], on: t, grantee: r })` |
| `GRANT SELECT ON t TO r` | `GRANT SELECT ON t TO r WITH GRANT OPTION` | `Grant(Privilege { ops: [SELECT], on: t, grantee: r, with_grant_option: true })` |

**Partial grant/revoke**: The diff engine computes the **set difference** between current and desired operations, then emits only the incremental `Grant` (for added operations) and `Revoke` (for removed operations). It does **not** revoke all and re-grant — this avoids disrupting active sessions that hold existing privileges.

**`ALL` expansion**: The dialect normalizer is responsible for expanding `ALL` (or `ALL PRIVILEGES`) into the concrete set of operations for the target object type (e.g., `SELECT, INSERT, UPDATE, DELETE, TRUNCATE, REFERENCES, TRIGGER` for PostgreSQL tables). This ensures set-difference comparison works correctly. The `ALL` shorthand never appears in the IR after normalization.

```rust
pub struct Policy {
    pub name: Ident,
    pub table: QualifiedName,
    pub command: Option<PolicyCommand>,   // ALL, SELECT, INSERT, UPDATE, DELETE
    pub using_expr: Option<Expr>,
    pub check_expr: Option<Expr>,
    pub roles: Vec<Ident>,               // TO role_list
    pub permissive: bool,                 // PERMISSIVE (default) or RESTRICTIVE
}
```

#### Partition Definition

MySQL and PostgreSQL both support table partitioning, but with significantly different syntax and semantics. The IR models the common structure while using `extra` for dialect-specific details.

```rust
pub struct Partition {
    pub strategy: PartitionStrategy,
    pub columns: Vec<Ident>,              // partition key columns / expressions
    pub partitions: Vec<PartitionElement>,
}

pub enum PartitionStrategy {
    Range,
    List,
    Hash,
    /// MySQL KEY partitioning (uses internal hashing).
    Key,
}

pub struct PartitionElement {
    pub name: Ident,
    /// Boundary value. Interpretation depends on strategy:
    /// - Range: upper bound (e.g., `VALUES LESS THAN (100)`)
    /// - List:  value set  (e.g., `VALUES IN (1, 2, 3)`)
    /// - Hash/Key: None (partition count determines distribution)
    pub bound: Option<PartitionBound>,
    /// Dialect-specific attributes (e.g., MySQL tablespace, PostgreSQL WITH).
    pub extra: BTreeMap<String, Value>,
}

pub enum PartitionBound {
    /// `VALUES LESS THAN (expr, ...)` or `VALUES LESS THAN MAXVALUE`.
    LessThan(Vec<Expr>),
    /// `VALUES IN (expr, ...)`.
    In(Vec<Expr>),
    /// PostgreSQL range bound: `FROM (expr, ...) TO (expr, ...)`.
    FromTo { from: Vec<Expr>, to: Vec<Expr> },
    /// Special sentinel: `MAXVALUE` (MySQL), `MINVALUE`/`MAXVALUE` (PostgreSQL).
    MaxValue,
}
```

**PostgreSQL declarative partitioning**: PostgreSQL partition children are full tables (`CREATE TABLE child PARTITION OF parent FOR VALUES ...`). The dialect normalizer represents partition children as `PartitionElement` entries within the parent table's `Partition`, not as separate `SchemaObject::Table` values. This prevents the diff engine from generating independent `DropTable` / `CreateTable` for partition children.

**MySQL partitioning**: MySQL partitions are always subordinate to the parent table. Sub-partitions (e.g., `RANGE` then `HASH`) are representable by nesting: `PartitionElement.extra["mysql.subpartitions"]` stores the sub-partition specification as a `Value::String` containing the normalized SQL fragment.

#### Exclusion Constraint Definition

Exclusion constraints are PostgreSQL-specific. They use GiST (or other) indexes to enforce that no two rows satisfy a given set of operator comparisons simultaneously.

```rust
pub struct ExclusionConstraint {
    pub name: Option<Ident>,
    pub index_method: String,             // "gist", "spgist", etc.
    pub elements: Vec<ExclusionElement>,
    pub where_clause: Option<Expr>,       // partial exclusion
    pub deferrable: Option<Deferrable>,
}

pub struct ExclusionElement {
    pub expr: Expr,                       // column or expression
    pub operator: String,                 // e.g., "=", "&&", "WITH =", etc.
    pub opclass: Option<String>,          // operator class override
    pub order: Option<SortOrder>,         // ASC/DESC (rare but allowed)
    pub nulls: Option<NullsOrder>,        // NULLS FIRST/LAST
}

pub enum Deferrable {
    Deferrable { initially_deferred: bool },
    NotDeferrable,
}

pub enum SortOrder { Asc, Desc }
pub enum NullsOrder { First, Last }
```

**Diff semantics**: Exclusion constraints are compared by name (when named) or by structural equality of `elements` + `where_clause` (when unnamed). A difference produces `DropExclusion` + `AddExclusion` — there is no `ALTER EXCLUSION CONSTRAINT`.

#### Expression Representation

The IR uses a structured expression AST with a controlled `Raw` escape hatch, with canonicalization required for idempotent comparison.
Decision history is captured in [ADR-0004](docs/adr/0004-expression-representation-and-canonicalization.md) and [ADR-0015](docs/adr/0015-equivalence-policy-injection.md).

The expression AST must be rich enough to support the normalization patterns required for idempotent comparison, particularly those identified in the Go implementation's `normalize.go` (1,412 lines). The following variants are the minimum set needed to avoid pushing too many expressions into `Raw`:

```rust
pub enum Expr {
    // --- Leaf nodes ---
    Literal(Literal),
    Ident(Ident),
    QualifiedIdent { qualifier: Ident, name: Ident },
    Null,

    // --- Operators ---
    BinaryOp { left: Box<Expr>, op: BinaryOperator, right: Box<Expr> },
    UnaryOp { op: UnaryOperator, expr: Box<Expr> },
    Comparison {
        left: Box<Expr>,
        op: ComparisonOp,  // =, <>, <, >, <=, >=
        right: Box<Expr>,
        /// ANY/ALL modifier on the right operand.
        quantifier: Option<SetQuantifier>,
    },
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    Is { expr: Box<Expr>, test: IsTest },  // IS NULL, IS NOT NULL, IS TRUE, etc.

    // --- Range ---
    Between { expr: Box<Expr>, low: Box<Expr>, high: Box<Expr>, negated: bool },
    In { expr: Box<Expr>, list: Vec<Expr>, negated: bool },

    // --- Grouping ---
    Paren(Box<Expr>),
    Tuple(Vec<Expr>),

    // --- Functions and type operations ---
    Function { name: String, args: Vec<Expr>, distinct: bool, over: Option<WindowSpec> },
    Cast { expr: Box<Expr>, data_type: DataType },
    Collate { expr: Box<Expr>, collation: String },

    // --- Compound ---
    Case {
        operand: Option<Box<Expr>>,
        when_clauses: Vec<(Expr, Expr)>,
        else_clause: Option<Box<Expr>>,
    },
    ArrayConstructor(Vec<Expr>),
    Exists(Box<SubQuery>),

    /// Raw SQL expression. Used only when structured representation
    /// is not feasible. The string must be in *canonical form*
    /// as returned by the database's own expression rendering,
    /// not from user input. See "Raw Expression Canonicalization" below.
    Raw(String),
}

pub enum IsTest {
    Null, NotNull, True, NotTrue, False, NotFalse, Unknown, NotUnknown,
}

pub enum SetQuantifier {
    Any, All,
}
```

The set of variants is derived from the Go implementation's normalization requirements:
- `Comparison` with `SetQuantifier` is needed for PostgreSQL's `IN` → `= ANY(ARRAY[...])` normalization.
- `Between` is needed for PostgreSQL's `BETWEEN` → `>= AND <=` expansion.
- `Paren` is needed to correctly remove redundant parentheses during normalization.
- `Case` is needed to handle PostgreSQL's implicit `ELSE NULL` removal.
- `Is` is needed to distinguish `IS NULL` from `= NULL` semantics.
- `ArrayConstructor` is needed for PostgreSQL array expression normalization.
- `Collate` is needed for charset/collation normalization.

Variants not included (handled by `Raw`): subqueries in expressions, window function frame clauses, and complex type constructor syntax. These are uncommon in CHECK/DEFAULT expressions and can be promoted to structured variants incrementally as false diffs are discovered.

#### Optional Semantic Equivalence Hooks

Normalization remains the primary mechanism for idempotent comparison. The optional `EquivalencePolicy` hook is a **secondary safety valve** for cases where normalized forms still differ textually but are equivalent in the target database.

Comparison order for expressions and `DataType::Custom`:
1. Compare normalized values with strict equality.
2. If unequal, call the injected policy (`is_equivalent_expr` / `is_equivalent_custom_type`).
3. Treat values as changed only if both checks fail.

Policy constraints:
- Must be pure and deterministic (no DB lookups, no I/O, no global mutable state).
- Must be symmetric and stable across runs.
- Must be narrowly scoped; broad "always true" rules are forbidden.

This keeps the diff engine dialect-agnostic while allowing dialect-specific equivalence knowledge to be injected without embedding dialect branches into core.

#### `DataType::Custom` and Normalizer Responsibility

`DataType::Custom(String)` is the catch-all for types not covered by the `DataType` enum variants. The dialect normalizer bears the **primary** responsibility for ensuring `Custom` values compare correctly, with `EquivalencePolicy::is_equivalent_custom_type` as a narrow fallback for residual equivalent forms. Specifically:

**When `Custom` is used**: Any type that has no dedicated `DataType` variant falls into `Custom`. Examples:
- MySQL: `TINYINT`, `MEDIUMINT`, `ENUM('a','b')`, `SET('x','y')`, `YEAR`, `BINARY(16)`
- PostgreSQL: `CIDR`, `INET`, `MACADDR`, `TSTZRANGE`, `HSTORE`, `VECTOR(3)` (pgvector)
- MSSQL: `NVARCHAR(MAX)`, `MONEY`, `HIERARCHYID`, `XML`
- SQLite: `TINYINT`, `MEDIUMINT` (SQLite has flexible type affinity)

**Normalizer contract for `Custom`**: Each dialect's `normalize()` must ensure that semantically identical types produce the identical `Custom` string. This means:
1. **Alias resolution**: The normalizer resolves aliases before storing in `Custom`. For example, MySQL's `BOOL` → `DataType::Boolean` (a structured variant), but `TINYINT(1)` → `Custom("tinyint")` (with display width stripped, since MySQL 8.0+ ignores it).
2. **Case normalization**: The `Custom` string must be in a consistent case (lowercase by convention).
3. **Parameter normalization**: Type parameters must be in a canonical form. For example, `VARCHAR(255)` uses the structured `DataType::Varchar`, but `ENUM('a', 'b')` → `Custom("enum('a','b')")` with consistent spacing and quoting.
4. **Default parameter elision**: Redundant defaults must be stripped. For example, PostgreSQL's `NUMERIC` (no precision) should not differ from `NUMERIC(0,0)` if the database treats them identically.

The **current-side** (from `adapter.export_schema()`) is already canonical because the database returns its own normalized form. The normalizer's primary work is on the **desired-side** (from user input).

A false diff caused by insufficient `Custom` normalization is a bug (consistent with §7.1's acceptance criterion).


### 3.5 Diff Engine

The diff engine operates on the canonical IR and produces `DiffOp` values. It is dialect-agnostic.

Every `SchemaObject` variant has corresponding `DiffOp` variants. This exhaustiveness is enforced by design: the diff engine's match on `SchemaObject` must produce `DiffOp` values for every case, so adding a new `SchemaObject` variant without adding matching `DiffOp` variants is a compile error.

```rust
pub enum DiffOp {
    // --- Table ---
    CreateTable(Table),
    DropTable(QualifiedName),
    RenameTable { from: QualifiedName, to: QualifiedName },

    // --- Column (scoped to a table) ---
    AddColumn { table: QualifiedName, column: Column, position: Option<ColumnPosition> },
    DropColumn { table: QualifiedName, column: Ident },
    AlterColumn { table: QualifiedName, column: Ident, changes: Vec<ColumnChange> },
    RenameColumn { table: QualifiedName, from: Ident, to: Ident },

    // --- Index (top-level, with owner) ---
    AddIndex(IndexDef),
    DropIndex { owner: IndexOwner, name: Ident },
    RenameIndex { owner: IndexOwner, from: Ident, to: Ident },

    // --- Foreign Key (scoped to a table) ---
    AddForeignKey { table: QualifiedName, fk: ForeignKey },
    DropForeignKey { table: QualifiedName, name: Ident },

    // --- Check Constraint (scoped to a table) ---
    AddCheck { table: QualifiedName, check: CheckConstraint },
    DropCheck { table: QualifiedName, name: Ident },

    // --- Exclusion Constraint (PostgreSQL, scoped to a table) ---
    AddExclusion { table: QualifiedName, exclusion: ExclusionConstraint },
    DropExclusion { table: QualifiedName, name: Ident },

    // --- Primary Key ---
    SetPrimaryKey { table: QualifiedName, pk: PrimaryKey },
    DropPrimaryKey { table: QualifiedName },

    // --- Partition (scoped to a table) ---
    AddPartition { table: QualifiedName, partition: Partition },
    DropPartition { table: QualifiedName, name: Ident },

    // --- View ---
    CreateView(View),
    DropView(QualifiedName),

    // --- Materialized View ---
    CreateMaterializedView(MaterializedView),
    DropMaterializedView(QualifiedName),

    // --- Sequence ---
    CreateSequence(Sequence),
    DropSequence(QualifiedName),
    AlterSequence { name: QualifiedName, changes: Vec<SequenceChange> },

    // --- Trigger ---
    CreateTrigger(Trigger),
    DropTrigger { name: QualifiedName, table: Option<QualifiedName> },

    // --- Function ---
    CreateFunction(Function),
    DropFunction(QualifiedName),

    // --- Type (ENUM, composite) ---
    CreateType(TypeDef),
    DropType(QualifiedName),
    AlterType { name: QualifiedName, change: TypeChange },

    // --- Domain (PostgreSQL) ---
    CreateDomain(Domain),
    DropDomain(QualifiedName),
    AlterDomain { name: QualifiedName, change: DomainChange },

    // --- Extension (PostgreSQL) ---
    CreateExtension(Extension),
    DropExtension(QualifiedName),

    // --- Schema ---
    CreateSchema(SchemaDef),
    DropSchema(QualifiedName),

    // --- Comment ---
    SetComment(Comment),
    DropComment { target: CommentTarget },

    // --- Privilege ---
    Grant(Privilege),
    Revoke(Privilege),

    // --- Policy (PostgreSQL RLS) ---
    CreatePolicy(Policy),
    DropPolicy { name: Ident, table: QualifiedName },

    // --- Table Options ---
    AlterTableOptions { table: QualifiedName, options: TableOptions },
}

pub enum ColumnChange {
    SetType(DataType),
    SetNotNull(bool),
    SetDefault(Option<Expr>),
    SetIdentity(Option<Identity>),
    SetGenerated(Option<GeneratedColumn>),
    SetCollation(Option<String>),
}

pub enum SequenceChange {
    SetType(DataType),
    SetIncrement(i64),
    SetMinValue(Option<i64>),
    SetMaxValue(Option<i64>),
    SetStart(i64),
    SetCache(i64),
    SetCycle(bool),
}

pub enum TypeChange {
    AddValue { value: String, position: Option<EnumValuePosition> },
    RenameValue { from: String, to: String },
}

pub enum DomainChange {
    SetDefault(Option<Expr>),
    SetNotNull(bool),
    AddConstraint { name: Option<Ident>, check: Expr },
    DropConstraint(Ident),
}
```

#### Ownership and Clone Policy in Diff Planning

IR values and `DiffOp` payloads are intentionally **owned** (`String`, `Vec`, struct values), not lifetime-borrowed references. For example, `CreateTable(Table)` owns `Table` by value.

Design rules:
- **Clone at phase boundaries is acceptable**: parse → normalize → diff → generate_ddl may duplicate IR fragments for clarity and safety.
- **Avoid lifetime-heavy public planning APIs**: introducing long-lived borrows across these phases increases complexity with limited benefit.
- **Optimize with evidence**: if profiling later shows clone hotspots, optimize locally (e.g., string interning or targeted `Arc` usage) without changing the ownership model of core APIs.

Rationale: schema-management workloads are typically small enough that the clarity and correctness benefits of owned transforms outweigh micro-optimizations.
Decision rationale is captured in [ADR-0016](docs/adr/0016-view-rebuild-scope-and-owned-diff-planning.md).

#### View and Materialized View Diff Semantics

The `DiffOp` enum intentionally does **not** include `AlterView` or `AlterMaterializedView`. When a view's or materialized view's definition changes (query text, column list, or options), the diff engine emits a `DropView` + `CreateView` pair (or `DropMaterializedView` + `CreateMaterializedView` for materialized views).

**Rationale**: Most databases do not support in-place alteration of view definitions beyond `CREATE OR REPLACE VIEW`, and even that has restrictive compatibility requirements. Representing view changes as Drop + Create at the `DiffOp` level keeps the core diff engine simple and dialect-agnostic.

**Dialect-level optimization**: The dialect's `generate_ddl()` may optimize a `DropView` + `CreateView` pair into `CREATE OR REPLACE VIEW` when the dialect supports it and the change is compatible. The compatibility conditions are dialect-specific:
- **PostgreSQL**: `CREATE OR REPLACE VIEW` is valid when all existing columns remain with identical types and only new columns are appended. If existing columns are removed or their types change, the dialect falls back to `DROP VIEW` + `CREATE VIEW`.
- **MySQL**: `CREATE OR REPLACE VIEW` is generally permitted. The dialect may always emit `CREATE OR REPLACE VIEW` instead of the Drop + Create pair.
- **SQLite, MSSQL**: These do not support `CREATE OR REPLACE VIEW`; the dialect emits `DROP VIEW` + `CREATE VIEW`.

**Dependent view rebuild scope (core responsibility)**: When a base view must be dropped/recreated, the **diff engine** computes the transitive closure of dependent views and emits explicit `DropView`/`CreateView` ops for the full rebuild set, including dependent views whose definitions are unchanged.

Rebuild rules:
1. Build view dependency graphs from current and desired schemas.
2. For a changed base view, collect reverse dependencies from current and emit `DropView` in reverse topological order (dependents first).
3. Collect desired definitions for the rebuild set and emit `CreateView` in topological order (dependencies first), including unchanged dependents.
4. If a dependent view exists only in current (not desired), emit only `DropView` (subject to `enable_drop` policy).

Dialect `generate_ddl()` may optimize the explicit sequence (e.g., use `CREATE OR REPLACE VIEW` where valid), but it must not discover additional hidden dependents from ad-hoc catalog inspection.
Decision rationale is captured in [ADR-0016](docs/adr/0016-view-rebuild-scope-and-owned-diff-planning.md).

**Materialized views**: Always `DROP` + `CREATE`. `CREATE OR REPLACE MATERIALIZED VIEW` is not part of standard SQL and is not supported by PostgreSQL.

#### SQL Generation: Batch-Based API

The diff engine produces `DiffOp` values, but SQL generation receives the **full batch** of operations, not individual ops. This is critical because many dialects need to:
- **Merge operations**: MySQL's `ALTER TABLE t CHANGE COLUMN c ...` combines type, nullability, and default changes into one statement. Multiple `AddColumn` ops on the same table can be merged into one `ALTER TABLE`.
- **Rewrite as table recreation**: SQLite lacks most `ALTER TABLE` support; the dialect must detect when column changes require the 12-step table recreation workflow (see "SQLite Table Recreation" below).
- **Enforce batch boundaries**: MSSQL requires that certain DDL statements run in separate batches (in the `sqlcmd` client this is expressed as `GO`). Since `GO` is a client-side directive and not valid SQL, the dialect emits `Statement::BatchBoundary` to mark these boundaries. The `Renderer` (dry-run/export) converts `BatchBoundary` to `"GO\n"` via `Dialect::batch_separator()`; the `Executor` (online) treats it as a synchronization point and never sends the boundary itself to the database. See §3.7 for the precise execution semantics.
- **Render explicit dependency plans**: for view rebuilds, the diff engine already expands dependent views into explicit `DropView`/`CreateView` ops; dialects render this plan rather than recomputing dependency closure.

```rust
/// A generated SQL statement with execution metadata.
pub enum Statement {
    /// A SQL statement to execute against the database.
    Sql {
        sql: String,
        /// Whether this statement can run inside a transaction.
        /// The executor uses this to determine transaction boundaries.
        transactional: bool,
        /// Optional high-level execution context for diagnostics.
        /// Used to enrich `ExecutionError` without parsing SQL text.
        context: Option<StatementContext>,
    },
    /// A batch boundary (e.g., MSSQL's GO).
    /// Not sent to the database. Acts as a synchronization point:
    /// the executor ensures the preceding statement has completed
    /// before executing the next one (see §3.7 for semantics).
    /// The renderer emits the dialect's batch separator string
    /// (e.g., "GO\n") for dry-run output.
    BatchBoundary,
}

/// Optional execution context attached to a generated statement.
pub enum StatementContext {
    SqliteTableRebuild {
        table: QualifiedName,
        step: SqliteRebuildStep,
    },
}

pub enum SqliteRebuildStep {
    CreateShadowTable,
    CopyData,
    DropOldTable,
    RenameShadowTable,
    RecreateIndexes,
    RecreateTriggers,
}
```

The diff algorithm:

```rust
pub struct DiffConfig {
    /// When false, DROP and REVOKE operations are suppressed.
    /// See "enable_drop policy" below for the precise semantics.
    pub enable_drop: bool,

    /// Schema search path, ordered by priority.
    /// Used for matching unqualified names against qualified names.
    /// When an unqualified name is encountered, the diff engine checks
    /// each schema in order and matches the first one found.
    ///
    /// Examples:
    /// - PostgreSQL: `["public"]` (default), or `["app", "public"]` for
    ///   `search_path = 'app,public'`.
    /// - MySQL: `["mydb"]` (the connected database).
    /// - MSSQL: `["dbo"]` (the default schema).
    /// - SQLite: empty (no schema concept).
    ///
    /// See ADR-0014 for the decision rationale.
    pub schema_search_path: Vec<String>,

    /// Semantic equivalence rules injected by the orchestrator.
    ///
    /// The diff engine remains dialect-agnostic: it depends on this policy
    /// interface, not on `&dyn Dialect`.
    pub equivalence_policy: Arc<dyn EquivalencePolicy>,
}

impl DiffEngine {
    /// Compare desired schema against current schema and produce operations.
    pub fn diff(
        &self,
        desired: &[SchemaObject],
        current: &[SchemaObject],
        config: &DiffConfig,
    ) -> Result<Vec<DiffOp>> {
        // 1. Build lookup maps for current and desired schemas
        // 2. For each desired object, find its current counterpart
        //    (respecting @renamed annotations for rename detection)
        // 3. If not found → Create
        // 4. If found → compare and generate change ops
        //    (using config.equivalence_policy for semantic equivalence hooks)
        // 5. If current object has no desired counterpart:
        //    - If enable_drop → generate Drop/Revoke ops
        //    - If !enable_drop → generate SkippedDrop diagnostic (see below)
        // 6. Sort the resulting ops by the ordering rules (see below)
    }
}
```

#### `enable_drop` Policy

When `DiffConfig.enable_drop` is `false`, the diff engine **does not emit** DROP-category or REVOKE operations. Instead, it returns them as diagnostics so the renderer can display them as comments (e.g., `-- Skipped: DROP TABLE ...`) in dry-run output.

Operations suppressed by `enable_drop: false`:
- `DropTable`, `DropView`, `DropMaterializedView`, `DropSequence`
- `DropTrigger`, `DropFunction`, `DropType`, `DropDomain`
- `DropExtension`, `DropSchema`, `DropPolicy`
- `DropColumn`, `DropIndex`, `DropForeignKey`, `DropCheck`, `DropExclusion`
- `DropPrimaryKey`, `DropPartition`, `DropComment`
- `Revoke`

Operations **not** suppressed (always emitted regardless of `enable_drop`):
- **Constraint modification drops**: a Drop op that is part of a drop-and-recreate pair for modifying a constraint. See "Constraint Modification Pairing" below.
- All CREATE and ALTER operations.

This matches the Go implementation's behavior where `DROP CONSTRAINT` for constraint modification is not suppressed because it is required for non-destructive schema changes.

**Constraint modification pairing**: The diff engine recognizes a drop-and-recreate pair when **all** of the following conditions hold:

1. The Drop op and the corresponding Add op target the **same table** and the **same constraint kind** (CHECK, EXCLUSION, FOREIGN KEY, or PRIMARY KEY).
2. The constraints are matched by **name**: the dropped constraint's name equals the added constraint's name. For unnamed constraints (common in CHECK), matching falls back to same-table, same-kind, and the diff engine confirms only one constraint of that kind is being dropped and one added.
3. The constraint **body differs** between current and desired (expression, columns, or referenced table). If the body is identical, no ops are emitted at all (idempotent).

When these conditions are met, the diff engine emits the Drop op paired with the Add op as a unit. The `enable_drop: false` policy recognizes this pair and allows the Drop through.

Constraint **renames** (same body, different name) are not supported as a modification pair. They produce a Drop + Add pair that **is** suppressed by `enable_drop: false`, because the rename itself is destructive (it changes the constraint's identity).

#### DiffOp Ordering

The diff engine sorts operations into a fixed priority order, with dependency-based sorting within each priority group:

```
 -- Drop phase (reverse dependency order) --
Priority 1:  DropPolicy        — must precede table/view drops
Priority 2:  DropTrigger       — must precede table drops to avoid dangling references
Priority 3:  DropView, DropMaterializedView — must precede column drops they depend on
Priority 4:  DropForeignKey    — must precede table/index drops and PK changes
Priority 5:  DropIndex         — must precede table drops
Priority 6:  DropTable         — sorted by reverse FK dependency order
Priority 7:  DropSequence      — after tables that reference them are dropped
Priority 8:  DropDomain        — after tables using the domain are dropped
Priority 9:  DropType          — after tables/domains using the type are dropped
Priority 10: DropFunction      — after triggers that call the function are dropped
Priority 11: DropSchema        — after all objects in the schema are dropped
Priority 12: DropExtension     — after types/functions provided by the extension are dropped

 -- Create / Alter phase (dependency order) --
Priority 13: CreateExtension   — extensions provide types/functions
Priority 14: CreateSchema      — namespaces must exist before objects
Priority 15: CreateType        — types used by table columns and domains
Priority 16: AlterType         — e.g., ADD VALUE to enum, must precede column changes using new value
Priority 17: CreateDomain      — domains used by table columns
Priority 18: AlterDomain       — must precede table columns that depend on domain constraints
Priority 19: CreateSequence    — sequences referenced by column defaults
Priority 20: AlterSequence     — must precede column default changes referencing sequence properties
Priority 21: CreateTable       — sorted by FK dependency order
Priority 22: Table-scoped modifications (see "Intra-Table Operation Order" below)
Priority 23: AddForeignKey     — must follow table creation
Priority 24: CreateView        — sorted by view dependency order
Priority 25: CreateMaterializedView
Priority 26: AddIndex          — after table/view creation
Priority 27: CreateTrigger, CreateFunction
Priority 28: CreatePolicy
Priority 29: SetComment, DropComment
Priority 30: Grant, Revoke
```

Within the same priority group, operations are sorted by:
1. **FK dependency order** for tables: tables that are referenced by foreign keys are created before the tables that reference them.
2. **View dependency order** for views: views that reference other views/tables are created after their dependencies. This ordering applies to both directly changed views and unchanged dependents included by rebuild-scope expansion.
3. **Original declaration order** for independent objects: preserves the user's intended ordering.

#### Intra-Table Operation Order (Priority 22)

Operations within Priority 22 that target the **same table** are further sorted into sub-priorities. Operations targeting **different tables** are ordered by original declaration order.

```
22a: RenameTable
22b: RenameColumn
22c: AlterColumn         — type, nullability, default, identity, generated, collation
22d: AddColumn
22e: DropColumn
22f: SetPrimaryKey, DropPrimaryKey
22g: AddCheck, DropCheck, AddExclusion, DropExclusion
22h: AddPartition, DropPartition
22i: AlterTableOptions
```

**Ordering rationale**:
- **RenameTable/RenameColumn first (22a–22b)**: Subsequent operations reference the new names. A `RenameColumn` followed by `AlterColumn` on the same column must use the new name.
- **AlterColumn before AddColumn (22c before 22d)**: Existing column type changes may affect DEFAULT expressions that reference the column. Completing type changes before adding new columns avoids ambiguity.
- **DropColumn after AddColumn (22e after 22d)**: MySQL's `AFTER` position clause in `AddColumn` references the column layout after additions, so drops must come later to avoid referencing a dropped column.
- **PK changes after column changes (22f)**: Primary key modifications depend on final column types and nullability constraints being in place.
- **AlterTableOptions last (22i)**: Table-level options (e.g., `COMMENT`, `ENGINE`) are independent of column layout and have no ordering dependencies.

#### Circular Dependency Handling

Foreign key dependencies can be circular (table A references table B, and table B references table A). The diff engine handles this as follows:

1. **Self-referential FKs** are excluded from the dependency graph (they cannot cause ordering issues since the table already exists when the FK is added).
2. **Circular FK dependencies** among CREATE TABLE operations: if Kahn's algorithm detects a cycle, the engine falls back to the original declaration order for the cycle participants. The foreign keys that form the cycle are emitted as separate `AddForeignKey` ops (Priority 23) rather than being embedded in `CreateTable`. This allows the tables to be created first, then the circular FK constraints added afterward.
3. **Circular FK dependencies** among DROP TABLE operations: if a cycle is detected, the engine falls back to the original declaration order. In practice, `DropForeignKey` ops (Priority 4) run before `DropTable` ops (Priority 6), which breaks the dependency.

Key design differences from sqldef:
- The diff engine produces **structured `DiffOp` values**, not SQL strings. The dialect's `generate_ddl(&[DiffOp])` receives the full plan and converts it to SQL. This cleanly separates "what changed" from "how to express it in SQL".
- The diff engine is fully dialect-agnostic — it does not take `&dyn Dialect`. Normalization is applied to the IR before diffing, and optional semantic equivalence is provided through `DiffConfig.equivalence_policy`.

#### SQLite Table Recreation

SQLite supports only a limited subset of `ALTER TABLE` (add column, rename column, rename table). Any other column modification (type change, adding/removing NOT NULL, changing defaults, modifying CHECK constraints) requires the dialect's `generate_ddl` to synthesize a **table recreation sequence**. This sequence includes DML statements (`INSERT INTO ... SELECT`) alongside DDL.

The SQLite dialect's `generate_ddl` detects when a batch of `AlterColumn`, `DropColumn`, `AddCheck`, `DropCheck`, or `AddExclusion` operations on the same table cannot be expressed as simple `ALTER TABLE` statements, and rewrites them as:

```
1. CREATE TABLE _new_t (... desired columns and constraints ...)
2. INSERT INTO _new_t (col1, col2, ...) SELECT col1, col2, ... FROM t
3. DROP TABLE t
4. ALTER TABLE _new_t RENAME TO t
5. (re-create indexes on t)
6. (re-create triggers on t)
```

All statements in this sequence are emitted as `Statement::Sql { transactional: true, context: Some(StatementContext::SqliteTableRebuild { ... }) }`. The executor wraps them in a single transaction, ensuring atomicity: if any step fails (e.g., data type incompatibility during INSERT), the entire recreation is rolled back.

The `INSERT INTO ... SELECT` is a DML statement, but it appears within the `Vec<Statement>` output from `generate_ddl`. This is by design: the `Statement` type represents any SQL to execute, not exclusively DDL. The `transactional: true` flag ensures correct transaction grouping.

If execution fails during this flow, `ExecutionError` includes the `StatementContext::SqliteTableRebuild` payload so the CLI can present a targeted hint (for example, "failed while copying data into shadow table during SQLite table rebuild").

### 3.6 Per-Dialect Parsers

Each dialect has its own parser. The selected parser policy is:

| Dialect | Parser Strategy | Rationale |
|---------|-----------------|-----------|
| PostgreSQL | `pg_query` (via `pg_query.rs` crate) | Native PostgreSQL parser via libpg_query. Full fidelity. |
| MySQL | `sqlparser-rs` with MySQL dialect (initially) | No widely-used native MySQL parser exists. |
| SQLite | `sqlparser-rs` with SQLite dialect (initially) | SQLite's SQL is relatively simple. |
| MSSQL | `sqlparser-rs` with MSSQL dialect (initially) | T-SQL is complex but the DDL subset is manageable for v1. |

`sqlparser-rs`-based dialects can be extended or replaced when coverage gaps are found.
Decision history and trade-offs are captured in [ADR-0010](docs/adr/0010-parser-selection-policy.md).

#### Parser Architecture

```rust
/// Each dialect implements parsing independently.
/// Every parse() implementation follows the 3-step pipeline from §3.4:
///   1. Extract @renamed annotations (core-provided)
///   2. Parse annotation-free SQL (per-dialect)
///   3. Attach annotations to matching SchemaObjects
///
/// This example shows the PostgreSQL implementation using pg_query.rs.
impl Dialect for PostgresDialect {
    fn parse(&self, sql: &str) -> Result<Vec<SchemaObject>> {
        // Step 1: Extract annotations before parsing (§3.4).
        let (clean_sql, annotations) = AnnotationExtractor::extract(sql)?;

        // Step 2: Parse annotation-free SQL.
        let result = pg_query::parse(&clean_sql)?;
        let mut objects = Vec::new();
        for (idx, stmt) in result.protobuf.stmts.into_iter().enumerate() {
            // Every statement must be converted. If convert_statement
            // cannot handle a statement type, it returns Err, not None.
            // This prevents silent data loss that could cause false DROPs.
            let obj = self.convert_statement(stmt.clone()).map_err(|source| {
                ParseError::StatementConversion {
                    statement_index: idx,
                    source_sql: stmt.to_string(),
                    source: Box::new(source),
                }
            })?;
            objects.push(obj);
        }

        // Step 3: Attach annotations to matching objects.
        // If an annotation references an object not produced by the parser,
        // this returns Err (prevents stale annotations from being ignored).
        attach_annotations(&mut objects, &annotations)?;
        Ok(objects)
    }
}

/// MySQL implementation using sqlparser-rs.
impl Dialect for MysqlDialect {
    fn parse(&self, sql: &str) -> Result<Vec<SchemaObject>> {
        // Step 1: Extract annotations before parsing (§3.4).
        let (clean_sql, annotations) = AnnotationExtractor::extract(sql)?;

        // Step 2: Parse annotation-free SQL.
        let dialect = sqlparser::dialect::MySqlDialect {};
        let ast = sqlparser::parser::Parser::parse_sql(&dialect, &clean_sql)?;
        let mut objects = Vec::new();
        for (idx, stmt) in ast.into_iter().enumerate() {
            let obj = self.convert_statement(stmt.clone()).map_err(|source| {
                ParseError::StatementConversion {
                    statement_index: idx,
                    source_sql: stmt.to_string(),
                    source: Box::new(source),
                }
            })?;
            objects.push(obj);
        }

        // Step 3: Attach annotations to matching objects.
        attach_annotations(&mut objects, &annotations)?;
        Ok(objects)
    }
}
```

To reduce boilerplate in parser adapters, implementations may wrap this pattern in a local helper macro or function (e.g., `map_stmt_err!(idx, stmt, convert_statement(...))`) as long as the same context fields are preserved.

### 3.7 Database Adapters and Transaction Model

The executor receives `Vec<Statement>` from `Dialect::generate_ddl()`. It processes statements as follows:

- `Statement::Sql { transactional: true, context }` — executed within a transaction via `Transaction::execute()`.
- `Statement::Sql { transactional: false, context }` — executed outside any transaction (e.g., `CREATE INDEX CONCURRENTLY` in PostgreSQL).
- `Statement::BatchBoundary` — a synchronization point. Not sent to the database. Used by MSSQL dialect to model `GO` semantics. See below for details.

#### BatchBoundary semantics

`GO` in MSSQL is a client-side batch separator, **not** a transaction control statement. A single transaction can span multiple batches, and a single batch can contain multiple transactions. The `BatchBoundary` marker in this design reflects this:

- **It does NOT imply commit.** The executor does not call `commit()` upon encountering a `BatchBoundary`.
- **It is a synchronization point.** Each `Statement::Sql` is sent to the database via an individual `execute()` call (or `Transaction::execute()` call). `BatchBoundary` ensures the executor waits for the preceding statement to complete and its effects to be visible before executing the next statement. This matters for databases where certain DDL must be fully processed before subsequent DDL can reference it (e.g., MSSQL's `ALTER TABLE` after `CREATE TABLE`).
- **Online execution**: The executor treats `BatchBoundary` as a no-op in terms of SQL sent to the database. Its effect is purely a synchronization guarantee between adjacent statements.
- **Dry-run rendering**: The `Renderer` emits the dialect's batch separator string (e.g., `"GO\n"` for MSSQL).

```
Statements:  [T, T, B, T, T, N, T, T]

T = Sql { transactional: true, context: _ }
N = Sql { transactional: false, context: _ }
B = BatchBoundary (sync point, NOT commit)
```

Transaction lifecycle is controlled by the executor's wrapping strategy, not by `BatchBoundary`:
- The executor wraps the **entire plan** in a single transaction (if all statements are transactional and the database supports transactional DDL).
- Non-transactional statements force a commit-before / begin-after boundary.
- `BatchBoundary` does not affect transaction boundaries.
- `context` does not affect execution behavior; it is propagated only for diagnostics.

If any statement fails, the executor rolls back the current transaction and stops. Non-transactional statements that have already been executed cannot be rolled back (this is inherent to DDL in most databases).

For dry-run / `--export` output, the `Renderer` converts `BatchBoundary` to the dialect's separator string (e.g., `"GO\n"` for MSSQL, empty for other dialects).
Detailed decision rationale is captured in [ADR-0007](docs/adr/0007-batchboundary-semantics.md).

#### Connection Model: Single Connection

The `DatabaseAdapter` represents a **single, dedicated database connection**. All operations (`export_schema`, `execute`, `begin`) and the resulting `Transaction` handle's methods (`execute`, `commit`) occur on this one connection, guaranteeing transaction consistency. This is not a connection pool.

**Rationale**: Schema management is inherently sequential — DDL statements have ordering dependencies and many databases auto-commit DDL. A single connection eliminates the risk of `begin()` and `execute()` running on different connections (which would silently bypass the transaction).
Detailed decision rationale is captured in [ADR-0008](docs/adr/0008-single-connection-adapter.md).

All adapter methods use **synchronous I/O**. Async database drivers (e.g., `tokio-postgres`) are wrapped in blocking calls scoped to the adapter internals; the async boundary does not leak into the trait or any core API.
Decision rationale is captured in [ADR-0011](docs/adr/0011-synchronous-io.md).

```rust
/// A single database connection.
///
/// Implementations must guarantee that all method calls operate on
/// the same underlying connection. This is NOT a connection pool.
/// The connection is established via `Dialect::connect()` and lives
/// for the duration of the schema management operation.
pub trait DatabaseAdapter: Send {
    /// Export the current schema as raw SQL DDL from the database's
    /// system catalog. This is NOT the final --export output.
    /// The orchestrator parses the result via Dialect::parse(),
    /// normalizes, and re-renders via Dialect::to_sql().
    fn export_schema(&self) -> Result<String>;

    /// Execute a single DDL statement on this connection.
    fn execute(&self, sql: &str) -> Result<()>;

    /// Begin a transaction on this connection.
    /// Returns a `Transaction` handle that enforces RAII:
    /// if dropped without calling `commit()`, it rolls back.
    fn begin(&mut self) -> Result<Transaction<'_>>;

    /// Get the schema search path, ordered by priority.
    /// Returns the effective search path for the current connection.
    /// E.g., `["public"]` for PostgreSQL default, `["app", "public"]`
    /// for `search_path = 'app,public'`.
    fn schema_search_path(&self) -> Vec<String>;

    /// Get the database server version.
    fn server_version(&self) -> Result<Version>;
}

/// RAII transaction handle.
/// Automatically rolls back on drop unless `commit()` is called.
/// All SQL execution within a transaction goes through this handle,
/// enforcing that the transaction and execution share the same connection.
pub struct Transaction<'a> {
    adapter: &'a mut dyn DatabaseAdapter,
    committed: bool,
}

impl<'a> Transaction<'a> {
    /// Execute a DDL statement within this transaction.
    pub fn execute(&mut self, sql: &str) -> Result<()>;

    /// Commit the transaction. Consumes the handle.
    pub fn commit(self) -> Result<()>;

    // Drop impl: if !self.committed, execute ROLLBACK.
}
```

#### Executor and Renderer

```rust
/// Statement executor that handles transaction grouping.
/// This is provided by core, not implemented by each dialect.
pub struct Executor<'a> {
    adapter: &'a mut dyn DatabaseAdapter,
}

impl<'a> Executor<'a> {
    /// Execute a plan of statements.
    ///
    /// - `Sql { transactional: true, context }` — the executor opens a transaction
    ///   (via `adapter.begin()`) and executes statements through the
    ///   `Transaction` handle, guaranteeing same-connection execution.
    /// - `Sql { transactional: false, context }` — commits the current transaction
    ///   (if any), then executes the statement directly on the adapter.
    /// - `BatchBoundary` — synchronization point. Does NOT commit or
    ///   affect transaction state. Never sent to the database.
    ///
    /// On failure, the `Transaction` handle's RAII drop triggers rollback.
    /// Returns the error along with failed-statement context
    /// (index, SQL text, optional source location, optional statement context).
    pub fn execute_plan(&mut self, stmts: &[Statement]) -> Result<(), ExecutionError>;
}

/// Renderer for dry-run / --export output.
/// Converts `Vec<Statement>` to human-readable SQL text.
pub struct Renderer<'a> {
    dialect: &'a dyn Dialect,
}

impl<'a> Renderer<'a> {
    /// Render statements as SQL text.
    /// `BatchBoundary` is rendered as the dialect's batch separator
    /// (e.g., "GO\n" for MSSQL, empty for others).
    pub fn render(&self, stmts: &[Statement]) -> String;
}
```

### 3.8 CLI Design

The CLI shape is fixed as a single binary with dialect subcommands:

```
stateql mysql --host localhost --user root mydb < schema.sql
stateql postgres --host localhost --user postgres mydb --apply --file schema.sql
stateql mysql --export mydb > current.sql
```

This project is intentionally **not** a drop-in replacement for sqldef and does not ship `*def` compatibility aliases/symlinks.
CLI arguments should stay close to familiar database-client conventions where useful, but architecture clarity and safety take precedence over strict flag-level compatibility.
Decision details and trade-offs are captured in [ADR-0009](docs/adr/0009-single-binary-cli-shape.md).

#### CLI Flags

The following flags are common to all dialect subcommands:

| Flag | Description | Default |
|------|-------------|---------|
| `--apply` | Apply the desired schema to the database | |
| `--dry-run` | Print the DDL that would be executed, without applying | default when input is provided |
| `--export` | Export the current database schema to stdout | |
| `--file <path>` | Read desired schema from a file (alternative to stdin) | stdin |
| `--enable-drop` | Allow DROP and REVOKE operations | `false` |

Connection flags are dialect-specific, matching the conventions of each database's CLI tool:

| Flag | mysql | postgres | mssql | sqlite |
|------|-------|----------|-------|--------|
| `--host` | ✓ | ✓ | ✓ | — |
| `--port` | ✓ | ✓ | ✓ | — |
| `--user` | ✓ | ✓ | ✓ | — |
| `--password` | ✓ | ✓ | ✓ | — |
| `<database>` | positional | positional | positional | positional (file path) |

Additional dialect-specific flags (e.g., `--socket` for MySQL, `--sslmode` for PostgreSQL) are defined per dialect and documented in the respective `cmd-*def.md` files.

The `--dry-run` mode is the default when neither `--apply` nor `--export` is specified and stdin or `--file` provides input. Users must pass `--apply` explicitly to execute changes against the database. This follows the sqldef v4 approach: safe by default, destructive only when explicitly requested.

### 3.9 Orchestrator

The orchestrator is the control-flow coordinator between CLI, Dialect, Core, and Adapter. It is implemented in the `core` crate (not per-dialect) and drives three modes: `--apply`, `--dry-run`, and `--export`.

#### `--apply` / `--dry-run` Flow

```
CLI
 │  parse CLI args, select dialect
 ▼
Dialect::connect(config)
 │  returns Box<dyn DatabaseAdapter>
 ▼
adapter.export_schema()
 │  returns raw SQL string from system catalog
 ▼
dialect.parse(raw_sql)          ← current schema
 │  returns Vec<SchemaObject>
 ▼
dialect.normalize(&mut obj)     ← called on each current object
 │
 ▼
dialect.parse(desired_sql)      ← desired schema (from --file or stdin)
 │  returns Vec<SchemaObject>
 ▼
dialect.normalize(&mut obj)     ← called on each desired object
 │
 ▼
DiffEngine::diff(desired, current, config)
 │  returns Vec<DiffOp>
 ▼
dialect.generate_ddl(&ops)
 │  returns Vec<Statement>
 ▼
┌─────────────────────────────────────────────────┐
│  --apply:   Executor::execute_plan(&stmts)      │
│  --dry-run: Renderer::render(&stmts) → stdout   │
└─────────────────────────────────────────────────┘
```

**Responsibility boundaries**:
- The **orchestrator** calls `dialect.normalize()` on each `SchemaObject` after parsing and before diffing. The diff engine never calls normalize; it receives pre-normalized IR.
- The **orchestrator** passes `DiffConfig` (including `enable_drop`, `schema_search_path`, and `equivalence_policy` from the selected dialect) to the diff engine.
- The **orchestrator** selects `Executor` or `Renderer` based on the CLI mode. The dialect is unaware of which mode is active.

#### `--export` Flow

```
CLI
 │
 ▼
Dialect::connect(config)
 │
 ▼
adapter.export_schema()
 │  returns raw SQL string
 ▼
dialect.parse(raw_sql)
 │  returns Vec<SchemaObject>
 ▼
dialect.normalize(&mut obj)     ← called on each object
 │
 ▼
dialect.to_sql(&obj)            ← called on each object
 │  returns String
 ▼
stdout (concatenated, one statement per line)
```

The `--export` output must be **idempotent**: re-parsing and re-exporting the output must produce the same text. This is the round-trip invariant that the test harness verifies.

#### Error Handling

All three flows follow the fail-fast policy ([ADR-0013](docs/adr/0013-fail-fast-error-handling.md)). On the first error from any stage, the orchestrator:

1. **Parse/Diff/Generate error**: reports the error and exits immediately. No statements are executed.
2. **Execution error**: the `Executor`'s RAII `Transaction` handle triggers rollback on drop. The error includes the statement index that failed, optional source location (line/column), optional statement context (for example `SqliteTableRebuild` step), and a summary of partial-apply state (how many statements succeeded before the failure). Non-transactional statements that already executed cannot be rolled back; the error message warns the user of this.

The user's recovery path after a partial failure is: run `--export` to inspect the current state, fix the desired schema, and re-apply.

### 3.10 Error Types

The error type hierarchy spans four categories, one per processing stage. Each category carries structured context sufficient for users to locate the failure.

```rust
/// Top-level error type for the orchestrator.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Parse(ParseError),
    #[error(transparent)]
    Diff(DiffError),
    #[error(transparent)]
    Generate(GenerateError),
    #[error(transparent)]
    Execute(ExecutionError),
}
```

#### Required Context by Category

| Category | Required Context | Purpose |
|----------|-----------------|---------|
| `ParseError` | statement index (0-based), source SQL fragment, underlying parser error, optional source location (`line`, `column`) | Locate which SQL statement failed to parse, and where in file input |
| `DiffError` | target object name, operation kind (e.g., "rename annotation mismatch") | Identify annotation errors, circular dependencies, missing owners |
| `GenerateError` | DiffOp kind, target object name, dialect name | Surface unsupported DiffOp for the dialect |
| `ExecutionError` | statement index (0-based), SQL text, underlying adapter error, count of successfully executed statements, optional source location (`line`, `column`), optional `StatementContext` | Locate failure point, map it to user SQL, and assess partial-apply state |

`SourceLocation` is modeled as:

```rust
pub struct SourceLocation {
    pub line: usize,           // 1-based
    pub column: Option<usize>, // 1-based
}
```

Location attachment policy:
- For `--file` input, parsers should attach `SourceLocation` when location data is available from the parser library.
- If parser-native location is unavailable, implementations should attach at least statement start line using the core statement splitter.
- For non-file sources where stable mapping is not available, `SourceLocation` may be `None`.

#### Propagation Rules

- Lower-level errors are wrapped by higher-level errors, preserving the original error as `source` for chaining.
- The orchestrator catches the first error and stops (fail-fast per [ADR-0013](docs/adr/0013-fail-fast-error-handling.md)).
- `ExecutionError` includes how many statements succeeded before the failure, enabling the user to understand the partial-apply state.
- When `Statement::Sql.context` is present, the executor propagates it into `ExecutionError` for diagnostics without changing execution semantics.

#### Error Implementation Layering (`thiserror` / `anyhow` / `miette`)

To keep rich diagnostics without losing typed error guarantees, error crates are used by layer:

- **Core and dialect crates**: Use strongly-typed error enums (`ParseError`, `DiffError`, `GenerateError`, `ExecutionError`) implemented with `thiserror`. Public trait boundaries (`Dialect`, `DatabaseAdapter`, `DiffEngine`) return these typed errors, not `anyhow::Error`.
- **Parser conversion boundary**: When converting parser AST statements to IR, implementations must attach statement-level context immediately (statement index and SQL fragment; source location when available), so conversion failures satisfy the `ParseError` context contract.
- **CLI boundary**: `anyhow` is allowed only in the top-level command path to add operator-facing context (`.context("...")`) while wiring components. Before crossing into core APIs, errors are mapped back to typed categories.
- **Human-facing diagnostics**: `miette` (or equivalent) may be used in the CLI presentation layer to render rich diagnostics. It is an output concern and must not replace the typed core error model.

Example conversion pattern for statement-level context:

```rust
for (idx, stmt) in ast.into_iter().enumerate() {
    let obj = self.convert_statement(stmt.clone()).map_err(|source| {
        ParseError::StatementConversion {
            statement_index: idx,
            source_sql: stmt.to_string(),
            source_location: statement_location(stmt),
            source: Box::new(source),
        }
    })?;
    objects.push(obj);
}
```

#### `generate_ddl` Contract for Unsupported DiffOps

When a dialect's `generate_ddl()` receives a `DiffOp` it does not support (e.g., SQLite receiving `AddExclusion`), it must return `GenerateError`. Silent ignoring is prohibited — this is the dialect-side complement to the core's fail-fast parse behavior. Together they guarantee: no `DiffOp` is silently dropped at any stage of the pipeline.

---


## 4. Comparison: sqldef vs. Proposed Architecture

| Aspect | sqldef (Go) | Proposed (Rust) |
|--------|-------------|-----------------|
| **Parser** | Single yacc grammar for all dialects | Per-dialect parser (pg_query, sqlparser-rs, custom) |
| **AST** | Single AST type shared by all dialects | Per-dialect AST → canonical IR |
| **Diff engine** | Monolithic function with switch on mode | Dialect-agnostic diff producing `DiffOp` values |
| **DDL generation** | Embedded in diff engine with mode switches | Delegated to `Dialect::generate_ddl(&[DiffOp])` (batch) |
| **Normalization** | Global normalizer compensating for parser | Per-dialect normalizer producing clean IR |
| **Extensibility** | Fork required for new database | New dialect: implement `Dialect` trait + recompile. New object kind: core change required |
| **Binary** | 4 separate binaries | 1 binary with subcommands (no `*def` aliases) |
| **Test format** | YAML (Go test runner) | Same YAML (Rust test runner) |
| **I/O model** | Synchronous (with goroutine concurrency for export) | Synchronous (single-threaded, no async runtime) |
| **Dependencies** | pgquery via Wasm, database drivers | pg_query.rs (native), rusqlite, sqlx (blocking), etc. |

---

## 5. Implementation Strategy

### 5.1 Phase 1: Foundation

1. Set up Cargo workspace with `core` and `testkit` crates.
2. Define the canonical IR types (`ir.rs`).
3. Implement the diff engine for tables/columns/indexes (`diff.rs`).
4. Implement topological sort (`ordering.rs`).
5. Implement the YAML test loader in `testkit`.
6. **IR validation**: write unit tests that verify the IR can represent key dialect-specific patterns from MySQL and MSSQL without IR changes. Specifically:
   - MySQL `CHANGE COLUMN` (full column redefinition): representable as `AlterColumn` with all `ColumnChange` variants set simultaneously.
   - MySQL column positioning (`AFTER`): representable via `ColumnPosition` in `AddColumn`.
   - MSSQL named DEFAULT constraints: representable via `Column.extra` (constraint name stored as dialect-specific attribute).
   - MSSQL `sp_rename`: representable as `RenameTable` / `RenameColumn` / `RenameIndex` (the dialect generator emits `EXEC sp_rename` SQL).
   - MySQL `AUTO_INCREMENT` two-phase handling: representable via `Column.extra` for the auto-increment flag, with the dialect generator handling the PK-change-before-auto-increment ordering.

### 5.2 Phase 2: First Dialect (PostgreSQL)

PostgreSQL is recommended as the first dialect because:
- `pg_query.rs` provides a production-quality parser.
- PostgreSQL has the richest feature set (schemas, types, domains, policies), so it exercises the IR design most thoroughly.
- sqldef already has the most PostgreSQL test cases.

Steps:
1. Implement `dialect-postgres` parser using `pg_query.rs`.
2. Implement PostgreSQL DDL generator.
3. Implement PostgreSQL database adapter.
4. Port PostgreSQL YAML tests and get them passing.

### 5.3 Phase 3: SQLite

SQLite is the simplest dialect and provides a good second validation of the architecture:
- Limited ALTER TABLE support forces testing of table recreation patterns.
- No authentication simplifies adapter development.
- Fast test execution (in-memory databases).

### 5.4 Phase 4: MySQL

MySQL has the most users and the most dialect-specific features:
- `CHANGE COLUMN` syntax
- `AUTO_INCREMENT`
- Column positioning (`AFTER`)
- Partitioning
- `lower_case_table_names`

### 5.5 Phase 5: MSSQL

MSSQL has the most complex DDL generation:
- T-SQL batch boundaries modeled as `Statement::BatchBoundary` (rendered as `GO` in dry-run output, acts as synchronization point during execution — see §3.7)
- `sp_rename` for renames
- `IDENTITY` with seed/increment
- Clustered indexes
- `NOT FOR REPLICATION`

### 5.6 Phase 6: Dialect Trait Documentation and API Stabilization

- Document the `Dialect` trait contract.
- Publish `core` and `testkit` crates.
- Provide a template for third-party dialect implementations.

---

## 6. Testing Strategy

### 6.1 Test Layers

```
Unit Tests (per-crate)
├── core/diff tests:      DiffOp generation from IR pairs
├── core/ordering tests:  Topological sort correctness
├── dialect/parser tests: SQL → IR conversion
└── dialect/gen tests:    DiffOp → SQL conversion

Integration Tests (YAML-driven)
├── Online tests:   Full round-trip with real database
│   parse → diff → generate_ddl → execute → export → verify idempotency
└── Offline tests:  String-level comparison
    parse → diff → generate_ddl → assert SQL matches expected
```

### 6.2 YAML Test Runner

```rust
// testkit/src/yaml_runner.rs

pub struct TestCase {
    pub current: String,
    pub desired: String,
    pub up: Option<String>,
    pub down: Option<String>,
    pub error: Option<String>,
    pub min_version: Option<String>,
    pub max_version: Option<String>,
    pub flavor: Option<String>,
    pub enable_drop: Option<bool>,
    pub offline: bool,
    // ...
}

/// Online test execution (with a real database).
pub fn run_online_test(
    dialect: &dyn Dialect,
    adapter: &mut dyn DatabaseAdapter,
    test: &TestCase,
) -> TestResult {
    let version = adapter.server_version().ok();
    // 1. Apply current schema
    // 2. Verify idempotency of current schema
    // 3. Generate DDLs: current → desired, assert matches `up`
    // 4. Apply generated DDLs
    // 5. Verify idempotency of desired schema
    // 6. Generate DDLs: desired → current, assert matches `down`
    // 7. Apply reverse DDLs
    // 8. Verify idempotency after reverse
}

/// Offline test execution (no database, string comparison only).
/// Used for proprietary SQL dialects (e.g., Aurora DSQL) or
/// unit-testing DDL generation without a running database.
pub fn run_offline_test(
    dialect: &dyn Dialect,
    test: &TestCase,
) -> TestResult {
    // 1. Parse current and desired SQL
    // 2. Generate DDLs: current → desired, assert matches `up`
    // 3. Generate DDLs: desired → current, assert matches `down`
    // (No database interaction, no idempotency verification)
}
```

### 6.3 Test Migration from sqldef

The YAML test schema is preserved, but test case content requires adaptation.
Porting rationale and costs are documented in [ADR-0005](docs/adr/0005-yaml-test-reuse.md).

| sqldef field | New tool field | Migration effort |
|--------------|----------------|------------------|
| `current` | `current` | Low — input SQL is usually portable |
| `desired` | `desired` | Low — input SQL is usually portable |
| `up` | `up` | **High** — expected DDL output will differ in quoting, whitespace, statement structure |
| `down` | `down` | **High** — same as `up` |
| `error` | `error` | Medium — error messages will differ |
| `min_version` | `min_version` | Identical |
| `max_version` | `max_version` | Identical |
| `flavor` | `flavor` | Identical |
| `enable_drop` | `enable_drop` | Identical |
| `offline` | `offline` | Identical |
| `legacy_ignore_quotes` | (removed) | **High** — all ~150 cases with this flag need expected output rewritten for quote-aware mode |

**Recommended porting order per dialect**: Start with idempotency-only tests (no `up`/`down`), then tackle assertion tests grouped by feature (tables → indexes → constraints → views).

### 6.4 Safety Regression Tests

The following test categories verify safety-critical behaviors and must be implemented as explicit regression tests before v1 release. These are not part of the YAML test corpus; they are dedicated unit/integration tests in the `core` and `testkit` crates.

#### S1: Fail-fast on unknown DDL

```
Input (desired):  CREATE TABLE t (...); CREATE FOOBAR baz;
Expected:         Error referencing "CREATE FOOBAR" as unsupported
Must NOT produce: DROP of any existing object
```

Variants: unknown statement types, partially-supported DDL (e.g., `CREATE MATERIALIZED VIEW` in a dialect that hasn't implemented it), and DML mixed with DDL.

#### S2: Annotation extraction failure does not produce DROP

```
Input (desired):  CREATE TABLE new_name (...);
                  -- @renamed from=old_name  (comment on wrong line / orphaned)
Input (current):  CREATE TABLE old_name (...);
Expected:         Error: annotation "@renamed from=old_name" does not match any object
Must NOT produce: DROP TABLE old_name; CREATE TABLE new_name;
```

#### S3: Non-transactional statement boundary behavior

```
Input (plan):     [Sql(T), Sql(T), Sql(N), Sql(T), Sql(T)]
                  where N = CREATE INDEX CONCURRENTLY
Expected:         Transaction 1 wraps first two T statements and is committed.
                  N executes outside any transaction.
                  Transaction 2 wraps last two T statements.
Failure in N:     Tx1 already committed (cannot roll back), error reported.
Failure in Tx2:   Tx2 rolled back, Tx1 committed, N committed, error reported.
```

#### S4: BatchBoundary does not commit

```
Input (plan):     [Sql(T), Sql(T), BatchBoundary, Sql(T)]
                  (All statements are transactional)
Expected:         All three Sql statements execute within the same transaction.
                  BatchBoundary is a synchronization point, not a transaction boundary.
Failure in Sql(3):Transaction covering all three statements is rolled back.
```

#### S5: Index owner validation

```
Input (desired):  CREATE INDEX idx ON nonexistent_table (...);
Expected:         Error: table "nonexistent_table" not found in schema
Must NOT produce: Any DiffOp involving this index
```

#### S6: Transaction RAII rollback on drop

```
Scenario:         Executor calls adapter.begin(), then Transaction handle
                  is dropped without commit() (e.g., due to early return on error).
Expected:         ROLLBACK is executed automatically.
Must NOT produce: Partial schema changes left in the database.
```

#### S7: enable_drop suppression

```
Input (desired):  (empty)
Input (current):  CREATE TABLE t (...); GRANT SELECT ON t TO role;
Config:           enable_drop: false
Expected:         No DiffOp emitted. Diagnostics contain:
                  "-- Skipped: DROP TABLE t"
                  "-- Skipped: REVOKE SELECT ON t FROM role"
Must NOT produce: DropTable or Revoke DiffOp values.
```

#### S8: enable_drop allows constraint modification drop

```
Input (desired):  CREATE TABLE t (a int CHECK (a > 10));
Input (current):  CREATE TABLE t (a int CHECK (a > 0));
Config:           enable_drop: false
Expected:         DropCheck + AddCheck DiffOps are emitted (constraint modification).
Rationale:        Drop-and-recreate for constraint changes is non-destructive.
```

#### S9: Circular FK ordering

```
Input (desired):  CREATE TABLE a (id int PK, b_id int REFERENCES b(id));
                  CREATE TABLE b (id int PK, a_id int REFERENCES a(id));
Input (current):  (empty)
Expected:         CreateTable(a) and CreateTable(b) are emitted (without FKs).
                  AddForeignKey(a, b_id→b) and AddForeignKey(b, a_id→a) follow.
Must NOT:         Fail with "circular dependency" error.
```

#### S10: SQLite table recreation atomicity

```
Scenario:         SQLite column type change triggers table recreation.
                  INSERT INTO _new_t SELECT ... fails due to type mismatch.
Expected:         Entire transaction is rolled back. Original table is unchanged.
Must NOT produce: Partial state (e.g., _new_t exists but original table is dropped).
```

#### S11: View rebuild expands unchanged dependents

```
Input (current):  CREATE VIEW base_v AS SELECT 1 AS c;
                  CREATE VIEW dep_v  AS SELECT c FROM base_v;
Input (desired):  CREATE VIEW base_v AS SELECT 2 AS c;
                  CREATE VIEW dep_v  AS SELECT c FROM base_v;   -- unchanged text
Expected:         DiffOps include:
                  DropView(dep_v), DropView(base_v),
                  CreateView(base_v), CreateView(dep_v)
Must NOT produce: Only DropView/CreateView for base_v while leaving dep_v untouched.
```

---

## 7. Risk Assessment

### 7.1 High Risk: Expression Comparison

**Problem**: CHECK constraints and DEFAULT expressions may be semantically equivalent but syntactically different across database versions and platforms. sqldef's normalization handles this with 1,400 lines of code. `Expr::Raw` + string comparison will produce false diffs for common patterns like `0` vs `'0'::integer`.

**Mitigation** (see also §3.4):
- Raw expressions in `current` (from database export) are already canonical.
- Per-dialect `normalize_expr()` canonicalizes known patterns in `desired`.
- Dialect-specific `EquivalencePolicy` hooks handle semantically equivalent residual cases after normalization (e.g., literal/cast form differences).
- Database-side normalization (e.g., `pg_get_expr`) as an optional fallback for complex expressions.
- Incremental coverage: prioritize the patterns that cause the most false diffs first (type casts, whitespace, parenthesization).
- Acceptance criterion: a test case that produces a false diff is a bug to be fixed, not a known limitation to accept indefinitely.

### 7.2 Medium Risk: Schema Export Fidelity

**Problem**: Each database's system catalog queries produce output in specific formats. The adapter must reconstruct SQL that, when parsed back, produces the same IR.

**Mitigation**:
- Port sqldef's export queries directly (they're well-tested).
- Idempotency tests catch any round-trip issues.

### 7.3 Medium Risk: sqlparser-rs Coverage Gaps

**Problem**: `sqlparser-rs` may not parse all DDL constructs used in production schemas.

**Mitigation**:
- The YAML test corpus will reveal gaps quickly.
- Due to the fail-fast design (§3.3), unsupported constructs cause an explicit error rather than silent data loss. Users see exactly which statement is unsupported.
- Can contribute upstream fixes to `sqlparser-rs` or switch to a custom parser for specific dialects if gaps are too large.

### 7.4 Low Risk: Rust Ecosystem Maturity

**Problem**: Rust database drivers and SQL parsers are less mature than Go's.

**Mitigation**:
- `rusqlite`, `sqlx` (with blocking runtime), and `pg_query.rs` are all production-ready.
- `pg_query.rs` wraps the same C library used by PostgreSQL itself.
- The synchronous I/O decision (ADR-0011) allows using synchronous driver APIs directly, avoiding async ecosystem complexity.

---

## 8. Resolved Product Decisions

1. **Naming**: The tool name is **`stateql`**. This project is sqldef-inspired, but a distinct product with distinct command names. See [ADR-0009](docs/adr/0009-single-binary-cli-shape.md).

2. **CLI compatibility depth**: v1 is **not** a drop-in replacement for sqldef. Useful concepts and familiar options may be kept, but strict flag-level/output compatibility is not required when it conflicts with design clarity or safety. See [ADR-0009](docs/adr/0009-single-binary-cli-shape.md).

3. **Minimum supported database versions (v1)** (see [ADR-0012](docs/adr/0012-minimum-supported-database-versions.md)):
   - PostgreSQL: `13+`
   - MySQL: `8.0+`
   - SQL Server: `2019+`
   - SQLite: `3.35+`

4. **Error recovery**: Parser/execution flow is **fail-fast**. The tool stops at the first unsupported construct or parse error to prevent silent omission and accidental destructive diffs. See [ADR-0013](docs/adr/0013-fail-fast-error-handling.md).

### Decision Traceability

Resolved architecture decisions are tracked in:
- [ADR index](docs/adr/README.md)
- [ADR-0001 (canonical IR)](docs/adr/0001-canonical-ir.md)
- [ADR-0002 (DiffOp batch SQL generation)](docs/adr/0002-diffop-batch-sql-generation.md)
- [ADR-0003 (source-level extensibility and plugin scope)](docs/adr/0003-source-level-extensibility.md)
- [ADR-0004 (expression representation)](docs/adr/0004-expression-representation-and-canonicalization.md)
- [ADR-0005 (YAML test reuse)](docs/adr/0005-yaml-test-reuse.md)
- [ADR-0006 (`@renamed` policy)](docs/adr/0006-explicit-rename-annotations.md)
- [ADR-0007 (BatchBoundary semantics)](docs/adr/0007-batchboundary-semantics.md)
- [ADR-0008 (single connection adapter)](docs/adr/0008-single-connection-adapter.md)
- [ADR-0009 (single binary CLI)](docs/adr/0009-single-binary-cli-shape.md)
- [ADR-0010 (parser selection policy)](docs/adr/0010-parser-selection-policy.md)
- [ADR-0011 (synchronous I/O)](docs/adr/0011-synchronous-io.md)
- [ADR-0012 (minimum supported database versions)](docs/adr/0012-minimum-supported-database-versions.md)
- [ADR-0013 (fail-fast error handling)](docs/adr/0013-fail-fast-error-handling.md)
- [ADR-0014 (schema search path for name resolution)](docs/adr/0014-schema-search-path.md)
- [ADR-0015 (semantic equivalence policy injection)](docs/adr/0015-equivalence-policy-injection.md)
- [ADR-0016 (view rebuild scope and owned diff planning)](docs/adr/0016-view-rebuild-scope-and-owned-diff-planning.md)
