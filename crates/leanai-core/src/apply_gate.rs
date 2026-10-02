//! Decides what happens to a validated change: apply it without review, ask
//! for the usual review, or ask for a careful review (ADR 0013).
//!
//! One switch is not enough, because the bar for acting without a person
//! depends on what a mistake would cost: a one-line label change and an edit
//! to a lockfile are not the same risk. Every input here is deterministic and
//! every decision carries its reasons, so the user can see why a change was
//! held back. Nothing in this module is a calibrated probability; the
//! thresholds are policy.
//!
//! The validator (`agent::validate_proposal`) runs first and already blocks
//! changes that must never be offered, such as a patch containing a
//! high-confidence secret. This gate only ranks changes that passed it.

use serde::{Deserialize, Serialize};

use crate::agent::{PatchProposal, ValidatorVerdict};
use crate::routing::{distribution_confidence, PromptAnalysis};

/// Most files an automatically applied change may touch.
pub const MAX_AUTO_FILES: usize = 3;
/// Most added plus deleted lines an automatically applied change may have.
pub const MAX_AUTO_CHANGED_LINES: usize = 60;
/// Prompt risk (0-10) from which a change always waits for review. Base risk
/// for a code change is 1.5; terms such as auth, session or payment raise it.
pub const CONFIRM_RISK: f64 = 3.0;
/// Prompt risk (0-10) from which a change needs a careful review, for example
/// requests that mention production data or bulk deletion.
pub const CAREFUL_RISK: f64 = 6.0;
/// Below this confidence that the request asked for a change at all, the
/// change always waits for review.
pub const MIN_CHANGE_CONFIDENCE: f64 = 0.5;

/// How much human attention a change gets before it is written. Ordered from
/// least to most.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplyLevel {
    /// Low risk, and the user allowed automatic apply.
    AutoApply,
    /// The usual review: diff shown, one click to apply or discard.
    Confirm,
    /// Review with the reasons shown first; never applied automatically.
    CarefulReview,
}

/// One reason a change was not applied automatically.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GateReason {
    pub level: ApplyLevel,
    pub text: String,
}

/// The gate's decision for one change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyDecision {
    pub level: ApplyLevel,
    /// Why the change was held back, most serious first. Empty when it was
    /// applied automatically, or when automatic apply is off and nothing else
    /// held it back.
    pub reasons: Vec<GateReason>,
}

/// Everything the gate looks at.
pub struct GateInput<'a> {
    /// The user's "apply low-risk changes automatically" setting.
    pub auto_apply_enabled: bool,
    pub analysis: &'a PromptAnalysis,
    pub proposal: &'a PatchProposal,
    pub verdict: &'a ValidatorVerdict,
    /// Replies or edits rejected during the run, plus times the model said a
    /// task was beyond it.
    pub corrections: usize,
    /// Files read during the run that contained text resembling instructions
    /// to the model.
    pub injection_warnings: usize,
}

