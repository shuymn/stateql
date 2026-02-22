use crate::DiffOp;

#[derive(Debug, Clone, PartialEq)]
pub struct DiffOutcome {
    pub ops: Vec<DiffOp>,
    pub diagnostics: DiffDiagnostics,
}

impl DiffOutcome {
    #[must_use]
    pub fn new(ops: Vec<DiffOp>, diagnostics: DiffDiagnostics) -> Self {
        Self { ops, diagnostics }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct DiffDiagnostics {
    pub skipped_ops: Vec<SkippedOpDiagnostic>,
}

impl DiffDiagnostics {
    #[must_use]
    pub fn from_enable_drop(full_ops: &[DiffOp], emitted_ops: &[DiffOp]) -> Self {
        let mut unmatched_emitted = emitted_ops.to_vec();
        let mut skipped_ops = Vec::new();

        for op in full_ops {
            let Some(kind) = skipped_op_kind(op) else {
                continue;
            };

            if let Some(position) = unmatched_emitted
                .iter()
                .position(|emitted_op| emitted_op == op)
            {
                unmatched_emitted.remove(position);
                continue;
            }

            skipped_ops.push(SkippedOpDiagnostic {
                kind,
                op: op.clone(),
            });
        }

        Self { skipped_ops }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.skipped_ops.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SkippedOpDiagnostic {
    pub kind: SkippedOpKind,
    pub op: DiffOp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkippedOpKind {
    DropTable,
    DropView,
    DropMaterializedView,
    DropSequence,
    DropTrigger,
    DropFunction,
    DropType,
    DropDomain,
    DropExtension,
    DropSchema,
    DropPolicy,
    DropColumn,
    DropIndex,
    DropForeignKey,
    DropCheck,
    DropExclusion,
    DropPrimaryKey,
    DropPartition,
    DropComment,
    Revoke,
}

pub const SUPPRESSED_OP_KINDS: [SkippedOpKind; 20] = [
    SkippedOpKind::DropTable,
    SkippedOpKind::DropView,
    SkippedOpKind::DropMaterializedView,
    SkippedOpKind::DropSequence,
    SkippedOpKind::DropTrigger,
    SkippedOpKind::DropFunction,
    SkippedOpKind::DropType,
    SkippedOpKind::DropDomain,
    SkippedOpKind::DropExtension,
    SkippedOpKind::DropSchema,
    SkippedOpKind::DropPolicy,
    SkippedOpKind::DropColumn,
    SkippedOpKind::DropIndex,
    SkippedOpKind::DropForeignKey,
    SkippedOpKind::DropCheck,
    SkippedOpKind::DropExclusion,
    SkippedOpKind::DropPrimaryKey,
    SkippedOpKind::DropPartition,
    SkippedOpKind::DropComment,
    SkippedOpKind::Revoke,
];

#[must_use]
pub fn skipped_op_kind(op: &DiffOp) -> Option<SkippedOpKind> {
    SUPPRESSED_OP_KINDS
        .into_iter()
        .find(|kind| kind.matches(op))
}

impl SkippedOpKind {
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::DropTable => "DROP TABLE",
            Self::DropView => "DROP VIEW",
            Self::DropMaterializedView => "DROP MATERIALIZED VIEW",
            Self::DropSequence => "DROP SEQUENCE",
            Self::DropTrigger => "DROP TRIGGER",
            Self::DropFunction => "DROP FUNCTION",
            Self::DropType => "DROP TYPE",
            Self::DropDomain => "DROP DOMAIN",
            Self::DropExtension => "DROP EXTENSION",
            Self::DropSchema => "DROP SCHEMA",
            Self::DropPolicy => "DROP POLICY",
            Self::DropColumn => "DROP COLUMN",
            Self::DropIndex => "DROP INDEX",
            Self::DropForeignKey => "DROP FOREIGN KEY",
            Self::DropCheck => "DROP CHECK",
            Self::DropExclusion => "DROP EXCLUSION",
            Self::DropPrimaryKey => "DROP PRIMARY KEY",
            Self::DropPartition => "DROP PARTITION",
            Self::DropComment => "DROP COMMENT",
            Self::Revoke => "REVOKE",
        }
    }

    fn matches(self, op: &DiffOp) -> bool {
        match self {
            Self::DropTable => matches!(op, DiffOp::DropTable(_)),
            Self::DropView => matches!(op, DiffOp::DropView(_)),
            Self::DropMaterializedView => matches!(op, DiffOp::DropMaterializedView(_)),
            Self::DropSequence => matches!(op, DiffOp::DropSequence(_)),
            Self::DropTrigger => matches!(op, DiffOp::DropTrigger { .. }),
            Self::DropFunction => matches!(op, DiffOp::DropFunction(_)),
            Self::DropType => matches!(op, DiffOp::DropType(_)),
            Self::DropDomain => matches!(op, DiffOp::DropDomain(_)),
            Self::DropExtension => matches!(op, DiffOp::DropExtension(_)),
            Self::DropSchema => matches!(op, DiffOp::DropSchema(_)),
            Self::DropPolicy => matches!(op, DiffOp::DropPolicy { .. }),
            Self::DropColumn => matches!(op, DiffOp::DropColumn { .. }),
            Self::DropIndex => matches!(op, DiffOp::DropIndex { .. }),
            Self::DropForeignKey => matches!(op, DiffOp::DropForeignKey { .. }),
            Self::DropCheck => matches!(op, DiffOp::DropCheck { .. }),
            Self::DropExclusion => matches!(op, DiffOp::DropExclusion { .. }),
            Self::DropPrimaryKey => matches!(op, DiffOp::DropPrimaryKey { .. }),
            Self::DropPartition => matches!(op, DiffOp::DropPartition { .. }),
            Self::DropComment => matches!(op, DiffOp::DropComment { .. }),
            Self::Revoke => matches!(op, DiffOp::Revoke(_)),
        }
    }
}
