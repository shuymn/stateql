use stateql_core::{Dialect, DiffError, Error, Ident, ParseError, SchemaObject};
use stateql_dialect_mysql::MysqlDialect;

#[test]
fn unsupported_statement_reports_statement_context() {
    let dialect = MysqlDialect;
    let sql = "CREATE TABLE users (id bigint);\nDROP TABLE users;";

    let error = dialect
        .parse(sql)
        .expect_err("unsupported statement should fail fast");

    match error {
        Error::Parse(ParseError::StatementConversion {
            statement_index,
            source_sql,
            source_location,
            ..
        }) => {
            assert_eq!(statement_index, 1);
            assert!(source_sql.contains("DROP TABLE users"));
            assert_eq!(
                source_location.as_ref().map(|location| location.line),
                Some(2)
            );
        }
        other => panic!("expected parse statement conversion error, got {other:?}"),
    }
}

#[test]
fn trailing_renamed_annotation_is_attached_to_table() {
    let dialect = MysqlDialect;
    let sql = "CREATE TABLE users (id bigint); -- @renamed from=legacy_users\n";

    let objects = dialect.parse(sql).expect("mysql parse pipeline");

    assert_eq!(objects.len(), 1);
    let SchemaObject::Table(table) = &objects[0] else {
        panic!("expected table object");
    };
    assert_eq!(table.name.name, Ident::unquoted("users"));
    assert_eq!(table.renamed_from, Some(Ident::unquoted("legacy_users")));
}

#[test]
fn orphan_renamed_annotation_fails_fast() {
    let dialect = MysqlDialect;
    let sql = "CREATE TABLE users (id bigint);\n-- @renamed from=legacy_users\n";

    let error = dialect
        .parse(sql)
        .expect_err("orphan annotation must not be silently ignored");

    match error {
        Error::Diff(DiffError::ObjectComparison { target, operation }) => {
            assert!(target.contains("annotation @renamed from=legacy_users on line 2"));
            assert_eq!(operation, "rename annotation mismatch");
        }
        other => panic!("expected orphan annotation mismatch, got {other:?}"),
    }
}
