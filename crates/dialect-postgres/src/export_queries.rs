pub(crate) const SHOW_SERVER_VERSION_QUERY: &str = "SHOW server_version";
pub(crate) const SHOW_SEARCH_PATH_QUERY: &str = "SHOW search_path";

pub(crate) const TABLE_NAMES_QUERY: &str = r#"
SELECT
  n.nspname AS table_schema,
  c.relname AS table_name,
  CASE
    WHEN c.relkind = 'p' THEN pg_catalog.pg_get_partkeydef(c.oid)
    ELSE NULL
  END AS partition_key,
  am.amname AS access_method,
  ts.spcname AS tablespace_name
FROM pg_catalog.pg_class c
INNER JOIN pg_catalog.pg_namespace n ON c.relnamespace = n.oid
LEFT JOIN pg_catalog.pg_am am ON c.relam = am.oid
LEFT JOIN pg_catalog.pg_tablespace ts ON c.reltablespace = ts.oid
WHERE n.nspname NOT IN ('information_schema', 'pg_catalog', 'sys')
  AND c.relkind IN ('r', 'p')
  AND c.relpersistence IN ('p', 'u')
  AND c.relispartition = false
  AND NOT EXISTS (
    SELECT 1
    FROM pg_catalog.pg_depend d
    WHERE c.oid = d.objid
      AND d.classid = (SELECT oid FROM pg_catalog.pg_class WHERE relname = 'pg_class')
      AND d.deptype = 'e'
  )
ORDER BY n.nspname ASC, c.relname ASC;
"#;

pub(crate) const TABLE_COLUMNS_QUERY: &str = r#"
SELECT
  a.attname AS column_name,
  pg_catalog.format_type(a.atttypid, a.atttypmod) AS data_type,
  a.attnotnull AS not_null,
  pg_catalog.pg_get_expr(ad.adbin, ad.adrelid) AS default_expr,
  CASE a.attidentity
    WHEN 'a' THEN 'ALWAYS'
    WHEN 'd' THEN 'BY DEFAULT'
    ELSE NULL
  END AS identity_generation
FROM pg_catalog.pg_attribute a
INNER JOIN pg_catalog.pg_class c ON c.oid = a.attrelid
INNER JOIN pg_catalog.pg_namespace n ON c.relnamespace = n.oid
LEFT JOIN pg_catalog.pg_attrdef ad ON ad.adrelid = a.attrelid AND ad.adnum = a.attnum
WHERE n.nspname = $1
  AND c.relname = $2
  AND c.relkind IN ('r', 'p')
  AND a.attnum > 0
  AND NOT a.attisdropped
ORDER BY a.attnum ASC;
"#;

pub(crate) const PARTITION_CHILD_TABLES_QUERY: &str = r#"
SELECT
  n.nspname AS partition_schema,
  c.relname AS partition_name,
  pn.nspname AS parent_schema,
  pc.relname AS parent_name,
  pg_catalog.pg_get_expr(c.relpartbound, c.oid) AS partition_bound
FROM pg_catalog.pg_class c
INNER JOIN pg_catalog.pg_namespace n ON c.relnamespace = n.oid
INNER JOIN pg_catalog.pg_inherits i ON c.oid = i.inhrelid
INNER JOIN pg_catalog.pg_class pc ON i.inhparent = pc.oid
INNER JOIN pg_catalog.pg_namespace pn ON pc.relnamespace = pn.oid
WHERE c.relispartition = true
  AND n.nspname NOT IN ('information_schema', 'pg_catalog', 'sys')
  AND NOT EXISTS (
    SELECT 1
    FROM pg_catalog.pg_depend d
    WHERE c.oid = d.objid
      AND d.classid = (SELECT oid FROM pg_catalog.pg_class WHERE relname = 'pg_class')
      AND d.deptype = 'e'
  )
ORDER BY n.nspname ASC, c.relname ASC;
"#;
