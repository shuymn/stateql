use stateql_core::{SchemaObject, Table};

use crate::fake_dialect::FakeDialect;

pub const CURRENT_SQL: &str = "CURRENT_SQL";
pub const DESIRED_SQL_IGNORED: &str = "DESIRED_SQL_IGNORED";

pub fn export_fixture_objects() -> Vec<SchemaObject> {
    vec![
        SchemaObject::Table(Table::named("users")),
        SchemaObject::Table(Table::named("orders")),
    ]
}

pub fn configure_export_fixture(dialect: &FakeDialect) -> Vec<SchemaObject> {
    let objects = export_fixture_objects();
    dialect.set_export_schema_sql(CURRENT_SQL);
    dialect.set_parse_result(CURRENT_SQL, objects.clone());
    objects
}

pub fn expected_export_sql(objects: &[SchemaObject]) -> String {
    objects
        .iter()
        .map(|object| format!("{object:?}\n"))
        .collect::<String>()
}
