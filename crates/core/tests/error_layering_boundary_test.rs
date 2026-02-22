use std::{fs, path::PathBuf};

fn read_file(path: &str) -> String {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = manifest_dir.join(path);

    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read file {}: {error}", path.display()))
}

#[test]
fn core_boundary_uses_thiserror_and_avoids_anyhow_miette() {
    let cargo_toml = read_file("Cargo.toml");
    let error_source = read_file("src/error.rs");
    let lib_source = read_file("src/lib.rs");
    let dialect_source = read_file("src/dialect.rs");
    let adapter_source = read_file("src/adapter.rs");

    assert!(
        cargo_toml.contains("thiserror"),
        "core crate must depend on `thiserror` for typed public errors",
    );
    assert!(
        !cargo_toml.contains("anyhow"),
        "core crate must not depend on `anyhow`",
    );
    assert!(
        !cargo_toml.contains("miette"),
        "core crate must not depend on `miette`",
    );

    assert!(
        error_source.contains("thiserror::Error"),
        "core error types must be declared with `thiserror::Error`",
    );
    assert!(
        !error_source.contains("anyhow"),
        "core error types must not reference `anyhow`",
    );
    assert!(
        !error_source.contains("miette"),
        "core error types must not reference `miette`",
    );

    for (name, source) in [
        ("src/lib.rs", lib_source),
        ("src/dialect.rs", dialect_source),
        ("src/adapter.rs", adapter_source),
    ] {
        assert!(
            !source.contains("anyhow::Error"),
            "{name} must not expose anyhow::Error in the public boundary",
        );
    }
}
