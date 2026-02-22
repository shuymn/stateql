use std::cmp::Ordering;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataType {
    Boolean,
    SmallInt,
    Integer,
    BigInt,
    Real,
    DoublePrecision,
    Numeric {
        precision: Option<u32>,
        scale: Option<u32>,
    },
    Text,
    Varchar {
        length: Option<u32>,
    },
    Char {
        length: Option<u32>,
    },
    Blob,
    Date,
    Time {
        with_timezone: bool,
    },
    Timestamp {
        with_timezone: bool,
    },
    Json,
    Jsonb,
    Uuid,
    Array(Box<DataType>),
    Custom(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    String(String),
    Integer(i64),
    Float(f64),
    Bool(bool),
    Null,
}

pub fn float_total_cmp(left: f64, right: f64) -> Ordering {
    left.total_cmp(&right)
}

pub fn value_total_eq(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Float(left), Value::Float(right)) => float_total_cmp(*left, *right).is_eq(),
        _ => left == right,
    }
}
