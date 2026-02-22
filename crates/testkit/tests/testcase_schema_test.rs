use stateql_testkit::load_test_cases_from_str;

#[test]
fn defaults_offline_to_false_when_omitted() {
    let yaml = r#"
default_offline:
  current: CREATE TABLE users (id INT);
  desired: CREATE TABLE users (id INT);
"#;

    let cases = load_test_cases_from_str(yaml).expect("yaml must parse");
    let case = cases
        .get("default_offline")
        .expect("named testcase must be present");

    assert!(!case.offline, "offline omitted must default to false");
}

#[test]
fn preserves_error_and_version_and_flavor_fields() {
    let yaml = r#"
metadata:
  current: CREATE TABLE users (id INT);
  desired: CREATE TABLE users (id INT, name TEXT);
  error: unsupported op
  min_version: "13.2"
  max_version: "15.0"
  flavor: pgvector
"#;

    let cases = load_test_cases_from_str(yaml).expect("yaml must parse");
    let case = cases
        .get("metadata")
        .expect("named testcase must be present");

    assert_eq!(case.error.as_deref(), Some("unsupported op"));
    assert_eq!(case.min_version.as_deref(), Some("13.2"));
    assert_eq!(case.max_version.as_deref(), Some("15.0"));
    assert_eq!(case.flavor.as_deref(), Some("pgvector"));
}

#[test]
fn preserves_enable_drop_as_tristate_option() {
    let yaml = r#"
omitted:
  current: CREATE TABLE users (id INT);
  desired: CREATE TABLE users (id INT);
enabled:
  current: CREATE TABLE users (id INT);
  desired: CREATE TABLE users (id INT);
  enable_drop: true
disabled:
  current: CREATE TABLE users (id INT);
  desired: CREATE TABLE users (id INT);
  enable_drop: false
"#;

    let cases = load_test_cases_from_str(yaml).expect("yaml must parse");

    let omitted = cases.get("omitted").expect("omitted testcase must exist");
    let enabled = cases.get("enabled").expect("enabled testcase must exist");
    let disabled = cases.get("disabled").expect("disabled testcase must exist");

    assert_eq!(omitted.enable_drop, None);
    assert_eq!(enabled.enable_drop, Some(true));
    assert_eq!(disabled.enable_drop, Some(false));
}
