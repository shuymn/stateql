use stateql_core::{DatabaseAdapter, Result, Transaction, Version};

#[path = "support/fake_adapter.rs"]
mod fake_adapter;

use fake_adapter::{BEGIN_SQL, COMMIT_SQL, FakeAdapter};

fn assert_database_adapter_sync_contract<T: DatabaseAdapter>() {
    let _: fn(&T) -> Result<String> = T::export_schema;
    let _: fn(&T, &str) -> Result<()> = T::execute;
    let _: for<'a> fn(&'a mut T) -> Result<Transaction<'a>> = T::begin;
    let _: fn(&T) -> Vec<String> = T::schema_search_path;
    let _: fn(&T) -> Result<Version> = T::server_version;
}

#[test]
fn database_adapter_contract_has_no_async_boundaries() {
    assert_database_adapter_sync_contract::<FakeAdapter>();
}

#[test]
fn execute_uses_shared_reference_and_begin_uses_mutable_reference() {
    let mut adapter = FakeAdapter::default();

    adapter
        .execute("CREATE TABLE projects (id INT);")
        .expect("execute with shared reference");

    {
        let mut tx = adapter.begin().expect("begin transaction");
        tx.execute("ALTER TABLE projects ADD COLUMN name TEXT;")
            .expect("execute inside transaction");
        tx.commit().expect("commit transaction");
    }

    assert_eq!(
        adapter.executed_sql(),
        vec![
            "CREATE TABLE projects (id INT);".to_string(),
            BEGIN_SQL.to_string(),
            "ALTER TABLE projects ADD COLUMN name TEXT;".to_string(),
            COMMIT_SQL.to_string(),
        ],
    );
}
