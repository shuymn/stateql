use crate::{
    ConnectionConfig, DatabaseAdapter, DiffOp, Ident, Result, SchemaObject, Statement,
    diff::{DEFAULT_EQUIVALENCE_POLICY, EquivalencePolicy},
};

/// Dialect contract for parse, normalize, diff planning, and connection lifecycle.
///
/// Contract requirements:
/// - Follow fail-fast typed error handling (ADR-0013). Unsupported parse/generate/execute
///   scenarios are errors and must never be silently skipped.
/// - Keep diff planning and SQL rendering split (ADR-0002): core produces [`DiffOp`] batches
///   and dialects convert the full batch into [`Statement`] values.
/// - If a [`DiffOp`] cannot be rendered for this dialect, return
///   [`crate::GenerateError::UnsupportedDiffOp`] immediately.
///
/// A complete implementation template also exists in
/// `crates/core/examples/dialect_template.rs`.
///
/// ```rust
/// use std::collections::BTreeMap;
///
/// use stateql_core::{
///     ConnectionConfig, DatabaseAdapter, Dialect, DiffOp, GenerateError, Ident, Result,
///     SchemaObject, Statement, Table, Transaction, Version,
/// };
///
/// #[derive(Debug, Default)]
/// struct ExampleDialect;
///
/// #[derive(Debug, Default)]
/// struct ExampleAdapter;
///
/// impl DatabaseAdapter for ExampleAdapter {
///     fn export_schema(&self) -> Result<String> {
///         Ok(String::new())
///     }
///
///     fn execute(&self, _sql: &str) -> Result<()> {
///         Ok(())
///     }
///
///     fn begin(&mut self) -> Result<Transaction<'_>> {
///         Ok(Transaction::new(self))
///     }
///
///     fn schema_search_path(&self) -> Vec<String> {
///         vec!["public".to_string()]
///     }
///
///     fn server_version(&self) -> Result<Version> {
///         Ok(Version {
///             major: 13,
///             minor: 0,
///             patch: 0,
///         })
///     }
/// }
///
/// impl Dialect for ExampleDialect {
///     fn name(&self) -> &str {
///         "example"
///     }
///
///     fn parse(&self, _sql: &str) -> Result<Vec<SchemaObject>> {
///         Ok(vec![SchemaObject::Table(Table::named("users"))])
///     }
///
///     fn generate_ddl(&self, _ops: &[DiffOp]) -> Result<Vec<Statement>> {
///         Ok(vec![Statement::Sql {
///             sql: "CREATE TABLE users(id int);".to_string(),
///             transactional: true,
///             context: None,
///         }])
///     }
///
///     fn to_sql(&self, _obj: &SchemaObject) -> Result<String> {
///         Err(GenerateError::UnsupportedDiffOp {
///             diff_op: "ToSql".to_string(),
///             target: "users".to_string(),
///             dialect: self.name().to_string(),
///         }
///         .into())
///     }
///
///     fn normalize(&self, _obj: &mut SchemaObject) {}
///
///     fn quote_ident(&self, ident: &Ident) -> String {
///         format!("\"{}\"", ident.value)
///     }
///
///     fn connect(&self, _config: &ConnectionConfig) -> Result<Box<dyn DatabaseAdapter>> {
///         Ok(Box::<ExampleAdapter>::default())
///     }
/// }
///
/// let dialect = ExampleDialect;
/// let connection = ConnectionConfig {
///     host: None,
///     port: None,
///     user: None,
///     password: None,
///     database: "db".to_string(),
///     socket: None,
///     extra: BTreeMap::new(),
/// };
/// let adapter = dialect
///     .connect(&connection)
///     .expect("connect should succeed");
/// assert_eq!(adapter.export_schema().expect("export should succeed"), "");
/// assert_eq!(dialect.name(), "example");
///
/// let err = dialect
///     .to_sql(&SchemaObject::Table(Table::named("users")))
///     .expect_err("unsupported to_sql path should fail fast");
/// assert!(matches!(
///     err,
///     stateql_core::Error::Generate(GenerateError::UnsupportedDiffOp { .. })
/// ));
/// ```
pub trait Dialect: Send + Sync {
    fn name(&self) -> &str;
    fn parse(&self, sql: &str) -> Result<Vec<SchemaObject>>;
    fn generate_ddl(&self, ops: &[DiffOp]) -> Result<Vec<Statement>>;
    fn to_sql(&self, obj: &SchemaObject) -> Result<String>;
    fn normalize(&self, obj: &mut SchemaObject);
    fn equivalence_policy(&self) -> &'static dyn EquivalencePolicy {
        &DEFAULT_EQUIVALENCE_POLICY
    }
    fn quote_ident(&self, ident: &Ident) -> String;
    fn batch_separator(&self) -> &str {
        ""
    }
    fn connect(&self, config: &ConnectionConfig) -> Result<Box<dyn DatabaseAdapter>>;
}
