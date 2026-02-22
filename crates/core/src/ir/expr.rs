use super::{DataType, Ident, Value};

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    // Leaf and fallback expressions
    Literal(Literal),
    Ident(Ident),
    QualifiedIdent {
        qualifier: Ident,
        name: Ident,
    },
    Null,
    Raw(String),

    // Operators and logical combinators
    BinaryOp {
        left: Box<Expr>,
        op: BinaryOperator,
        right: Box<Expr>,
    },
    UnaryOp {
        op: UnaryOperator,
        expr: Box<Expr>,
    },
    Comparison {
        left: Box<Expr>,
        op: ComparisonOp,
        right: Box<Expr>,
        quantifier: Option<SetQuantifier>,
    },
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    Is {
        expr: Box<Expr>,
        test: IsTest,
    },

    // Range and grouping
    Between {
        expr: Box<Expr>,
        low: Box<Expr>,
        high: Box<Expr>,
        negated: bool,
    },
    In {
        expr: Box<Expr>,
        list: Vec<Expr>,
        negated: bool,
    },
    Paren(Box<Expr>),
    Tuple(Vec<Expr>),

    // Function and type operations
    Function {
        name: String,
        args: Vec<Expr>,
        distinct: bool,
        over: Option<WindowSpec>,
    },
    Cast {
        expr: Box<Expr>,
        data_type: DataType,
    },
    Collate {
        expr: Box<Expr>,
        collation: String,
    },

    // Compound expressions
    Case {
        operand: Option<Box<Expr>>,
        when_clauses: Vec<(Expr, Expr)>,
        else_clause: Option<Box<Expr>>,
    },
    ArrayConstructor(Vec<Expr>),
    Exists(Box<SubQuery>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Value(Value),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    StringConcat,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnaryOperator {
    Plus,
    Minus,
    Not,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComparisonOp {
    Equal,
    NotEqual,
    GreaterThan,
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
    Like,
    ILike,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IsTest {
    Null,
    NotNull,
    True,
    NotTrue,
    False,
    NotFalse,
    Unknown,
    NotUnknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetQuantifier {
    Any,
    Some,
    All,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WindowSpec {
    pub partition_by: Vec<Expr>,
    pub order_by: Vec<Expr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubQuery {
    pub sql: String,
}
