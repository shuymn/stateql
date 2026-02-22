# stateql v1 Implementation Plan

**Source**: `DESIGN.md`
**Goal**: `DESIGN.md` §5.1-§5.6 の要求を、skeleton 前提に縮退せず、dialect ごとの parser/generator/adapter/normalize と YAML 移植まで含めて実装完了する。
**Architecture**: `crates/core` で型契約・diff・実行・オーケストレーションを確立し、`crates/dialect-*` が parser/generator/adapter/normalize を提供する。CLI は `stateql <dialect>` の単一バイナリ形状を維持する。
**Tech Stack**: Rust workspace, `thiserror`, `clap`, `pg_query`, `sqlparser`, `serde`, `serde_yaml`, `assert_cmd`
**Reference Implementation**: sqldef の参照コードは `reference/sqldef/` に配置されている。参照専用のため編集しない。
**Module Note**: `DESIGN.md` §3.2 の crate 構成に従い、`crates/core/src/plan.rs` を v1 範囲で実装する（Task 22b）。

## DESIGN Phase Mapping

| DESIGN.md Phase | Plan Phase / Tasks |
|---|---|
| §5.1 Foundation | Phase A-B + Task 13 + Task 13b + Task 38 + Task 38b |
| §5.2 PostgreSQL | Task 31-32 + Task 33a-33c + Task 30b + Task 43 |
| §5.3 SQLite | Task 34 + Task 34b + Task 34e + Task 35 + Task 43b-43d |
| §5.4 MySQL | Task 34 + Task 34c + Task 34e + Task 36 + Task 43b-43d |
| §5.5 MSSQL | Task 34 + Task 34d + Task 34e + Task 37 + Task 43b-43d |
| §5.6 API Stabilization | Task 43e + Task 44 + Task 45 + Task 45b + Task 45c |

## Requirement Index (from DESIGN.md)

- `R01` Fail-fast typed errors (`ParseError`, `DiffError`, `GenerateError`, `ExecutionError`, top `Error`) (`§3.10`, `ADR-0013`)
- `R02` Full `Dialect` contract (`parse`, `generate_ddl`, `to_sql`, `normalize`, `equivalence_policy`, `quote_ident`, `batch_separator`, `connect`) (`§3.3`)
- `R03` Canonical IR foundations (`Ident`, `QualifiedName`, `DataType`, `Expr`, `Value`) (`§3.4`, `ADR-0001`, `ADR-0004`)
- `R04` Closed `SchemaObject` family and related structs (`§3.4`, `ADR-0001`)
- `R05` Pre-parse `@renamed` extraction and orphan error (`§3.4`, `ADR-0006`)
- `R06` Full `DiffOp` model and `DiffConfig` (`§3.5`, `ADR-0002`, `ADR-0014`, `ADR-0015`)
- `R07` `enable_drop` suppression with diagnostics and constraint-modification pairing (`§3.5`)
- `R08` Ordering rules and circular FK handling (`§3.5`)
- `R09` View rebuild closure expansion in core (`§3.5`, `ADR-0016`)
- `R10` Single-connection adapter + RAII transaction + minimum supported version gate (`§3.7`, `§8`, `ADR-0008`, `ADR-0012`)
- `R11` Executor semantics (`transactional`, `non-transactional`, `BatchBoundary`) (`§3.7`, `ADR-0007`, `ADR-0011`)
- `R12` Renderer uses dialect separator (`§3.7`)
- `R13` Orchestrator flows (`--apply`, `--dry-run`, `--export`) (`§3.9`)
- `R14` Parser selection policy by dialect (`§3.6`, `ADR-0010`)
- `R15` Dialect-specific generation semantics (SQLite rebuild / MySQL merge / MSSQL boundary) (`§3.5`, `§3.7`)
- `R16` Testkit YAML schema + online/offline runner + flavor handling (`§6.2`, `§6.3`, `ADR-0005`)
- `R17` Safety regression coverage `S1..S11` (`§6.4`)
- `R18` CLI single binary with dialect subcommands (`§3.8`, `ADR-0009`)
- `R19` Dialect API documentation stabilization (`§5.6`)
- `R20` `core`/`testkit` publish readiness (`cargo publish --dry-run`) (`§5.6`)
- `R21` `crates/core/src/plan.rs` を含む crate 構成の実装一致 (`§3.2`)

## Task Dependency Graph

- `Task 0`: none
- `Task 1`: Task 0
- `Task 2`: Task 1
- `Task 3`: Task 2
- `Task 4`: Task 0, 1, 2, 3
- `Task 5`: Task 4, 3
- `Task 6`: Task 4
- `Task 7`: Task 6
- `Task 8`: Task 7
- `Task 9`: Task 8
- `Task 10`: Task 6
- `Task 11`: Task 6
- `Task 12`: Task 11, 9
- `Task 13`: Task 9
- `Task 13b`: Task 13, 9
- `Task 14`: Task 13, 4
- `Task 15`: Task 13, 14, 9
- `Task 15b`: Task 15
- `Task 15c`: Task 15, 15b, 8
- `Task 16`: Task 12, 15
- `Task 17`: Task 15
- `Task 18`: Task 15, 14
- `Task 19`: Task 18, 15
- `Task 20`: Task 15, 9
- `Task 21`: Task 15, 15b
- `Task 22`: Task 15, 15b, 15c, 17, 20, 21
- `Task 22b`: Task 22
- `Task 23`: Task 22, 22b, 15
- `Task 24`: Task 5, 10, 22b
- `Task 25`: Task 24
- `Task 26`: Task 24
- `Task 27`: Task 24, 10, 2
- `Task 28`: Task 10, 4
- `Task 29`: Task 4, 17, 18, 22b, 24, 28
- `Task 30`: Task 29
- `Task 30b`: Task 30
- `Task 31`: Task 11, 12, 9, 4, 13b
- `Task 32`: Task 31, 13
- `Task 33a`: Task 31, 5
- `Task 33b`: Task 33a, 32
- `Task 33c`: Task 33a, 14, 15
- `Task 34`: Task 11, 12, 9, 4, 13b
- `Task 34b`: Task 34, 5, 14, 17
- `Task 34c`: Task 34, 5, 14, 17
- `Task 34d`: Task 34, 5, 14, 17
- `Task 34e`: Task 34b, 34c, 34d, 14, 15
- `Task 35`: Task 34, 34b, 10
- `Task 36`: Task 34, 34c, 13
- `Task 37`: Task 34, 34d, 10, 28
- `Task 38`: Task 2
- `Task 38b`: Task 38
- `Task 39`: Task 38, 38b, 22
- `Task 40`: Task 39, 5, 31, 33a, 33b, 33c, 34c, 34d
- `Task 41`: Task 12, 15, 25, 26, 31, 32
- `Task 42`: Task 5, 18, 19, 21, 23, 35
- `Task 43`: Task 31, 39
- `Task 43b`: Task 43, 34, 34e, 39
- `Task 43c`: Task 43b
- `Task 43d`: Task 43c
- `Task 43e`: Task 4
- `Task 44`: Task 29, 30, 30b, 31, 34, 43e
- `Task 45`: Task 4, 33a, 33b, 33c, 34b, 34c, 34d, 34e
- `Task 45b`: Task 1, 2, 44
- `Task 45c`: Task 45, 45b

## Phase A: Bootstrap Removal and Core Contracts

### Task 0: Remove Bootstrap Placeholders

**達成する仕様**: `R01` の前提整備
**目的**: 既存 bootstrap (`plan_diff`, 2-variant IR/DiffOp) を除去し、本実装への置換境界を固定する。
**依存関係ポリシー**: このタスクでは「削除のみ」でなく、Task 4 までビルドを維持するための最小 stub 型を明示的に残す。

**Files:**
- Modify: `crates/core/src/lib.rs`
- Modify: `crates/core/src/ir.rs`
- Modify: `crates/core/src/diff.rs`
- Create: `crates/core/tests/bootstrap_boundary_test.rs`

**RED**
- `bootstrap_boundary_test.rs` で bootstrap 記号の不在を検証する。

**GREEN**
- bootstrap 関数/型を削除し、module-export ベースへ切り替える。
- `crates/core/src/lib.rs` 内の `smoke_parse_diff_render` を削除し、Task 0 以降の検証は `crates/core/tests/` の独立テストへ移す。
- ただし以下は compile 維持用の暫定 stub として追加する:
  `CoreError/CoreResult`, `Ident { value: String, quoted: bool }`, `SchemaObject`, `DiffOp`, `Statement`, `Dialect`（最小形）
- `SchemaObject` の暫定形状をここで固定する（Task 8 着手まで変更しない）:
  `enum SchemaObject { Table { name: String }, Index { name: String, table: String } }`
- Task 4 までの dialect crate 側は、この暫定 `SchemaObject` に依存した網羅的 `match` を実装しない（型境界確認に限定）。
- `ParseError` / `GenerateError` は Task 1 で正式追加する（Task 0 では先行導入しない）。

**REFACTOR**
- re-export 整理 (`pub use ...`) のみ行う。

**DoD**
- `cargo nextest run -p stateql-core --test bootstrap_boundary_test` が PASS。
- `plan_diff` と `CreateObject/DropObject` がコードベースから消える。
- `crates/core/src/lib.rs` から `smoke_parse_diff_render` が削除される。
- `cargo check -p stateql-core` が PASS（Task 4 着手前にビルドが落ちない）。
- Task 0 完了時点の公開エラー型が `CoreError/CoreResult` であることを確認（Task 1 で置換予定）。

**Commit**
```bash
git add crates/core/src/lib.rs crates/core/src/ir.rs crates/core/src/diff.rs crates/core/tests/bootstrap_boundary_test.rs
git commit -m "refactor(core): remove bootstrap placeholders"
```

### Task 1: Add Stage-Typed Error Enums

**達成する仕様**: `R01`
**目的**: parse/diff/generate/execute 各段の失敗を型で区別可能にする。

**Files:**
- Modify: `crates/core/src/error.rs`
- Modify: `crates/core/Cargo.toml`
- Create: `crates/core/tests/error_types_test.rs`

**RED**
- `ParseError`, `DiffError`, `GenerateError`, `ExecutionError` と `SourceLocation { line, column }` の存在をテスト。

**GREEN**
- `thiserror` ベースで4種エラー + `SourceLocation` を導入。

**REFACTOR**
- 表示文言を statement index / target / dialect を含む最小情報へ統一。

**DoD**
- `cargo nextest run -p stateql-core --test error_types_test` PASS。
- `ParseError::StatementConversion` が statement context と `SourceLocation` を保持。

**Commit**
```bash
git add crates/core/src/error.rs crates/core/Cargo.toml crates/core/tests/error_types_test.rs
git commit -m "feat(core): add typed stage error enums"
```

### Task 2: Add Top-Level `Error` and `Result<T>`

**達成する仕様**: `R01`
**目的**: orchestrator 境界での fail-fast 伝播型を確立する。

**Files:**
- Modify: `crates/core/src/error.rs`
- Modify: `crates/core/src/lib.rs`
- Create: `crates/core/tests/error_wrap_test.rs`

**RED**
- `Error::{Parse,Diff,Generate,Execute}` ラップと `From` 変換をテスト。

**GREEN**
- top-level `Error` と `pub type Result<T>` を追加。

**REFACTOR**
- 既存 API の戻り値を `Result<T>` に寄せる。

**DoD**
- `cargo nextest run -p stateql-core --test error_wrap_test` PASS。
- 以後 core 公開 API が `Result<T>` を返す。

**Commit**
```bash
git add crates/core/src/error.rs crates/core/src/lib.rs crates/core/tests/error_wrap_test.rs
git commit -m "feat(core): add top-level error wrapper"
```

### Task 3: Add `Version` and `ConnectionConfig`

**達成する仕様**: `R02`, `R10`
**目的**: dialect `connect()` / adapter `server_version()` の型基盤を固定する。

**Files:**
- Modify: `crates/core/src/lib.rs`
- Create: `crates/core/src/config.rs`
- Create: `crates/core/tests/config_types_test.rs`

**RED**
- `Version { major, minor, patch }` と `ConnectionConfig` 生成をテスト。

**GREEN**
- `config.rs` を追加し re-export。
- `ConnectionConfig` の v1 フィールドを明示:
  `host: Option<String>`, `port: Option<u16>`, `user: Option<String>`, `password: Option<String>`, `database: String`, `socket: Option<String>`, `extra: BTreeMap<String, String>`。

**REFACTOR**
- 文字列版 version 参照があれば排除。

**DoD**
- `cargo nextest run -p stateql-core --test config_types_test` PASS。

**Commit**
```bash
git add crates/core/src/config.rs crates/core/src/lib.rs crates/core/tests/config_types_test.rs
git commit -m "feat(core): add version and connection config types"
```

### Task 4: Expand `Dialect` Trait to Full Contract

**達成する仕様**: `R02`
**目的**: DESIGN.md 準拠の dialect 境界を確定し、後続実装のズレを防ぐ。
**依存**: Task 0, 1, 2, 3（Task 0 の `Ident`/`SchemaObject` stub を利用して trait 署名を先に固定する）

**Files:**
- Modify: `crates/core/src/dialect.rs`
- Modify: `crates/core/src/lib.rs`
- Create: `crates/core/tests/dialect_contract_test.rs`
- Modify: `crates/dialect-postgres/src/lib.rs`
- Modify: `crates/dialect-mysql/src/lib.rs`
- Modify: `crates/dialect-sqlite/src/lib.rs`
- Modify: `crates/dialect-mssql/src/lib.rs`

**RED**
- `Dialect` 実装に `to_sql/normalize/equivalence_policy/quote_ident/batch_separator/connect` が必須であることをテスト。

**GREEN**
- trait を拡張し、既存 dialect stub をすべてコンパイル可能に更新。
- この時点では `equivalence_policy()` は暫定的に `DEFAULT_EQUIVALENCE_POLICY` を返す実装に統一し、詳細比較ロジックは Task 14 で完成させる。
- `to_sql` / `normalize` / `connect` の未実装経路は `todo!()` ではなく、Task 1/2 で導入した stage-typed `Error` (`Parse`/`Generate`/`Execute`) を明示的に返す。

**REFACTOR**
- `name()` の戻り値を `&str` へ統一。

**DoD**
- `cargo nextest run -p stateql-core --test dialect_contract_test` PASS。
- 全 dialect crate がコンパイル通過。
- Task 6-9 に入る前の stub 実装状態で `cargo check --workspace` が PASS。

**Commit**
```bash
git add crates/core/src/dialect.rs crates/core/src/lib.rs crates/core/tests/dialect_contract_test.rs crates/dialect-postgres/src/lib.rs crates/dialect-mysql/src/lib.rs crates/dialect-sqlite/src/lib.rs crates/dialect-mssql/src/lib.rs
git commit -m "feat(core): align dialect trait with design contract"
```

