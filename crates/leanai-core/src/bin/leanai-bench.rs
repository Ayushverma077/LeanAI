//! Benchmark runner: `leanai-bench <suite.json> [--json out.json] [--markdown out.md]`
//!
//! The suite file is a JSON array of `BenchmarkTask`. Relative repository paths
//! resolve against the suite file's directory.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use leanai_core::benchmark::{self, BenchmarkTask};
use leanai_core::policy::Policy;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(suite_path) = args.next() else {
        eprintln!("usage: leanai-bench <suite.json> [--json <path>] [--markdown <path>]");
        return ExitCode::from(2);
    };

    let mut json_out: Option<PathBuf> = None;
    let mut markdown_out: Option<PathBuf> = None;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--json" => json_out = args.next().map(PathBuf::from),
            "--markdown" => markdown_out = args.next().map(PathBuf::from),
            other => {
                eprintln!("unknown argument `{other}`");
                return ExitCode::from(2);
            }
        }
    }

    let suite_path = Path::new(&suite_path);
    let raw = match std::fs::read_to_string(suite_path) {
        Ok(raw) => raw,
        Err(error) => {
            eprintln!("could not read {}: {error}", suite_path.display());
            return ExitCode::FAILURE;
        }
    };
    let mut tasks: Vec<BenchmarkTask> = match serde_json::from_str(&raw) {
        Ok(tasks) => tasks,
        Err(error) => {
            eprintln!(
                "{} is not a valid task suite: {error}",
                suite_path.display()
            );
            return ExitCode::FAILURE;
        }
    };

    let base = suite_path.parent().unwrap_or(Path::new("."));
    for task in &mut tasks {
        if task.repository.is_relative() {
            task.repository = base.join(&task.repository);
        }
    }

    let report = benchmark::run_suite(&tasks, &Policy::default());
    let markdown = benchmark::render_markdown(&report);

    if let Some(path) = markdown_out {
        if let Err(error) = std::fs::write(&path, &markdown) {
            eprintln!("could not write {}: {error}", path.display());
            return ExitCode::FAILURE;
        }
    } else {
        println!("{markdown}");
    }

    if let Some(path) = json_out {
        match serde_json::to_string_pretty(&report) {
            Ok(json) => {
                if let Err(error) = std::fs::write(&path, json) {
                    eprintln!("could not write {}: {error}", path.display());
                    return ExitCode::FAILURE;
                }
            }
            Err(error) => {
                eprintln!("could not serialise the report: {error}");
                return ExitCode::FAILURE;
            }
        }
    }

    if report.tasks.is_empty() {
        eprintln!("no task completed successfully");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
