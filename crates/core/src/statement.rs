use crate::QualifiedName;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    Sql {
        sql: String,
        transactional: bool,
        context: Option<StatementContext>,
    },
    BatchBoundary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatementContext {
    SqliteTableRebuild {
        table: QualifiedName,
        step: SqliteRebuildStep,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SqliteRebuildStep {
    CreateShadowTable,
    CopyData,
    DropOldTable,
    RenameShadowTable,
    RecreateIndexes,
    RecreateTriggers,
}
