use clap::Parser;
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = rustak_cli::Cli::parse();
    match rustak_cli::run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(error.exit_code())
        }
    }
}