### Task 5: Implement Single-Connection Adapter + RAII Transaction

**達成する仕様**: `R10`
**目的**: begin/commit/rollback の接続一貫性と drop時 rollback を保証する。

**Files:**
- Modify: `crates/core/src/adapter.rs`
- Modify: `crates/core/src/lib.rs`
- Create: `crates/core/tests/transaction_raii_test.rs`
- Create: `crates/core/tests/adapter_sync_contract_test.rs`
- Create: `crates/core/tests/support/fake_adapter.rs`

**RED**
- transaction handle 未 commit drop 時に rollback が発行されるテストを追加。
- S6 と同一要件（RAII rollback）をこのタスクの失敗テストとして先行固定する。
- `DatabaseAdapter` が `async fn` / `Future` 戻り値を公開しないことを型テストで固定する。

**GREEN**
- `DatabaseAdapter` を `export_schema/execute/begin/schema_search_path/server_version` に確定。
- trait の最終シグネチャを固定し、Task 5 完了時点で以下と一致させる:
  ```rust
  pub trait DatabaseAdapter: Send {
      fn export_schema(&self) -> Result<String>;
      fn execute(&self, sql: &str) -> Result<()>;
      fn begin(&mut self) -> Result<Transaction<'_>>;
      fn schema_search_path(&self) -> Vec<String>;
      fn server_version(&self) -> Result<Version>;
  }

  impl<'a> Transaction<'a> {
      pub fn execute(&mut self, sql: &str) -> Result<()>;
      pub fn commit(self) -> Result<()>;
  }
  ```
- `execute` シグネチャを bootstrap の `execute(&mut self, statement: &Statement)` から、設計仕様の `execute(&self, sql: &str)` へ明示的に変更する（必要な可変状態は実装側で内部可変性を使って扱う）。
- `Transaction<'_>::execute/commit` と `Drop` rollback を実装。
- 借用戦略を明記: `execute(&self)` と `begin(&mut self)` の共存のため、アダプタ内部状態は interior mutability で保持する。
  推奨は「本番実装: `Mutex` ベース」「単体テスト fake: `RefCell` ベース」。
- 同期 I/O 契約（`DESIGN.md` §3.7）を trait doc comment へ明記し、core API へ async 境界を漏らさない。
- `docs/testing.md` に合わせ、ここで作る test double は `#[cfg(test)]` の in-memory fake に限定する。
- test double の位置づけを明記: ここで使うのは mock（呼び出し期待の検証）ではなく fake（状態を持つ簡易実装）。

**REFACTOR**
- trait に `rollback()/commit()` を持たせない（設計準拠）。

**DoD**
- `cargo nextest run -p stateql-core --test transaction_raii_test` PASS。
- `cargo nextest run -p stateql-core --test adapter_sync_contract_test` PASS。
- `crates/core/tests/support/fake_adapter.rs` が Task 24-30 の再利用テスト基盤として利用可能。

**Commit**
```bash
git add crates/core/src/adapter.rs crates/core/src/lib.rs crates/core/tests/transaction_raii_test.rs crates/core/tests/adapter_sync_contract_test.rs crates/core/tests/support/fake_adapter.rs
git commit -m "feat(core): add single-connection adapter and raii transaction"
```

## Phase B: Canonical IR and Annotation Pipeline

### Task 6: Add IR Foundations (`Ident`, `QualifiedName`, `Value`, `DataType`)

**達成する仕様**: `R03`
**目的**: parser/diff 共有の最小土台を確立する。
**依存**: Task 4（Task 0 で仮置きした `Ident` stub を正式型へ置換し、IR 基礎型を拡張する）

**Files:**
- Modify: `crates/core/src/ir.rs`
- Create: `crates/core/src/ir/ident.rs`
- Create: `crates/core/src/ir/types.rs`
- Modify: `crates/core/src/lib.rs`
- Create: `crates/core/tests/ir_foundation_test.rs`

**RED**
- quoted/unquoted identifier と `DataType::Custom` の保持をテスト。

**GREEN**
- `ir.rs` を module root とし、`ir/ident.rs` と `ir/types.rs` へ分割して foundational types を追加。
- `Value::Float(f64)` を維持しつつ、`Value`/`Expr` 系の比較方針を明示:
  `PartialEq` を基本とし、`Eq` が必要な経路では `f64::total_cmp` ベースの補助比較関数を使う（`ordered-float` は導入しない）。

**REFACTOR**
- `Ident::quoted/unquoted` ヘルパ追加。

**DoD**
- `cargo nextest run -p stateql-core --test ir_foundation_test` PASS。
- `cargo check -p stateql-dialect-postgres -p stateql-dialect-mysql -p stateql-dialect-sqlite -p stateql-dialect-mssql` PASS。

**Commit**
```bash
git add crates/core/src/ir.rs crates/core/src/ir/ident.rs crates/core/src/ir/types.rs crates/core/src/lib.rs crates/core/tests/ir_foundation_test.rs
git commit -m "feat(core): add canonical ir foundational types"
```

### Task 7: Add Expression AST Core Variants

**達成する仕様**: `R03`
**目的**: equivalence/normalization 対応のために `Expr` を構造化する。

**Files:**
- Modify: `crates/core/src/ir.rs`
- Create: `crates/core/src/ir/expr.rs`
- Create: `crates/core/tests/expr_ast_test.rs`

**RED**
- DESIGN.md で要求される主要群を検証するテストを追加:
  - leaf: `Literal`, `Ident`, `QualifiedIdent`, `Null`
  - operator: `BinaryOp`, `UnaryOp`, `Comparison`, `And`, `Or`, `Not`, `Is`
  - range/grouping: `Between`, `In`, `Paren`, `Tuple`
  - function/type: `Function`, `Cast`, `Collate`
  - compound: `Case`, `ArrayConstructor`, `Exists`
  - fallback: `Raw`

**GREEN**
- `Expr` と関連 enum/struct (`Literal`, `BinaryOperator`, `UnaryOperator`, `ComparisonOp`, `IsTest`, `SetQuantifier`, `WindowSpec`, `SubQuery`) を実装。
- `WindowSpec` と `SubQuery` を Task 7 のスコープに含めることを明示し、後続タスクに送らない。

**REFACTOR**
- 将来追加しやすい enum grouping に整理。

**DoD**
- `cargo nextest run -p stateql-core --test expr_ast_test` PASS。
- `cargo check -p stateql-dialect-postgres -p stateql-dialect-mysql -p stateql-dialect-sqlite -p stateql-dialect-mssql` PASS。

**Commit**
```bash
git add crates/core/src/ir.rs crates/core/src/ir/expr.rs crates/core/tests/expr_ast_test.rs
git commit -m "feat(core): add expression ast core variants"
```

### Task 8: Add Full `SchemaObject` Family (Part 1)

**達成する仕様**: `R04`
**目的**: table/view/index/sequence を top-level object として定義する。
This task exceeds the 2-5 min guideline because: SchemaObject の土台を崩さずに `ir` 分割と型定義を同時に成立させる必要がある。

**Files:**
- Modify: `crates/core/src/ir.rs`
- Create: `crates/core/src/ir/schema_object.rs`
- Create: `crates/core/tests/schema_object_part1_test.rs`

**RED**
- `SchemaObject::{Table,View,MaterializedView,Index,Sequence}` の保持を検証。

**GREEN**
- object structs + enums を追加し、実装順を固定:
  1) top-level `SchemaObject::{Table,View,MaterializedView,Index,Sequence}`
  2) `Table`/`Index` の直下型
  3) `Partition` 系
- このタスクで含める型を明示:
  `Table`, `Column`, `Identity`, `GeneratedColumn`, `PrimaryKey`, `TableOptions`,
  `IndexDef`, `IndexColumn`, `IndexOwner`,
  `Partition`, `PartitionStrategy`, `PartitionElement`, `PartitionBound`,
  `Sequence`, `ColumnPosition`。

**REFACTOR**
- simple constructor (`Table::named`, `View::new`) を追加。

**DoD**
- `cargo nextest run -p stateql-core --test schema_object_part1_test` PASS。
- `cargo check -p stateql-dialect-postgres -p stateql-dialect-mysql -p stateql-dialect-sqlite -p stateql-dialect-mssql` PASS。

**Commit**
```bash
git add crates/core/src/ir.rs crates/core/src/ir/schema_object.rs crates/core/tests/schema_object_part1_test.rs
git commit -m "feat(core): add schemaobject family part1"
```

### Task 9: Add Full `SchemaObject` Family (Part 2)

**達成する仕様**: `R04`
**目的**: trigger/function/type/domain/extension/schema/comment/privilege/policy を追加する。

**Files:**
- Modify: `crates/core/src/ir.rs`
- Modify: `crates/core/src/ir/schema_object.rs`
- Create: `crates/core/tests/schema_object_part2_test.rs`

**RED**
- 残り variant の生成/一致テストを追加。

**GREEN**
- 残り object types を実装。
- 欠落しやすい支援 enum/struct をこのタスクで明示的に追加:
  `CheckConstraint`, `ExclusionConstraint`, `ExclusionElement`, `Deferrable`,
  `SortOrder`, `NullsOrder`,
  `ForeignKey`, `ForeignKeyAction`,
  `CheckOption`, `ViewSecurity`,
  `TriggerTiming`, `TriggerEvent`, `TriggerForEach`,
  `FunctionParam`, `FunctionParamMode`, `Volatility`, `FunctionSecurity`,
  `PrivilegeOp`, `PrivilegeObject`,
  `PolicyCommand`,
  `TypeKind`, `EnumValuePosition`,
  `CommentTarget`。

**REFACTOR**
- `Privilege::empty` 等の test helper を追加。

**DoD**
- `cargo nextest run -p stateql-core --test schema_object_part2_test` PASS。
- `cargo check -p stateql-dialect-postgres -p stateql-dialect-mysql -p stateql-dialect-sqlite -p stateql-dialect-mssql` PASS。

**Commit**
```bash
git add crates/core/src/ir.rs crates/core/src/ir/schema_object.rs crates/core/tests/schema_object_part2_test.rs
git commit -m "feat(core): add schemaobject family part2"
```

### Task 10: Add Statement Context Model

**達成する仕様**: `R01`, `R11`, `R15`
**目的**: SQLite再構築失敗時の診断に必要な文脈 (`QualifiedName`) を保持する。

**Files:**
- Modify: `crates/core/src/statement.rs`
- Modify: `crates/core/src/lib.rs`
- Create: `crates/core/tests/statement_context_test.rs`

**RED**
- `Statement::Sql { context: Some(SqliteTableRebuild{...}) }` を検証。

**GREEN**
- `StatementContext`, `SqliteRebuildStep` を実装。

**REFACTOR**
- `Statement` ヘルパ constructor を最小限追加（必要なら）。

**DoD**
- `cargo nextest run -p stateql-core --test statement_context_test` PASS。

**Commit**
```bash
git add crates/core/src/statement.rs crates/core/src/lib.rs crates/core/tests/statement_context_test.rs
git commit -m "feat(core): add statement context for execution diagnostics"
```

### Task 11: Add Annotation Extractor (`@renamed`)

**達成する仕様**: `R05`
**目的**: parser前段でコメントから rename annotation を抽出し、行数マッピングを維持する。

**Files:**
- Create: `crates/core/src/annotation.rs`
- Modify: `crates/core/src/lib.rs`
- Create: `crates/core/tests/annotation_extractor_test.rs`

**RED**
- コメント内 annotation のみ抽出し、文字列リテラル内 `@renamed` を無視するテストを追加。

**GREEN**
- extractor 実装。

**REFACTOR**
- deprecated alias `@rename` を warning扱いで受理する入口を作る（実際の warning 発火は後続可）。

**DoD**
- `cargo nextest run -p stateql-core --test annotation_extractor_test` PASS。

**Commit**
```bash
git add crates/core/src/annotation.rs crates/core/src/lib.rs crates/core/tests/annotation_extractor_test.rs
git commit -m "feat(core): add renamed annotation extraction"
```

### Task 12: Add Annotation Attachment Validation

**達成する仕様**: `R05`
**目的**: orphan annotation をエラーにして silent ignore を防ぐ。

**Files:**
- Modify: `crates/core/src/annotation.rs`
- Create: `crates/core/tests/annotation_attach_test.rs`

**RED**
- annotation が対象 object に紐づかない場合 `DiffError` 相当で失敗するテストを追加。
- S2（orphan annotation は DROP計画を出さずに fail-fast）を同テストで固定する。

**GREEN**
- `attach_annotations` 実装。

**REFACTOR**
- attach時の object match を name/key比較関数に抽出。

**DoD**
- `cargo nextest run -p stateql-core --test annotation_attach_test` PASS。

**Commit**
```bash
git add crates/core/src/annotation.rs crates/core/tests/annotation_attach_test.rs
git commit -m "feat(core): validate orphan renamed annotations"
```

## Phase C: Diff Engine

### Task 13: Add Full `DiffOp` and Change Enums

**達成する仕様**: `R06`
**目的**: DESIGN.md の diff surface を型で固定する。

**Files:**
- Modify: `crates/core/src/diff.rs`
- Create: `crates/core/src/diff/types.rs`
- Create: `crates/core/src/diff/engine.rs`
- Modify: `crates/core/src/lib.rs`
- Create: `crates/core/tests/diffop_surface_test.rs`
- Create: `crates/core/tests/support/diffop_fixtures.rs`

**RED**
- 主要 variant (`DropForeignKey`, `Grant`, `AlterDomain`, etc.) 生成可否をテスト。

**GREEN**
- full `DiffOp`, `ColumnChange`, `SequenceChange`, `TypeChange`, `DomainChange` 実装。
- `diff.rs` を module root にし、`types`/`engine` へ責務を分割する（Phase C で `diff.rs` を肥大化させない）。

**REFACTOR**
- enum grouping コメントを DESIGN と同順に整理。
- `DiffOp` 全 variant を dummy データ付きで生成する test helper（`all_diffop_variants()`）を `crates/core/tests/support/diffop_fixtures.rs` に追加する。Tasks 32/35/36/37 の `generator_contract_test` から再利用する前提。

**DoD**
- `cargo nextest run -p stateql-core --test diffop_surface_test` PASS。
- `DiffOp` の variant 群が `crates/core/src/diff/types.rs` に集約され、比較ロジックと分離されている。
- `all_diffop_variants()` が全 variant を網羅しており、variant 追加時にコンパイルエラーで検知されること。

**Commit**
```bash
git add crates/core/src/diff.rs crates/core/src/diff/types.rs crates/core/src/diff/engine.rs crates/core/src/lib.rs crates/core/tests/diffop_surface_test.rs crates/core/tests/support/diffop_fixtures.rs
git commit -m "feat(core): add full diffop model"
```

