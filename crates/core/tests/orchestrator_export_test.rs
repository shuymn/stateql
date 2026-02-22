use stateql_core::{Mode, Orchestrator, OrchestratorOptions, OrchestratorOutput};

#[path = "support/fake_dialect.rs"]
mod fake_dialect;
#[path = "support/orchestrator_export_fixture.rs"]
mod orchestrator_export_fixture;

use fake_dialect::{FakeDialect, test_connection_config};
use orchestrator_export_fixture::{
    DESIRED_SQL_IGNORED, configure_export_fixture, expected_export_sql,
};

#[test]
fn export_routes_current_schema_through_to_sql() {
    let dialect = FakeDialect::default();
    let expected_objects = configure_export_fixture(&dialect);

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

    let expected_sql = expected_export_sql(&expected_objects);
    assert_eq!(exported_sql, expected_sql);
}
