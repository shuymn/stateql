mod adapter;
mod dialect;
mod diff;
mod error;
mod ir;
mod statement;

pub use adapter::DatabaseAdapter;
pub use dialect::Dialect;
pub use diff::DiffOp;
pub use error::{CoreError, CoreResult};
pub use ir::SchemaObject;
pub use statement::Statement;

pub fn plan_diff(desired: &[SchemaObject], current: &[SchemaObject]) -> Vec<DiffOp> {
    let mut ops = Vec::new();

    for object in desired {
        if !current.contains(object) {
            ops.push(DiffOp::CreateObject(object.clone()));
        }
    }

    for object in current {
        if !desired.contains(object) {
            ops.push(DiffOp::DropObject(object.clone()));
        }
    }

    ops
}

#[cfg(test)]
mod tests {
    use super::{plan_diff, Dialect, DiffOp, SchemaObject, Statement};

    struct StubDialect;

    impl Dialect for StubDialect {
        fn name(&self) -> &'static str {
            "postgres"
        }

        fn parse(&self, sql: &str) -> super::CoreResult<Vec<SchemaObject>> {
            let mut objects = Vec::new();
            for line in sql.lines().map(str::trim).filter(|line| !line.is_empty()) {
                if let Some(name) = line.strip_prefix("table:") {
                    objects.push(SchemaObject::Table {
                        name: name.trim().to_string(),
                    });
                }
            }
            Ok(objects)
        }

        fn generate_ddl(&self, ops: &[DiffOp]) -> super::CoreResult<Vec<Statement>> {
            let mut statements = Vec::new();
            for op in ops {
                match op {
                    DiffOp::CreateObject(SchemaObject::Table { name }) => {
                        statements.push(Statement::Sql {
                            sql: format!("CREATE TABLE {name} ();"),
                            transactional: true,
                        });
                    }
                    DiffOp::DropObject(SchemaObject::Table { name }) => {
                        statements.push(Statement::Sql {
                            sql: format!("DROP TABLE {name};"),
                            transactional: true,
                        });
                    }
                    DiffOp::CreateObject(SchemaObject::Index { name, table }) => {
                        statements.push(Statement::Sql {
                            sql: format!("CREATE INDEX {name} ON {table} ();"),
                            transactional: true,
                        });
                    }
                    DiffOp::DropObject(SchemaObject::Index { name, .. }) => {
                        statements.push(Statement::Sql {
                            sql: format!("DROP INDEX {name};"),
                            transactional: true,
                        });
                    }
                }
            }
            Ok(statements)
        }
    }

    #[test]
    fn smoke_parse_diff_render() {
        let dialect = StubDialect;
        let desired = dialect.parse("table:users").expect("parse should succeed");
        let current = Vec::new();

        let ops = plan_diff(&desired, &current);
        assert_eq!(ops.len(), 1);

        let rendered = dialect
            .generate_ddl(&ops)
            .expect("ddl generation should succeed");

        assert_eq!(
            rendered,
            vec![Statement::Sql {
                sql: "CREATE TABLE users ();".to_string(),
                transactional: true,
            }],
        );
    }
}
