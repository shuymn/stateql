use stateql_core::{
    ColumnChange, DataType, Dialect, DiffOp, Ident, Identity, PrimaryKey, QualifiedName, Statement,
};
use stateql_dialect_mysql::MysqlDialect;

#[test]
fn auto_increment_change_runs_after_primary_key_changes() {
    let dialect = MysqlDialect;
    let table = qualified_name("users");

    let ops = vec![
        DiffOp::AlterColumn {
            table: table.clone(),
            column: ident("id"),
            changes: vec![
                ColumnChange::SetType(DataType::BigInt),
                ColumnChange::SetNotNull(true),
                ColumnChange::SetDefault(None),
                ColumnChange::SetIdentity(Some(auto_increment_identity())),
                ColumnChange::SetGenerated(None),
                ColumnChange::SetCollation(None),
            ],
        },
        DiffOp::DropPrimaryKey {
            table: table.clone(),
        },
        DiffOp::SetPrimaryKey {
            table,
            pk: PrimaryKey {
                name: None,
                columns: vec![ident("id")],
            },
        },
    ];

    let statements = dialect
        .generate_ddl(&ops)
        .expect("mysql generator should support PK + AUTO_INCREMENT sequencing");

    let sql_statements = sql_only(&statements);
    let drop_pk_index = sql_statements
        .iter()
        .position(|sql| sql.contains("DROP PRIMARY KEY"))
        .expect("DROP PRIMARY KEY statement should exist");
    let add_pk_index = sql_statements
        .iter()
        .position(|sql| sql.contains("ADD PRIMARY KEY"))
        .expect("ADD PRIMARY KEY statement should exist");
    let auto_increment_index = sql_statements
        .iter()
        .position(|sql| sql.contains("AUTO_INCREMENT"))
        .expect("AUTO_INCREMENT change statement should exist");

    assert!(
        drop_pk_index < add_pk_index,
        "primary key drop must happen before primary key set: {sql_statements:?}"
    );
    assert!(
        add_pk_index < auto_increment_index,
        "AUTO_INCREMENT column rewrite must happen after PK change: {sql_statements:?}"
    );
}

fn sql_only(statements: &[Statement]) -> Vec<String> {
    statements
        .iter()
        .filter_map(|statement| match statement {
            Statement::Sql { sql, .. } => Some(sql.clone()),
            Statement::BatchBoundary => None,
        })
        .collect()
}

fn auto_increment_identity() -> Identity {
    Identity {
        always: true,
        start: Some(1),
        increment: Some(1),
        min_value: None,
        max_value: None,
        cache: None,
        cycle: false,
    }
}

fn qualified_name(name: &str) -> QualifiedName {
    QualifiedName {
        schema: None,
        name: ident(name),
    }
}

fn ident(value: &str) -> Ident {
    Ident::unquoted(value)
}
