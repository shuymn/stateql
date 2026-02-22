use stateql_core::{Executor, Statement};

#[path = "support/fake_adapter.rs"]
mod fake_adapter;

use fake_adapter::{BEGIN_SQL, COMMIT_SQL, FakeAdapter};

#[test]
fn execute_plan_commits_before_non_transactional_and_restarts_after() {
    let mut adapter = FakeAdapter::default();
    let statements = vec![
        Statement::Sql {
            sql: "CREATE TABLE users (id INT PRIMARY KEY);".to_string(),
            transactional: true,
            context: None,
        },
        Statement::Sql {
            sql: "ALTER TABLE users ADD COLUMN name TEXT;".to_string(),
            transactional: true,
            context: None,
        },
        Statement::Sql {
            sql: "CREATE INDEX CONCURRENTLY idx_users_name ON users(name);".to_string(),
            transactional: false,
            context: None,
        },
        Statement::Sql {
            sql: "ALTER TABLE users ADD COLUMN email TEXT;".to_string(),
            transactional: true,
            context: None,
        },
        Statement::Sql {
            sql: "ALTER TABLE users ADD COLUMN age INT;".to_string(),
            transactional: true,
            context: None,
        },
    ];

    let mut executor = Executor::new(&mut adapter);
    executor
        .execute_plan(&statements)
        .expect("non-transactional boundaries should flush and restart transactions");

    assert_eq!(adapter.begin_count(), 2);
    assert_eq!(adapter.commit_count(), 2);
    assert_eq!(
        adapter.executed_sql(),
        vec![
            BEGIN_SQL.to_string(),
            "CREATE TABLE users (id INT PRIMARY KEY);".to_string(),
            "ALTER TABLE users ADD COLUMN name TEXT;".to_string(),
            COMMIT_SQL.to_string(),
            "CREATE INDEX CONCURRENTLY idx_users_name ON users(name);".to_string(),
            BEGIN_SQL.to_string(),
            "ALTER TABLE users ADD COLUMN email TEXT;".to_string(),
            "ALTER TABLE users ADD COLUMN age INT;".to_string(),
            COMMIT_SQL.to_string(),
        ],
    );
}
