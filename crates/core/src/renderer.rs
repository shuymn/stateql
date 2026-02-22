use crate::{Dialect, Statement};

const SKIPPED_DIAGNOSTICS_HEADER: &str = "-- Skipped operations (enable_drop=false):";

pub struct Renderer<'a> {
    dialect: &'a dyn Dialect,
}

impl<'a> Renderer<'a> {
    #[must_use]
    pub const fn new(dialect: &'a dyn Dialect) -> Self {
        Self { dialect }
    }

    #[must_use]
    pub fn render(&self, statements: &[Statement]) -> String {
        let mut rendered = String::new();
        self.render_skipped_diagnostics(&mut rendered, &[]);

        for statement in statements {
            match statement {
                Statement::Sql { sql, .. } => {
                    rendered.push_str(sql);
                    rendered.push('\n');
                }
                Statement::BatchBoundary => self.push_batch_separator(&mut rendered),
            }
        }

        rendered
    }

    fn push_batch_separator(&self, rendered: &mut String) {
        let separator = self.dialect.batch_separator();
        if separator.is_empty() {
            return;
        }

        rendered.push_str(separator);
        if !separator.ends_with('\n') {
            rendered.push('\n');
        }
    }

    pub(crate) fn render_skipped_diagnostics(&self, rendered: &mut String, diagnostics: &[String]) {
        if diagnostics.is_empty() {
            return;
        }

        self.render_diagnostics_header(rendered, SKIPPED_DIAGNOSTICS_HEADER);
        for message in diagnostics {
            rendered.push_str("-- Skipped: ");
            rendered.push_str(message);
            rendered.push('\n');
        }
        rendered.push('\n');
    }

    fn render_diagnostics_header(&self, rendered: &mut String, header: &str) {
        rendered.push_str(header);
        rendered.push('\n');
    }
}
