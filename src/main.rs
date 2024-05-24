use anstream::println;
use cargo_depcheck::{BlameMode, Deps, MultiVerDepParents, MultiVerDepResults, MultiVerDeps};
use cargo_lock::Lockfile;
use clap::Parser;

// TODO: Make this different sizes based on collection size?
const BUFFER_SIZE: usize = 32768;

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

    /// Display packages that are to blame for multi version dependencies
    #[arg(long, short, value_enum)]
    blame: Option<BlameMode>,

    /// Display the multi version dependency names that each package is responsible for
    #[arg(long, short = 'd')]
    blame_detail: bool,
}

fn load_and_process_lock_file(
    cli: &CargoCli,
) -> Result<(MultiVerDepResults, String), Box<dyn std::error::Error>> {
    let lock_path = cli
        .lock_path
        .clone()
        .unwrap_or(std::path::PathBuf::from("Cargo.lock"));
    let lock_file = Lockfile::load(lock_path)?;

    let deps = Deps::from_lock_file(lock_file)?;
    // Finding just duplicate packages with no other information is cheap, always do it
    let multi_ver_deps = MultiVerDeps::from_deps(&deps);

    // Only blame uses multi version parents, so don't build if we don't need to
    let multi_ver_parents = if cli.blame.is_some() {
        MultiVerDepParents::build(&deps, &multi_ver_deps)?
    } else {
        MultiVerDepParents::default()
    };

    let results = MultiVerDepResults::build(&deps, &multi_ver_parents, multi_ver_deps, cli.blame)?;

    let mut buffer = String::with_capacity(BUFFER_SIZE);
    results.render(&mut buffer, deps.count(), cli.blame, cli.blame_detail)?;

    Ok((results, buffer))
}

fn main() {
    let cli = CargoCli::parse();

    match load_and_process_lock_file(&cli) {
        Ok((dup_dep_results, buffer)) => {
            println!("{buffer}");

            if dup_dep_results.return_error(cli.blame) {
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };
}
