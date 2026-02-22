#[allow(dead_code)]
#[path = "../../core/tests/support/diffop_fixtures.rs"]
mod diffop_fixtures;

use diffop_fixtures::all_diffop_variants;
use stateql_core::{Dialect, DiffOp, Ident, QualifiedName, Statement, View};
use stateql_dialect_postgres::PostgresDialect;

#[test]
fn supported_diffop_families_generate_sql_statements() {
    let dialect = PostgresDialect;
    let supported_ops = all_diffop_variants()
        .into_iter()
        .filter(is_supported_diffop)
        .collect::<Vec<_>>();

    let statements = dialect
        .generate_ddl(&supported_ops)
        .expect("supported PostgreSQL diff ops should generate SQL");

    assert!(
        !supported_ops.is_empty(),
        "fixture should include supported ops"
    );
    assert!(
        !statements.is_empty(),
        "supported ops should emit SQL statements"
    );
    assert!(
        statements
            .iter()
            .all(|statement| matches!(statement, Statement::Sql { .. })),
        "PostgreSQL generator should emit SQL statements only"
    );
}

#[test]
fn drop_create_view_pair_uses_create_or_replace_when_compatible() {
    let dialect = PostgresDialect;
    let view_name = qualified(Some("public"), "active_users");
    let view = View::new(
        view_name.clone(),
        "SELECT id, email FROM users WHERE active",
    );

    let statements = dialect
        .generate_ddl(&[DiffOp::DropView(view_name), DiffOp::CreateView(view)])
        .expect("compatible view replacement should generate SQL");

    assert_eq!(statements.len(), 1);
    let Statement::Sql { sql, .. } = &statements[0] else {
        panic!("expected SQL statement");
    };
    assert!(sql.starts_with("CREATE OR REPLACE VIEW"));
}

#[test]
fn drop_create_view_pair_keeps_drop_create_when_not_compatible() {
    let dialect = PostgresDialect;
    let view_name = qualified(Some("public"), "active_users");
    let mut view = View::new(
        view_name.clone(),
        "SELECT id, email FROM users WHERE active",
    );
    view.columns = vec![ident("id"), ident("email")];

    let statements = dialect
        .generate_ddl(&[DiffOp::DropView(view_name), DiffOp::CreateView(view)])
        .expect("incompatible view replacement should still generate SQL");

    assert_eq!(statements.len(), 2);
    let Statement::Sql { sql: first, .. } = &statements[0] else {
        panic!("expected SQL statement");
    };
    let Statement::Sql { sql: second, .. } = &statements[1] else {
        panic!("expected SQL statement");
    };

    assert!(first.starts_with("DROP VIEW"));
    assert!(second.starts_with("CREATE VIEW"));
    assert!(!second.starts_with("CREATE OR REPLACE VIEW"));
}

fn is_supported_diffop(op: &DiffOp) -> bool {
    match op {
        DiffOp::CreateTable(_)
        | DiffOp::DropTable(_)
        | DiffOp::RenameTable { .. }
        | DiffOp::AddColumn { .. }
        | DiffOp::DropColumn { .. }
        | DiffOp::AlterColumn { .. }
        | DiffOp::RenameColumn { .. }
        | DiffOp::AddIndex(_)
        | DiffOp::DropIndex { .. }
        | DiffOp::RenameIndex { .. }
        | DiffOp::AddForeignKey { .. }
        | DiffOp::DropForeignKey { .. }
        | DiffOp::AddCheck { .. }
        | DiffOp::DropCheck { .. }
        | DiffOp::AddExclusion { .. }
        | DiffOp::DropExclusion { .. }
        | DiffOp::SetPrimaryKey { .. }
        | DiffOp::DropPrimaryKey { .. }
        | DiffOp::AddPartition { .. }
        | DiffOp::DropPartition { .. }
        | DiffOp::CreateView(_)
        | DiffOp::DropView(_)
        | DiffOp::CreateMaterializedView(_)
        | DiffOp::DropMaterializedView(_)
        | DiffOp::CreateSequence(_)
        | DiffOp::DropSequence(_)
        | DiffOp::AlterSequence { .. }
        | DiffOp::CreateTrigger(_)
        | DiffOp::DropTrigger { .. }
        | DiffOp::CreateFunction(_)
        | DiffOp::DropFunction(_)
        | DiffOp::CreateType(_)
        | DiffOp::DropType(_)
        | DiffOp::AlterType { .. }
        | DiffOp::CreateDomain(_)
        | DiffOp::DropDomain(_)
        | DiffOp::AlterDomain { .. }
        | DiffOp::CreateExtension(_)
        | DiffOp::DropExtension(_)
        | DiffOp::CreateSchema(_)
        | DiffOp::DropSchema(_)
        | DiffOp::SetComment(_)
        | DiffOp::DropComment { .. }
        | DiffOp::Grant(_)
        | DiffOp::Revoke(_)
        | DiffOp::CreatePolicy(_)
        | DiffOp::DropPolicy { .. }
        | DiffOp::AlterTableOptions { .. } => true,
    }
}

fn ident(value: &str) -> Ident {
    Ident::unquoted(value)
}

fn qualified(schema: Option<&str>, name: &str) -> QualifiedName {
    QualifiedName {
        schema: schema.map(Ident::unquoted),
        name: Ident::unquoted(name),
    }
}
