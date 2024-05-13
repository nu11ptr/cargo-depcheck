use cargo_lock::Lockfile;
use clap::Parser;

use cargo_depcheck::{Deps, DuplicateDep};

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
) -> Result<Vec<DuplicateDep>, Box<dyn std::error::Error>> {
    let lock_path = lock_path.unwrap_or(std::path::PathBuf::from("Cargo.lock"));
    let lock_file = Lockfile::load(lock_path)?;
    let deps = Deps::from_lock_file(lock_file)?;
    deps.duplicate_versions()
}

fn main() {
    let cli = CargoCli::parse();

    match load_and_process_lock_file(cli.lock_path) {
        Ok(dup_versions) if !dup_versions.is_empty() => {
            for dep in &dup_versions {
                println!("{}", dep);
            }

            println!(
                "Found {} package(s) with duplicate versions",
                dup_versions.len()
            );
            std::process::exit(1);
        }
        Ok(_) => {
            println!("No packages have duplicate versions");
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };
}
