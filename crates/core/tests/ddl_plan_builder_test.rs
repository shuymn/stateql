use stateql_core::{
    DdlPlanner, DiffOp, Ident, QualifiedName, SchemaDef, Table, TableOptions, build_ddl_plan,
};

fn ident(value: &str) -> Ident {
    Ident::unquoted(value)
}

fn qualified(schema: Option<&str>, name: &str) -> QualifiedName {
    QualifiedName {
        schema: schema.map(Ident::unquoted),
        name: Ident::unquoted(name),
    }
}

fn table(name: &str) -> Table {
    Table {
        name: qualified(Some("public"), name),
        columns: Vec::new(),
        primary_key: None,
        foreign_keys: Vec::new(),
        checks: Vec::new(),
        exclusions: Vec::new(),
        options: TableOptions::default(),
        partition: None,
        renamed_from: None,
    }
}

#[test]
fn builds_execution_plan_from_ordered_diffops() {
    let ordered_ops = vec![
        DiffOp::DropTable(qualified(Some("public"), "users")),
        DiffOp::CreateSchema(SchemaDef {
            name: ident("analytics"),
        }),
        DiffOp::CreateTable(table("users")),
    ];

    let planner = DdlPlanner::new();
    let plan = planner.build(ordered_ops.clone());

    assert_eq!(plan.ops(), ordered_ops.as_slice());
}

#[test]
fn planner_sorts_unordered_ops_using_priority_rules() {
    let unordered_ops = vec![
        DiffOp::CreateTable(table("users")),
        DiffOp::CreateSchema(SchemaDef {
            name: ident("analytics"),
        }),
        DiffOp::DropTable(qualified(Some("public"), "users")),
    ];

    let plan = build_ddl_plan(unordered_ops);
    let sorted_tags = plan
        .ops()
        .iter()
        .map(|op| match op {
            DiffOp::DropTable(_) => "DropTable",
            DiffOp::CreateSchema(_) => "CreateSchema",
            DiffOp::CreateTable(_) => "CreateTable",
            other => panic!("unexpected op in plan: {other:?}"),
        })
        .collect::<Vec<_>>();

    assert_eq!(
        sorted_tags,
        vec!["DropTable", "CreateSchema", "CreateTable"]
    );
}