/// Ranks a validated change.
pub fn decide(input: &GateInput) -> ApplyDecision {
    let mut reasons = Vec::new();
    let mut hold = |level: ApplyLevel, text: String| reasons.push(GateReason { level, text });

    // What the change touches.
    for patch in &input.proposal.patches {
        if patch.is_deleted {
            hold(
                ApplyLevel::CarefulReview,
                format!("Deletes {}.", patch.path),
            );
        }
        if let Some(kind) = consequential_path(&patch.path) {
            hold(
                ApplyLevel::CarefulReview,
                format!("Changes {} ({kind}).", patch.path),
            );
        }
    }
    let files = input.proposal.patches.len();
    if files > MAX_AUTO_FILES {
        hold(
            ApplyLevel::Confirm,
            format!("Changes {files} files (automatic apply allows at most {MAX_AUTO_FILES})."),
        );
    }
    let lines: usize = input
        .proposal
        .patches
        .iter()
        .map(|p| p.lines_added + p.lines_deleted)
        .sum();
    if lines > MAX_AUTO_CHANGED_LINES {
        hold(
            ApplyLevel::Confirm,
            format!(
                "Changes {lines} lines (automatic apply allows at most {MAX_AUTO_CHANGED_LINES})."
            ),
        );
    }

    // What the request asked for.
    let share = input.analysis.change_share;
    if share < 0.5 {
        hold(
            ApplyLevel::CarefulReview,
            "Your request reads as a question, but the model proposed a code change.".to_string(),
        );
    } else if distribution_confidence(share, 2) < MIN_CHANGE_CONFIDENCE {
        hold(
            ApplyLevel::Confirm,
            format!(
                "It is unclear whether your request asked for a change (confidence {:.0}%).",
                distribution_confidence(share, 2) * 100.0
            ),
        );
    }
    let risk = input.analysis.signals.risk;
    if risk >= CAREFUL_RISK {
        hold(
            ApplyLevel::CarefulReview,
            format!("Your request describes a high-risk change (risk {risk:.1}/10)."),
        );
    } else if risk >= CONFIRM_RISK {
        hold(
            ApplyLevel::Confirm,
            format!(
                "Your request touches sensitive areas such as auth, sessions or payments (risk {risk:.1}/10)."
            ),
        );
    }

    // How the run went.
    for warning in &input.verdict.warnings {
        hold(
            ApplyLevel::CarefulReview,
            format!("Validator warning: {warning}"),
        );
    }
    if input.injection_warnings > 0 {
        hold(
            ApplyLevel::CarefulReview,
            "Files read during this run contained text that looked like instructions to the model."
                .to_string(),
        );
    }
    if input.corrections > 0 {
        hold(
            ApplyLevel::Confirm,
            format!(
                "The model needed {} correction{} before producing this change.",
                input.corrections,
                if input.corrections == 1 { "" } else { "s" }
            ),
        );
    }

    reasons.sort_by_key(|r| std::cmp::Reverse(r.level));
    let level = match reasons.first() {
        Some(most_serious) => most_serious.level,
        None if input.auto_apply_enabled => ApplyLevel::AutoApply,
        None => ApplyLevel::Confirm,
    };
    ApplyDecision { level, reasons }
}