### Task 13b: Add IR Validation Tests for Dialect-Specific Patterns

**達成する仕様**: `R03`, `R04`, `R06`, `R15`
**目的**: DESIGN.md §5.1 step 6 の検証を固定し、dialect 実装着手前に IR 表現力の不足を検出する。

**Files:**
- Create: `crates/core/tests/ir_dialect_pattern_validation_test.rs`
- Modify: `crates/core/src/ir.rs`
- Modify: `crates/core/src/diff/types.rs`

**RED**
- 以下のケースが IR/DiffOp で表現できることを要求する失敗テストを追加:
  - MySQL `CHANGE COLUMN` full redefinition -> `AlterColumn` + 複数 `ColumnChange`
  - MySQL `AFTER` -> `AddColumn { position: Some(ColumnPosition::After(...)) }`
  - MSSQL named DEFAULT constraint -> `Column.extra["mssql.default_constraint_name"]`
  - MSSQL rename -> `RenameTable` / `RenameColumn` / `RenameIndex`
  - MySQL `AUTO_INCREMENT` two-phase handling metadata -> `Column.extra["mysql.auto_increment"]`

**GREEN**
- 上記ケースをすべて IR/DiffOp で表現できる状態にする。
- `extra` key は raw string を禁止し、テスト側も constants 経由で参照する。
- 変更制約: `ir.rs` / `diff/types.rs` への変更は「追加のみ」とし、既存 field/variant の削除・改名・意味変更は禁止する。

**REFACTOR**
- `ir_pattern_fixtures` を test helper module として分離し、後続 dialect crate test から再利用できる形にする。

**DoD**
- `cargo nextest run -p stateql-core --test ir_dialect_pattern_validation_test` PASS。
- Task 31/34 着手前ゲートとして、このテストを CI 必須セットへ追加。
- Task 6-13 で追加した既存テストがこのタスク変更で退行しない（`cargo nextest run -p stateql-core` で確認）。

**Commit**
```bash
git add crates/core/tests/ir_dialect_pattern_validation_test.rs crates/core/src/ir.rs crates/core/src/diff/types.rs
git commit -m "test(core): validate ir coverage for mysql and mssql patterns"
```

### Task 14: Add `EquivalencePolicy` and `DiffConfig`

**達成する仕様**: `R06`
**目的**: diff の dialect非依存性を維持したまま semantic equivalence を注入する。

**Files:**
- Modify: `crates/core/src/diff.rs`
- Create: `crates/core/src/diff/policy.rs`
- Modify: `crates/core/src/dialect.rs`
- Create: `crates/core/tests/equivalence_policy_test.rs`

**RED**
- custom policy 注入時に expr/custom type 比較が緩和されるテストを追加。
- policy contract（symmetric / stable across runs）を満たさない実装が検知される失敗テストを追加。

**GREEN**
- `EquivalencePolicy`, `DEFAULT_EQUIVALENCE_POLICY`, `DiffConfig` 実装。
- `DiffConfig.equivalence_policy` を `Arc<dyn EquivalencePolicy>` で実装し、所有権境界を明示する。
- contract test helper を追加し、`is_equivalent_expr` / `is_equivalent_custom_type` の対称性・反復安定性を固定する。

**REFACTOR**
- strict-eq fallback を helper 関数化。

**DoD**
- `cargo nextest run -p stateql-core --test equivalence_policy_test` PASS。

**Commit**
```bash
git add crates/core/src/diff.rs crates/core/src/diff/policy.rs crates/core/src/dialect.rs crates/core/tests/equivalence_policy_test.rs
git commit -m "feat(core): add equivalence policy and diff config"
```

### Task 15: Implement Core Object Comparison (Table/Column/Index Baseline)

**達成する仕様**: `R06`
**目的**: name解決や ordering に入る前に、IR同士の「何が変化か」を決定するコア比較を確立する。

**Files:**
- Modify: `crates/core/src/diff.rs`
- Create: `crates/core/src/diff/compare.rs`
- Create: `crates/core/tests/diff_core_compare_test.rs`

**RED**
- 以下を検証する失敗テストを追加:
  - currentに無く desiredにある table -> `CreateTable`
  - desiredに無く currentにある table -> `DropTable`（`enable_drop=true`）
  - column type/default/not-null 差分 -> `AlterColumn`
  - table配下 index 差分 -> `AddIndex`/`DropIndex`
  - default/check の `Expr` 比較で `DiffConfig.equivalence_policy` が差分判定に反映される
  - S5（index owner 不在時は error）を fail-fast で検証

**GREEN**
- object-kind ごとの差分比較関数（table, column, index）を実装し、`Vec<DiffOp>` を返す。
- strict equality 判定後に policy を評価する比較順序（strict -> policy）をこのタスクで実装する。

**REFACTOR**
- `compare_table`, `compare_columns`, `compare_indexes` を分離し、Task 17 以降の name解決/ordering から独立させる。

**DoD**
- `cargo nextest run -p stateql-core --test diff_core_compare_test` PASS。
- `DiffEngine::diff` が「比較」と「解決/並び替え」の関心を分離できる構造になる。

**Commit**
```bash
git add crates/core/src/diff.rs crates/core/src/diff/compare.rs crates/core/tests/diff_core_compare_test.rs
git commit -m "feat(core): implement baseline object comparison in diff engine"
```

### Task 15b: Implement Remaining `SchemaObject` Comparison

**達成する仕様**: `R06`
**目的**: `SchemaObject` 14 variants の比較を網羅し、未比較 variant を残さない。加えて cross-object invariant（sequence 重複禁止）を diff 前検証として固定する。

**Files:**
- Modify: `crates/core/src/diff.rs`
- Create: `crates/core/src/diff/compare_remaining.rs`
- Create: `crates/core/tests/diff_remaining_objects_test.rs`

**RED**
- 以下の差分テストを追加:
  - `View`, `MaterializedView` の create/drop/definition-change
  - `Sequence`, `Trigger`, `Function`, `TypeDef`, `Domain`
  - `Extension`, `SchemaDef`, `Comment`, `Policy`
- 未対応 variant を黙殺せず `DiffError` で fail-fast になることを検証。
- Sequence duplicate invariant（DESIGN.md §3.4）: `SchemaObject::Sequence("tbl_id_seq")` と `SchemaObject::Table` 内の `Column { identity: Some(Identity { ... }) }` が同一 sequence 名を参照する入力で `DiffError` が返らない失敗テストを追加。desired 側・current 側の双方で検証する（normalizer バグがどちら側で起きても検出）。

**GREEN**
- 上記 variant の比較を実装し、必要な `DiffOp` を生成。
- `DiffEngine` の `match SchemaObject` が exhaustive であることを保持（default arm で逃がさない）。
- diff 前処理として `Vec<SchemaObject>` 全体を走査し、sequence 名 vs identity column implicit sequence の overlap を検出して `DiffError` を返すバリデーションを実装する。

**REFACTOR**
- remaining object 比較を object-kind ごとの小関数に分離。
- sequence duplicate 検証を `validate_sequence_invariant(...)` として分離。

**DoD**
- `cargo nextest run -p stateql-core --test diff_remaining_objects_test` PASS。
- `SchemaObject` 追加時に比較実装漏れがコンパイル時に検知される構造を維持。
- `diff_remaining_objects_test` で duplicate sequence invariant violation → `DiffError` が desired/current 双方で検証されること。

**Commit**
```bash
git add crates/core/src/diff.rs crates/core/src/diff/compare_remaining.rs crates/core/tests/diff_remaining_objects_test.rs
git commit -m "feat(core): cover remaining schemaobject comparison paths"
```

### Task 15c: Implement Partition Diff Comparison (`AddPartition` / `DropPartition`)

**達成する仕様**: `R06`, `R08`
**目的**: `Partition` 差分を table 比較の暗黙処理から分離し、追加/削除/変更を明示 DiffOp へ変換する。

**Files:**
- Modify: `crates/core/src/diff/compare.rs`
- Create: `crates/core/src/diff/partition.rs`
- Create: `crates/core/tests/partition_diff_test.rs`

**RED**
- 以下の失敗テストを追加:
  - desired のみ partition 定義あり -> `AddPartition`
  - current のみ partition 定義あり -> `DropPartition`
  - partition element 名・bound 差分 -> 既存 drop + add へ分解
  - MySQL `MAXVALUE` / PostgreSQL `FromTo` を含む bound 比較

**GREEN**
- partition 比較を実装し、`AddPartition` / `DropPartition` を生成する。
- table compare 本体は `partition::diff_partition(...)` を呼ぶだけにして責務分離する。

**REFACTOR**
- partition key 比較の正規化ヘルパを抽出（順序依存比較の重複を排除）。

**DoD**
- `cargo nextest run -p stateql-core --test partition_diff_test` PASS。
- Task 22 の 22h ordering テストに partition op を追加して PASS。

**Commit**
```bash
git add crates/core/src/diff/compare.rs crates/core/src/diff/partition.rs crates/core/tests/partition_diff_test.rs
git commit -m "feat(core): add explicit partition diff comparison"
```

### Task 16: Implement Rename Detection from `renamed_from`

**達成する仕様**: `R05`, `R06`
**目的**: annotation で付与された `renamed_from` を使い、drop+create ではなく rename 系 DiffOp を生成する。

**Files:**
- Modify: `crates/core/src/diff.rs`
- Create: `crates/core/src/diff/rename.rs`
- Create: `crates/core/tests/rename_detection_test.rs`

**RED**
- 以下の失敗テストを追加:
  - table rename annotation -> `RenameTable`
  - column rename annotation -> `RenameColumn`
  - index rename annotation -> `RenameIndex`
  - annotation 無しの名前変更 -> `Drop* + Create*`（rename しない）

**GREEN**
- `renamed_from` 優先マッチロジックを追加し、rename DiffOp を生成。

**REFACTOR**
- rename candidate 解決を `resolve_rename_match(...)` に抽出。

**DoD**
- `cargo nextest run -p stateql-core --test rename_detection_test` PASS。

**Commit**
```bash
git add crates/core/src/diff.rs crates/core/src/diff/rename.rs crates/core/tests/rename_detection_test.rs
git commit -m "feat(core): implement explicit rename detection from annotations"
```

### Task 17: Implement Name Matching with `schema_search_path`

**達成する仕様**: `R06`, `R08`
**目的**: qualified/unqualified 名の比較規則を ADR-0014 準拠で実装する。

**Files:**
- Modify: `crates/core/src/diff.rs`
- Create: `crates/core/src/diff/name_resolution.rs`
- Create: `crates/core/tests/schema_search_path_test.rs`

**RED**
- `app.users` と `users` が search path によって一致/不一致するテストを追加。

**GREEN**
- name resolution 実装。

**REFACTOR**
- resolution ロジックを関数分離。

**DoD**
- `cargo nextest run -p stateql-core --test schema_search_path_test` PASS。

**Commit**
```bash
git add crates/core/src/diff.rs crates/core/src/diff/name_resolution.rs crates/core/tests/schema_search_path_test.rs
git commit -m "feat(core): implement schema search path matching"
```

### Task 18: Implement `enable_drop` Suppression Diagnostics

**達成する仕様**: `R07`
**目的**: destructive ops を抑止し、dry-runに `Skipped:` 診断を出す。

**Files:**
- Modify: `crates/core/src/diff.rs`
- Create: `crates/core/src/diff/enable_drop.rs`
- Create: `crates/core/tests/enable_drop_test.rs`

**RED**
- `enable_drop=false` で `DropTable/Revoke` が出ないことをテスト。
- S7（suppression + diagnostics）の期待値をこのタスクで固定する。
- suppression の結果が「単なる op 削除」ではなく、renderer 側へ渡せる diagnostics payload として保持される失敗テストを追加。

**GREEN**
- suppression 実装。
- suppressed op を diagnostics として返すデータ構造を実装し、後続 Task 29 の dry-run 経路で `-- Skipped: ...` を描画できる状態にする。

**REFACTOR**
- suppressed-op 判定テーブルを定数化。

**DoD**
- `cargo nextest run -p stateql-core --test enable_drop_test` PASS。
- `enable_drop_test` で suppressed op 数と diagnostics payload の内容（対象 op 種別）が検証される。

**Commit**
```bash
git add crates/core/src/diff.rs crates/core/src/diff/enable_drop.rs crates/core/tests/enable_drop_test.rs
git commit -m "feat(core): add enable_drop suppression with diagnostics"
```

### Task 19: Implement Constraint Modification Pairing

**達成する仕様**: `R07`
**目的**: 非破壊変更に必要な drop+add ペアは `enable_drop=false` でも通す。

**Files:**
- Modify: `crates/core/src/diff.rs`
- Create: `crates/core/src/diff/constraint_pairing.rs`
- Create: `crates/core/tests/constraint_pairing_test.rs`

**RED**
- CHECK 変更で `DropCheck+AddCheck` がペアで残るテストを追加。
- S8（`enable_drop=false` 下でも変更ペアは許可）をこのタスクの RED に組み込む。

**GREEN**
- pairing 実装。

**REFACTOR**
- constraint key match を helper 化。

**DoD**
- `cargo nextest run -p stateql-core --test constraint_pairing_test` PASS。

**Commit**
```bash
git add crates/core/src/diff.rs crates/core/src/diff/constraint_pairing.rs crates/core/tests/constraint_pairing_test.rs
git commit -m "feat(core): allow paired constraint drops under enable_drop=false"
```

### Task 20: Implement Privilege Set-Difference Semantics

**達成する仕様**: `R06`
**目的**: `Privilege` 比較を operations の集合差分で扱い、`Grant/Revoke` を最小化する。

**Files:**
- Modify: `crates/core/src/diff.rs`
- Create: `crates/core/src/diff/privilege.rs`
- Create: `crates/core/tests/privilege_diff_test.rs`

**RED**
- 以下を検証する失敗テストを追加:
  - `SELECT -> SELECT,INSERT` で `Grant(INSERT)` のみ
  - `SELECT,INSERT -> SELECT` で `Revoke(INSERT)` のみ
  - `WITH GRANT OPTION` 変更時の差分
  - `ALL` 展開済み操作集合での差分比較

**GREEN**
- `(on, grantee)` キーで privilege をマッチし、set difference ベースで `Grant/Revoke` を生成。

**REFACTOR**
- privilege set compare を専用 helper (`diff_privilege_ops`) に抽出。

**DoD**
- `cargo nextest run -p stateql-core --test privilege_diff_test` PASS。

**Commit**
```bash
git add crates/core/src/diff.rs crates/core/src/diff/privilege.rs crates/core/tests/privilege_diff_test.rs
git commit -m "feat(core): implement privilege set-difference diff semantics"
```

### Task 21: Implement View Rebuild Closure Expansion

