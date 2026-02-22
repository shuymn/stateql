use stateql_core::DatabaseAdapter;

#[path = "support/fake_adapter.rs"]
mod fake_adapter;

use fake_adapter::{BEGIN_SQL, COMMIT_SQL, FakeAdapter, ROLLBACK_SQL};

#[test]
fn s6_drop_without_commit_triggers_rollback() {
    let mut adapter = FakeAdapter::default();

    {
        let mut tx = adapter.begin().expect("begin transaction");
        tx.execute("CREATE TABLE users (id INT);")
            .expect("execute inside transaction");
    }

    assert_eq!(
        adapter.executed_sql(),
        vec![
            BEGIN_SQL.to_string(),
            "CREATE TABLE users (id INT);".to_string(),
            ROLLBACK_SQL.to_string(),
        ],
    );
    assert_eq!(adapter.begin_count(), 1);
    assert_eq!(adapter.commit_count(), 0);
    assert_eq!(adapter.rollback_count(), 1);
}

#[test]
fn committed_transaction_does_not_rollback_on_drop() {
    let mut adapter = FakeAdapter::default();

    {
        let mut tx = adapter.begin().expect("begin transaction");
        tx.execute("ALTER TABLE users ADD COLUMN name TEXT;")
            .expect("execute inside transaction");
        tx.commit().expect("commit transaction");
    }

    assert_eq!(
        adapter.executed_sql(),
        vec![
            BEGIN_SQL.to_string(),
            "ALTER TABLE users ADD COLUMN name TEXT;".to_string(),
            COMMIT_SQL.to_string(),
        ],
    );
    assert_eq!(adapter.begin_count(), 1);
    assert_eq!(adapter.commit_count(), 1);
    assert_eq!(adapter.rollback_count(), 0);
}
