use stateql_core::{SchemaObject, Table};

mod yaml_runner;

pub use yaml_runner::{
    TestCase, TestResult, load_test_cases_from_path, load_test_cases_from_str, matches_flavor,
    run_offline_test, run_online_test,
};

pub fn single_table_fixture(name: &str) -> Vec<SchemaObject> {
    vec![SchemaObject::Table(Table::named(name))]
}
