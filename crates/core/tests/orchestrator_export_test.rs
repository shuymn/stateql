use stateql_core::{
    Mode, Orchestrator, OrchestratorOptions, OrchestratorOutput, SchemaObject, Table,
};

#[path = "support/fake_dialect.rs"]
mod fake_dialect;

use fake_dialect::{FakeDialect, test_connection_config};

const CURRENT_SQL: &str = "CURRENT_SQL";
const DESIRED_SQL_IGNORED: &str = "DESIRED_SQL_IGNORED";

#[test]
fn export_routes_current_schema_through_to_sql() {
    let dialect = FakeDialect::default();
    dialect.set_export_schema_sql(CURRENT_SQL);

    let expected_objects = vec![
        SchemaObject::Table(Table::named("users")),
        SchemaObject::Table(Table::named("orders")),
    ];
    dialect.set_parse_result(CURRENT_SQL, expected_objects.clone());

    let output = Orchestrator::new(&dialect)
        .run(
            &test_connection_config(),
            DESIRED_SQL_IGNORED,
            OrchestratorOptions {
                mode: Mode::Export,
                enable_drop: true,
            },
        )
        .expect("export should succeed");

    let exported_sql = match output {
        OrchestratorOutput::ExportSql(sql) => sql,
        other => panic!("unexpected output: {other:?}"),
    };

    assert_eq!(dialect.to_sql_calls(), expected_objects);
    assert!(
        dialect.generated_ops().is_empty(),
        "export must not run diff/generate paths",
    );
    assert!(
        dialect.executed_sql().is_empty(),
        "export must not execute SQL",
    );

    let expected_sql = expected_objects
        .iter()
        .map(|obj| format!("{obj:?}\n"))
        .collect::<String>();
    assert_eq!(exported_sql, expected_sql);
}
