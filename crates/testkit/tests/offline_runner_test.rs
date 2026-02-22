#[path = "support/offline_fake_dialect.rs"]
mod offline_fake_dialect;

use offline_fake_dialect::OfflineFakeDialect;
use stateql_core::DiffOp;
use stateql_testkit::{TestCase, TestResult, run_offline_test};

#[test]
fn offline_runner_validates_up_and_down_expectations() {
    let dialect = OfflineFakeDialect::default();
    let mut testcase = create_drop_case(Some(true));
    testcase.up = Some("CREATE TABLE unexpected;".to_string());

    let result = run_offline_test(&dialect, &testcase);
    assert!(
        matches!(result, TestResult::Failed(_)),
        "offline runner must fail when generated up SQL does not match expectation"
    );
}

#[test]
fn offline_runner_enforces_expected_error_contract() {
    let dialect = OfflineFakeDialect::default();
    let expected = OfflineFakeDialect::expected_error_message("parse current failed");

    let mut matching_error = create_drop_case(Some(true));
    matching_error.current = "ERROR: parse current failed".to_string();
    matching_error.error = Some(expected.clone());
    matching_error.up = None;
    matching_error.down = None;

    assert!(
        matches!(
            run_offline_test(&dialect, &matching_error),
            TestResult::Passed
        ),
        "matching expected error must pass"
    );

    let mut missing_error = create_drop_case(Some(true));
    missing_error.error = Some(expected);
    missing_error.up = None;
    missing_error.down = None;

    assert!(
        matches!(
            run_offline_test(&dialect, &missing_error),
            TestResult::Failed(_)
        ),
        "successful execution with `error` expectation must fail"
    );
}

#[test]
fn offline_runner_passes_enable_drop_to_diff_config() {
    assert_enable_drop_behavior(None, false);
    assert_enable_drop_behavior(Some(false), false);
    assert_enable_drop_behavior(Some(true), true);
}

fn assert_enable_drop_behavior(enable_drop: Option<bool>, expect_drop: bool) {
    let dialect = OfflineFakeDialect::default();
    let testcase = create_drop_case(enable_drop);

    let result = run_offline_test(&dialect, &testcase);
    assert!(
        matches!(result, TestResult::Passed),
        "offline runner should pass for valid fixture (enable_drop={enable_drop:?})"
    );

    let generated_batches = dialect.generated_batches();
    assert!(
        !generated_batches.is_empty(),
        "offline runner must call dialect.generate_ddl (enable_drop={enable_drop:?})"
    );

    let forward_has_drop = generated_batches[0]
        .iter()
        .any(|op| matches!(op, DiffOp::DropTable(_)));
    assert_eq!(
        forward_has_drop, expect_drop,
        "forward diff must reflect DiffConfig.enable_drop={enable_drop:?}"
    );
}

fn create_drop_case(enable_drop: Option<bool>) -> TestCase {
    let up = if enable_drop.unwrap_or(false) {
        "DROP TABLE users;"
    } else {
        ""
    };

    TestCase {
        current: "tables:users".to_string(),
        desired: String::new(),
        up: Some(up.to_string()),
        down: Some("CREATE TABLE users;".to_string()),
        enable_drop,
        ..TestCase::default()
    }
}