/// Why changing `path` matters beyond the file itself, if it does: build,
/// dependency, CI, deployment, migration and permission files.
pub fn consequential_path(path: &str) -> Option<&'static str> {
    const MANIFESTS: &[&str] = &[
        "cargo.toml",
        "package.json",
        "pyproject.toml",
        "setup.py",
        "setup.cfg",
        "pipfile",
        "go.mod",
        "go.sum",
        "gemfile",
        "composer.json",
        "pom.xml",
        "build.gradle",
        "build.gradle.kts",
        "settings.gradle",
        "settings.gradle.kts",
        "package.swift",
        "podfile",
        "build.rs",
        "makefile",
        ".npmrc",
    ];
    const CI_FILES: &[&str] = &[
        ".gitlab-ci.yml",
        "azure-pipelines.yml",
        "jenkinsfile",
        "bitbucket-pipelines.yml",
    ];
    const CI_DIRS: &[&str] = &[".github", ".circleci", ".buildkite", ".husky"];
    const INFRA_DIRS: &[&str] = &["terraform", "k8s", "kubernetes", "helm", "deploy", "infra"];
    const PERMISSION_FILES: &[&str] = &[".gitignore", ".aiignore", "tauri.conf.json"];

    let normalized = path.trim().trim_start_matches("./").replace('\\', "/");
    let lower = normalized.to_lowercase();
    let segments: Vec<&str> = lower.split('/').collect();
    let (dirs, name) = segments.split_at(segments.len().saturating_sub(1));
    let name = name.first().copied().unwrap_or_default();
    let in_dir = |candidates: &[&str]| dirs.iter().any(|d| candidates.contains(d));

    if crate::classify::is_lockfile(&normalized) {
        Some("dependency lockfile")
    } else if MANIFESTS.contains(&name)
        || (name.starts_with("requirements") && name.ends_with(".txt"))
        || name.ends_with(".csproj")
        || name.starts_with("dockerfile")
        || name.starts_with("docker-compose")
        || name.starts_with("vite.config.")
        || name.starts_with("webpack.config.")
        || in_dir(&[".cargo"])
    {
        Some("build or dependency configuration")
    } else if CI_FILES.contains(&name) || in_dir(CI_DIRS) {
        Some("CI configuration")
    } else if in_dir(INFRA_DIRS) || name.ends_with(".tf") || name.ends_with(".tfvars") {
        Some("deployment configuration")
    } else if in_dir(&["migrations", "migration"]) {
        Some("database migration")
    } else if PERMISSION_FILES.contains(&name) || in_dir(&["capabilities"]) {
        Some("ignore rules or app permissions")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::FilePatch;
    use crate::routing::analyze_prompt;

    fn patch(path: &str, added: usize) -> FilePatch {
        FilePatch {
            path: path.into(),
            unified_diff: "@@".into(),
            is_new_file: false,
            is_deleted: false,
            lines_added: added,
            lines_deleted: added,
        }
    }

    fn proposal(patches: Vec<FilePatch>) -> PatchProposal {
        PatchProposal {
            summary: "s".into(),
            rationale: String::new(),
            affected_files: patches.iter().map(|p| p.path.clone()).collect(),
            patches,
            sha256_hash: String::new(),
        }
    }

    fn clean_verdict() -> ValidatorVerdict {
        ValidatorVerdict {
            is_valid: true,
            checks: Vec::new(),
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    fn decide_for(prompt: &str, proposal: &PatchProposal, auto: bool) -> ApplyDecision {
        let analysis = analyze_prompt(prompt, None);
        decide(&GateInput {
            auto_apply_enabled: auto,
            analysis: &analysis,
            proposal,
            verdict: &clean_verdict(),
            corrections: 0,
            injection_warnings: 0,
        })
    }

    const EDIT: &str = "Change the Login button text to Sign In.";

    #[test]
    fn small_clean_change_applies_only_when_allowed() {
        let p = proposal(vec![patch("src/Login.tsx", 1)]);
        let on = decide_for(EDIT, &p, true);
        assert_eq!(on.level, ApplyLevel::AutoApply);
        assert!(on.reasons.is_empty());
        let off = decide_for(EDIT, &p, false);
        assert_eq!(off.level, ApplyLevel::Confirm);
        assert!(off.reasons.is_empty(), "the setting alone is not a reason");
    }

    #[test]
    fn size_limits_hold_changes_for_review() {
        let many = proposal((0..4).map(|i| patch(&format!("src/f{i}.ts"), 1)).collect());
        assert_eq!(decide_for(EDIT, &many, true).level, ApplyLevel::Confirm);
        let long = proposal(vec![patch("src/a.ts", 31)]);
        let decision = decide_for(EDIT, &long, true);
        assert_eq!(decision.level, ApplyLevel::Confirm);
        assert!(decision.reasons[0].text.contains("62 lines"));
    }

    #[test]
    fn build_ci_and_permission_files_need_careful_review() {
        for path in [
            "Cargo.lock",
            "package.json",
            "src-tauri/Cargo.toml",
            ".github/workflows/ci.yml",
            "db/migrations/0004_users.sql",
            "src-tauri/capabilities/default.json",
            ".aiignore",
            "infra/main.tf",
        ] {
            let d = decide_for(EDIT, &proposal(vec![patch(path, 1)]), true);
            assert_eq!(d.level, ApplyLevel::CarefulReview, "{path}");
        }
        assert_eq!(consequential_path("src/components/Login.tsx"), None);
        assert_eq!(consequential_path("docs/deployment-notes.md"), None);
    }

    #[test]
    fn request_intent_and_risk_scale_the_bar() {
        let p = proposal(vec![patch("src/a.ts", 1)]);
        let question = decide_for("Explain how recursion works.", &p, true);
        assert_eq!(question.level, ApplyLevel::CarefulReview);
        let sensitive = decide_for("Refactor the session store to use a Map", &p, true);
        assert_eq!(sensitive.level, ApplyLevel::Confirm);
        let dangerous = decide_for(
            "Migrate existing users to the new auth table in production",
            &p,
            true,
        );
        assert_eq!(dangerous.level, ApplyLevel::CarefulReview);
    }

    #[test]
    fn run_history_holds_changes_and_reasons_are_ordered() {
        let p = proposal(vec![patch("package.json", 1)]);
        let analysis = analyze_prompt(EDIT, None);
        let mut verdict = clean_verdict();
        verdict
            .warnings
            .push("Medium-risk secret finding in patch for package.json".into());
        let d = decide(&GateInput {
            auto_apply_enabled: true,
            analysis: &analysis,
            proposal: &p,
            verdict: &verdict,
            corrections: 2,
            injection_warnings: 1,
        });
        assert_eq!(d.level, ApplyLevel::CarefulReview);
        assert_eq!(d.reasons.len(), 4);
        assert!(d.reasons.windows(2).all(|w| w[0].level >= w[1].level));
        assert!(d.reasons.last().unwrap().text.contains("2 corrections"));
    }
}
