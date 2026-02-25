# stateql

Declarative SQL schema management tool. Define your desired schema in SQL, and stateql computes the minimal DDL to migrate your database to match.

Inspired by [sqldef](https://github.com/sqldef/sqldef), but not a drop-in replacement.

## Supported Databases

| Database   | Feature flag | Default |
|------------|-------------|---------|
| PostgreSQL | `postgres`  | Yes     |
| MySQL      | `mysql`     | Yes     |
| SQLite     | `sqlite`    | Yes     |
| SQL Server | `mssql`     | No      |

## Installation

```sh
cargo install --path crates/cli
```

To include SQL Server support:

```sh
cargo install --path crates/cli --features mssql
```

## Usage

```
stateql <dialect> [OPTIONS] <DATABASE>
```

### Modes

| Flag         | Description                                    |
|--------------|------------------------------------------------|
| `--dry-run`  | Show DDL that would be applied (default)       |
| `--apply`    | Execute DDL against the database               |
| `--export`   | Export the current database schema as SQL       |

### Options

| Option           | Description                              | Dialects                         |
|------------------|------------------------------------------|----------------------------------|
| `--file <PATH>`  | Read desired schema from file (else stdin)| All                              |
| `--enable-drop`  | Allow DROP operations                    | All                              |
| `--host <HOST>`  | Database host                            | PostgreSQL, MySQL, SQL Server    |
| `--port <PORT>`  | Database port                            | PostgreSQL, MySQL, SQL Server    |
| `--user <USER>`  | Username                                 | PostgreSQL, MySQL, SQL Server    |
| `--password <PW>`| Password                                 | PostgreSQL, MySQL, SQL Server    |
| `--sslmode <MODE>`| SSL mode                                | PostgreSQL                       |
| `--socket <PATH>`| Unix socket path                         | MySQL                            |

### Examples

Preview changes (dry-run):

```sh
stateql postgres --host localhost --user admin mydb < schema.sql
```

Apply changes:

```sh
stateql postgres --host localhost --user admin mydb --apply --file schema.sql
```

Export current schema:

```sh
stateql sqlite mydb.db --export > schema.sql
```

## Library Crates

stateql is also usable as a library:

| Crate                      | Description                              |
|----------------------------|------------------------------------------|
| `stateql-core`             | Schema IR, diff engine, and execution contracts |
| `stateql-dialect-postgres` | PostgreSQL dialect implementation         |
| `stateql-dialect-mysql`    | MySQL dialect implementation              |
| `stateql-dialect-sqlite`   | SQLite dialect implementation             |
| `stateql-dialect-mssql`    | SQL Server dialect implementation         |
| `stateql-testkit`          | YAML-driven test runner for dialect testing |

## License

[MIT](./LICENSE)
