use stateql_core::{Dialect, DiffConfig, DiffEngine};
use stateql_dialect_mysql::MysqlDialect;

#[test]
fn normalize_absorbs_lower_case_table_names_differences() {
    let dialect = MysqlDialect;
    let desired_sql = "CREATE TABLE `users` (id bigint);";
    let current_sql = "CREATE TABLE `Users` (id bigint);";

    let mut desired = dialect
        .parse(desired_sql)
        .expect("desired schema parse should succeed");
    let mut current = dialect
        .parse(current_sql)
        .expect("current schema parse should succeed");

    for object in &mut desired {
        dialect.normalize(object);
    }
    for object in &mut current {
        dialect.normalize(object);
    }

    let ops = DiffEngine::new()
        .diff(&desired, &current, &DiffConfig::default())
        .expect("diff should succeed after normalization");

    assert!(
        ops.is_empty(),
        "normalization should remove lower_case_table_names false diffs, got: {ops:?}"
    );
}
