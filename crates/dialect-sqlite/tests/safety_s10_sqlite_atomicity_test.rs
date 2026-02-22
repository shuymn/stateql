use stateql_core::{Dialect, Executor};
use stateql_dialect_sqlite::SqliteDialect;

#[path = "support/sqlite_atomicity_fixture.rs"]
mod sqlite_atomicity_fixture;

use sqlite_atomicity_fixture::{
    assert_copy_step_failure, assert_rollback_left_original_table, prepare_users_with_null_age,
    set_not_null_age_op,
};

#[test]
fn s10_sqlite_table_recreation_rolls_back_atomically_on_copy_failure() {
    let dialect = SqliteDialect;
    let mut adapter = prepare_users_with_null_age(&dialect);

    let statements = dialect
        .generate_ddl(&[set_not_null_age_op()])
        .expect("rebuild plan should generate");

    let mut executor = Executor::new(adapter.as_mut());
    let error = executor
        .execute_plan(&statements)
        .expect_err("copy step should fail for NULL -> NOT NULL migration");

    assert_copy_step_failure(error);
    assert_rollback_left_original_table(adapter.as_ref());
}
