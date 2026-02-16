use std::env;
use std::path::Path;
use std::process::{Command, ExitCode};

#[derive(Debug)]
struct AppError {
    code: u8,
    message: String,
}

impl AppError {
    fn usage(message: impl Into<String>) -> Self {
        Self {
            code: 2,
            message: message.into(),
        }
    }

    fn command(message: impl Into<String>) -> Self {
        Self {
            code: 1,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Step {
    name: &'static str,
    program: &'static str,
    args: &'static [&'static str],
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{}", err.message);
            ExitCode::from(err.code)
        }
    }
}

fn run() -> Result<(), AppError> {
    let mut args = env::args().skip(1);
    let Some(subcommand) = args.next() else {
        return Err(AppError::usage(usage()));
    };

    if args.next().is_some() {
        return Err(AppError::usage(format!(
            "Unexpected extra arguments for `{subcommand}`.\n\n{}",
            usage()
        )));
    }

    match subcommand.as_str() {
        "ci" => run_ci(),
        "fuzz-smoke" => run_fuzz_smoke(),
        "release-check" => run_release_check(),
        "help" | "--help" | "-h" => {
            println!("{}", usage());
            Ok(())
        }
        _ => Err(AppError::usage(format!(
            "Unknown xtask subcommand `{subcommand}`.\n\n{}",
            usage()
        ))),
    }
}

fn run_ci() -> Result<(), AppError> {
    let steps = [
        Step {
            name: "Format check",
            program: "cargo",
            args: &["fmt", "--all", "--", "--check"],
        },
        Step {
            name: "Clippy lint",
            program: "cargo",
            args: &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--",
                "-D",
                "warnings",
            ],
        },
        Step {
            name: "Workspace tests",
            program: "cargo",
            args: &["test", "--workspace"],
        },
    ];

    run_steps("ci", &steps)
}

fn run_fuzz_smoke() -> Result<(), AppError> {
    if !Path::new("fuzz/Cargo.toml").exists() {
        println!("No fuzz workspace detected at `fuzz/Cargo.toml`; running baseline smoke check.");
        let fallback = [Step {
            name: "Workspace check",
            program: "cargo",
            args: &["check", "--workspace", "--all-targets"],
        }];
        return run_steps("fuzz-smoke", &fallback);
    }

    let steps = [Step {
        name: "List fuzz targets",
        program: "cargo",
        args: &["fuzz", "list"],
    }];

    run_steps("fuzz-smoke", &steps)
}

fn run_release_check() -> Result<(), AppError> {
    let steps = [
        Step {
            name: "Workspace check",
            program: "cargo",
            args: &["check", "--workspace", "--all-targets"],
        },
        Step {
            name: "All-feature tests",
            program: "cargo",
            args: &["test", "--workspace", "--all-features"],
        },
        Step {
            name: "Documentation build",
            program: "cargo",
            args: &["doc", "--workspace", "--no-deps"],
        },
    ];

    run_steps("release-check", &steps)
}

fn run_steps(name: &str, steps: &[Step]) -> Result<(), AppError> {
    println!("Running xtask `{name}` with {} step(s).", steps.len());
    for step in steps {
        run_step(step)?;
    }
    println!("xtask `{name}` completed successfully.");
    Ok(())
}

fn run_step(step: &Step) -> Result<(), AppError> {
    println!("-> {}: {} {}", step.name, step.program, step.args.join(" "));
    let status = Command::new(step.program)
        .args(step.args)
        .status()
        .map_err(|error| {
            AppError::command(format!(
                "Failed to launch step `{}` ({} {}): {}",
                step.name,
                step.program,
                step.args.join(" "),
                error
            ))
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(AppError::command(format!(
            "Step `{}` failed with status {}.",
            step.name, status
        )))
    }
}

fn usage() -> &'static str {
    "Usage: cargo run -p xtask -- <subcommand>\n\nSubcommands:\n  ci             Run fmt, clippy, and workspace tests\n  fuzz-smoke     Run cargo-fuzz target listing (or fallback workspace smoke check)\n  release-check  Run workspace check, all-feature tests, and docs build\n  help           Print this help\n\nExit codes:\n  0  Success\n  1  Command execution failure\n  2  Usage error"
}
