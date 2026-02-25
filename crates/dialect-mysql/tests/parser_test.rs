use stateql_core::{Dialect, DiffError, Error, Ident, ParseError, SchemaObject, ViewSecurity};
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

#[test]
fn create_sql_security_view_is_supported() {
    let dialect = MysqlDialect;
    let sql = "CREATE SQL SECURITY DEFINER VIEW `masked_entities_view` AS select `sample_schema`.`sample_entities`.`member_id` AS `member_id` from `sample_schema`.`sample_entities`;";

    let objects = dialect.parse(sql).expect("mysql parse pipeline");

    assert_eq!(objects.len(), 1);
    let SchemaObject::View(view) = &objects[0] else {
        panic!("expected view object");
    };
    assert_eq!(view.name.name, Ident::quoted("masked_entities_view"));
    assert_eq!(view.security, Some(ViewSecurity::Definer));
    assert!(view.query.contains("sample_schema"));
}

#[test]
fn trailing_renamed_annotation_is_attached_to_view() {
    let dialect = MysqlDialect;
    let sql =
        "CREATE VIEW users_view AS SELECT id FROM users; -- @renamed from=legacy_users_view\n";

    let objects = dialect.parse(sql).expect("mysql parse pipeline");

    assert_eq!(objects.len(), 1);
    let SchemaObject::View(view) = &objects[0] else {
        panic!("expected view object");
    };
    assert_eq!(
        view.renamed_from,
        Some(Ident::unquoted("legacy_users_view"))
    );
}
