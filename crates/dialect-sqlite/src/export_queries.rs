// Ported from reference/sqldef/database/sqlite3/database.go with deterministic ORDER BY
// clauses added for stable export output in stateql tests.

pub(crate) const SHOW_SERVER_VERSION_QUERY: &str = "SELECT sqlite_version()";
pub(crate) const TABLE_NAMES_QUERY: &str = r#"
SELECT tbl_name
FROM sqlite_master
WHERE type = 'table' AND tbl_name NOT LIKE 'sqlite_%'
ORDER BY tbl_name ASC;
"#;

pub(crate) const TABLE_DDL_QUERY: &str = r#"
SELECT sql
FROM sqlite_master
WHERE tbl_name = ?1 AND type = 'table';
"#;

pub(crate) const VIEW_DDLS_QUERY: &str = r#"
SELECT sql
FROM sqlite_master
WHERE type = 'view' AND sql IS NOT NULL
ORDER BY tbl_name ASC;
"#;

// Exclude automatically generated indexes (for example, UNIQUE constraints)
// by filtering out rows with NULL SQL definitions.
pub(crate) const INDEX_DDLS_QUERY: &str = r#"
SELECT sql
FROM sqlite_master
WHERE type = 'index' AND sql IS NOT NULL
ORDER BY sql ASC;
"#;

pub(crate) const TRIGGER_DDLS_QUERY: &str = r#"
SELECT sql
FROM sqlite_master
WHERE type = 'trigger' AND sql IS NOT NULL
ORDER BY tbl_name ASC, name ASC;
"#;
