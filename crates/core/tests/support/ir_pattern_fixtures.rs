use std::collections::BTreeMap;

use stateql_core::{Column, DataType, Ident, QualifiedName};

pub fn qualified(schema: Option<&str>, name: &str) -> QualifiedName {
    QualifiedName {
        schema: schema.map(Ident::unquoted),
        name: Ident::unquoted(name),
    }
}

pub fn sample_column(name: &str) -> Column {
    Column {
        name: Ident::unquoted(name),
        data_type: DataType::Text,
        not_null: false,
        default: None,
        identity: None,
        generated: None,
        comment: None,
        collation: None,
        renamed_from: None,
        extra: BTreeMap::new(),
    }
}
