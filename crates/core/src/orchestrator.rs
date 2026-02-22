use std::sync::Arc;

use crate::{
    ConnectionConfig, DatabaseAdapter, Dialect, DiffConfig, DiffDiagnostics, DiffEngine,
    EquivalencePolicy, Executor, Expr, OrchestratorOutput::*, Renderer, Result, SchemaObject,
    Statement,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Apply,
    DryRun,
    Export,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrchestratorOptions {
    pub mode: Mode,
    pub enable_drop: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrchestratorOutput {
    Applied,
    DryRunSql(String),
    ExportSql(String),
}

pub struct Orchestrator<'a> {
    dialect: &'a dyn Dialect,
    diff_engine: DiffEngine,
}

impl<'a> Orchestrator<'a> {
    #[must_use]
    pub fn new(dialect: &'a dyn Dialect) -> Self {
        Self {
            dialect,
            diff_engine: DiffEngine::new(),
        }
    }

    pub fn run(
        &self,
        connection_config: &ConnectionConfig,
        desired_sql: &str,
        options: OrchestratorOptions,
    ) -> Result<OrchestratorOutput> {
        let mut adapter = self.dialect.connect(connection_config)?;
        let current_sql = adapter.export_schema()?;
        match options.mode {
            Mode::Export => Ok(ExportSql(self.export_sql_from_input(&current_sql)?)),
            Mode::Apply | Mode::DryRun => {
                let current = self.parse_and_normalize(&current_sql)?;
                let desired = self.parse_and_normalize(desired_sql)?;
                let diff_config = self.diff_config(adapter.as_ref(), options.enable_drop);
                let diff_outcome =
                    self.diff_engine
                        .diff_with_diagnostics(&desired, &current, &diff_config)?;
                let statements = self.dialect.generate_ddl(&diff_outcome.ops)?;

                if options.mode == Mode::Apply {
                    let mut executor = Executor::new(adapter.as_mut());
                    executor.execute_plan(&statements)?;
                    Ok(OrchestratorOutput::Applied)
                } else {
                    let rendered = self.render_dry_run(&statements, &diff_outcome.diagnostics);
                    Ok(DryRunSql(rendered))
                }
            }
        }
    }

    pub fn export_roundtrip_matches(&self, exported_sql: &str) -> Result<bool> {
        let re_exported_sql = self.export_sql_from_input(exported_sql)?;
        Ok(exported_sql == re_exported_sql)
    }

    fn parse_and_normalize(&self, sql: &str) -> Result<Vec<SchemaObject>> {
        let mut objects = self.dialect.parse(sql)?;
        for object in &mut objects {
            self.dialect.normalize(object);
        }
        Ok(objects)
    }

    fn export_sql_from_input(&self, sql: &str) -> Result<String> {
        let objects = self.parse_and_normalize(sql)?;
        self.render_export(&objects)
    }

    fn diff_config(&self, adapter: &dyn DatabaseAdapter, enable_drop: bool) -> DiffConfig {
        DiffConfig::new(
            enable_drop,
            adapter.schema_search_path(),
            Arc::new(DelegatingEquivalencePolicy {
                inner: self.dialect.equivalence_policy(),
            }),
        )
    }

    fn render_dry_run(&self, statements: &[Statement], diagnostics: &DiffDiagnostics) -> String {
        let renderer = Renderer::new(self.dialect);
        let mut rendered = String::new();

        renderer.render_skipped_diagnostics(&mut rendered, &skipped_messages(diagnostics));
        rendered.push_str(&renderer.render(statements));

        rendered
    }

    fn render_export(&self, objects: &[SchemaObject]) -> Result<String> {
        let mut rendered = String::new();
        for object in objects {
            rendered.push_str(&self.dialect.to_sql(object)?);
            rendered.push('\n');
        }
        Ok(rendered)
    }
}

struct DelegatingEquivalencePolicy {
    inner: &'static dyn EquivalencePolicy,
}

impl EquivalencePolicy for DelegatingEquivalencePolicy {
    fn is_equivalent_expr(&self, left: &Expr, right: &Expr) -> bool {
        self.inner.is_equivalent_expr(left, right)
    }

    fn is_equivalent_custom_type(&self, left: &str, right: &str) -> bool {
        self.inner.is_equivalent_custom_type(left, right)
    }
}

fn skipped_messages(diagnostics: &DiffDiagnostics) -> Vec<String> {
    diagnostics
        .skipped_ops
        .iter()
        .map(|diagnostic| diagnostic.kind.tag().to_string())
        .collect()
}
