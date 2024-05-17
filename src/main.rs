use cargo_lock::Lockfile;
use clap::Parser;

use cargo_depcheck::{Deps, DupDepResults};

#[derive(Parser)]
#[command(bin_name = "cargo depcheck")]
#[command(
    version,
    about = "Check for duplicate dependencies in Cargo.lock",
    long_about = None,
    styles = clap_cargo::style::CLAP_STYLING
)]
struct CargoCli {
    /// Path to Cargo.lock
    #[arg(long, short)]
    lock_path: Option<std::path::PathBuf>,
}

fn load_and_process_lock_file(
    lock_path: Option<std::path::PathBuf>,
    verbose: bool,
) -> Result<DupDepResults, Box<dyn std::error::Error>> {
    let lock_path = lock_path.unwrap_or(std::path::PathBuf::from("Cargo.lock"));
    let lock_file = Lockfile::load(lock_path)?;
    let deps = Deps::from_lock_file(lock_file)?;
    let dup_dep_results = deps.build_dup_dep_results(verbose)?;
    Ok(dup_dep_results)
}

fn main() {
    let cli = CargoCli::parse();

    match load_and_process_lock_file(cli.lock_path, false) {
        Ok(dup_dep_results) => {
            println!("{dup_dep_results}");
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };
}
