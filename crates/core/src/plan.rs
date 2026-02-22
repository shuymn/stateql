use crate::DiffOp;
pub use crate::ordering::sort_diff_ops;

#[derive(Debug, Clone, PartialEq)]
pub struct DdlPlan {
    ordered_ops: Vec<DiffOp>,
}

impl DdlPlan {
    #[must_use]
    pub fn new(ordered_ops: Vec<DiffOp>) -> Self {
        Self { ordered_ops }
    }

    #[must_use]
    pub fn ops(&self) -> &[DiffOp] {
        &self.ordered_ops
    }

    #[must_use]
    pub fn into_ops(self) -> Vec<DiffOp> {
        self.ordered_ops
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct DdlPlanner;

impl DdlPlanner {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    #[must_use]
    pub fn build(&self, ops: Vec<DiffOp>) -> DdlPlan {
        DdlPlan::new(sort_diff_ops(ops))
    }
}

#[must_use]
pub fn build_ddl_plan(ops: Vec<DiffOp>) -> DdlPlan {
    DdlPlanner::new().build(ops)
}
