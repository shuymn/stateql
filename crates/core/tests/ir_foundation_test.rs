use stateql_core::{DataType, Ident, QualifiedName, Value, value_total_eq};

#[test]
fn identifier_quoting_and_qualified_name_are_preserved() {
    let quoted = Ident::quoted("User");
    let unquoted = Ident::unquoted("users");

    assert_eq!(quoted.value, "User");
    assert!(quoted.quoted);
    assert_eq!(unquoted.value, "users");
    assert!(!unquoted.quoted);

    let qualified = QualifiedName {
        schema: Some(unquoted.clone()),
        name: quoted.clone(),
    };
    assert_eq!(qualified.schema, Some(unquoted));
    assert_eq!(qualified.name, quoted);
}

#[test]
fn custom_data_type_and_float_value_are_preserved() {
    let data_type = DataType::Custom("citext".to_string());
    assert!(matches!(data_type, DataType::Custom(name) if name == "citext"));

    let nan = f64::from_bits(0x7ff8_0000_0000_0000);
    let left = Value::Float(nan);
    let right = Value::Float(nan);

    assert_ne!(left, right);
    assert!(value_total_eq(&left, &right));
}