**達成する仕様**: `R09`
**目的**: 変更viewに依存する未変更viewまで drop/create 範囲を拡張する。

**Files:**
- Modify: `crates/core/src/diff.rs`
- Create: `crates/core/src/diff/view_rebuild.rs`
- Create: `crates/core/tests/view_rebuild_scope_test.rs`

**RED**
- `base_v` 変更時に `dep_v` も再作成対象になるテストを追加。
- S11（unchanged dependent view への closure 展開）をこのタスクで先行固定する。

**GREEN**
- transitive closure と drop/create 順序実装。

**REFACTOR**
- dependency graph builder を分離。

**DoD**
- `cargo nextest run -p stateql-core --test view_rebuild_scope_test` PASS。

**Commit**
```bash
git add crates/core/src/diff.rs crates/core/src/diff/view_rebuild.rs crates/core/tests/view_rebuild_scope_test.rs
git commit -m "feat(core): expand view rebuild scope to dependent views"
```

### Task 22: Implement Diff Ordering Priorities

**達成する仕様**: `R08`
**目的**: drop/create の priority order を固定し、実行順依存バグを減らす。

**Files:**
- Create: `crates/core/src/ordering.rs`
- Modify: `crates/core/src/diff.rs`
- Modify: `crates/core/src/lib.rs`
- Create: `crates/core/tests/diff_ordering_test.rs`

**RED**
- 以下の順序テストを追加:
  - priority 1..30 の全区分を固定順で検証（省略なし）
  - Intra-table sub-priority（22a-22i）
  - 同一 priority 内の FK dependency order / view dependency order
  - 同値優先度内の declaration order fallback

**GREEN**
- priority sorter 実装。
- `ordering` 実装は `crates/core/src/ordering.rs` に統一し、`diff` module から利用する（`diff/ordering.rs` は作らない）。

**REFACTOR**
- ordering key を明示 enum で管理。

**DoD**
- `cargo nextest run -p stateql-core --test diff_ordering_test` PASS。
- `diff_ordering_test` が `DESIGN.md` の priority 1..30 全行を fixture として照合する。

**Commit**
```bash
git add crates/core/src/ordering.rs crates/core/src/diff.rs crates/core/src/lib.rs crates/core/tests/diff_ordering_test.rs
git commit -m "feat(core): add deterministic diff ordering"
```

### Task 22b: Implement Dedicated DDL Planner Module (`plan.rs`)

**達成する仕様**: `R13`, `R21`
**目的**: `DESIGN.md` §3.2 の crate 構成に合わせ、diff + ordering の結果を `plan.rs` に集約して実行計画を構築する。

**Files:**
- Create: `crates/core/src/plan.rs`
- Modify: `crates/core/src/lib.rs`
- Modify: `crates/core/src/diff.rs`
- Create: `crates/core/tests/ddl_plan_builder_test.rs`

**RED**
- `crates/core/src/plan.rs` が存在せず、ordered `DiffOp` から実行計画を構築できない失敗テストを追加。
- planner が priority 適用前の `DiffOp` をそのまま出力してしまう失敗テストを追加。

**GREEN**
- `plan.rs` に DDL planner を実装し、`DiffOp` 列から deterministic な実行計画を構築する。
- planner は ordering の責務を再実装せず、Task 22 の sorter を利用する。

**REFACTOR**
- planning 向けの型/関数を `diff.rs` から `plan.rs` へ移動し、責務境界を明確化する。

**DoD**
- `cargo nextest run -p stateql-core --test ddl_plan_builder_test` PASS。
- `crates/core/src/lib.rs` 経由で planner API が公開され、Task 29 以降から利用できる。

**Commit**
```bash
git add crates/core/src/plan.rs crates/core/src/lib.rs crates/core/src/diff.rs crates/core/tests/ddl_plan_builder_test.rs
git commit -m "feat(core): add dedicated ddl planner module"
```

### Task 23: Implement Circular FK Fallback

**達成する仕様**: `R08`
**目的**: cycle検出時に create table 後 add fk 戦略へフォールバックする。

**Files:**
- Modify: `crates/core/src/diff.rs`
- Create: `crates/core/src/diff/cycle.rs`
- Create: `crates/core/tests/circular_fk_test.rs`

**RED**
- 相互参照2テーブルで cycle error にならないテストを追加。
- S9（circular FK fallback）をこのタスクの RED として固定する。
- 自己参照 FK（同一 table 内参照）が cycle と誤検出されない失敗テストを追加。
- DROP 側の循環依存（A↔B）で cycle error にならず、`DropForeignKey` 先行 + `DropTable` declaration-order fallback になる失敗テストを追加。

**GREEN**
- CREATE 側 cycle fallback（table 作成後に `AddForeignKey` へ分離）を実装。
- DROP 側 cycle fallback（declaration order へフォールバック）を実装。
- 自己参照 FK を dependency graph から除外する実装を追加。

**REFACTOR**
- graph cycle handling を独立関数化。

**DoD**
- `cargo nextest run -p stateql-core --test circular_fk_test` PASS。
- `circular_fk_test` で CREATE 側 / DROP 側 / self-FK 除外の3観点が明示検証される。

**Commit**
```bash
git add crates/core/src/diff.rs crates/core/src/diff/cycle.rs crates/core/tests/circular_fk_test.rs
git commit -m "feat(core): handle circular fk dependencies"
```

## Phase D: Execution Layer

### Task 24: Implement Executor Transaction Grouping

**達成する仕様**: `R11`
**目的**: transactional statement を1トランザクションで束ねる。

**Files:**
- Create: `crates/core/src/executor.rs`
- Modify: `crates/core/src/lib.rs`
- Create: `crates/core/tests/executor_grouping_test.rs`

**RED**
- transactional連続 statements が1回 commit になるテストを追加。
- Task 5 の in-memory fake adapter を使い、実DBに依存しない純粋な実行順検証に限定する。

**GREEN**
- `Executor::execute_plan` 実装。

**REFACTOR**
- tx state machine を小関数に分割。

**DoD**
- `cargo nextest run -p stateql-core --test executor_grouping_test` PASS。

**Commit**
```bash
git add crates/core/src/executor.rs crates/core/src/lib.rs crates/core/tests/executor_grouping_test.rs
git commit -m "feat(core): implement executor transaction grouping"
```

### Task 25: Implement Non-Transactional Boundary Handling

**達成する仕様**: `R11`
**目的**: `transactional:false` の前後で commit-before / begin-after を行う。

**Files:**
- Modify: `crates/core/src/executor.rs`
- Create: `crates/core/tests/executor_non_transactional_test.rs`

**RED**
- `T,T,N,T,T` で commit 回数と実行順を検証。
- S3（non-transactional boundary behavior）をこのタスクで固定する。

**GREEN**
- non-transactional boundary を実装。

**REFACTOR**
- `flush_tx_if_open` helper を導入。

**DoD**
- `cargo nextest run -p stateql-core --test executor_non_transactional_test` PASS。

**Commit**
```bash
git add crates/core/src/executor.rs crates/core/tests/executor_non_transactional_test.rs
git commit -m "feat(core): handle non-transactional statement boundaries"
```

### Task 26: Implement BatchBoundary No-Commit Semantics

**達成する仕様**: `R11`
**目的**: `BatchBoundary` を同期点として扱い commit に影響させない。

**Files:**
- Modify: `crates/core/src/executor.rs`
- Create: `crates/core/tests/executor_batchboundary_test.rs`

**RED**
- `T,T,B,T` で commit 1回であることを検証。
- S4（`BatchBoundary` は commit を誘発しない）をこのタスクで固定する。

**GREEN**
- `BatchBoundary` no-op 処理を追加。

**REFACTOR**
- statement dispatch 分岐を簡素化。

**DoD**
- `cargo nextest run -p stateql-core --test executor_batchboundary_test` PASS。

**Commit**
```bash
git add crates/core/src/executor.rs crates/core/tests/executor_batchboundary_test.rs
git commit -m "feat(core): implement batchboundary synchronization semantics"
```

### Task 27: Propagate Execution Context into `ExecutionError`

**達成する仕様**: `R01`, `R11`
**目的**: 失敗 statement index/sql/context/executed_count を返す。

**Files:**
- Modify: `crates/core/src/executor.rs`
- Modify: `crates/core/src/error.rs`
- Create: `crates/core/tests/execution_error_context_test.rs`

**RED**
- 失敗時に `ExecutionError::StatementFailed` が context を保持するテストを追加。

**GREEN**
- error mapping 実装。

**REFACTOR**
- error builder helper を導入。

**DoD**
- `cargo nextest run -p stateql-core --test execution_error_context_test` PASS。

**Commit**
```bash
git add crates/core/src/executor.rs crates/core/src/error.rs crates/core/tests/execution_error_context_test.rs
git commit -m "feat(core): propagate statement context into execution errors"
```

### Task 28: Implement Renderer with Dialect Separator

**達成する仕様**: `R12`
**目的**: dry-run出力で `BatchBoundary` を dialect separator にレンダリングする。

**Files:**
- Create: `crates/core/src/renderer.rs`
- Modify: `crates/core/src/lib.rs`
- Create: `crates/core/tests/renderer_test.rs`

**RED**
- mssql-like dialect で `GO` が出力されるテストを追加。

**GREEN**
- `Renderer::new(&dyn Dialect)` 実装。

**REFACTOR**
- diagnostics header rendering を共通化。

**DoD**
- `cargo nextest run -p stateql-core --test renderer_test` PASS。

**Commit**
```bash
git add crates/core/src/renderer.rs crates/core/src/lib.rs crates/core/tests/renderer_test.rs
git commit -m "feat(core): render statements with dialect batch separator"
```

### Task 29: Implement Orchestrator `--dry-run` and `--apply`

**達成する仕様**: `R13`
**目的**: parse→normalize→diff→generate→render/execute の主経路を接続する。

**Files:**
- Create: `crates/core/src/orchestrator.rs`
- Modify: `crates/core/src/lib.rs`
- Create: `crates/core/tests/orchestrator_apply_dryrun_test.rs`
- Create: `crates/core/tests/orchestrator_diffconfig_wiring_test.rs`
- Create: `crates/core/tests/support/fake_dialect.rs`

**RED**
- dry-runはSQLを返し execute しない、applyは execute することを検証。
- `crates/core/tests/support/fake_dialect.rs` の最小 dialect 実装（`parse/normalize/generate_ddl/to_sql/connect`）を使って、orchestrator テストが未実装 dialect に依存しないことを固定する。
- orchestrator が `DiffConfig` へ `enable_drop` / `schema_search_path` / `equivalence_policy` を正しく注入しない失敗テストを追加。
- `enable_drop=false` の dry-run で `-- Skipped: ...` コメントが出力されない失敗テストを追加（diff→renderer の end-to-end）。

**GREEN**
- `Mode::{Apply,DryRun}` を実装。
- orchestrator で `DiffConfig` を構築し、`enable_drop`（CLI/runner 設定）、`schema_search_path`（adapter）、`equivalence_policy`（dialect）を diff engine に渡す。
- diff diagnostics を renderer 出力へ接続し、dry-run で `-- Skipped: ...` を表示する。

**REFACTOR**
- current/desired parse-normalize 共通関数化。

**DoD**
- `cargo nextest run -p stateql-core --test orchestrator_apply_dryrun_test` PASS。
- `cargo nextest run -p stateql-core --test orchestrator_diffconfig_wiring_test` PASS。
- `orchestrator_apply_dryrun_test` が `stateql-dialect-*` crate の実装進捗に依存せず安定して実行できる。
- `orchestrator_diffconfig_wiring_test` で `enable_drop/schema_search_path/equivalence_policy` 注入と `-- Skipped:` 描画が明示検証される。

**Commit**
```bash
git add crates/core/src/orchestrator.rs crates/core/src/lib.rs crates/core/tests/orchestrator_apply_dryrun_test.rs crates/core/tests/orchestrator_diffconfig_wiring_test.rs crates/core/tests/support/fake_dialect.rs
git commit -m "feat(core): implement orchestrator apply and dry-run flows"
```

### Task 30: Implement Orchestrator `--export`

**達成する仕様**: `R13`
**目的**: `export_schema -> parse -> normalize -> to_sql` の単一路を確立する。

**Files:**
- Modify: `crates/core/src/orchestrator.rs`
- Create: `crates/core/tests/orchestrator_export_test.rs`

**RED**
- export時に `to_sql` が呼ばれることを検証。

**GREEN**
- `Mode::Export` を実装。

**REFACTOR**
- mode dispatch を match 一本化。

**DoD**
- `cargo nextest run -p stateql-core --test orchestrator_export_test` PASS。

**Commit**
```bash
git add crates/core/src/orchestrator.rs crates/core/tests/orchestrator_export_test.rs
git commit -m "feat(core): implement orchestrator export flow"
```

### Task 30b: Verify `--export` Round-Trip Idempotency

**達成する仕様**: `R13`, `R16`
**目的**: `--export` の round-trip invariant（export -> parse -> normalize -> to_sql 再実行で不変）をテストで固定する。

**Files:**
- Create: `crates/core/tests/orchestrator_export_roundtrip_test.rs`
- Modify: `crates/core/src/orchestrator.rs`

**RED**
- 1回目と2回目の export 出力が一致しない失敗ケースを作り、差分を検出するテストを追加。

**GREEN**
- round-trip 比較を行う検証ヘルパを実装し、fixture dialect で不変性を満たす。

**REFACTOR**
- export test fixture loader を `orchestrator_export_test` と共有する。

**DoD**
- `cargo nextest run -p stateql-core --test orchestrator_export_roundtrip_test` PASS。
- 2回連続 export の結果文字列が完全一致することを CI で確認。
- dialect 実装固有の round-trip 差分は Task 33a/33b/33c/34b/34c/34d の adapter テストで検証する前提を明記。

**Commit**
```bash
git add crates/core/tests/orchestrator_export_roundtrip_test.rs crates/core/src/orchestrator.rs
git commit -m "test(core): enforce export roundtrip idempotency"
```

## Phase E: Dialect Implementations

**Scope（明示）**
- Phase E は skeleton を置かず、DESIGN.md §5.2-§5.5 を full 実装する。
- 各 dialect は `parse/generate_ddl/normalize/to_sql/connect` に加え、adapter の `export_schema/schema_search_path/server_version` を実装する。
- `export_schema` は `reference/sqldef/` の query を移植し、差分が必要な場合は理由を task 内で明記する。
- 各 dialect の `connect()` は `server_version()` を使って minimum supported version（PG13+/MySQL8.0+/MSSQL2019+/SQLite3.35+）を fail-fast 検証する。
- PostgreSQL 以外の dialect も `equivalence_policy()` の方針（default/custom）を明示し、false diff 回帰テストを持つ。

