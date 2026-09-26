//! Context-strategy benchmark harness (backlog 5.7-5.9).
//!
//! # What this measures, and what it does not
//!
//! The harness compares four ways of assembling context for a task and reports
//! what can be measured deterministically on this machine:
//!
//! - how many files each strategy selects,
//! - how many tokens that costs (labelled as an OpenAI-family estimate),
//! - how long assembly takes,
//! - precision and recall against a human-declared set of relevant files.
//!
//! It deliberately does **not** report task success, validation pass rate,
//! retries, latency of a model call, or money. Those need a real provider and a
//! real task run, and Instructions.md §12.2 is explicit that a lower token
//! count is not a win on its own. Every report carries that caveat, so the
//! numbers cannot be quoted as a savings claim.

use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::concat::{self, BundleOptions};
use crate::context::{self, GenerateOptions};
use crate::error::Result;
use crate::inventory::Inventory;
use crate::policy::Policy;
use crate::selection::{self, SelectionSpec};
use crate::tokenizer;
use crate::walker::{self, CancelToken, ScanOptions};

/// One benchmark task: a repository plus the files a human says are needed.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkTask {
    pub id: String,
    /// Path to the repository, absolute or relative to the suite file.
    pub repository: PathBuf,
    /// What a developer is trying to do. Recorded for the report only.
    pub task: String,
    /// Task class from Instructions §9.1, e.g. `multi_file_planning`.
    pub task_class: String,
    /// Files a human judged necessary to do the task. Ground truth for
    /// precision and recall.
    pub relevant_files: Vec<String>,
    /// Whether this repository could plausibly be in a model's training data.
    /// Public repositories make repository-reasoning results unreliable
    /// (Instructions §12.2).
    pub leakage_risk: LeakageRisk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeakageRisk {
    /// Private or newly written; safe for repository-reasoning evaluation.
    Controlled,
    /// Public and old enough to be in a training set.
    Public,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Strategy {
    /// Every selectable file in the repository.
    RawAll,
    /// Only the human-declared relevant files.
    UserPinned,
    /// The deterministic context index, with no file bodies.
    DeterministicMap,
    /// The context index plus the pinned file bodies.
    Hybrid,
}

impl Strategy {
    pub const ALL: [Strategy; 4] = [
        Strategy::RawAll,
        Strategy::UserPinned,
        Strategy::DeterministicMap,
        Strategy::Hybrid,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Strategy::RawAll => "raw (all files)",
            Strategy::UserPinned => "user-pinned",
            Strategy::DeterministicMap => "repo map only",
            Strategy::Hybrid => "LeanAI hybrid",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategyResult {
    pub strategy: Strategy,
    pub files_included: usize,
    pub bytes: u64,
    pub tokens: usize,
    /// Always an estimate here; see the module docs.
    pub token_estimate_kind: String,
    pub assembly_ms: u64,
    /// Fraction of included files that are in the ground-truth set.
    pub precision: Option<f64>,
    /// Fraction of ground-truth files that were included.
    pub recall: Option<f64>,
    /// Ground-truth files this strategy left out.
    pub missed_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskReport {
    pub task: BenchmarkTask,
    pub source_revision: String,
    pub files_scanned: usize,
    pub selectable_files: usize,
    pub results: Vec<StrategyResult>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkReport {
    pub generated_at_ms: u64,
    pub machine: MachineProfile,
    pub tasks: Vec<TaskReport>,
    pub not_measured: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineProfile {
    pub os: String,
    pub arch: String,
    pub cpus: usize,
    pub policy_version: u32,
    pub tokenizer: String,
}

fn machine_profile() -> MachineProfile {
    MachineProfile {
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        cpus: std::thread::available_parallelism()
            .map(|value| value.get())
            .unwrap_or(0),
        policy_version: crate::policy::POLICY_VERSION,
        tokenizer: "cl100k_base".to_string(),
    }
}

/// Everything this harness cannot determine without a real provider run.
/// Reproduced verbatim in the report so the numbers are never quoted alone.
pub const NOT_MEASURED: &[&str] = &[
    "Task success: whether a model actually completed the task with this context.",
    "Validation and test pass rate after the model's proposal.",
    "Retries, tool calls and total wall-clock time for the whole task.",
    "Billed input, output and cached tokens, and therefore money.",
    "Provider-exact token counts: every number here is a local OpenAI-family estimate.",
    "Answer quality, correctness and the human correction time afterwards.",
];

/// Runs every strategy against one task.
pub fn run_task(task: &BenchmarkTask, policy: &Policy) -> Result<TaskReport> {
    let root = crate::project::canonical_root(&task.repository)?;
    let inventory = walker::scan(
        &root,
        &ScanOptions {
            policy: policy.clone(),
            hash_contents: true,
            progress_every: 0,
        },
        &CancelToken::new(),
        |_| {},
    )?;

    let mut notes = Vec::new();
    if task.leakage_risk != LeakageRisk::Controlled {
        notes.push(
            "This repository may be in a model's training data, so repository-reasoning results from it are not trustworthy on their own.".to_string(),
        );
    }
    let missing_ground_truth: Vec<&String> = task
        .relevant_files
        .iter()
        .filter(|path| inventory.get(path).is_none())
        .collect();
    if !missing_ground_truth.is_empty() {
        notes.push(format!(
            "{} ground-truth file(s) are not present in this revision: {}",
            missing_ground_truth.len(),
            missing_ground_truth
                .iter()
                .map(|path| path.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    let mut results = Vec::new();
    for strategy in Strategy::ALL {
        results.push(run_strategy(&root, &inventory, task, policy, strategy)?);
    }

    Ok(TaskReport {
        task: task.clone(),
        source_revision: inventory.source_revision.clone(),
        files_scanned: inventory.files.len(),
        selectable_files: inventory.selectable().count(),
        results,
        notes,
    })
}

fn run_strategy(
    root: &Path,
    inventory: &Inventory,
    task: &BenchmarkTask,
    policy: &Policy,
    strategy: Strategy,
) -> Result<StrategyResult> {
    let started = Instant::now();
    let options = BundleOptions {
        include_tree: true,
        ..Default::default()
    };

    let (text, files): (String, Vec<String>) = match strategy {
        Strategy::RawAll => {
            let spec = SelectionSpec {
                directories: vec![String::new()],
                ..Default::default()
            };
            let resolved = selection::resolve(inventory, &spec, policy)?;
            let bundle = concat::build(root, inventory, &resolved, &options)?;
            (bundle.text, resolved.files)
        }
        Strategy::UserPinned => {
            let spec = SelectionSpec::from_files(task.relevant_files.clone());
            let resolved = selection::resolve(inventory, &spec, policy)?;
            let bundle = concat::build(root, inventory, &resolved, &options)?;
            (bundle.text, resolved.files)
        }
        Strategy::DeterministicMap => {
            let document = context::generate(root, inventory, &GenerateOptions::default())?;
            (context::render(&document), Vec::new())
        }
        Strategy::Hybrid => {
            // The shipped composition order from Instructions §8.4: project map
            // first, then the source files the task needs.
            let document = context::generate(root, inventory, &GenerateOptions::default())?;
            let spec = SelectionSpec::from_files(task.relevant_files.clone());
            let resolved = selection::resolve(inventory, &spec, policy)?;
            let bundle = concat::build(root, inventory, &resolved, &options)?;
            (
                format!("{}\n{}", context::render(&document), bundle.text),
                resolved.files,
            )
        }
    };

    let estimate = tokenizer::estimate(&text);
    let ground_truth: std::collections::BTreeSet<&str> =
        task.relevant_files.iter().map(String::as_str).collect();
    let included: std::collections::BTreeSet<&str> = files.iter().map(String::as_str).collect();

    // A map-only strategy carries no file bodies, so file-level precision and
    // recall do not apply to it; reporting 0 would be misleading.
    let scored = strategy != Strategy::DeterministicMap;
    let hits = included.intersection(&ground_truth).count();

    Ok(StrategyResult {
        strategy,
        files_included: files.len(),
        bytes: text.len() as u64,
        tokens: estimate.value,
        token_estimate_kind: estimate.kind.label(),
        assembly_ms: started.elapsed().as_millis() as u64,
        precision: (scored && !included.is_empty()).then(|| hits as f64 / included.len() as f64),
        recall: (scored && !ground_truth.is_empty())
            .then(|| hits as f64 / ground_truth.len() as f64),
        missed_files: if scored {
            ground_truth
                .difference(&included)
                .map(|path| (*path).to_string())
                .collect()
        } else {
            Vec::new()
        },
    })
}

pub fn run_suite(tasks: &[BenchmarkTask], policy: &Policy) -> BenchmarkReport {
    let mut reports = Vec::new();
    for task in tasks {
        match run_task(task, policy) {
            Ok(report) => reports.push(report),
            Err(error) => eprintln!("task `{}` failed: {error}", task.id),
        }
    }
    BenchmarkReport {
        generated_at_ms: crate::project::now_ms(),
        machine: machine_profile(),
        tasks: reports,
        not_measured: NOT_MEASURED.iter().map(|note| note.to_string()).collect(),
    }
}

/// Renders the report as Markdown, caveats included.
pub fn render_markdown(report: &BenchmarkReport) -> String {
    let mut out = String::new();
    out.push_str("# LeanAI context-strategy benchmark\n\n");
    out.push_str(&format!(
        "Machine: {} / {} · {} cores · policy v{} · tokenizer `{}`\n\n",
        report.machine.os,
        report.machine.arch,
        report.machine.cpus,
        report.machine.policy_version,
        report.machine.tokenizer
    ));
    out.push_str("> **Read this before quoting any number.** Every token figure below is a local `cl100k_base` estimate, which is an OpenAI-family count and not what another provider bills. A smaller context is not a result on its own: a strategy only wins if it holds or improves task quality at acceptable total cost and latency, and this harness does not measure quality.\n\n");

    for task in &report.tasks {
        out.push_str(&format!("## {} — {}\n\n", task.task.id, task.task.task));
        out.push_str(&format!(
            "Class `{}` · revision `{}` · {} files scanned, {} selectable · leakage risk: {:?}\n\n",
            task.task.task_class,
            task.source_revision,
            task.files_scanned,
            task.selectable_files,
            task.task.leakage_risk
        ));
        out.push_str(
            "| Strategy | Files | Tokens (est.) | Bytes | Assembly | Precision | Recall |\n",
        );
        out.push_str("| --- | ---: | ---: | ---: | ---: | ---: | ---: |\n");
        for result in &task.results {
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} ms | {} | {} |\n",
                result.strategy.label(),
                result.files_included,
                result.tokens,
                result.bytes,
                result.assembly_ms,
                fraction(result.precision),
                fraction(result.recall),
            ));
        }
        out.push('\n');
        for result in &task.results {
            if !result.missed_files.is_empty() {
                out.push_str(&format!(
                    "- `{}` missed {} relevant file(s): {}\n",
                    result.strategy.label(),
                    result.missed_files.len(),
                    result.missed_files.join(", ")
                ));
            }
        }
        for note in &task.notes {
            out.push_str(&format!("- ⚠ {note}\n"));
        }
        out.push('\n');
    }

    out.push_str("## Not measured here\n\n");
    for note in &report.not_measured {
        out.push_str(&format!("- {note}\n"));
    }
    out.push_str("\nUntil these are measured against a real provider on controlled repositories, LeanAI must not publish a token- or cost-savings percentage (Instructions.md §2, N.3).\n");
    out
}

fn fraction(value: Option<f64>) -> String {
    match value {
        Some(value) => format!("{:.0}%", value * 100.0),
        None => "n/a".to_string(),
    }
}
