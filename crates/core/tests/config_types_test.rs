use std::collections::BTreeMap;

use stateql_core::{ConnectionConfig, Version};

#[test]
fn version_exposes_major_minor_patch() {
    let version = Version {
        major: 16,
        minor: 3,
        patch: 1,
    };

    assert_eq!(version.major, 16);
    assert_eq!(version.minor, 3);
    assert_eq!(version.patch, 1);
}

#[test]
fn connection_config_exposes_v1_fields() {
    let mut extra = BTreeMap::new();
    extra.insert("sslmode".to_string(), "require".to_string());

    let config = ConnectionConfig {
        host: Some("db.internal".to_string()),
        port: Some(5432),
        user: Some("stateql".to_string()),
        password: Some("secret".to_string()),
        database: "app".to_string(),
        socket: None,
        extra,
    };

    assert_eq!(config.host.as_deref(), Some("db.internal"));
    assert_eq!(config.port, Some(5432));
    assert_eq!(config.user.as_deref(), Some("stateql"));
    assert_eq!(config.password.as_deref(), Some("secret"));
    assert_eq!(config.database, "app");
    assert_eq!(config.socket, None);
    assert_eq!(config.extra.get("sslmode"), Some(&"require".to_string()));
}
