use std::process::ExitCode;

use benchmark::cli::Cli;
use benchmark::commands::{self, Context};
use benchmark::failure::{EXIT_CANNOT_RUN, EXIT_USAGE};
use benchmark::out;
use clap::Parser;

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            let _ = error.print();
            // --help and --version print and succeed; a mistyped command line is a usage error.
            return ExitCode::from(if error.use_stderr() { EXIT_USAGE } else { 0 } as u8);
        }
    };

    let context = match Context::new() {
        Ok(context) => context,
        Err(error) => {
            out::log(format!("cannot run: {error}"));
            return ExitCode::from(EXIT_CANNOT_RUN as u8);
        }
    };

    match commands::dispatch(cli.command, &context) {
        Ok(code) => ExitCode::from(code as u8),
        Err(failure) => {
            out::log(&failure);
            ExitCode::from(failure.exit_code() as u8)
        }
    }
}
