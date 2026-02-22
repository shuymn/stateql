use crate::{
    CheckConstraint, Column, ColumnPosition, Comment, CommentTarget, DataType, Domain,
    EnumValuePosition, ExclusionConstraint, Expr, Extension, ForeignKey, Function, GeneratedColumn,
    Ident, Identity, IndexDef, IndexOwner, MaterializedView, Partition, Policy, PrimaryKey,
    Privilege, QualifiedName, SchemaDef, Sequence, Table, TableOptions, Trigger, TypeDef, View,
};

#[derive(Debug, Clone, PartialEq)]
pub enum DiffOp {
    // --- Table ---
    CreateTable(Table),
    DropTable(QualifiedName),
    RenameTable {
        from: QualifiedName,
        to: QualifiedName,
    },

    // --- Column (scoped to a table) ---
    AddColumn {
        table: QualifiedName,
        column: Box<Column>,
        position: Option<ColumnPosition>,
    },
    DropColumn {
        table: QualifiedName,
        column: Ident,
    },
    AlterColumn {
        table: QualifiedName,
        column: Ident,
        changes: Vec<ColumnChange>,
    },
    RenameColumn {
        table: QualifiedName,
        from: Ident,
        to: Ident,
    },

    // --- Index (top-level, with owner) ---
    AddIndex(IndexDef),
    DropIndex {
        owner: IndexOwner,
        name: Ident,
    },
    RenameIndex {
        owner: IndexOwner,
        from: Ident,
        to: Ident,
    },

    // --- Foreign Key (scoped to a table) ---
    AddForeignKey {
        table: QualifiedName,
        fk: ForeignKey,
    },
    DropForeignKey {
        table: QualifiedName,
        name: Ident,
    },

    // --- Check Constraint (scoped to a table) ---
    AddCheck {
        table: QualifiedName,
        check: CheckConstraint,
    },
    DropCheck {
        table: QualifiedName,
        name: Ident,
    },

    // --- Exclusion Constraint (PostgreSQL, scoped to a table) ---
    AddExclusion {
        table: QualifiedName,
        exclusion: ExclusionConstraint,
    },
    DropExclusion {
        table: QualifiedName,
        name: Ident,
    },

    // --- Primary Key ---
    SetPrimaryKey {
        table: QualifiedName,
        pk: PrimaryKey,
    },
    DropPrimaryKey {
        table: QualifiedName,
    },

    // --- Partition (scoped to a table) ---
    AddPartition {
        table: QualifiedName,
        partition: Partition,
    },
    DropPartition {
        table: QualifiedName,
        name: Ident,
    },

    // --- View ---
    CreateView(View),
    DropView(QualifiedName),

    // --- Materialized View ---
    CreateMaterializedView(MaterializedView),
    DropMaterializedView(QualifiedName),

    // --- Sequence ---
    CreateSequence(Sequence),
    DropSequence(QualifiedName),
    AlterSequence {
        name: QualifiedName,
        changes: Vec<SequenceChange>,
    },

    // --- Trigger ---
    CreateTrigger(Trigger),
    DropTrigger {
        name: QualifiedName,
        table: Option<QualifiedName>,
    },

    // --- Function ---
    CreateFunction(Function),
    DropFunction(QualifiedName),

    // --- Type (ENUM, composite) ---
    CreateType(TypeDef),
    DropType(QualifiedName),
    AlterType {
        name: QualifiedName,
        change: TypeChange,
    },

    // --- Domain (PostgreSQL) ---
    CreateDomain(Domain),
    DropDomain(QualifiedName),
    AlterDomain {
        name: QualifiedName,
        change: DomainChange,
    },

    // --- Extension (PostgreSQL) ---
    CreateExtension(Extension),
    DropExtension(QualifiedName),

    // --- Schema ---
    CreateSchema(SchemaDef),
    DropSchema(QualifiedName),

    // --- Comment ---
    SetComment(Comment),
    DropComment {
        target: CommentTarget,
    },

    // --- Privilege ---
    Grant(Privilege),
    Revoke(Privilege),

    // --- Policy (PostgreSQL RLS) ---
    CreatePolicy(Policy),
    DropPolicy {
        name: Ident,
        table: QualifiedName,
    },

    // --- Table Options ---
    AlterTableOptions {
        table: QualifiedName,
        options: TableOptions,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ColumnChange {
    SetType(DataType),
    SetNotNull(bool),
    SetDefault(Option<Expr>),
    SetIdentity(Option<Identity>),
    SetGenerated(Option<GeneratedColumn>),
    SetCollation(Option<String>),
}

pub fn is_mysql_change_column_full_redefinition(changes: &[ColumnChange]) -> bool {
    let mut has_set_type = false;
    let mut has_set_not_null = false;
    let mut has_set_default = false;
    let mut has_set_identity = false;
    let mut has_set_generated = false;
    let mut has_set_collation = false;

    for change in changes {
        match change {
            ColumnChange::SetType(_) => has_set_type = true,
            ColumnChange::SetNotNull(_) => has_set_not_null = true,
            ColumnChange::SetDefault(_) => has_set_default = true,
            ColumnChange::SetIdentity(_) => has_set_identity = true,
            ColumnChange::SetGenerated(_) => has_set_generated = true,
            ColumnChange::SetCollation(_) => has_set_collation = true,
        }
    }

    has_set_type
        && has_set_not_null
        && has_set_default
        && has_set_identity
        && has_set_generated
        && has_set_collation
}

#[derive(Debug, Clone, PartialEq)]
pub enum SequenceChange {
    SetType(DataType),
    SetIncrement(i64),
    SetMinValue(Option<i64>),
    SetMaxValue(Option<i64>),
    SetStart(i64),
    SetCache(i64),
    SetCycle(bool),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeChange {
    AddValue {
        value: String,
        position: Option<EnumValuePosition>,
    },
    RenameValue {
        from: String,
        to: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum DomainChange {
    SetDefault(Option<Expr>),
    SetNotNull(bool),
    AddConstraint { name: Option<Ident>, check: Expr },
    DropConstraint(Ident),
}
