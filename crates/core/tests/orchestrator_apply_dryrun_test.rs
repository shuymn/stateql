use stateql_core::{
    Mode, Orchestrator, OrchestratorOptions, OrchestratorOutput, SchemaObject, Table,
};

#[path = "support/fake_dialect.rs"]
mod fake_dialect;

use fake_dialect::{FakeDialect, test_connection_config};

const CURRENT_SQL: &str = "CURRENT_SQL";
const DESIRED_SQL: &str = "DESIRED_SQL";

fn configure_create_table_fixture(dialect: &FakeDialect) {
    dialect.set_export_schema_sql(CURRENT_SQL);
    dialect.set_parse_result(CURRENT_SQL, vec![]);
    dialect.set_parse_result(
        DESIRED_SQL,
        vec![SchemaObject::Table(Table::named("users"))],
    );
}

#[test]
fn dry_run_returns_sql_and_does_not_execute() {
    let dialect = FakeDialect::default();
    configure_create_table_fixture(&dialect);

    let orchestrator = Orchestrator::new(&dialect);
    let output = orchestrator
        .run(
            &test_connection_config(),
            DESIRED_SQL,
            OrchestratorOptions {
                mode: Mode::DryRun,
                enable_drop: true,
            },
        )
        .expect("dry-run should succeed");

    match output {
        OrchestratorOutput::DryRunSql(sql) => {
            assert!(
                sql.contains("CREATE TABLE users;"),
                "dry-run output should include generated SQL",
            );
        }
        other => panic!("unexpected output: {other:?}"),
    }

    assert!(
        dialect.executed_sql().is_empty(),
        "dry-run must not execute statements",
    );
}

#[test]
fn apply_executes_generated_statements() {
    let dialect = FakeDialect::default();
    configure_create_table_fixture(&dialect);

    let orchestrator = Orchestrator::new(&dialect);
    let output = orchestrator
        .run(
            &test_connection_config(),
            DESIRED_SQL,
            OrchestratorOptions {
                mode: Mode::Apply,
                enable_drop: true,
            },
        )
        .expect("apply should succeed");

    assert_eq!(output, OrchestratorOutput::Applied);
    assert_eq!(
        dialect.executed_sql(),
        vec![
            "BEGIN".to_string(),
            "CREATE TABLE users;".to_string(),
            "COMMIT".to_string(),
        ],
    );
}