**Test Strategy（実DB調達）**
- PostgreSQL/MySQL/MSSQL の adapter integration test は `testcontainers`（または同等のコンテナ起動）を前提にする。
- コンテナ必須テストは `#[ignore = "requires container runtime"]` で区別し、通常 `cargo nextest run` では unit/offline を優先する。
- CI では DB コンテナを起動する専用 job で ignore テストを実行する（SQLite は in-memory のため常時実行）。

### Task 31: PostgreSQL Parse Pipeline with `pg_query`

**達成する仕様**: `R14`, `R05`
**目的**: annotation extraction + fail-fast statement conversion を接続する。

**Files:**
- Create: `crates/dialect-postgres/src/parser.rs`
- Create: `crates/dialect-postgres/src/extra_keys.rs`
- Modify: `crates/dialect-postgres/src/lib.rs`
- Modify: `crates/dialect-postgres/Cargo.toml`
- Create: `crates/dialect-postgres/tests/parser_test.rs`

**RED**
- unsupported statement で `statement_index` / `source_sql fragment` / `SourceLocation` を含む parse error を確認。
- S1（unknown/unsupported DDL fail-fast, no silent drop）を parser段で固定する。

**GREEN**
- `pg_query` parser + conversion loop 実装。
- `extra` map の key を raw string で持たず、`extra_keys` module の `pub(crate) const` で定義する規約を導入。
- `ParseError::StatementConversion` へ `statement_index` / `source_sql` / `source_location` を必ず付与する。

**REFACTOR**
- statement error wrapping helper を抽出。

**DoD**
- `cargo nextest run -p stateql-dialect-postgres --test parser_test` PASS。
- `parser_test` で `source_sql` と `source_location.line` が検証される（column は取得不能時 `None` を許容）。

**Commit**
```bash
git add crates/dialect-postgres/src/parser.rs crates/dialect-postgres/src/extra_keys.rs crates/dialect-postgres/src/lib.rs crates/dialect-postgres/Cargo.toml crates/dialect-postgres/tests/parser_test.rs
git commit -m "feat(postgres): implement fail-fast parser pipeline"
```

### Task 32: PostgreSQL Full DDL Generator + Unsupported-Op Contract

**達成する仕様**: `R02`, `R15`
**目的**: DESIGN.md §5.2 Step 2 を満たす PostgreSQL generator を実装し、未対応 `DiffOp` は必ず `GenerateError` にする。

**Files:**
- Create: `crates/dialect-postgres/src/generator.rs`
- Modify: `crates/dialect-postgres/src/lib.rs`
- Create: `crates/dialect-postgres/tests/generator_contract_test.rs`
- Create: `crates/dialect-postgres/tests/generator_supported_ops_test.rs`

**RED**
- supported `DiffOp`（table/column/index/fk/check/exclusion/view/materialized view/sequence/trigger/function/type/domain/extension/schema/comment/privilege/policy）の SQL 生成失敗を再現するテストを追加。
- `DiffOp` 全 variant を列挙し、各 variant が supported（有効 SQL 生成）か unsupported（`GenerateError`）かを明示分類するテストを追加。未分類 variant があれば失敗。
- `DropView + CreateView` の `CREATE OR REPLACE VIEW` 最適化条件（互換時のみ）をテストで固定する。

**GREEN**
- PostgreSQL 向け `generate_ddl(&[DiffOp]) -> Vec<Statement>` を full 実装。
- view 最適化は互換条件に合致する場合のみ `CREATE OR REPLACE VIEW` を使い、不一致時は `DROP + CREATE` を維持する。

**REFACTOR**
- object-kind ごとの SQL builder を module 分割し、unsupported 判定を終端1箇所に集約。

**DoD**
- `cargo nextest run -p stateql-dialect-postgres --test generator_contract_test` PASS。
- `cargo nextest run -p stateql-dialect-postgres --test generator_supported_ops_test` PASS。
- `generator_contract_test` が Task 13 で作成した `all_diffop_variants()` を利用し、`DiffOp` 全 variant を exhaustive に分類していること（新 variant 追加時にテストが落ちてカバー漏れを検出できること）。

**Commit**
```bash
git add crates/dialect-postgres/src/generator.rs crates/dialect-postgres/src/lib.rs crates/dialect-postgres/tests/generator_contract_test.rs crates/dialect-postgres/tests/generator_supported_ops_test.rs
git commit -m "feat(postgres): implement full ddl generator with fail-fast coverage"
```

### PostgreSQL Export/Normalize/Equivalence Track (Task 33a-33c)

Task 33 は集約見出しであり、実行タスクは 33a/33b/33c のみとする。

### Task 33a: PostgreSQL Normalizer

**達成する仕様**: `R02`, `R13`
**目的**: normalize 責務（型/式/sequence/partition 表現の安定化）を先に閉じ、後続 adapter/to_sql と独立に検証可能にする。

**Files:**
- Create: `crates/dialect-postgres/src/normalize.rs`
- Modify: `crates/dialect-postgres/src/lib.rs`
- Create: `crates/dialect-postgres/tests/normalize_representation_rules_test.rs`

**RED**
- Sequence representation rules（explicit sequence / serial / identity）違反を検知する失敗テストを追加。
- `DataType::Custom` alias の表記ゆれで false diff が出る失敗ケースを追加。
- Partition child folding: `export_schema()` が返す `CREATE TABLE child PARTITION OF parent FOR VALUES ...` が separate `SchemaObject::Table` として残り、diff engine が意図しない `DropTable`/`CreateTable` を生成する失敗テストを追加。

**GREEN**
- normalizer で `DataType::Custom` 正規化、alias 解決、Sequence contract を実装。
- Partition child folding: partition child を検出し、parent の `Partition.partitions` に `PartitionElement` として折り畳む処理を実装する（DESIGN.md §3.4「PostgreSQL declarative partitioning」準拠）。

**REFACTOR**
- normalize helper を `types` / `expr` / `sequence` / `partition` に分割し責務を明確化。

**DoD**
- `cargo nextest run -p stateql-dialect-postgres --test normalize_representation_rules_test` PASS。
- partition child → `PartitionElement` 折り畳みが同テスト内で検証されること。

**Commit**
```bash
git add crates/dialect-postgres/src/normalize.rs crates/dialect-postgres/src/lib.rs crates/dialect-postgres/tests/normalize_representation_rules_test.rs
git commit -m "feat(postgres): implement normalizer with sequence and partition rules"
```

### Task 33b: PostgreSQL `to_sql` + Adapter + Export Queries

**達成する仕様**: `R02`, `R10`, `R13`
**目的**: adapter/export/to_sql の実運用経路を normalize から分離して実装し、round-trip 安定性を検証する。

**Files:**
- Create: `crates/dialect-postgres/src/to_sql.rs`
- Create: `crates/dialect-postgres/src/adapter.rs`
- Create: `crates/dialect-postgres/src/export_queries.rs`
- Modify: `crates/dialect-postgres/src/lib.rs`
- Modify: `crates/dialect-postgres/Cargo.toml`
- Create: `crates/dialect-postgres/tests/adapter_export_schema_test.rs`
- Create: `crates/dialect-postgres/tests/dialect_surface_test.rs`

**RED**
- `export_schema -> parse -> normalize -> to_sql` で round-trip が崩れるケースを再現するテストを追加。
- `schema_search_path()` が `SHOW search_path` を正しく反映しない失敗ケースを追加。
- （container runtime 前提）catalog query 差異で export 欠落が発生する失敗ケースを追加。
- PostgreSQL `13` 未満で `connect()` が fail-fast にならない失敗ケースを追加。

**GREEN**
- `connect()`、`export_schema()`、`schema_search_path()`、`server_version()` を実装。
- `reference/sqldef/` の PostgreSQL catalog query を `export_queries.rs` に移植。
- `to_sql()` を full 実装し、`--export` の canonical rendering を提供。
- `server_version()` 取得後に PostgreSQL `13+` を満たさない接続を `Error` で拒否する preflight check を実装。

**REFACTOR**
- query 組み立てを constants 化し、テスト fixture と文字列共有しない構造へ整理。

**DoD**
- `cargo nextest run -p stateql-dialect-postgres --test dialect_surface_test` PASS。
- `cargo nextest run -p stateql-dialect-postgres --test adapter_export_schema_test` PASS。
- コンテナ前提テストは `cargo nextest run -p stateql-dialect-postgres --test adapter_export_schema_test --run-ignored ignored-only` で PASS。
- `rg -n "pub async fn" crates/dialect-postgres/src` が 0 hit。
- `dialect_surface_test` で `13` 未満の version 文字列が reject されることを明示検証する。

**Commit**
```bash
git add crates/dialect-postgres/src/to_sql.rs crates/dialect-postgres/src/adapter.rs crates/dialect-postgres/src/export_queries.rs crates/dialect-postgres/src/lib.rs crates/dialect-postgres/Cargo.toml crates/dialect-postgres/tests/adapter_export_schema_test.rs crates/dialect-postgres/tests/dialect_surface_test.rs
git commit -m "feat(postgres): implement adapter export and canonical to_sql"
```

### Task 33c: PostgreSQL EquivalencePolicy Implementation

**達成する仕様**: `R06`
**目的**: DESIGN.md §7.1 の high risk（cast/literal 由来 false diff）を PostgreSQL dialect 側 policy で吸収する。

**Files:**
- Create: `crates/dialect-postgres/src/equivalence.rs`
- Modify: `crates/dialect-postgres/src/lib.rs`
- Create: `crates/dialect-postgres/tests/equivalence_policy_test.rs`

**RED**
- `0` vs `'0'::integer`、不要括弧、空白差異で false diff が発生する失敗テストを追加。
- normalization 後でも残る式差異が default policy では吸収されないことを確認する失敗ケースを追加。
- 対称性（`a,b` と `b,a`）/反復安定性（同一入力で同一結果）が破れる失敗テストを追加。

**GREEN**
- PostgreSQL 専用 `EquivalencePolicy` を実装し、`Dialect::equivalence_policy()` から返す。
- policy は strict-eq 後の残差にのみ適用し、構造不一致の緩和は行わない。

**REFACTOR**
- normalize で処理する責務と policy で処理する責務の境界を doc comment に明記。

**DoD**
- `cargo nextest run -p stateql-dialect-postgres --test equivalence_policy_test` PASS。
- false diff 回帰ケースを core diff 経路で再現しないことを確認。
- `equivalence_policy_test` で symmetry / stability contract が明示検証される。

**Commit**
```bash
git add crates/dialect-postgres/src/equivalence.rs crates/dialect-postgres/src/lib.rs crates/dialect-postgres/tests/equivalence_policy_test.rs
git commit -m "feat(postgres): implement dialect-specific equivalence policy"
```

### Task 34: Add MySQL/SQLite/MSSQL Parse Pipelines (`sqlparser-rs`)

**達成する仕様**: `R14`, `R05`
**目的**: 3 dialect で annotation extraction + fail-fast statement conversion loop を full 実装する。
**実行単位**: 本タスクは `34-mysql` / `34-sqlite` / `34-mssql` の3単位で実施し、各単位の DoD を個別に満たしてから Task 34 完了とする。

**Files:**
- Create: `crates/dialect-mysql/src/parser.rs`
- Create: `crates/dialect-sqlite/src/parser.rs`
- Create: `crates/dialect-mssql/src/parser.rs`
- Create: `crates/dialect-mysql/src/extra_keys.rs`
- Create: `crates/dialect-sqlite/src/extra_keys.rs`
- Create: `crates/dialect-mssql/src/extra_keys.rs`
- Modify: `crates/dialect-mysql/src/lib.rs`
- Modify: `crates/dialect-sqlite/src/lib.rs`
- Modify: `crates/dialect-mssql/src/lib.rs`
- Create: `crates/dialect-mysql/tests/parser_test.rs`
- Create: `crates/dialect-sqlite/tests/parser_test.rs`
- Create: `crates/dialect-mssql/tests/parser_test.rs`

**RED**
- `34-mysql`: unsupported statement が `statement_index` / `source_sql fragment` / `SourceLocation` 付き `ParseError` になる失敗テストを追加。
- `34-sqlite`: unsupported statement が `statement_index` / `source_sql fragment` / `SourceLocation` 付き `ParseError` になる失敗テストを追加。
- `34-mssql`: unsupported statement が `statement_index` / `source_sql fragment` / `SourceLocation` 付き `ParseError` になる失敗テストを追加。
- 各 dialect 共通で orphan annotation が parse 成功扱いにならない失敗テストを追加。

**GREEN**
- `34-mysql`: MySQL parser 実装（`CHANGE COLUMN` / `AFTER` / `AUTO_INCREMENT` に必要な statement conversion 前段まで）。
- `34-sqlite`: SQLite parser 実装（table recreation に必要な statement conversion 前段まで）。
- `34-mssql`: MSSQL parser 実装（T-SQL 方言の statement conversion 前段まで）。
- 各 dialect とも `extra` key は constants 経由でのみ参照する。
- parser が列番号を提供できない場合でも、core statement splitter で statement 開始行を `SourceLocation.line` に付与する。

**REFACTOR**
- 共通の statement error wrapping helper を抽出しつつ、dialect ごとの差異点は helper に押し込めず各 crate に残す。

**DoD**
- `cargo nextest run -p stateql-dialect-mysql --test parser_test` PASS。
- `cargo nextest run -p stateql-dialect-sqlite --test parser_test` PASS。
- `cargo nextest run -p stateql-dialect-mssql --test parser_test` PASS。
- 3 dialect すべてで parser test が `statement_index` / `source_sql` / `source_location.line` の保持を検証する。
- 上記3コマンドを個別 task completion の証跡として扱い、1つでも未達なら Task 34 は未完了とする。

**Commit**
```bash
git add crates/dialect-mysql/src/parser.rs crates/dialect-sqlite/src/parser.rs crates/dialect-mssql/src/parser.rs crates/dialect-mysql/src/extra_keys.rs crates/dialect-sqlite/src/extra_keys.rs crates/dialect-mssql/src/extra_keys.rs crates/dialect-mysql/src/lib.rs crates/dialect-sqlite/src/lib.rs crates/dialect-mssql/src/lib.rs crates/dialect-mysql/tests/parser_test.rs crates/dialect-sqlite/tests/parser_test.rs crates/dialect-mssql/tests/parser_test.rs
git commit -m "feat(dialects): implement fail-fast parse pipelines for mysql sqlite mssql"
```

### Task 34b: SQLite Full Normalize / to_sql / Adapter

**達成する仕様**: `R02`, `R10`, `R13`
**目的**: SQLite の export-normalize-render 経路を full 実装し、in-memory DB で検証可能にする。

