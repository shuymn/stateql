use stateql_core::SchemaObject;

pub fn single_table_fixture(name: &str) -> Vec<SchemaObject> {
    vec![SchemaObject::Table {
        name: name.to_string(),
    }]
}
