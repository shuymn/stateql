use stateql_core::{AnnotationExtractor, Ident, RenameAnnotation};

#[test]
fn extracts_renamed_annotations_from_comments_only_and_preserves_lines() {
    let sql = concat!(
        "CREATE TABLE users (\n",
        "  id bigint NOT NULL,\n",
        "  note text DEFAULT '@renamed from=literal'\n",
        "); -- @renamed from=legacy_users\n"
    );

    let (clean_sql, annotations) = AnnotationExtractor::extract(sql).expect("extract annotations");

    assert_eq!(
        annotations,
        vec![RenameAnnotation {
            line: 4,
            from: Ident::unquoted("legacy_users"),
            deprecated_alias: false,
        }]
    );
    assert_eq!(sql.lines().count(), clean_sql.lines().count());
    assert!(clean_sql.contains("'@renamed from=literal'"));
    assert!(!clean_sql.contains("@renamed from=legacy_users"));
}

#[test]
fn accepts_deprecated_rename_alias_for_future_warning_handling() {
    let sql = "CREATE TABLE users(id bigint); -- @rename from=\"legacy_users\"\n";

    let (clean_sql, annotations) = AnnotationExtractor::extract(sql).expect("extract annotations");

    assert_eq!(
        annotations,
        vec![RenameAnnotation {
            line: 1,
            from: Ident::quoted("legacy_users"),
            deprecated_alias: true,
        }]
    );
    assert!(!clean_sql.contains("@rename from"));
}
