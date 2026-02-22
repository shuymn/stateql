use stateql_core::{SchemaObject, Table};

mod yaml_runner;

pub use yaml_runner::{TestCase, load_test_cases_from_str};

pub fn single_table_fixture(name: &str) -> Vec<SchemaObject> {
    vec![SchemaObject::Table(Table::named(name))]
}