**Files:**
- Create: `crates/dialect-sqlite/src/normalize.rs`
- Create: `crates/dialect-sqlite/src/to_sql.rs`
- Create: `crates/dialect-sqlite/src/adapter.rs`
- Create: `crates/dialect-sqlite/src/export_queries.rs`
- Modify: `crates/dialect-sqlite/src/lib.rs`
- Create: `crates/dialect-sqlite/tests/adapter_export_schema_test.rs`
- Create: `crates/dialect-sqlite/tests/dialect_surface_test.rs`

**RED**
- in-memory SQLite で `export_schema()` の不足（欠落 object や順序不安定）を再現するテストを追加。
- normalize 後 `to_sql` 出力が不安定なケースを追加。
- SQLite `3.35` 未満で `connect()` が fail-fast にならない失敗ケースを追加。
- `to_sql` variant 網羅: SQLite がサポートする全 `SchemaObject` variant（Table, View, Index, Trigger）に対して `to_sql` が有効な SQL を返さない失敗テストを追加。サポート外 variant（Extension, Domain, Policy, Sequence 等）に対して silent に空文字列を返す（エラーにならない）失敗テストを追加。

**GREEN**
- `connect/export_schema/schema_search_path/server_version/normalize/to_sql` を実装。
- `reference/sqldef/` の SQLite export query を移植し、必要差分をコメントで明記。
- `server_version()` 取得後に SQLite `3.35+` を満たさない接続を `Error` で拒否する preflight check を実装。
- `to_sql` がサポート variant で有効 SQL を返し、非サポート variant で明示的 `Error` を返す実装にする。

**REFACTOR**
- SQLite pragma/query helper を `export_queries` module に集約。

**DoD**
- `cargo nextest run -p stateql-dialect-sqlite --test dialect_surface_test` PASS。
- `cargo nextest run -p stateql-dialect-sqlite --test adapter_export_schema_test` PASS。
- `rg -n "pub async fn" crates/dialect-sqlite/src` が 0 hit。
- `dialect_surface_test` で `3.35` 未満の version 文字列が reject されることを明示検証する。
- `dialect_surface_test` で「サポート variant → valid SQL」「非サポート variant → Error」の網羅が検証されること。

**Commit**
```bash
git add crates/dialect-sqlite/src/normalize.rs crates/dialect-sqlite/src/to_sql.rs crates/dialect-sqlite/src/adapter.rs crates/dialect-sqlite/src/export_queries.rs crates/dialect-sqlite/src/lib.rs crates/dialect-sqlite/tests/adapter_export_schema_test.rs crates/dialect-sqlite/tests/dialect_surface_test.rs
git commit -m "feat(sqlite): implement full adapter normalization and export rendering"
```

### Task 34c: MySQL Full Normalize / to_sql / Adapter

**達成する仕様**: `R02`, `R10`, `R13`
**目的**: MySQL の export-normalize-render 経路を full 実装し、`lower_case_table_names` を含む正規化差分を吸収する。

**Files:**
- Create: `crates/dialect-mysql/src/normalize.rs`
- Create: `crates/dialect-mysql/src/to_sql.rs`
- Create: `crates/dialect-mysql/src/adapter.rs`
- Create: `crates/dialect-mysql/src/export_queries.rs`
- Modify: `crates/dialect-mysql/src/lib.rs`
- Create: `crates/dialect-mysql/tests/adapter_export_schema_test.rs`
- Create: `crates/dialect-mysql/tests/normalize_lower_case_table_names_test.rs`
- Create: `crates/dialect-mysql/tests/dialect_surface_test.rs`

**RED**
- `lower_case_table_names` 設定差異で false diff が出る失敗テストを追加。
- export SQL の parse-normalize-to_sql round-trip 不一致ケースを追加。
- MySQL `8.0` 未満で `connect()` が fail-fast にならない失敗ケースを追加。
- `to_sql` variant 網羅: MySQL がサポートする全 `SchemaObject` variant（Table, View, Index, Trigger, Function 等）に対して `to_sql` が有効な SQL を返さない失敗テストを追加。サポート外 variant（Domain, Extension, Policy 等）に対して silent に空文字列を返す（エラーにならない）失敗テストを追加。

**GREEN**
- `connect/export_schema/schema_search_path/server_version/normalize/to_sql` を実装。
- `reference/sqldef/` の MySQL export query を移植し、`AUTO_INCREMENT`/partitioning 関連 metadata を `extra` に保持する。
- `server_version()` 取得後に MySQL `8.0+` を満たさない接続を `Error` で拒否する preflight check を実装。
- `to_sql` がサポート variant で有効 SQL を返し、非サポート variant で明示的 `Error` を返す実装にする。

**REFACTOR**
- MySQL type alias/collation 正規化を helper module 化。

**DoD**
- `cargo nextest run -p stateql-dialect-mysql --test dialect_surface_test` PASS。
- `cargo nextest run -p stateql-dialect-mysql --test adapter_export_schema_test` PASS。
- `cargo nextest run -p stateql-dialect-mysql --test normalize_lower_case_table_names_test` PASS。
- コンテナ前提テストは `cargo nextest run -p stateql-dialect-mysql --test adapter_export_schema_test --run-ignored ignored-only` で PASS。
- `rg -n "pub async fn" crates/dialect-mysql/src` が 0 hit。
- `dialect_surface_test` で「サポート variant → valid SQL」「非サポート variant → Error」の網羅が検証されること。
- `dialect_surface_test` で `8.0` 未満の version 文字列が reject されることを明示検証する。

**Commit**
```bash
git add crates/dialect-mysql/src/normalize.rs crates/dialect-mysql/src/to_sql.rs crates/dialect-mysql/src/adapter.rs crates/dialect-mysql/src/export_queries.rs crates/dialect-mysql/src/lib.rs crates/dialect-mysql/tests/adapter_export_schema_test.rs crates/dialect-mysql/tests/normalize_lower_case_table_names_test.rs crates/dialect-mysql/tests/dialect_surface_test.rs
git commit -m "feat(mysql): implement full adapter normalization and export rendering"
```

### Task 34d: MSSQL Full Normalize / to_sql / Adapter

**達成する仕様**: `R02`, `R10`, `R13`
**目的**: MSSQL の export-normalize-render 経路を full 実装し、IDENTITY/clustered/NOT FOR REPLICATION の表現を安定化する。

**Files:**
- Create: `crates/dialect-mssql/src/normalize.rs`
- Create: `crates/dialect-mssql/src/to_sql.rs`
- Create: `crates/dialect-mssql/src/adapter.rs`
- Create: `crates/dialect-mssql/src/export_queries.rs`
- Modify: `crates/dialect-mssql/src/lib.rs`
- Create: `crates/dialect-mssql/tests/adapter_export_schema_test.rs`
- Create: `crates/dialect-mssql/tests/dialect_surface_test.rs`

**RED**
- IDENTITY seed/increment と clustered index 情報が round-trip で失われる失敗テストを追加。
- `schema_search_path`（通常 `dbo`）が不一致となるケースを追加。
- SQL Server `2019` 未満で `connect()` が fail-fast にならない失敗ケースを追加。
- `to_sql` variant 網羅: MSSQL がサポートする全 `SchemaObject` variant（Table, View, Index, Trigger, Function, Schema 等）に対して `to_sql` が有効な SQL を返さない失敗テストを追加。サポート外 variant（Extension, Domain, Policy 等）に対して silent に空文字列を返す（エラーにならない）失敗テストを追加。

**GREEN**
- `connect/export_schema/schema_search_path/server_version/normalize/to_sql` を実装。
- `reference/sqldef/` の MSSQL export query を移植。
- async driver を内部利用する場合も adapter 公開境界は同期 API に固定する。
- `server_version()` 取得後に SQL Server `2019+` を満たさない接続を `Error` で拒否する preflight check を実装。
- `to_sql` がサポート variant で有効 SQL を返し、非サポート variant で明示的 `Error` を返す実装にする。

**REFACTOR**
- MSSQL-specific identifier quoting/normalization helper を抽出。

**DoD**
- `cargo nextest run -p stateql-dialect-mssql --test dialect_surface_test` PASS。
- `cargo nextest run -p stateql-dialect-mssql --test adapter_export_schema_test` PASS。
- コンテナ前提テストは `cargo nextest run -p stateql-dialect-mssql --test adapter_export_schema_test --run-ignored ignored-only` で PASS。
- `rg -n "pub async fn" crates/dialect-mssql/src` が 0 hit。
- `dialect_surface_test` で `2019` 未満の version 文字列が reject されることを明示検証する。
- `dialect_surface_test` で「サポート variant → valid SQL」「非サポート variant → Error」の網羅が検証されること。

**Commit**
```bash
git add crates/dialect-mssql/src/normalize.rs crates/dialect-mssql/src/to_sql.rs crates/dialect-mssql/src/adapter.rs crates/dialect-mssql/src/export_queries.rs crates/dialect-mssql/src/lib.rs crates/dialect-mssql/tests/adapter_export_schema_test.rs crates/dialect-mssql/tests/dialect_surface_test.rs
git commit -m "feat(mssql): implement full adapter normalization and export rendering"
```

### Task 34e: Add MySQL/SQLite/MSSQL Equivalence Policy Coverage

**達成する仕様**: `R06`
**目的**: PostgreSQL 以外の3 dialect で `equivalence_policy()` の方針（default か custom か）を明示し、false diff 回帰を固定する。

**Files:**
- Modify: `crates/dialect-mysql/src/lib.rs`
- Modify: `crates/dialect-sqlite/src/lib.rs`
- Modify: `crates/dialect-mssql/src/lib.rs`
- Create: `crates/dialect-mysql/tests/equivalence_policy_test.rs`
- Create: `crates/dialect-sqlite/tests/equivalence_policy_test.rs`
- Create: `crates/dialect-mssql/tests/equivalence_policy_test.rs`
- Create: `docs/2026-02-21-equivalence-policy-matrix.md`

**RED**
- MySQL/SQLite/MSSQL それぞれで known false diff（型 alias / literal cast / 括弧・空白差異）を再現する失敗テストを追加。
- default policy のままでは吸収できない差分が存在するケースを失敗テストとして固定する。

**GREEN**
- 各 dialect ごとに `equivalence_policy()` を明示実装し、「default policy 維持」または「custom policy 導入」を選択して根拠を残す。
- normalize で吸収すべき差分と policy で吸収すべき差分を分離し、残差のみ policy で扱う。
- `docs/2026-02-21-equivalence-policy-matrix.md` に dialect ごとの方針・対象パターン・非対象パターンを記載する。

**REFACTOR**
- core の symmetry/stability contract helper を各 dialect テストから再利用し、検証観点を統一する。

**DoD**
- `cargo nextest run -p stateql-dialect-mysql --test equivalence_policy_test` PASS。
- `cargo nextest run -p stateql-dialect-sqlite --test equivalence_policy_test` PASS。
- `cargo nextest run -p stateql-dialect-mssql --test equivalence_policy_test` PASS。
- `docs/2026-02-21-equivalence-policy-matrix.md` に 3 dialect の方針差分が明記される。

**Commit**
```bash
git add crates/dialect-mysql/src/lib.rs crates/dialect-sqlite/src/lib.rs crates/dialect-mssql/src/lib.rs crates/dialect-mysql/tests/equivalence_policy_test.rs crates/dialect-sqlite/tests/equivalence_policy_test.rs crates/dialect-mssql/tests/equivalence_policy_test.rs docs/2026-02-21-equivalence-policy-matrix.md
git commit -m "test(dialects): fix and document non-postgres equivalence policy coverage"
```

### Task 35: SQLite Full Generator (Rebuild + Unsupported-Op Contract)

**達成する仕様**: `R15`, `R11`
**目的**: SQLite の table recreation を含む full generator を実装し、非対応 op は fail-fast で落とす。

**Files:**
- Create: `crates/dialect-sqlite/src/generator.rs`
- Modify: `crates/dialect-sqlite/src/lib.rs`
- Create: `crates/dialect-sqlite/tests/rebuild_generator_test.rs`
- Create: `crates/dialect-sqlite/tests/generator_contract_test.rs`

**RED**
- `AlterColumn` が rebuild step (`CopyData` など) に展開されるテストを追加。
- S10（SQLite table recreation atomicity）として、途中失敗時に全体 rollback されることを検証。
- `DiffOp` 全 variant を列挙し（Task 13 で作成した `all_diffop_variants()` を利用）、各 variant が supported（有効 SQL 生成）か unsupported（`GenerateError`）かを明示分類するテストを追加。未分類 variant があれば失敗。

**GREEN**
- simple ALTER / rebuild rewrite / unsupported-op fail-fast を実装。
- `StatementContext::SqliteTableRebuild` を rebuild 各 step に付与する。

**REFACTOR**
- rebuild statement builder を helper 化。

**DoD**
- `cargo nextest run -p stateql-dialect-sqlite --test rebuild_generator_test` PASS。
- `cargo nextest run -p stateql-dialect-sqlite --test generator_contract_test` PASS。
- `generator_contract_test` が `DiffOp` 全 variant を exhaustive に分類していること（新 variant 追加時にテストが落ちてカバー漏れを検出できること）。

**Commit**
```bash
git add crates/dialect-sqlite/src/generator.rs crates/dialect-sqlite/src/lib.rs crates/dialect-sqlite/tests/rebuild_generator_test.rs crates/dialect-sqlite/tests/generator_contract_test.rs
git commit -m "feat(sqlite): implement full generator with rebuild and fail-fast contracts"
```

### Task 36: MySQL Full Generator (CHANGE COLUMN / AUTO_INCREMENT / AFTER / Partitioning)

**達成する仕様**: `R15`
**目的**: MySQL 特有の DDL 合成規則を含む full generator を実装する。

**Files:**
- Create: `crates/dialect-mysql/src/generator.rs`
- Modify: `crates/dialect-mysql/src/lib.rs`
- Create: `crates/dialect-mysql/tests/change_column_merge_test.rs`
- Create: `crates/dialect-mysql/tests/auto_increment_ordering_test.rs`
- Create: `crates/dialect-mysql/tests/column_position_after_test.rs`
- Create: `crates/dialect-mysql/tests/partition_generator_test.rs`
- Create: `crates/dialect-mysql/tests/generator_contract_test.rs`

**RED**
- 同一列への複数 `AlterColumn` が 1 つの `CHANGE COLUMN` に統合される失敗テストを追加。
- PK 変更 + `AUTO_INCREMENT` で two-phase ordering が守られないケースを追加。
- `AddColumn ... AFTER` の位置が崩れるケースを追加。
- `DiffOp` 全 variant を列挙し（Task 13 で作成した `all_diffop_variants()` を利用）、各 variant が supported（有効 SQL 生成）か unsupported（`GenerateError`）かを明示分類するテストを追加。未分類 variant があれば失敗。

