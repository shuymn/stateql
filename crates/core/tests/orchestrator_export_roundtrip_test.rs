use stateql_core::{
    Mode, Orchestrator, OrchestratorOptions, OrchestratorOutput, SchemaObject, Table,
};

#[path = "support/fake_dialect.rs"]
mod fake_dialect;
#[path = "support/orchestrator_export_fixture.rs"]
mod orchestrator_export_fixture;

use fake_dialect::{FakeDialect, test_connection_config};
use orchestrator_export_fixture::{
    DESIRED_SQL_IGNORED, configure_export_fixture, expected_export_sql,
};

fn export_sql(output: OrchestratorOutput) -> String {
    match output {
        OrchestratorOutput::ExportSql(sql) => sql,
        other => panic!("unexpected output: {other:?}"),
    }
}

#[test]
fn export_is_idempotent_across_two_passes() {
    let dialect = FakeDialect::default();
    let expected_objects = configure_export_fixture(&dialect);

    let orchestrator = Orchestrator::new(&dialect);
    let first_export = export_sql(
        orchestrator
            .run(
                &test_connection_config(),
                DESIRED_SQL_IGNORED,
                OrchestratorOptions {
                    mode: Mode::Export,
                    enable_drop: false,
                },
            )
            .expect("first export should succeed"),
    );
    assert_eq!(first_export, expected_export_sql(&expected_objects));

    dialect.set_export_schema_sql(&first_export);
    dialect.set_parse_result(&first_export, expected_objects);

    let second_export = export_sql(
        orchestrator
            .run(
                &test_connection_config(),
                DESIRED_SQL_IGNORED,
                OrchestratorOptions {
                    mode: Mode::Export,
                    enable_drop: false,
                },
            )
            .expect("second export should succeed"),
    );

    assert_eq!(first_export, second_export);
    assert!(
        orchestrator
            .export_roundtrip_matches(&first_export)
            .expect("round-trip helper should succeed"),
        "export output should remain identical after parse -> normalize -> to_sql",
    );
}

#[test]
fn export_roundtrip_helper_detects_mismatch() {
    let dialect = FakeDialect::default();
    let _ = configure_export_fixture(&dialect);

    let orchestrator = Orchestrator::new(&dialect);
    let first_export = export_sql(
        orchestrator
            .run(
                &test_connection_config(),
                DESIRED_SQL_IGNORED,
                OrchestratorOptions {
                    mode: Mode::Export,
                    enable_drop: false,
                },
            )
            .expect("first export should succeed"),
    );

    dialect.set_parse_result(
        &first_export,
        vec![SchemaObject::Table(Table::named("users_only"))],
    );

    assert!(
        !orchestrator
            .export_roundtrip_matches(&first_export)
            .expect("round-trip helper should succeed"),
        "helper should report non-idempotent export text",
    );
}
