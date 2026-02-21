fn main() {
    let mut args = std::env::args().skip(1);

    match args.next().as_deref() {
        Some("postgres" | "mysql" | "sqlite" | "mssql") => {
            println!("stateql bootstrap CLI placeholder");
        }
        _ => {
            eprintln!("usage: stateql <postgres|mysql|sqlite|mssql>");
            std::process::exit(2);
        }
    }
}