**GREEN**
- `CHANGE COLUMN` merge、`AUTO_INCREMENT` two-phase、`AFTER` 位置制御、partitioning SQL 生成を実装。
- `DropView + CreateView` を `CREATE OR REPLACE VIEW` に置換できるケースは置換し、不可時は元の順序を維持する。

**REFACTOR**
- per-table aggregation map と SQL builder を分離。

**DoD**
- `cargo nextest run -p stateql-dialect-mysql --test change_column_merge_test` PASS。
- `cargo nextest run -p stateql-dialect-mysql --test auto_increment_ordering_test` PASS。
- `cargo nextest run -p stateql-dialect-mysql --test column_position_after_test` PASS。
- `cargo nextest run -p stateql-dialect-mysql --test partition_generator_test` PASS。
- `cargo nextest run -p stateql-dialect-mysql --test generator_contract_test` PASS。
- `generator_contract_test` が `DiffOp` 全 variant を exhaustive に分類していること（新 variant 追加時にテストが落ちてカバー漏れを検出できること）。

**Commit**
```bash
git add crates/dialect-mysql/src/generator.rs crates/dialect-mysql/src/lib.rs crates/dialect-mysql/tests/change_column_merge_test.rs crates/dialect-mysql/tests/auto_increment_ordering_test.rs crates/dialect-mysql/tests/column_position_after_test.rs crates/dialect-mysql/tests/partition_generator_test.rs crates/dialect-mysql/tests/generator_contract_test.rs
git commit -m "feat(mysql): implement full ddl generator semantics"
```

### Task 37: MSSQL Full Generator (`BatchBoundary` / `sp_rename` / `IDENTITY`)

**達成する仕様**: `R15`, `R12`
**目的**: MSSQL generator のコア仕様（batch boundary, rename, identity, clustered, NFR）を full 実装する。

**Files:**
- Create: `crates/dialect-mssql/src/generator.rs`
- Modify: `crates/dialect-mssql/src/lib.rs`
- Create: `crates/dialect-mssql/tests/mssql_generator_test.rs`
- Create: `crates/dialect-mssql/tests/identity_and_clustered_test.rs`
- Create: `crates/dialect-mssql/tests/generator_contract_test.rs`

**RED**
- create+rename ops で `BatchBoundary` と `sp_rename` が含まれる失敗テストを追加。
- `IDENTITY(seed, increment)`、clustered index、`NOT FOR REPLICATION` を含むケースを追加。
- `DiffOp` 全 variant を列挙し（Task 13 で作成した `all_diffop_variants()` を利用）、各 variant が supported（有効 SQL 生成）か unsupported（`GenerateError`）かを明示分類するテストを追加。未分類 variant があれば失敗。

**GREEN**
- mssql generator と `batch_separator() -> "GO\n"` を実装。
- `sp_rename` の対象種別（table/column/index）ごとの SQL 生成を実装。

**REFACTOR**
- batch injection 条件と rename SQL builder を関数化。

**DoD**
- `cargo nextest run -p stateql-dialect-mssql --test mssql_generator_test` PASS。
- `cargo nextest run -p stateql-dialect-mssql --test identity_and_clustered_test` PASS。
- `cargo nextest run -p stateql-dialect-mssql --test generator_contract_test` PASS。
- `generator_contract_test` が `DiffOp` 全 variant を exhaustive に分類していること（新 variant 追加時にテストが落ちてカバー漏れを検出できること）。

**Commit**
```bash
git add crates/dialect-mssql/src/generator.rs crates/dialect-mssql/src/lib.rs crates/dialect-mssql/tests/mssql_generator_test.rs crates/dialect-mssql/tests/identity_and_clustered_test.rs crates/dialect-mssql/tests/generator_contract_test.rs
git commit -m "feat(mssql): implement full ddl generator semantics"
```

## Phase F: TestKit, Safety, CLI, Docs

### Task 38: Add YAML `TestCase` Schema with Defaults

**達成する仕様**: `R16`
**目的**: `offline` omitted 時の default に加え、`error` / `enable_drop` / `min_version` / `max_version` / `flavor` を含む TestCase 契約を固定する。

**Files:**
- Create: `crates/testkit/src/yaml_runner.rs`
- Modify: `crates/testkit/src/lib.rs`
- Modify: `crates/testkit/Cargo.toml`
- Create: `crates/testkit/tests/testcase_schema_test.rs`

**RED**
- `offline` 省略 YAML の parse テストを追加。
- `error` フィールドが取り込まれない失敗テストを追加。
- `enable_drop` が omitted / true / false を区別できない失敗テストを追加。
- `min_version` / `max_version` が保持されない失敗テストを追加。
- `flavor` 文字列が保持されない失敗テストを追加（判定ロジック自体は Task 38b で実装）。

**GREEN**
- `TestCase` struct + `#[serde(default)]` 実装。
- `error: Option<String>` と `enable_drop: Option<bool>` を schema に含め、三値（`None/Some(true)/Some(false)`）を保持する。
- `min_version: Option<String>` / `max_version: Option<String>` / `flavor: Option<String>` を schema に含め、入力YAMLの値を保持する。
- runner 側の解決規則（`enable_drop: None` は実行時に `false` として扱う）は Task 39/40 で検証する前提を明記する。

**REFACTOR**
- loader エラー型を core `Error` に寄せる。

**DoD**
- `cargo nextest run -p stateql-testkit --test testcase_schema_test` PASS。
- `testcase_schema_test` で `error` 文字列保持、`enable_drop` 三値保持、`min_version/max_version/flavor` 保持が検証される。

**Commit**
```bash
git add crates/testkit/src/yaml_runner.rs crates/testkit/src/lib.rs crates/testkit/Cargo.toml crates/testkit/tests/testcase_schema_test.rs
git commit -m "feat(testkit): add yaml testcase schema with defaults"
```

### Task 38b: Implement YAML `flavor` Matching Semantics

**達成する仕様**: `R16`
**目的**: `flavor` フィールド（positive / negative match）の実行規則を固定し、移植ケースの誤注釈を検出可能にする。

**Files:**
- Modify: `crates/testkit/src/yaml_runner.rs`
- Create: `crates/testkit/tests/yaml_flavor_filter_test.rs`

**RED**
- `flavor: mysql`（positive）と `flavor: !tidb`（negative）の判定が欠落している失敗テストを追加。
- flavor 不一致ケースで「期待通り失敗したら skip / 通ってしまったら fail」となる検証が欠落している失敗テストを追加。

**GREEN**
- `flavor` 判定器を実装し、positive/negative matching を runner に統合する。
- sqldef 参照実装と同様に、flavor 不一致では「失敗を期待する」モードで実行して注釈の妥当性を検証する。

**REFACTOR**
- flavor 判定ロジックを pure function として切り出し、online/offline runner の両方から再利用する。

**DoD**
- `cargo nextest run -p stateql-testkit --test yaml_flavor_filter_test` PASS。
- `run_online_test` / `run_offline_test` の双方で `flavor` 判定が適用される。

**Commit**
```bash
git add crates/testkit/src/yaml_runner.rs crates/testkit/tests/yaml_flavor_filter_test.rs
git commit -m "feat(testkit): implement yaml flavor matching semantics"
```

### Task 39: Add Offline Runner

**達成する仕様**: `R16`
**目的**: DB非依存で parse→normalize→diff→generate 比較を実行可能にし、orchestrator 完了前に test feedback loop を確立する。

**Files:**
- Modify: `crates/testkit/src/yaml_runner.rs`
- Create: `crates/testkit/tests/offline_runner_test.rs`
- Create: `crates/testkit/tests/support/offline_fake_dialect.rs`

**RED**
- `offline_fake_dialect` を使い、orchestrator 未実装でも `run_offline_test` が失敗する状態を再現する。
- `error` 指定ケースで「期待エラー不一致」や「本来失敗すべきケースの成功」を見逃す失敗テストを追加。
- `enable_drop` 指定（`None/Some(false)/Some(true)`）が `DiffConfig.enable_drop` へ反映されない失敗テストを追加。

**GREEN**
- offline runner 実装（dialect.parse/normalize/diff/generate_ddl 直結、adapter 不要）。
- `error` フィールドを期待失敗アサーションとして評価する。
- `enable_drop` を `DiffConfig` に渡す（`None` は `false` 解決）。

**REFACTOR**
- expected up/down assertion helper を抽出。

**DoD**
- `cargo nextest run -p stateql-testkit --test offline_runner_test` PASS。
- `offline_runner_test` が `crates/core/src/orchestrator.rs` への依存なしで実行できる。
- `offline_runner_test` で `error` 期待値判定と `enable_drop` 反映が検証される。

**Commit**
```bash
git add crates/testkit/src/yaml_runner.rs crates/testkit/tests/offline_runner_test.rs crates/testkit/tests/support/offline_fake_dialect.rs
git commit -m "feat(testkit): add offline yaml runner"
```

### Task 40: Add Online Runner

**達成する仕様**: `R16`
**目的**: real adapter を使う往復実行（apply -> verify idempotency -> reverse -> verify）を実装する。

**Files:**
- Modify: `crates/testkit/src/yaml_runner.rs`
- Create: `crates/testkit/tests/online_runner_test.rs`

**RED**
- fake adapter で online runner call path が通るテストを追加。
- current/desired/reverse の各フェーズで idempotency 検証が欠落すると失敗するテストを追加。
- PostgreSQL/MySQL/MSSQL の container-backed adapter で 8-step flow が崩れる失敗テスト（ignore付き）を追加。
- `error` 指定ケースで期待失敗判定が効かない失敗テストを追加。
- `enable_drop` 指定が online flow の diff へ反映されない失敗テストを追加。

**GREEN**
- online runner 実装（DESIGN.md §6.2 の 8-step flow に準拠）。
- unit 経路（fake adapter）と integration 経路（testcontainers）を同一 runner API で検証できるようにする。
- `error` を online 経路でも期待失敗アサーションとして適用する。
- `enable_drop` を online 経路の `DiffConfig` へ反映する（`None` は `false` 解決）。

**REFACTOR**
- version gate (`min_version`/`max_version`) 判定関数抽出。

**DoD**
- `cargo nextest run -p stateql-testkit --test online_runner_test` PASS。
- container runtime あり環境で `cargo nextest run -p stateql-testkit --test online_runner_test --run-ignored ignored-only` PASS。
- `online_runner_test` で `error` 期待値判定と `enable_drop` 反映が検証される。

**Commit**
```bash
git add crates/testkit/src/yaml_runner.rs crates/testkit/tests/online_runner_test.rs
git commit -m "feat(testkit): implement online yaml runner flow"
```

### Task 41: Add Safety Regressions S1-S5

**達成する仕様**: `R17`
**目的**: Task 12/15/25/26/31 で先行導入した Safety 要件を cross-module 回帰として再固定する。

**Files:**
- Create: `crates/core/tests/safety_s1_s5_test.rs`

**RED**
- S1, S2, S3, S4, S5 の回帰テストを追加（実装タスク近傍テストの再検証セット）。

**GREEN**
- 既存実装の不足を埋める修正を行う。

**REFACTOR**
- shared fixture を関数化。

**DoD**
- `cargo nextest run -p stateql-core --test safety_s1_s5_test` PASS。

**Commit**
```bash
git add crates/core/tests/safety_s1_s5_test.rs
git commit -m "test(core): add safety regressions s1 to s5"
```

### Task 42: Add Safety Regressions S6-S11

**達成する仕様**: `R17`
**目的**: Task 5/18/19/21/23/35 で導入した Safety 要件を統合回帰として固定化する。

**Files:**
- Create: `crates/core/tests/safety_s6_s11_test.rs`
- Create: `crates/dialect-sqlite/tests/safety_s10_sqlite_atomicity_test.rs`

**RED**
- S6..S11 テスト追加（実装タスク近傍テストの再検証セット）。

**GREEN**
- 必要な実装修正を行う。

**REFACTOR**
- sqlite atomicity fixture を reusable にする。

**DoD**
- `cargo nextest run -p stateql-core --test safety_s6_s11_test` PASS。
- `cargo nextest run -p stateql-dialect-sqlite --test safety_s10_sqlite_atomicity_test` PASS。

**Commit**
```bash
git add crates/core/tests/safety_s6_s11_test.rs crates/dialect-sqlite/tests/safety_s10_sqlite_atomicity_test.rs
git commit -m "test(safety): add regressions s6 to s11"
```

### Task 43: Port PostgreSQL YAML Seed Cases

**達成する仕様**: `R16`, `R14`
**目的**: YAML再利用の最小成功ルートを確保する。

**Files:**
- Create: `tests/postgres/idempotency/0001-basic-create.yml`
- Create: `tests/postgres/idempotency/0002-add-index.yml`
- Create: `crates/dialect-postgres/tests/yaml_seed_test.rs`

**RED**
- seed case が offline runner で失敗する状態を作る。

**GREEN**
- case とテストを整備し PASS させる。

**REFACTOR**
- loader path helper を testkit 側へ集約。

**DoD**
- `cargo nextest run -p stateql-dialect-postgres --test yaml_seed_test` PASS。

**Commit**
```bash
git add tests/postgres/idempotency/0001-basic-create.yml tests/postgres/idempotency/0002-add-index.yml crates/dialect-postgres/tests/yaml_seed_test.rs
git commit -m "test(postgres): port initial yaml seed cases"
```

### YAML Migration Completion Criteria (`R16` Done Gate)

**判断方針（count / coverage / feature group の妥当性）**
- 件数のみ: 容易なケース偏重で達成できるため不十分。
- カバレッジ率のみ: 分母定義が曖昧だと品質指標として機能しないため不十分。
- feature group の有無のみ: 幅は担保できるが深さが担保できないため不十分。
- よって `件数 + カバレッジ率 + feature group + skipped理由管理` のハイブリッド基準を採用する。

**測定ソース**
- 分母は `tests/migration/idempotency-manifest.yml` と `tests/migration/assertion-manifest.yml` の `v1 scope` を唯一の基準とする。
- 進捗・判定結果は `docs/2026-02-21-yaml-migration-status.md` に記録する。

**完了判定（すべて必須）**
1. 管理ゲート:
   - `v1 scope` の全ケースが `ported` または `skipped(reason)` のいずれかに分類され、未分類が 0 件。
   - `skipped` は必ず理由と追跡先（issue/ADR/タスクID）を持つ。
2. 件数ゲート:
   - idempotency: 各 dialect で `ported >= 25`。
   - assertion: 各 dialect の各 feature group（tables/indexes/constraints/views）で `ported >= 5`。
3. カバレッジ率ゲート:
   - idempotency: 各 dialect ごとに `ported / (ported + skipped) >= 70%`。
   - assertion: 各 dialect・各 feature group ごとに `ported / (ported + skipped) >= 70%`。
   - 全体（idempotency + assertion 合算）で `ported / (ported + skipped) >= 75%`。
