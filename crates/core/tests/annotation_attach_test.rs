use std::collections::BTreeMap;

use stateql_core::{
    AnnotationAttachment, AnnotationTarget, Column, DataType, DiffError, Error, Expr, Ident,
    Literal, QualifiedName, RenameAnnotation, SchemaObject, Table, attach_annotations,
};

fn qualified(name: &str) -> QualifiedName {
    QualifiedName {
        schema: None,
        name: Ident::unquoted(name),
    }
}

fn make_table(name: &str, column_names: &[&str]) -> Table {
    let mut table = Table::named(name);
    table.columns = column_names
        .iter()
        .map(|column_name| Column {
            name: Ident::unquoted(*column_name),
            data_type: DataType::Text,
            not_null: false,
            default: Some(Expr::Literal(Literal::String("default".to_string()))),
            identity: None,
            generated: None,
            comment: None,
            collation: None,
            renamed_from: None,
            extra: BTreeMap::new(),
        })
        .collect();
    table
}

#[test]
fn attaches_annotations_to_table_and_column_targets() {
    let mut objects = vec![SchemaObject::Table(make_table("users", &["id", "user_id"]))];
    let annotations = vec![
        RenameAnnotation {
            line: 2,
            from: Ident::unquoted("legacy_users"),
            deprecated_alias: false,
        },
        RenameAnnotation {
            line: 3,
            from: Ident::quoted("username"),
            deprecated_alias: false,
        },
    ];
    let attachments = vec![
        AnnotationAttachment {
            line: 2,
            target: AnnotationTarget::Table(qualified("users")),
        },
        AnnotationAttachment {
            line: 3,
            target: AnnotationTarget::TableColumn {
                table: qualified("users"),
                column: Ident::unquoted("user_id"),
            },
        },
    ];

    attach_annotations(&mut objects, &annotations, &attachments).expect("attach annotations");

    let SchemaObject::Table(table) = &objects[0] else {
        panic!("expected table object");
    };
    assert_eq!(table.renamed_from, Some(Ident::unquoted("legacy_users")));
    assert_eq!(
        table.columns[1].renamed_from,
        Some(Ident::quoted("username"))
    );
}

#[test]
fn orphan_annotations_fail_fast_without_mutating_targets() {
    let mut objects = vec![SchemaObject::Table(make_table("new_name", &["id"]))];
    let annotations = vec![RenameAnnotation {
        line: 8,
        from: Ident::unquoted("old_name"),
        deprecated_alias: false,
    }];
    let attachments = vec![AnnotationAttachment {
        line: 2,
        target: AnnotationTarget::Table(qualified("new_name")),
    }];

    let result = attach_annotations(&mut objects, &annotations, &attachments);
    assert!(matches!(
        result,
        Err(Error::Diff(DiffError::ObjectComparison { operation, .. }))
            if operation == "rename annotation mismatch"
    ));

    let SchemaObject::Table(table) = &objects[0] else {
        panic!("expected table object");
    };
    assert_eq!(table.renamed_from, None);
}
