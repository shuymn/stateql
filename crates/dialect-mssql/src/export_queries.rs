// Ported from reference/sqldef/database/mssql/database.go with deterministic ORDER BY
// clauses and string-cast projections for stable export rendering in stateql tests.

pub(crate) const SHOW_SERVER_VERSION_QUERY: &str =
    "SELECT CAST(SERVERPROPERTY('ProductVersion') AS nvarchar(128));";
pub(crate) const CURRENT_SCHEMA_QUERY: &str = "SELECT COALESCE(SCHEMA_NAME(), 'dbo');";

pub(crate) const TABLE_NAMES_QUERY: &str = r#"
SELECT
    SCHEMA_NAME(t.schema_id) AS table_schema,
    t.name AS table_name
FROM sys.tables AS t
ORDER BY table_schema ASC, table_name ASC;
"#;

pub(crate) const COLUMN_DEFINITIONS_QUERY_TEMPLATE: &str = r#"
SELECT
    c.name AS column_name,
    ty.name AS data_type,
    CAST(c.max_length AS nvarchar(32)) AS max_length,
    CAST(c.precision AS nvarchar(32)) AS precision,
    CAST(c.scale AS nvarchar(32)) AS scale,
    CASE WHEN c.is_nullable = 1 THEN '1' ELSE '0' END AS is_nullable,
    CASE WHEN c.is_identity = 1 THEN '1' ELSE '0' END AS is_identity,
    COALESCE(CAST(ic.seed_value AS nvarchar(64)), '') AS seed_value,
    COALESCE(CAST(ic.increment_value AS nvarchar(64)), '') AS increment_value,
    CASE WHEN COALESCE(ic.is_not_for_replication, 0) = 1 THEN '1' ELSE '0' END AS identity_not_for_replication
FROM sys.columns AS c
JOIN sys.types AS ty ON c.user_type_id = ty.user_type_id
LEFT JOIN sys.identity_columns AS ic ON c.object_id = ic.object_id AND c.column_id = ic.column_id
WHERE c.object_id = OBJECT_ID(N'{object_id_literal}')
ORDER BY c.column_id ASC;
"#;

pub(crate) const PRIMARY_KEY_QUERY_TEMPLATE: &str = r#"
SELECT
    i.name AS pk_name,
    i.type_desc AS pk_type_desc,
    c.name AS column_name,
    CASE WHEN ic.is_descending_key = 1 THEN '1' ELSE '0' END AS is_descending
FROM sys.indexes AS i
JOIN sys.index_columns AS ic ON i.object_id = ic.object_id AND i.index_id = ic.index_id
JOIN sys.columns AS c ON ic.object_id = c.object_id AND ic.column_id = c.column_id
WHERE i.object_id = OBJECT_ID(N'{object_id_literal}')
  AND i.is_primary_key = 1
ORDER BY ic.key_ordinal ASC;
"#;
