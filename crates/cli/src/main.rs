fn enabled_dialects() -> Vec<&'static str> {
    vec![
        #[cfg(feature = "mysql")]
        "mysql",
        #[cfg(feature = "postgres")]
        "postgres",
        #[cfg(feature = "sqlite")]
        "sqlite",
        #[cfg(feature = "mssql")]
        "mssql",
    ]
}

fn usage(enabled_dialects: &[&str]) -> String {
    if enabled_dialects.is_empty() {
        return "usage: stateql <no-dialects-enabled>".to_owned();
    }

    format!("usage: stateql <{}>", enabled_dialects.join("|"))
}

fn main() {
    let enabled_dialects = enabled_dialects();
    let selected_dialect = std::env::args().nth(1);

    if let Some(dialect) = selected_dialect.as_deref()
        && enabled_dialects.contains(&dialect)
    {
        println!("stateql bootstrap CLI placeholder");
        return;
    }

    eprintln!("{}", usage(&enabled_dialects));
    std::process::exit(2);
}
