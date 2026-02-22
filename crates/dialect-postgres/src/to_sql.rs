use stateql_core::{DiffOp, GenerateError, Result, SchemaObject, Statement};

use crate::generator;

const TO_SQL_TARGET: &str = "dialect export renderer";
const BATCH_BOUNDARY_OP: &str = "BatchBoundary";

pub(crate) fn render_object(dialect_name: &str, object: &SchemaObject) -> Result<String> {
    let ops = create_ops(object);
    let statements = generator::generate_ddl(dialect_name, &ops)?;
    render_statements(dialect_name, &statements)
}

fn create_ops(object: &SchemaObject) -> Vec<DiffOp> {
    match object {
        SchemaObject::Table(table) => {
            let mut ops = vec![DiffOp::CreateTable(table.clone())];
            if let Some(partition) = &table.partition
                && !partition.partitions.is_empty()
            {
                ops.push(DiffOp::AddPartition {
                    table: table.name.clone(),
                    partition: partition.clone(),
                });
            }
            ops
        }
        SchemaObject::View(view) => vec![DiffOp::CreateView(view.clone())],
        SchemaObject::MaterializedView(view) => {
            vec![DiffOp::CreateMaterializedView(view.clone())]
        }
        SchemaObject::Index(index) => vec![DiffOp::AddIndex(index.clone())],
        SchemaObject::Sequence(sequence) => vec![DiffOp::CreateSequence(sequence.clone())],
        SchemaObject::Trigger(trigger) => vec![DiffOp::CreateTrigger(trigger.clone())],
        SchemaObject::Function(function) => vec![DiffOp::CreateFunction(function.clone())],
        SchemaObject::Type(ty) => vec![DiffOp::CreateType(ty.clone())],
        SchemaObject::Domain(domain) => vec![DiffOp::CreateDomain(domain.clone())],
        SchemaObject::Extension(extension) => vec![DiffOp::CreateExtension(extension.clone())],
        SchemaObject::Schema(schema) => vec![DiffOp::CreateSchema(schema.clone())],
        SchemaObject::Comment(comment) => vec![DiffOp::SetComment(comment.clone())],
        SchemaObject::Privilege(privilege) => vec![DiffOp::Grant(privilege.clone())],
        SchemaObject::Policy(policy) => vec![DiffOp::CreatePolicy(policy.clone())],
    }
}

fn render_statements(dialect_name: &str, statements: &[Statement]) -> Result<String> {
    let mut rendered = Vec::with_capacity(statements.len());
    for statement in statements {
        match statement {
            Statement::Sql { sql, .. } => rendered.push(format!("{sql};")),
            Statement::BatchBoundary => {
                return Err(GenerateError::UnsupportedDiffOp {
                    diff_op: BATCH_BOUNDARY_OP.to_string(),
                    target: TO_SQL_TARGET.to_string(),
                    dialect: dialect_name.to_string(),
                }
                .into());
            }
        }
    }

    Ok(rendered.join("\n"))
}
