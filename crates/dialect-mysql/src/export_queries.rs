// Ported from reference/sqldef/database/mysql/database.go.
// stateql adds explicit ORDER BY clauses for deterministic export output.

pub(crate) const SHOW_SERVER_VERSION_QUERY: &str = "SELECT VERSION()";
pub(crate) const LOWER_CASE_TABLE_NAMES_QUERY: &str =
    "SHOW VARIABLES LIKE 'lower_case_table_names'";
pub(crate) const TABLE_NAMES_QUERY: &str = r#"
SHOW FULL TABLES
WHERE Table_Type != 'VIEW'
ORDER BY 1;
"#;

pub(crate) const VIEWS_QUERY: &str = r#"
SELECT TABLE_NAME, VIEW_DEFINITION, SECURITY_TYPE
FROM INFORMATION_SCHEMA.VIEWS
WHERE TABLE_SCHEMA = DATABASE()
ORDER BY TABLE_NAME ASC;
"#;

pub(crate) const TRIGGERS_QUERY: &str = r#"
SELECT TRIGGER_NAME, EVENT_MANIPULATION, EVENT_OBJECT_TABLE, ACTION_TIMING, ACTION_STATEMENT
FROM INFORMATION_SCHEMA.TRIGGERS
WHERE TRIGGER_SCHEMA = DATABASE()
ORDER BY TRIGGER_NAME ASC;
"#;
