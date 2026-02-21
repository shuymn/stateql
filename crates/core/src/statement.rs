#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    Sql { sql: String, transactional: bool },
    BatchBoundary,
}
