use stateql_core::{
    BinaryOperator, ComparisonOp, DataType, Expr, Ident, IsTest, Literal, SetQuantifier, SubQuery,
    UnaryOperator, Value, WindowSpec,
};

#[test]
fn expr_leaf_and_fallback_variants_are_constructible() {
    let literal = Expr::Literal(Literal::Integer(42));
    let ident = Expr::Ident(Ident::unquoted("age"));
    let qualified = Expr::QualifiedIdent {
        qualifier: Ident::unquoted("users"),
        name: Ident::unquoted("id"),
    };
    let null = Expr::Null;
    let raw = Expr::Raw("coalesce(age, 0)".to_string());

    assert!(matches!(literal, Expr::Literal(Literal::Integer(42))));
    assert!(matches!(ident, Expr::Ident(value) if value.value == "age" && !value.quoted));
    assert!(matches!(qualified, Expr::QualifiedIdent { qualifier, name }
        if qualifier.value == "users" && name.value == "id"));
    assert!(matches!(null, Expr::Null));
    assert!(matches!(raw, Expr::Raw(sql) if sql == "coalesce(age, 0)"));
}

#[test]
fn expr_operator_range_function_and_compound_variants_are_constructible() {
    let age = Expr::Ident(Ident::unquoted("age"));
    let min_age = Expr::Literal(Literal::Integer(18));
    let max_age = Expr::Literal(Literal::Integer(65));

    let binary_op = Expr::BinaryOp {
        left: Box::new(age.clone()),
        op: BinaryOperator::Add,
        right: Box::new(Expr::Literal(Literal::Integer(1))),
    };
    let unary_op = Expr::UnaryOp {
        op: UnaryOperator::Minus,
        expr: Box::new(min_age.clone()),
    };
    let comparison = Expr::Comparison {
        left: Box::new(age.clone()),
        op: ComparisonOp::GreaterThanOrEqual,
        right: Box::new(min_age.clone()),
        quantifier: Some(SetQuantifier::Any),
    };
    let and_expr = Expr::And(
        Box::new(comparison.clone()),
        Box::new(Expr::Not(Box::new(Expr::Null))),
    );
    let or_expr = Expr::Or(Box::new(and_expr.clone()), Box::new(Expr::Null));
    let is_expr = Expr::Is {
        expr: Box::new(age.clone()),
        test: IsTest::NotNull,
    };

    let between = Expr::Between {
        expr: Box::new(age.clone()),
        low: Box::new(min_age.clone()),
        high: Box::new(max_age.clone()),
        negated: false,
    };
    let in_expr = Expr::In {
        expr: Box::new(age.clone()),
        list: vec![min_age.clone(), max_age.clone()],
        negated: false,
    };
    let paren = Expr::Paren(Box::new(binary_op.clone()));
    let tuple = Expr::Tuple(vec![min_age.clone(), max_age.clone()]);

    let window = WindowSpec {
        partition_by: vec![Expr::Ident(Ident::unquoted("department"))],
        order_by: vec![age.clone()],
    };
    let function = Expr::Function {
        name: "max".to_string(),
        args: vec![age.clone()],
        distinct: true,
        over: Some(window),
    };
    let cast = Expr::Cast {
        expr: Box::new(Expr::Literal(Literal::String("42".to_string()))),
        data_type: DataType::Integer,
    };
    let collate = Expr::Collate {
        expr: Box::new(Expr::Ident(Ident::unquoted("name"))),
        collation: "en_US".to_string(),
    };

    let case = Expr::Case {
        operand: Some(Box::new(age.clone())),
        when_clauses: vec![(
            Expr::Comparison {
                left: Box::new(age.clone()),
                op: ComparisonOp::GreaterThan,
                right: Box::new(Expr::Literal(Literal::Integer(20))),
                quantifier: None,
            },
            Expr::Literal(Literal::String("adult".to_string())),
        )],
        else_clause: Some(Box::new(Expr::Literal(Literal::String(
            "minor".to_string(),
        )))),
    };
    let array = Expr::ArrayConstructor(vec![
        Expr::Literal(Literal::Integer(1)),
        Expr::Literal(Literal::Integer(2)),
    ]);
    let exists = Expr::Exists(Box::new(SubQuery {
        sql: "SELECT 1".to_string(),
    }));

    assert!(matches!(
        binary_op,
        Expr::BinaryOp {
            op: BinaryOperator::Add,
            ..
        }
    ));
    assert!(matches!(
        unary_op,
        Expr::UnaryOp {
            op: UnaryOperator::Minus,
            ..
        }
    ));
    assert!(matches!(
        comparison,
        Expr::Comparison {
            quantifier: Some(SetQuantifier::Any),
            ..
        }
    ));
    assert!(matches!(and_expr, Expr::And(_, _)));
    assert!(matches!(or_expr, Expr::Or(_, _)));
    assert!(matches!(
        is_expr,
        Expr::Is {
            test: IsTest::NotNull,
            ..
        }
    ));

    assert!(matches!(between, Expr::Between { negated: false, .. }));
    assert!(matches!(in_expr, Expr::In { negated: false, list, .. } if list.len() == 2));
    assert!(matches!(paren, Expr::Paren(_)));
    assert!(matches!(tuple, Expr::Tuple(values) if values.len() == 2));

    assert!(matches!(
        function,
        Expr::Function {
            distinct: true,
            over: Some(_),
            ..
        }
    ));
    assert!(matches!(
        cast,
        Expr::Cast {
            data_type: DataType::Integer,
            ..
        }
    ));
    assert!(matches!(collate, Expr::Collate { collation, .. } if collation == "en_US"));

    assert!(
        matches!(case, Expr::Case { when_clauses, else_clause: Some(_), .. } if when_clauses.len() == 1)
    );
    assert!(matches!(array, Expr::ArrayConstructor(values) if values.len() == 2));
    assert!(matches!(exists, Expr::Exists(query) if query.sql == "SELECT 1"));

    let literal_bool = Expr::Literal(Literal::Boolean(true));
    let literal_float = Expr::Literal(Literal::Float(std::f64::consts::PI));
    let literal_value = Expr::Literal(Literal::Value(Value::Integer(1)));

    assert!(matches!(
        literal_bool,
        Expr::Literal(Literal::Boolean(true))
    ));
    assert!(
        matches!(literal_float, Expr::Literal(Literal::Float(v)) if (v - std::f64::consts::PI).abs() < f64::EPSILON)
    );
    assert!(matches!(
        literal_value,
        Expr::Literal(Literal::Value(Value::Integer(1)))
    ));
}