4. 品質ゲート:
   - matrix/manifest テストが PASS。
   - YAML corpus から `legacy_ignore_quotes` が除去済み。

**v1 での `R16` 完了定義**
- Task `43b` `43c` `43d` の DoD をすべて満たし、上記 4 ゲートが同時に PASS した時点で `R16` を完了扱いとする。

### Task 43b: Port YAML Idempotency Cases for All Dialects

**達成する仕様**: `R16`
**目的**: ADR-0005 の順序に従い、dialect ごとの idempotency-only ケースを段階移植する。

**Files:**
- Modify: `tests/postgres/idempotency/*.yml`
- Create: `tests/sqlite/idempotency/*.yml`
- Create: `tests/mysql/idempotency/*.yml`
- Create: `tests/mssql/idempotency/*.yml`
- Create: `tests/migration/idempotency-manifest.yml`
- Create: `crates/testkit/tests/yaml_idempotency_matrix_test.rs`
- Create: `crates/testkit/tests/yaml_migration_manifest_test.rs`
- Create: `docs/2026-02-21-yaml-migration-status.md`

**RED**
- 各 dialect で `ported < 25` または coverage rate `< 70%` になる失敗状態を再現。
- `reference/sqldef` 由来ケースの移植状況（ported/skipped）が追跡されず、欠落を検知できない失敗テストを追加。

**GREEN**
- idempotency-only ケースを移植し、dialect matrix テストを通す。
- `tests/migration/idempotency-manifest.yml` を導入し、対象ケースごとに `ported`/`skipped(reason)` を管理する。
- `yaml_migration_manifest_test` で件数/coverage gate（`ported >= 25`, `coverage >= 70%`）を機械検証する。
- `docs/2026-02-21-yaml-migration-status.md` に dialect 別 `ported/skipped/coverage%` と未達項目を記録する。

**REFACTOR**
- testcase 読み込みの glob/fixture ヘルパを `testkit` に集約。
- migration manifest の読み込み/検証ヘルパを `testkit` 側に集約する。

**DoD**
- `cargo nextest run -p stateql-testkit --test yaml_idempotency_matrix_test` PASS。
- `cargo nextest run -p stateql-testkit --test yaml_migration_manifest_test` PASS。
- 各 dialect で idempotency `ported >= 25`。
- 各 dialect で idempotency coverage rate `>= 70%`。
- `tests/{postgres,sqlite,mysql,mssql}/idempotency/` が空でない。
- `idempotency-manifest.yml` の `skipped` エントリがすべて `reason` + 追跡先を持つ。
- `docs/2026-02-21-yaml-migration-status.md` に dialect 別 `ported/skipped/coverage%` と残件理由が記載される。

**Commit**
```bash
git add tests/postgres/idempotency tests/sqlite/idempotency tests/mysql/idempotency tests/mssql/idempotency tests/migration/idempotency-manifest.yml crates/testkit/tests/yaml_idempotency_matrix_test.rs crates/testkit/tests/yaml_migration_manifest_test.rs docs/2026-02-21-yaml-migration-status.md
git commit -m "test(yaml): port idempotency cases for all dialects"
```

### Task 43c: Port YAML Assertion Cases by Feature Group

**達成する仕様**: `R16`
**目的**: ADR-0005 推奨順（tables -> indexes -> constraints -> views）で `up/down` assertion ケースを移植する。

**Files:**
- Create: `tests/{postgres,sqlite,mysql,mssql}/assertions/tables/*.yml`
- Create: `tests/{postgres,sqlite,mysql,mssql}/assertions/indexes/*.yml`
- Create: `tests/{postgres,sqlite,mysql,mssql}/assertions/constraints/*.yml`
- Create: `tests/{postgres,sqlite,mysql,mssql}/assertions/views/*.yml`
- Create: `tests/migration/assertion-manifest.yml`
- Create: `crates/testkit/tests/yaml_assertion_matrix_test.rs`
- Create: `crates/testkit/tests/yaml_assertion_manifest_test.rs`
- Modify: `docs/2026-02-21-yaml-migration-status.md`

**RED**
- feature group ごとに `up/down` 不一致を検出する失敗テストを追加。
- assertion 対象ケースの欠落が manifest レベルで検知されない失敗テストを追加。

**GREEN**
- 上記順で assertion ケースを移植し、group 単位の matrix テストを PASS させる。
- `tests/migration/assertion-manifest.yml` で feature group ごとの対象ケースを明示し、未移植は理由付きで管理する。
- `yaml_assertion_manifest_test` で group 単位の件数/coverage gate（`ported >= 5`, `coverage >= 70%`）を機械検証する。
- `docs/2026-02-21-yaml-migration-status.md` を更新し、assertion 移植の進捗を group 別 `ported/skipped/coverage%` で記録する。

**REFACTOR**
- expected SQL 正規化（改行/末尾セミコロン）ヘルパを共有化。
- manifest と matrix の重複チェック処理を helper 化する。

**DoD**
- `cargo nextest run -p stateql-testkit --test yaml_assertion_matrix_test` PASS。
- `cargo nextest run -p stateql-testkit --test yaml_assertion_manifest_test` PASS。
- 4 dialect すべてで tables/indexes/constraints/views の assertion ケースが存在する。
- 各 dialect・各 feature group で assertion `ported >= 5`。
- 各 dialect・各 feature group で assertion coverage rate `>= 70%`。
- `assertion-manifest.yml` の `skipped` エントリがすべて `reason` + 追跡先を持つ。
- `docs/2026-02-21-yaml-migration-status.md` で group 別 `ported/skipped/coverage%` が更新される。

**Commit**
```bash
git add tests/postgres/assertions tests/sqlite/assertions tests/mysql/assertions tests/mssql/assertions tests/migration/assertion-manifest.yml crates/testkit/tests/yaml_assertion_matrix_test.rs crates/testkit/tests/yaml_assertion_manifest_test.rs docs/2026-02-21-yaml-migration-status.md
git commit -m "test(yaml): port assertion cases by feature group"
```

### Task 43d: Rewrite `legacy_ignore_quotes` Dependent Cases

**達成する仕様**: `R16`
**目的**: `legacy_ignore_quotes` 前提の既存ケースを quote-aware mode 前提へ書き換える。

**Files:**
- Modify: `tests/**/**/*.yml` (`legacy_ignore_quotes` を含むケース)
- Create: `crates/testkit/tests/yaml_quote_aware_regression_test.rs`

**RED**
- `legacy_ignore_quotes` を使うケースを検出し、現行期待値との差異が発生する失敗テストを追加。

**GREEN**
- 該当ケースを書き換え、`legacy_ignore_quotes` 依存を除去。

**REFACTOR**
- quote-aware 比較の共通ヘルパを `testkit` 側へ配置。

**DoD**
- `cargo nextest run -p stateql-testkit --test yaml_quote_aware_regression_test` PASS。
- YAML corpus から `legacy_ignore_quotes` フィールドが消える。
- `docs/2026-02-21-yaml-migration-status.md` に全体 coverage（idempotency + assertion 合算）が記載され、`>= 75%` を満たす。
- `R16` completion gate（本節の「YAML Migration Completion Criteria」）で品質ゲート条件を満たす。

**Commit**
```bash
git add tests crates/testkit/tests/yaml_quote_aware_regression_test.rs
git commit -m "test(yaml): rewrite legacy ignore quotes dependent cases"
```

### Task 43e: Add Cargo Feature Gates for Dialect Inclusion

**達成する仕様**: `R02`, `R18`
**目的**: DESIGN.md §3.2 の feature 設計を workspace/CLI に反映し、dialect 別のビルド構成を固定する。

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/cli/Cargo.toml`
- Modify: `crates/cli/src/main.rs`
- Create: `crates/cli/tests/feature_gate_test.rs`

**RED**
- `--no-default-features --features mssql` 等のビルド構成で未使用 dialect 参照により失敗するケースを再現する。

**GREEN**
- `default = [\"mysql\", \"postgres\", \"sqlite\"]` と dialect feature mapping を実装。
- CLI 側の dialect subcommand 登録を feature-gated にする。

**REFACTOR**
- dialect registration table を `cfg(feature = \"...\")` 単位で整理。

**DoD**
- `cargo nextest run -p stateql-cli --test feature_gate_test` PASS。
- `cargo check -p stateql-cli --no-default-features --features mssql` PASS。

**Commit**
```bash
git add Cargo.toml crates/cli/Cargo.toml crates/cli/src/main.rs crates/cli/tests/feature_gate_test.rs
git commit -m "build(cli): add cargo feature gates for dialect inclusion"
```

### Task 44: Implement CLI Single-Binary Subcommands

**達成する仕様**: `R18`
**目的**: `stateql <dialect>` 形状と mode flag 競合制約を実装する。

**Files:**
- Modify: `crates/cli/src/main.rs`
- Modify: `crates/cli/Cargo.toml`
- Create: `crates/cli/tests/cli_shape_test.rs`
- Create: `crates/cli/tests/cli_connection_flags_test.rs`

**RED**
- `--apply` と `--export` 同時指定が失敗するテストを追加。
- `--apply` / `--export` 未指定かつ stdin または `--file` 入力ありで `--dry-run` 扱いになる失敗テストを追加。
- `--enable-drop` が mode 競合判定や dry-run default と矛盾なく解釈される失敗テストを追加。
- dialect 別 connection flags（`--host/--port/--user/--password/<database>` + MySQL `--socket` + PostgreSQL `--sslmode`）の受理失敗テストを追加。

**GREEN**
- `clap` subcommands + common flags を実装し、Task 43e の feature gate 下で有効な dialect のみ公開する。
- dialect ごとの connection flags を `ConnectionConfig` へ正規化して orchestrator へ渡す。

**REFACTOR**
- dialectごとの args struct を必要最小で分離。

**DoD**
- `cargo nextest run -p stateql-cli --test cli_shape_test` PASS。
- `cli_shape_test` で dry-run default（safe-by-default）が明示的に検証される。
- `cargo nextest run -p stateql-cli --test cli_connection_flags_test` PASS。

**Commit**
```bash
git add crates/cli/src/main.rs crates/cli/Cargo.toml crates/cli/tests/cli_shape_test.rs crates/cli/tests/cli_connection_flags_test.rs
git commit -m "feat(cli): implement single-binary dialect subcommands"
```

### Task 45: Stabilize Dialect Contract Documentation

**達成する仕様**: `R19`
**目的**: fail-fast / unsupported diffop 契約を実装者向けに固定する。

**Files:**
- Modify: `crates/core/src/dialect.rs`
- Modify: `crates/core/src/lib.rs` (`#![deny(rustdoc::broken_intra_doc_links)]` 追加)
- Create: `crates/core/examples/dialect_template.rs`
- Create: `docs/2026-02-21-dialect-trait-stabilization.md`

**RED**
- `crates/core/src/dialect.rs` に追加する rustdoc の example/doctest が最初は失敗する状態を作る。

**GREEN**
- doc comment と template を更新し、example/doctest が通るようにする。

**REFACTOR**
- docs wording と intra-doc links を ADR-0013/0002 表現に一致させる。

**DoD**
- `cargo test -p stateql-core --doc` PASS。
- `cargo doc -p stateql-core --no-deps` が warning なしで完了。

**Commit**
```bash
git add crates/core/src/dialect.rs crates/core/src/lib.rs crates/core/examples/dialect_template.rs docs/2026-02-21-dialect-trait-stabilization.md
git commit -m "docs(core): stabilize dialect implementation contract"
```

### Task 45b: Enforce Error Layering Boundaries (`thiserror` / `anyhow` / `miette`)

**達成する仕様**: `R01`
**目的**: DESIGN.md §3.10 の層分離（core/dialect は typed errors、CLI で `anyhow`/`miette`）を実装とテストで固定する。

**Files:**
- Modify: `crates/core/Cargo.toml`
- Modify: `crates/cli/Cargo.toml`
- Create: `crates/core/tests/error_layering_boundary_test.rs`
- Create: `crates/cli/tests/error_presentation_test.rs`
- Create: `docs/2026-02-21-error-layering.md`

**RED**
- core 公開 API が `anyhow::Error` を返してしまう失敗ケースを検知するテストを追加。
- CLI で typed error の分類を落として単一文字列化してしまう失敗ケースを追加。

**GREEN**
- core/dialect 境界は `thiserror` ベース typed error のみを返すよう統一。
- CLI 境界でのみ `anyhow::Context` / `miette` 表示を適用し、分類情報は保持する。

**REFACTOR**
- エラー変換（typed -> presentation）を単一 module に集約する。

**DoD**
- `cargo nextest run -p stateql-core --test error_layering_boundary_test` PASS。
- `cargo nextest run -p stateql-cli --test error_presentation_test` PASS。
- `cargo tree -p stateql-core | rg 'anyhow|miette'` が 0 hit。

**Commit**
```bash
git add crates/core/Cargo.toml crates/cli/Cargo.toml crates/core/tests/error_layering_boundary_test.rs crates/cli/tests/error_presentation_test.rs docs/2026-02-21-error-layering.md
git commit -m "test(core,cli): enforce typed error layering boundaries"
```

### Task 45c: Prepare `core` and `testkit` for Publish

**達成する仕様**: `R20`
**目的**: DESIGN.md §5.6 の publish 要求を「実 publish ではなく dry-run 成功」で検証可能な状態にする。

**Files:**
- Modify: `crates/core/Cargo.toml`
- Modify: `crates/testkit/Cargo.toml`
- Create: `docs/2026-02-21-publish-readiness.md`

**RED**
- `cargo publish --dry-run -p stateql-core` / `-p stateql-testkit` が metadata 不足で失敗する状態を再現する。

**GREEN**
- `license` / `repository` / `description` / `readme` / `keywords` / `categories` など publish 必須 metadata を整備。
- package include/exclude を見直し、不要ファイルの同梱を防止。

**REFACTOR**
- 共通 metadata 方針を workspace ルートへ寄せ、crate との差分のみ個別指定する。

**DoD**
- `cargo publish --dry-run -p stateql-core` PASS。
- `cargo publish --dry-run -p stateql-testkit` PASS。
- publish 手順と preflight check が `docs/2026-02-21-publish-readiness.md` に記載される。

**Commit**
```bash
git add crates/core/Cargo.toml crates/testkit/Cargo.toml docs/2026-02-21-publish-readiness.md
git commit -m "build(release): add publish-readiness for core and testkit"
```

## Execution Rule

- この plan は実装開始前レビュー前提。
- `HARD-GATE`: ユーザー明示承認前は実装しない。
- 実装着手時は `execute-plan` を使用し、Task Dependency Graph のトポロジカル順で進める（単純な番号順ではない）。
- `Task 33` は集約見出しであり実行対象ではない。実行対象は `Task 33a/33b/33c`。
