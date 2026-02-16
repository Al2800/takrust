use std::env;
use std::fs;
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
    env: &'static [(&'static str, &'static str)],
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
        "hardening" => run_hardening(),
        "hardening-supply-chain" => run_hardening_supply_chain(),
        "hardening-loom" => run_hardening_loom(),
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
            env: &[],
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
            env: &[],
        },
        Step {
            name: "Workspace tests",
            program: "cargo",
            args: &["test", "--workspace"],
            env: &[],
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
            env: &[],
        }];
        return run_steps("fuzz-smoke", &fallback);
    }

    let steps = [Step {
        name: "List fuzz targets",
        program: "cargo",
        args: &["fuzz", "list"],
        env: &[],
    }];

    run_steps("fuzz-smoke", &steps)
}

fn run_release_check() -> Result<(), AppError> {
    let steps = [
        Step {
            name: "Workspace check",
            program: "cargo",
            args: &["check", "--workspace", "--all-targets"],
            env: &[],
        },
        Step {
            name: "All-feature tests",
            program: "cargo",
            args: &["test", "--workspace", "--all-features"],
            env: &[],
        },
        Step {
            name: "Documentation build",
            program: "cargo",
            args: &["doc", "--workspace", "--no-deps"],
            env: &[],
        },
    ];

    run_steps("release-check", &steps)
}

fn run_hardening() -> Result<(), AppError> {
    run_hardening_supply_chain()?;
    run_hardening_loom()
}

fn run_hardening_supply_chain() -> Result<(), AppError> {
    ensure_cargo_subcommand("deny", "cargo-deny")?;
    ensure_cargo_subcommand("audit", "cargo-audit")?;
    ensure_cargo_subcommand("vet", "cargo-vet")?;

    let steps = [
        Step {
            name: "cargo-deny policy checks",
            program: "cargo",
            args: &["deny", "check", "--config", "deny.toml"],
            env: &[],
        },
        Step {
            name: "cargo-audit vulnerability scan",
            program: "cargo",
            args: &["audit"],
            env: &[],
        },
        Step {
            name: "cargo-vet policy check",
            program: "cargo",
            args: &["vet"],
            env: &[],
        },
    ];

    run_steps("hardening-supply-chain", &steps)
}

fn run_hardening_loom() -> Result<(), AppError> {
    let steps = if workspace_has_loom_markers()? {
        vec![Step {
            name: "Workspace check under cfg(loom)",
            program: "cargo",
            args: &["check", "--workspace", "--all-targets"],
            env: &[("RUSTFLAGS", "--cfg loom")],
        }]
    } else {
        println!(
            "No loom markers detected in workspace sources; running baseline workspace check as loom smoke fallback."
        );
        vec![Step {
            name: "Workspace check (loom fallback)",
            program: "cargo",
            args: &["check", "--workspace", "--all-targets"],
            env: &[],
        }]
    };

    run_steps("hardening-loom", &steps)
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
    let env_string = if step.env.is_empty() {
        String::new()
    } else {
        format!(
            "{} ",
            step.env
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect::<Vec<_>>()
                .join(" ")
        )
    };

    println!(
        "-> {}: {}{} {}",
        step.name,
        env_string,
        step.program,
        step.args.join(" ")
    );

    let mut command = Command::new(step.program);
    command.args(step.args).envs(step.env.iter().copied());

    let status = command.status().map_err(|error| {
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

fn ensure_cargo_subcommand(
    subcommand: &'static str,
    install_package: &'static str,
) -> Result<(), AppError> {
    let output = Command::new("cargo")
        .args([subcommand, "--version"])
        .output()
        .map_err(|error| {
            AppError::command(format!(
                "Failed to probe `cargo {subcommand}` availability: {error}"
            ))
        })?;

    if output.status.success() {
        return Ok(());
    }

    Err(AppError::command(format!(
        "`cargo {subcommand}` is required for this hardening check; install it with `cargo install {install_package}`"
    )))
}

fn workspace_has_loom_markers() -> Result<bool, AppError> {
    const ROOTS: [&str; 4] = ["crates", "tests", "examples", "fuzz"];
    for root in ROOTS {
        let root_path = Path::new(root);
        if !root_path.exists() {
            continue;
        }
        if directory_has_loom_markers(root_path)? {
            return Ok(true);
        }
    }

    Ok(false)
}

fn directory_has_loom_markers(path: &Path) -> Result<bool, AppError> {
    let entries = fs::read_dir(path).map_err(|error| {
        AppError::command(format!(
            "Failed to scan `{}` for loom markers: {error}",
            path.display()
        ))
    })?;

    for entry in entries {
        let entry = entry.map_err(|error| {
            AppError::command(format!(
                "Failed to read entry under `{}`: {error}",
                path.display()
            ))
        })?;
        let entry_path = entry.path();

        if entry_path.is_dir() {
            if entry
                .file_name()
                .to_str()
                .map(|name| name == "target" || name.starts_with('.'))
                .unwrap_or(false)
            {
                continue;
            }
            if directory_has_loom_markers(&entry_path)? {
                return Ok(true);
            }
            continue;
        }

        if entry_path.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }

        let contents = fs::read(&entry_path).map_err(|error| {
            AppError::command(format!(
                "Failed to read `{}` while scanning loom markers: {error}",
                entry_path.display()
            ))
        })?;
        let text = String::from_utf8_lossy(&contents);
        if text.contains("#[cfg(loom")
            || text.contains("cfg!(loom")
            || text.contains("loom::model")
            || text.contains("loom::sync")
        {
            return Ok(true);
        }
    }

    Ok(false)
}

fn usage() -> &'static str {
    "Usage: cargo run -p xtask -- <subcommand>\n\nSubcommands:\n  ci                      Run fmt, clippy, and workspace tests\n  fuzz-smoke              Run cargo-fuzz target listing (or fallback workspace smoke check)\n  release-check           Run workspace check, all-feature tests, and docs build\n  hardening               Run supply-chain + loom smoke checks\n  hardening-supply-chain  Run cargo-deny/audit/vet checks\n  hardening-loom          Run workspace check under cfg(loom)\n  help                    Print this help\n\nExit codes:\n  0  Success\n  1  Command execution failure\n  2  Usage error"
}
