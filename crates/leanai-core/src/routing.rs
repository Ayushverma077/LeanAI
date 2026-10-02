use serde::{Deserialize, Serialize};

use crate::catalog::PriceCatalog;
use crate::error::{CoreError, Result};

/// Task classifications matching the routing policy in Instructions.md §9.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskClass {
    FileInventory,
    ContextDocumentation,
    MultiFilePlanning,
    CodeChangeProposal,
    MechanicalValidation,
}

/// User-visible routing policy configuration (ADR 0008).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingPolicy {
    pub low_cost_model_id: String,
    pub strong_model_id: String,
    pub auto_escalate_on_context_exceeded: bool,
    pub max_budget_usd: Option<f64>,
    pub require_local_only: bool,
}

impl Default for RoutingPolicy {
    fn default() -> Self {
        Self {
            low_cost_model_id: "openai/gpt-4o-mini".to_string(),
            strong_model_id: "openai/gpt-4o".to_string(),
            auto_escalate_on_context_exceeded: true,
            max_budget_usd: None,
            require_local_only: false,
        }
    }
}

/// Rationale for why a particular model was chosen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutingReason {
    TaskDefaultLowCost,
    TaskRequiresStrongReasoning,
    EscalatedContextExceededLowCostCap,
    LocalOnlyEnforced,
}

/// The auditable routing decision produced for a task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingDecision {
    pub task_class: TaskClass,
    pub selected_model_id: String,
    pub selected_provider: String,
    pub reason: RoutingReason,
    pub explanation: String,
    pub estimated_cost_usd: Option<f64>,
}

/// Evaluates routing policy deterministically against task requirements and context size.
pub fn route_task(
    task_class: TaskClass,
    context_tokens: usize,
    policy: &RoutingPolicy,
    catalog: &PriceCatalog,
) -> Result<RoutingDecision> {
    if policy.require_local_only {
        let local_entry = catalog
            .entries
            .iter()
            .find(|e| e.provider == "local")
            .ok_or_else(|| {
                CoreError::InvalidModel("No local model available in catalog".to_string())
            })?;

        if context_tokens > local_entry.context_cap {
            return Err(CoreError::LimitExceeded(format!(
                "Context size ({} tokens) exceeds local model capacity ({} tokens)",
                context_tokens, local_entry.context_cap
            )));
        }

        return Ok(RoutingDecision {
            task_class,
            selected_model_id: local_entry.model_id.clone(),
            selected_provider: local_entry.provider.clone(),
            reason: RoutingReason::LocalOnlyEnforced,
            explanation: "Policy requires local-only inference; selected local GGUF model."
                .to_string(),
            estimated_cost_usd: Some(0.0),
        });
    }

    let default_strong = match task_class {
        TaskClass::MultiFilePlanning | TaskClass::CodeChangeProposal => true,
        TaskClass::FileInventory
        | TaskClass::ContextDocumentation
        | TaskClass::MechanicalValidation => false,
    };

    let (chosen_model_id, reason, explanation) = if default_strong {
        (
            policy.strong_model_id.clone(),
            RoutingReason::TaskRequiresStrongReasoning,
            "Complex task requires strong reasoning model tier per policy.".to_string(),
        )
    } else {
        let low_entry = catalog.get(&policy.low_cost_model_id).ok_or_else(|| {
            CoreError::InvalidModel(format!(
                "Configured low-cost model `{}` not in catalog",
                policy.low_cost_model_id
            ))
        })?;

        if context_tokens > low_entry.context_cap {
            if policy.auto_escalate_on_context_exceeded {
                (
                    policy.strong_model_id.clone(),
                    RoutingReason::EscalatedContextExceededLowCostCap,
                    format!(
                        "Context ({} tokens) exceeds low-cost tier cap ({} tokens); escalated to strong tier.",
                        context_tokens, low_entry.context_cap
                    ),
                )
            } else {
                return Err(CoreError::LimitExceeded(format!(
                    "Context ({} tokens) exceeds low-cost tier limit ({} tokens)",
                    context_tokens, low_entry.context_cap
                )));
            }
        } else {
            (
                policy.low_cost_model_id.clone(),
                RoutingReason::TaskDefaultLowCost,
                "Standard task routed to default low-cost tier.".to_string(),
            )
        }
    };

    let entry = catalog.get(&chosen_model_id).ok_or_else(|| {
        CoreError::InvalidModel(format!("Selected model `{chosen_model_id}` not in catalog"))
    })?;

    if context_tokens > entry.context_cap {
        return Err(CoreError::LimitExceeded(format!(
            "Context ({} tokens) exceeds selected model context cap ({} tokens)",
            context_tokens, entry.context_cap
        )));
    }

    let estimated_cost = catalog.estimate_cost(&chosen_model_id, context_tokens, 500, 0);

    if let (Some(max_budget), Some(cost)) = (policy.max_budget_usd, estimated_cost) {
        if cost > max_budget {
            return Err(CoreError::BudgetExceeded(format!(
                "Estimated task cost (${cost:.4}) exceeds maximum configured budget (${max_budget:.4})"
            )));
        }
    }

    Ok(RoutingDecision {
        task_class,
        selected_model_id: entry.model_id.clone(),
        selected_provider: entry.provider.clone(),
        reason,
        explanation,
        estimated_cost_usd: estimated_cost,
    })
}

// ---------------------------------------------------------------------------
// Prompt routing: intent, complexity, context requirement, capabilities,
// context budget and minimum-sufficient model selection.
//
// Everything below is local and deterministic: routing a prompt costs zero
// provider tokens, and every number it produces can be explained from the
// `signals` it records. `route_task` above remains the task-class router used
// by the guided agent flow.
// ---------------------------------------------------------------------------

use std::collections::BTreeSet;

use crate::catalog::{CatalogEntry, ModelStrengths, ModelTier};

/// What the user is asking for. Kept open-ended: new intents are added by
/// extending [`INTENT_CUES`] rather than by branching on literal prompts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Intent {
    CodeEdit,
    BugFix,
    Refactor,
    Explanation,
    CreativeWriting,
    GeneralQa,
    Research,
    Summarization,
    Architecture,
    Other,
}

impl Intent {
    const ALL: [Intent; 10] = [
        Intent::CodeEdit,
        Intent::BugFix,
        Intent::Refactor,
        Intent::Explanation,
        Intent::CreativeWriting,
        Intent::GeneralQa,
        Intent::Research,
        Intent::Summarization,
        Intent::Architecture,
        Intent::Other,
    ];

    /// Whether satisfying this intent means changing repository files.
    pub fn modifies_code(&self) -> bool {
        matches!(self, Intent::CodeEdit | Intent::BugFix | Intent::Refactor)
    }

    /// Repository dependency before any prompt-specific evidence (0-10).
    fn base_repository_dependency(&self) -> f64 {
        match self {
            Intent::CodeEdit | Intent::BugFix | Intent::Refactor => 5.0,
            Intent::Architecture => 2.0,
            Intent::Explanation | Intent::Summarization => 0.5,
            Intent::Research | Intent::GeneralQa | Intent::Other => 0.5,
            Intent::CreativeWriting => 0.0,
        }
    }

    /// Reasoning difficulty before any prompt-specific evidence (0-10).
    fn base_reasoning(&self) -> f64 {
        match self {
            Intent::Architecture => 6.0,
            Intent::BugFix | Intent::Refactor | Intent::Research => 4.5,
            Intent::CreativeWriting => 3.0,
            Intent::CodeEdit => 2.0,
            Intent::Explanation | Intent::Summarization | Intent::GeneralQa => 2.0,
            Intent::Other => 2.5,
        }
    }
}

/// A weighted cue for intent detection. A cue is a word stem or a short
/// phrase; each intent's score is the sum of the weights of its cues found in
/// the prompt, plus structural evidence (questions, code artifacts, project
/// vocabulary). Deterministic, cheap and inspectable.
struct IntentCue {
    intent: Intent,
    weight: f64,
    phrases: &'static [&'static str],
}

const INTENT_CUES: &[IntentCue] = &[
    IntentCue {
        intent: Intent::CreativeWriting,
        weight: 3.0,
        phrases: &[
            "story",
            "poem",
            "poetry",
            "fiction",
            "novel",
            "narrative",
            "screenplay",
            "lyrics",
            "limerick",
            "sonnet",
            "fairy tale",
            "short story",
            "sci-fi",
            "fantasy",
            "plot",
            "protagonist",
            "creative",
            "haiku about",
        ],
    },
    IntentCue {
        intent: Intent::Explanation,
        weight: 2.0,
        phrases: &[
            "explain",
            "what is",
            "what are",
            "what does",
            "how does",
            "how do",
            "why does",
            "why is",
            "meaning of",
            "difference between",
            "understand",
            "describe",
            "teach me",
            "walk me through",
            "eli5",
        ],
    },
    IntentCue {
        intent: Intent::Summarization,
        weight: 3.0,
        phrases: &[
            "summarize",
            "summarise",
            "summary",
            "tl;dr",
            "tldr",
            "recap",
            "condense",
        ],
    },
    IntentCue {
        intent: Intent::Research,
        weight: 2.0,
        phrases: &[
            "research",
            "compare",
            "comparison",
            "investigate",
            "survey",
            "pros and cons",
            "alternatives",
            "evaluate",
            "which is better",
            "state of the art",
        ],
    },
    IntentCue {
        intent: Intent::Architecture,
        weight: 2.5,
        phrases: &[
            "architecture",
            "architect",
            "system design",
            "design a",
            "design the",
            "scalable",
            "microservice",
            "data model",
            "trade-off",
            "tradeoff",
            "how should we structure",
            "high-level design",
        ],
    },
    IntentCue {
        intent: Intent::BugFix,
        weight: 3.0,
        phrases: &[
            "fix",
            "bug",
            "broken",
            "crash",
            "error",
            "exception",
            "failing",
            "fails",
            "not working",
            "doesn't work",
            "does not work",
            "regression",
            "panic",
            "debug",
            "stack trace",
            "incorrect",
            "wrong result",
        ],
    },
    IntentCue {
        intent: Intent::Refactor,
        weight: 3.0,
        phrases: &[
            "refactor",
            "restructure",
            "migrate",
            "migration",
            "rewrite",
            "reorganize",
            "reorganise",
            "modularize",
            "decouple",
            "clean up",
            "cleanup",
            "extract into",
            "split into",
            "consolidate",
            "port to",
        ],
    },
    IntentCue {
        intent: Intent::CodeEdit,
        weight: 2.0,
        phrases: &[
            "change",
            "add",
            "update",
            "modify",
            "implement",
            "create",
            "remove",
            "delete",
            "replace",
            "rename",
            "make the",
            "set the",
            "increase",
            "decrease",
            "move the",
            "hide",
            "show the",
            "enable",
            "disable",
            "support for",
        ],
    },
    IntentCue {
        intent: Intent::CodeEdit,
        weight: 1.0,
        phrases: &[
            "button",
            "label",
            "component",
            "function",
            "method",
            "endpoint",
            "field",
            "column",
            "page",
            "screen",
            "modal",
            "css",
            "style",
            "color",
            "colour",
            "text",
            "placeholder",
            "tooltip",
            "header",
            "footer",
            "navbar",
        ],
    },
];

/// Words that indicate the prompt refers to *this* codebase rather than to a
/// topic in general.
const CODE_NOUNS: &[&str] = &[
    "button",
    "component",
    "function",
    "method",
    "class",
    "module",
    "file",
    "endpoint",
    "api",
    "route",
    "page",
    "screen",
    "modal",
    "form",
    "handler",
    "service",
    "controller",
    "model",
    "schema",
    "table",
    "query",
    "test",
    "tests",
    "config",
    "settings",
    "hook",
    "store",
    "state",
    "backend",
    "frontend",
    "server",
    "client",
    "database",
    "db",
    "ui",
    "codebase",
    "repo",
    "repository",
    "project",
    "app",
    "application",
    "bug",
    "login",
    "signup",
    "auth",
    "authentication",
    "session",
    "navbar",
    "sidebar",
    "header",
    "footer",
    "migration",
    "build",
    "pipeline",
    "command",
    "cli",
];

const DEICTIC: &[&str] = &[
    "the", "our", "my", "this", "that", "these", "its", "existing",
];

/// Architectural layers. Touching several of them is the strongest cheap
/// signal of cross-file scope.
const LAYERS: &[(&str, &[&str])] = &[
    (
        "frontend",
        &[
            "frontend",
            "ui",
            "button",
            "page",
            "component",
            "css",
            "react",
            "view",
            "screen",
            "front-end",
        ],
    ),
    (
        "backend",
        &[
            "backend",
            "server",
            "api",
            "apis",
            "endpoint",
            "endpoints",
            "handler",
            "back-end",
        ],
    ),
    (
        "data",
        &[
            "database",
            "db",
            "schema",
            "migration",
            "migrate",
            "table",
            "sql",
            "users",
            "records",
        ],
    ),
    (
        "auth",
        &[
            "auth",
            "authentication",
            "oauth",
            "login",
            "session",
            "jwt",
            "token",
            "sso",
            "password",
        ],
    ),
    ("tests", &["test", "tests", "testing", "spec", "coverage"]),
    (
        "build",
        &[
            "build",
            "ci",
            "pipeline",
            "deploy",
            "deployment",
            "docker",
            "release",
        ],
    ),
];

const DOMAIN_TERMS: &[(f64, &[&str])] = &[
    (
        2.0,
        &[
            "oauth",
            "jwt",
            "crypto",
            "encryption",
            "cryptography",
            "security",
            "authentication",
            "saml",
            "sso",
        ],
    ),
    (
        2.0,
        &[
            "concurrency",
            "concurrent",
            "thread",
            "threads",
            "race",
            "deadlock",
            "async",
            "lock",
            "mutex",
        ],
    ),
    (
        2.0,
        &[
            "distributed",
            "consensus",
            "replication",
            "kubernetes",
            "sharding",
        ],
    ),
    (
        1.5,
        &[
            "migration",
            "migrate",
            "schema",
            "transaction",
            "sql",
            "index",
            "database",
        ],
    ),
    (
        2.0,
        &[
            "compiler",
            "parser",
            "interpreter",
            "type system",
            "borrow checker",
        ],
    ),
    (1.5, &["payment", "billing", "invoice", "stripe"]),
    (
        1.5,
        &[
            "performance",
            "optimize",
            "optimise",
            "latency",
            "memory leak",
            "profiling",
        ],
    ),
];

const RISK_TERMS: &[(f64, &[&str])] = &[
    (
        3.0,
        &[
            "migrate existing",
            "existing users",
            "production",
            "prod data",
            "drop table",
            "wipe",
            "delete all",
            "remove all",
        ],
    ),
    (
        2.0,
        &[
            "auth",
            "authentication",
            "oauth",
            "password",
            "credential",
            "secret",
            "token",
            "permission",
            "encryption",
            "session",
        ],
    ),
    (
        1.5,
        &[
            "migration",
            "migrate",
            "payment",
            "billing",
            "delete",
            "security",
        ],
    ),
];

const HARD_REASONING: &[&str] = &[
    "why",
    "optimize",
    "optimise",
    "performance",
    "race",
    "deadlock",
    "intermittent",
    "algorithm",
    "prove",
    "tradeoff",
    "trade-off",
    "migrate",
    "migration",
    "root cause",
    "memory leak",
    "architecture",
    "security",
    "concurrency",
    "oauth",
];

const SCOPE_QUANTIFIERS: &[&str] = &[
    "all ",
    "every ",
    "entire",
    "whole ",
    "across",
    "everywhere",
    "codebase-wide",
    "project-wide",
    "throughout",
];

const ACTION_VERBS: &[&str] = &[
    "add",
    "change",
    "update",
    "fix",
    "refactor",
    "migrate",
    "remove",
    "delete",
    "create",
    "implement",
    "rename",
    "replace",
    "move",
    "write",
    "explain",
    "rewrite",
    "extract",
    "split",
    "convert",
    "port",
    "test",
    "document",
    "summarize",
    "compare",
    "design",
    "make",
    "set",
    "build",
    "deploy",
    "configure",
    "introduce",
    "support",
    "handle",
];

/// Distinctive names from the open project (file stems, directory names),
/// used to notice when a prompt refers to this repository.
#[derive(Debug, Clone, Default)]
pub struct ProjectVocabulary {
    terms: BTreeSet<String>,
}

impl ProjectVocabulary {
    /// Builds the vocabulary from project-relative paths.
    pub fn from_paths<'a>(paths: impl IntoIterator<Item = &'a str>) -> Self {
        const GENERIC: &[&str] = &[
            "src",
            "lib",
            "main",
            "index",
            "mod",
            "app",
            "test",
            "tests",
            "the",
            "and",
            "for",
            "with",
            "utils",
            "util",
            "types",
            "readme",
            "components",
            "pages",
            "docs",
            "public",
            "assets",
            "styles",
            "package",
            "config",
            "build",
            "dist",
            "bin",
        ];
        let mut terms = BTreeSet::new();
        for path in paths {
            for segment in path.split(['/', '\\']) {
                let stem = segment.split('.').next().unwrap_or(segment);
                for word in split_identifier(stem) {
                    if word.len() >= 4 && !GENERIC.contains(&word.as_str()) {
                        terms.insert(word);
                    }
                }
            }
        }
        Self { terms }
    }

    pub fn contains(&self, word: &str) -> bool {
        self.terms.contains(word)
    }

    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }
}

/// Splits `LoginButton`, `login_button` and `login-button` into lowercase words.
fn split_identifier(identifier: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut previous_lower = false;
    for ch in identifier.chars() {
        if ch == '_' || ch == '-' || ch == ' ' {
            if !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
            previous_lower = false;
            continue;
        }
        if ch.is_uppercase() && previous_lower && !current.is_empty() {
            words.push(std::mem::take(&mut current));
        }
        previous_lower = ch.is_lowercase() || ch.is_ascii_digit();
        current.extend(ch.to_lowercase());
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

/// The raw 0-10 signals behind every derived score.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptSignals {
    pub reasoning_difficulty: f64,
    pub scope: f64,
    pub repository_dependency: f64,
    pub domain_difficulty: f64,
    pub risk: f64,
    pub cross_file_dependency: f64,
    pub architecture_dependency: f64,
    pub project_knowledge: f64,
    pub operation_count: u32,
    pub layers: Vec<String>,
    pub code_artifacts: Vec<String>,
    pub project_terms: Vec<String>,
}

/// Result of analysing a prompt before any model is called.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptAnalysis {
    pub intent: Intent,
    /// How concentrated the intent evidence is, 0-1: 1 when every cue points
    /// at the winning intent, 0 when evidence is spread evenly over all
    /// intents. See [`distribution_confidence`]. A heuristic score from
    /// weighted cues, not a calibrated probability.
    pub intent_confidence: f64,
    /// Share of the intent evidence that points at changing code (edit, bug
    /// fix or refactor), 0-1. Used to notice a code change proposed for a
    /// request that reads as a question.
    pub change_share: f64,
    /// 0-10. How hard the task is.
    pub complexity: f64,
    /// 0-10. How much repository knowledge the task needs. Independent of
    /// complexity: a hard proof needs none, a one-word label change needs some.
    pub context_requirement: f64,
    pub signals: PromptSignals,
    /// Minimum model strengths this prompt needs.
    pub required: ModelStrengths,
    /// Human-readable capability names, strongest need first.
    pub required_capabilities: Vec<String>,
    pub explanation: Vec<String>,
}

/// Confidence of a choice among `options` whose winner holds share `top`
/// (0-1) of the evidence: `(options·top − 1) / (options − 1)`. 1 when all the
/// evidence is on one option, 0 when it is spread evenly. Unlike the raw
/// share, whose floor is `1/options`, the result is comparable across
/// choices with different numbers of options.
pub fn distribution_confidence(top: f64, options: usize) -> f64 {
    if options < 2 {
        return 1.0;
    }
    let n = options as f64;
    ((n * top.clamp(0.0, 1.0) - 1.0) / (n - 1.0)).clamp(0.0, 1.0)
}

fn clamp10(value: f64) -> f64 {
    (value.clamp(0.0, 10.0) * 10.0).round() / 10.0
}

fn contains_phrase(haystack: &str, phrase: &str) -> bool {
    // Phrases with a space or punctuation are matched as substrings; single
    // words match on word boundaries so "add" does not fire on "address".
    if phrase.contains(' ') || phrase.contains('-') || phrase.contains(';') {
        return haystack.contains(phrase);
    }
    haystack
        .split(|c: char| !c.is_alphanumeric() && c != '\'')
        .any(|word| word == phrase || (phrase.len() >= 4 && word.starts_with(phrase)))
}

/// Extracts things that look like code: paths, file names, identifiers in
/// backticks, `camelCase`/`snake_case` names and calls.
fn code_artifacts(prompt: &str) -> Vec<String> {
    const EXTENSIONS: &[&str] = &[
        ".rs", ".ts", ".tsx", ".js", ".jsx", ".py", ".go", ".java", ".kt", ".rb", ".php", ".cs",
        ".css", ".scss", ".html", ".json", ".toml", ".yaml", ".yml", ".sql", ".md", ".swift", ".c",
        ".cpp", ".h",
    ];
    let mut found = BTreeSet::new();
    for raw in prompt.split_whitespace() {
        let token = raw.trim_matches(|c: char| {
            matches!(c, ',' | ';' | ':' | '"' | '\'' | '(' | ')' | '?' | '!') || c == '.'
        });
        if token.is_empty() {
            continue;
        }
        let lower = token.to_lowercase();
        let is_path = token.contains('/') && token.len() > 2 && !token.starts_with("http");
        let has_ext = EXTENSIONS.iter().any(|ext| lower.ends_with(ext));
        let backticked = raw.starts_with('`') || raw.ends_with('`');
        let has_call = token.ends_with("()");
        let has_inner_upper = token.chars().skip(1).any(|c| c.is_uppercase())
            && token.chars().any(|c| c.is_lowercase())
            && token.chars().all(|c| c.is_alphanumeric() || c == '_');
        let snake = token.contains('_') && token.chars().all(|c| c.is_alphanumeric() || c == '_');
        if is_path || has_ext || backticked || has_call || has_inner_upper || snake {
            found.insert(token.trim_matches('`').to_string());
        }
    }
    found.into_iter().collect()
}

/// Counts distinct requested operations: clauses that carry an action verb.
fn operation_count(lower: &str) -> u32 {
    let normalized = lower
        .replace(", and then ", ",")
        .replace(" and then ", ",")
        .replace(", and ", ",")
        .replace(" and also ", ",")
        .replace(" then ", ",")
        .replace(" and ", ",")
        .replace(';', ",")
        .replace(". ", ",");
    let count = normalized
        .split(',')
        .filter(|clause| {
            clause.split_whitespace().any(|word| {
                ACTION_VERBS.contains(&word.trim_matches(|c: char| !c.is_alphanumeric()))
            })
        })
        .count() as u32;
    count.max(1)
}

/// Analyses a prompt locally. `vocabulary` is the open project's naming, when
/// a project is open; it only ever raises repository dependency.
pub fn analyze_prompt(prompt: &str, vocabulary: Option<&ProjectVocabulary>) -> PromptAnalysis {
    let lower = prompt.to_lowercase();
    let words: Vec<String> = lower
        .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '\'')
        .filter(|w| !w.is_empty())
        .map(|w| w.to_string())
        .collect();
    let artifacts = code_artifacts(prompt);
    let mut explanation = Vec::new();

    // --- Project references -------------------------------------------------
    let mut project_terms: BTreeSet<String> = BTreeSet::new();
    if let Some(vocab) = vocabulary {
        for word in &words {
            if word.len() >= 4 && vocab.contains(word) {
                project_terms.insert(word.clone());
            }
        }
        for artifact in &artifacts {
            for part in split_identifier(artifact) {
                if vocab.contains(&part) {
                    project_terms.insert(part);
                }
            }
        }
    }
    let deictic_code_refs = words
        .windows(3)
        .filter(|w| {
            DEICTIC.contains(&w[0].as_str())
                && (CODE_NOUNS.contains(&w[1].as_str()) || CODE_NOUNS.contains(&w[2].as_str()))
        })
        .count()
        + words
            .windows(2)
            .filter(|w| DEICTIC.contains(&w[0].as_str()) && CODE_NOUNS.contains(&w[1].as_str()))
            .count();
    let refers_to_code =
        deictic_code_refs > 0 || !artifacts.is_empty() || !project_terms.is_empty();

    // --- Intent -------------------------------------------------------------
    let mut scores: Vec<(Intent, f64)> = Intent::ALL.iter().map(|i| (*i, 0.0)).collect();
    let add = |scores: &mut Vec<(Intent, f64)>, intent: Intent, weight: f64| {
        if let Some(slot) = scores.iter_mut().find(|(i, _)| *i == intent) {
            slot.1 += weight;
        }
    };
    for cue in INTENT_CUES {
        let hits = cue
            .phrases
            .iter()
            .filter(|phrase| contains_phrase(&lower, phrase))
            .count();
        if hits > 0 {
            // Diminishing returns: repeated evidence for one intent should not
            // drown out a single strong cue for another.
            add(
                &mut scores,
                cue.intent,
                cue.weight * (1.0 + (hits as f64 - 1.0) * 0.5),
            );
        }
    }
    let is_question = prompt.trim_end().ends_with('?')
        || matches!(
            words.first().map(String::as_str),
            Some(
                "what"
                    | "why"
                    | "how"
                    | "who"
                    | "when"
                    | "where"
                    | "which"
                    | "is"
                    | "are"
                    | "can"
                    | "does"
                    | "do"
            )
        );
    if is_question {
        add(&mut scores, Intent::Explanation, 1.0);
        add(&mut scores, Intent::GeneralQa, 1.5);
    }
    if refers_to_code {
        // A reference to real code turns a vague verb into an edit request and
        // makes pure trivia less likely.
        add(&mut scores, Intent::CodeEdit, 1.0);
        add(&mut scores, Intent::BugFix, 0.5);
        add(&mut scores, Intent::Refactor, 0.5);
        if let Some(slot) = scores.iter_mut().find(|(i, _)| *i == Intent::GeneralQa) {
            slot.1 = (slot.1 - 1.0).max(0.0);
        }
    }
    // Asking *about* code is explanation, not an edit.
    let asks_about = is_question
        || words.first().is_some_and(|w| {
            matches!(
                w.as_str(),
                "explain" | "describe" | "summarize" | "summarise"
            )
        });
    if asks_about {
        for intent in [Intent::CodeEdit, Intent::Refactor] {
            if let Some(slot) = scores.iter_mut().find(|(i, _)| *i == intent) {
                slot.1 *= 0.4;
            }
        }
    }
    let total: f64 = scores.iter().map(|(_, s)| s).sum();
    let (intent, best) =
        scores.iter().copied().fold(
            (Intent::Other, 0.0),
            |acc, (i, s)| if s > acc.1 { (i, s) } else { acc },
        );
    let intent = if best <= 0.0 {
        if is_question {
            Intent::GeneralQa
        } else {
            Intent::Other
        }
    } else {
        intent
    };
    let round2 = |v: f64| (v * 100.0).round() / 100.0;
    let intent_confidence = if total > 0.0 {
        round2(distribution_confidence(best / total, Intent::ALL.len()))
    } else {
        0.0
    };
    let change_share = if total > 0.0 {
        round2(
            scores
                .iter()
                .filter(|(i, _)| i.modifies_code())
                .map(|(_, s)| s)
                .sum::<f64>()
                / total,
        )
    } else {
        0.0
    };
    explanation.push(format!(
        "Intent {intent:?} (confidence {:.0}%) from weighted cues{}{}.",
        intent_confidence * 100.0,
        if is_question { ", question form" } else { "" },
        if refers_to_code {
            ", references to project code"
        } else {
            ""
        },
    ));

    // --- Signals ------------------------------------------------------------
    let operations = operation_count(&lower);
    let layers: Vec<String> = LAYERS
        .iter()
        .filter(|(_, terms)| words.iter().any(|w| terms.contains(&w.as_str())))
        .map(|(name, _)| name.to_string())
        .collect();
    let quantifiers = SCOPE_QUANTIFIERS
        .iter()
        .filter(|q| lower.contains(*q))
        .count();

    let hard_cues = HARD_REASONING
        .iter()
        .filter(|cue| contains_phrase(&lower, cue))
        .count();
    let reasoning_difficulty = clamp10(
        intent.base_reasoning()
            + (hard_cues as f64 * 1.25).min(4.0)
            + (operations.saturating_sub(1)) as f64 * 0.75,
    );

    let scope = clamp10(
        match operations {
            1 => 1.5,
            2 => 3.5,
            3 => 5.5,
            _ => 7.5,
        } + (quantifiers as f64 * 1.5).min(3.0)
            + (layers.len().saturating_sub(1)) as f64 * 1.0,
    );

    let mut repository_dependency = intent.base_repository_dependency();
    if deictic_code_refs > 0 {
        repository_dependency += 2.0;
    }
    if !artifacts.is_empty() {
        repository_dependency += 2.0;
    }
    repository_dependency += (project_terms.len() as f64).min(3.0);
    if !refers_to_code && !intent.modifies_code() {
        // General knowledge questions and creative prompts stay near zero.
        repository_dependency = repository_dependency.min(1.0);
    }
    let repository_dependency = clamp10(repository_dependency);

    let domain_difficulty = clamp10(
        1.0 + DOMAIN_TERMS
            .iter()
            .filter(|(_, terms)| terms.iter().any(|t| contains_phrase(&lower, t)))
            .map(|(w, _)| *w)
            .sum::<f64>(),
    );

    let base_risk = if intent.modifies_code() { 1.5 } else { 0.5 };
    let risk = clamp10(
        base_risk
            + RISK_TERMS
                .iter()
                .filter(|(_, terms)| terms.iter().any(|t| contains_phrase(&lower, t)))
                .map(|(w, _)| *w)
                .sum::<f64>(),
    );

    let mut cross_file = layers.len() as f64 * 1.5
        + operations.saturating_sub(1) as f64 * 1.5
        + quantifiers as f64 * 1.5;
    if intent == Intent::BugFix && artifacts.is_empty() {
        // "Fix the X bug" without a location means exploring first.
        cross_file += 2.0;
    }
    let cross_file_dependency = clamp10(cross_file);

    let architecture_dependency = clamp10(
        match intent {
            Intent::Architecture => 6.0,
            Intent::Refactor => 4.0,
            _ => 0.0,
        } + (layers.len().saturating_sub(1)) as f64 * 1.5
            + if contains_phrase(&lower, "architecture") {
                2.0
            } else {
                0.0
            },
    );

    let project_knowledge = clamp10(
        repository_dependency * 0.5 + cross_file_dependency * 0.3 + architecture_dependency * 0.2,
    );

    // --- Derived scores -----------------------------------------------------
    let complexity = clamp10(
        0.30 * reasoning_difficulty
            + 0.20 * scope
            + 0.20 * repository_dependency
            + 0.15 * domain_difficulty
            + 0.15 * risk,
    );
    let raw_context = 0.35 * repository_dependency
        + 0.25 * cross_file_dependency
        + 0.20 * architecture_dependency
        + 0.20 * project_knowledge;
    // Without repository dependency no amount of scope needs project context.
    let context_requirement = clamp10(raw_context * (repository_dependency / 3.0).min(1.0));
    explanation.push(format!(
        "Complexity {complexity:.1} = 0.30·reasoning {reasoning_difficulty:.1} + 0.20·scope {scope:.1} + 0.20·repo {repository_dependency:.1} + 0.15·domain {domain_difficulty:.1} + 0.15·risk {risk:.1}."
    ));
    explanation.push(format!(
        "Context requirement {context_requirement:.1} from repo dependency {repository_dependency:.1}, cross-file {cross_file_dependency:.1}, architecture {architecture_dependency:.1}, project knowledge {project_knowledge:.1}."
    ));

    // --- Required capabilities ---------------------------------------------
    let coding = if intent.modifies_code() {
        3.0 + complexity * 0.6
    } else if repository_dependency >= 3.0 {
        3.0
    } else {
        0.0
    };
    let reasoning = (reasoning_difficulty * 0.8).max(complexity * 0.9);
    let creativity = match intent {
        Intent::CreativeWriting => 7.0,
        Intent::Explanation | Intent::GeneralQa => 2.0,
        _ => 0.0,
    };
    let long_context = context_requirement * 0.8;
    let instruction_following = if repository_dependency >= 3.0 {
        6.0
    } else {
        4.0
    };
    let round = |v: f64| (v.clamp(0.0, 10.0) * 10.0).round() / 10.0;
    let required = ModelStrengths::new(
        round(coding),
        round(reasoning),
        round(creativity),
        round(long_context),
        round(instruction_following),
    );
    let mut named = vec![
        ("coding", required.coding),
        ("reasoning", required.reasoning),
        ("creativity", required.creativity),
        ("long-context understanding", required.long_context),
        ("instruction following", required.instruction_following),
    ];
    if repository_dependency >= 3.0 {
        named.push(("tool use", 5.0));
    }
    named.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let required_capabilities = named
        .into_iter()
        .filter(|(_, v)| *v >= 4.0)
        .map(|(n, _)| n.to_string())
        .collect();

    PromptAnalysis {
        intent,
        intent_confidence,
        change_share,
        complexity,
        context_requirement,
        signals: PromptSignals {
            reasoning_difficulty,
            scope,
            repository_dependency,
            domain_difficulty,
            risk,
            cross_file_dependency,
            architecture_dependency,
            project_knowledge,
            operation_count: operations,
            layers,
            code_artifacts: artifacts,
            project_terms: project_terms.into_iter().collect(),
        },
        required,
        required_capabilities,
        explanation,
    }
}

/// How much project context is supplied up front.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextLevel {
    /// 0-2: no repository context at all.
    None,
    /// 3-5: a compact structural summary.
    Minimal,
    /// 6-7: selected PROJECT_CONTEXT sections.
    Sections,
    /// 8-10: broader architecture context, still bounded.
    Broad,
}

/// Token budget for one prompt run. Every ceiling here is hard: the
/// orchestrator refuses retrievals past `max_retrieved_tokens` and stops after
/// `max_rounds` model turns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextBudget {
    pub level: ContextLevel,
    /// PROJECT_CONTEXT section keys supplied initially, in order.
    pub sections: Vec<String>,
    pub initial_context_tokens: usize,
    pub max_retrieved_tokens: usize,
    pub max_file_tokens: usize,
    pub max_rounds: u32,
    pub max_output_tokens: usize,
    /// Whether the model may request files and search the project.
    pub allow_retrieval: bool,
}

impl ContextBudget {
    /// Upper bound on prompt tokens a single call can reach under this budget.
    pub fn max_prompt_tokens(&self) -> usize {
        2_000 + self.initial_context_tokens + self.max_retrieved_tokens
    }
}

/// Maps the context requirement onto a bounded budget.
pub fn plan_context_budget(analysis: &PromptAnalysis) -> ContextBudget {
    let cr = analysis.context_requirement;
    let needs_repo =
        analysis.intent.modifies_code() || analysis.signals.repository_dependency >= 3.0;
    let level = if !needs_repo && cr < 2.5 {
        ContextLevel::None
    } else if cr < 5.5 {
        ContextLevel::Minimal
    } else if cr < 7.5 {
        ContextLevel::Sections
    } else {
        ContextLevel::Broad
    };
    let keys: &[&str] = match level {
        ContextLevel::None => &[],
        ContextLevel::Minimal => &["overview", "structure"],
        ContextLevel::Sections => &["overview", "structure", "key_modules"],
        ContextLevel::Broad => &[
            "overview",
            "structure",
            "key_modules",
            "api_contracts",
            "dependencies",
            "workflows",
            "configuration",
        ],
    };
    let (initial, retrieved, per_file, rounds) = match level {
        ContextLevel::None => (0, 0, 0, 1),
        ContextLevel::Minimal => (1_500, 12_000, 6_000, 5),
        ContextLevel::Sections => (4_000, 30_000, 8_000, 7),
        ContextLevel::Broad => (9_000, 60_000, 10_000, 9),
    };
    let max_output_tokens = match analysis.intent {
        Intent::CreativeWriting => 4_000,
        _ if analysis.intent.modifies_code() => match level {
            ContextLevel::Broad => 16_000,
            ContextLevel::Sections => 8_000,
            _ => 4_000,
        },
        _ => 2_000,
    };
    ContextBudget {
        level,
        sections: keys.iter().map(|k| k.to_string()).collect(),
        initial_context_tokens: initial,
        max_retrieved_tokens: retrieved,
        max_file_tokens: per_file,
        max_rounds: rounds,
        max_output_tokens,
        allow_retrieval: level != ContextLevel::None,
    }
}

/// The model chosen for a prompt and why.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSelection {
    pub model_id: String,
    pub display_name: String,
    pub provider: String,
    pub api_model: String,
    pub tier: ModelTier,
    /// Lowest tier the prompt's complexity allows.
    pub minimum_tier: ModelTier,
    pub estimated_cost_usd: f64,
    /// Stronger models to escalate to, weakest first. Only used on concrete
    /// failure evidence.
    pub escalation_chain: Vec<String>,
    /// Requirements no available model meets, if any.
    pub capability_shortfall: Vec<String>,
    pub explanation: String,
}

/// Tier floor implied by complexity alone. Capability matching can raise the
/// choice above it; nothing lowers it.
pub fn minimum_tier(analysis: &PromptAnalysis) -> ModelTier {
    if analysis.complexity >= 7.0 {
        ModelTier::Powerful
    } else if analysis.complexity >= 4.5 {
        ModelTier::Balanced
    } else {
        ModelTier::Fast
    }
}

/// Expected spend for one run under `budget`: all initial context, half the
/// retrieval allowance (a conservative middle), and full output.
fn expected_cost(entry: &CatalogEntry, budget: &ContextBudget, prompt_tokens: usize) -> f64 {
    let input =
        prompt_tokens + 1_200 + budget.initial_context_tokens + budget.max_retrieved_tokens / 2;
    let turns = if budget.allow_retrieval { 3.0 } else { 1.0 };
    let per_turn_output = (budget.max_output_tokens / 2) as f64;
    (input as f64 * turns / 1_000_000.0) * entry.pricing.input_usd_per_1m
        + (per_turn_output / 1_000_000.0) * entry.pricing.output_usd_per_1m
}

/// Picks the least expensive available model that satisfies the prompt.
///
/// `available` lists catalog model IDs the user can actually call right now
/// (credential configured, or local runtime running).
pub fn select_model(
    analysis: &PromptAnalysis,
    budget: &ContextBudget,
    prompt_tokens: usize,
    catalog: &PriceCatalog,
    available: &[String],
    policy: &RoutingPolicy,
) -> Result<ModelSelection> {
    let floor = minimum_tier(analysis);
    let mut pool: Vec<&CatalogEntry> = catalog
        .entries
        .iter()
        .filter(|e| available.contains(&e.model_id))
        .filter(|e| !policy.require_local_only || e.is_private())
        .filter(|e| e.context_cap >= prompt_tokens + budget.initial_context_tokens)
        .collect();
    if pool.is_empty() {
        return Err(CoreError::InvalidModel(if policy.require_local_only {
            "No local or self-hosted model is available. Start one or add one on the Models page."
                .to_string()
        } else {
            "No model is available. Add an OpenAI or Anthropic API key, or start a local model, on the Models page.".to_string()
        }));
    }

    let within_budget = |e: &&CatalogEntry| {
        policy
            .max_budget_usd
            .is_none_or(|max| expected_cost(e, budget, prompt_tokens) <= max)
    };
    let fits = |e: &&CatalogEntry| {
        e.tier >= floor
            && e.strengths.shortfalls(&analysis.required).is_empty()
            && e.context_cap >= budget.max_prompt_tokens()
    };

    let mut sufficient: Vec<&CatalogEntry> = pool
        .iter()
        .copied()
        .filter(fits)
        .filter(within_budget)
        .collect();
    sufficient.sort_by(|a, b| {
        expected_cost(a, budget, prompt_tokens)
            .partial_cmp(&expected_cost(b, budget, prompt_tokens))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.tier.cmp(&b.tier))
    });

    let (chosen, shortfall, why) = if let Some(best) = sufficient.first() {
        (
            *best,
            Vec::new(),
            format!(
                "Cheapest available model meeting tier ≥ {} and required strengths; {} candidate(s) qualified.",
                floor.label(),
                sufficient.len()
            ),
        )
    } else {
        // Nothing meets every requirement: take the most capable model we
        // have and say what is missing rather than refusing outright.
        pool.sort_by(|a, b| {
            b.tier.cmp(&a.tier).then(
                b.strengths
                    .total()
                    .partial_cmp(&a.strengths.total())
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
        });
        let best = pool[0];
        let mut missing: Vec<String> = best
            .strengths
            .shortfalls(&analysis.required)
            .into_iter()
            .map(str::to_string)
            .collect();
        if best.tier < floor {
            missing.push(format!("{} tier", floor.label()));
        }
        (
            best,
            missing,
            "No available model meets every requirement; using the most capable available model."
                .to_string(),
        )
    };

    // Escalation: one step per stronger tier, cheapest first within a tier.
    let mut chain: Vec<&CatalogEntry> = pool
        .iter()
        .copied()
        .filter(|e| e.tier > chosen.tier)
        .collect();
    chain.sort_by(|a, b| {
        a.tier.cmp(&b.tier).then(
            expected_cost(a, budget, prompt_tokens)
                .partial_cmp(&expected_cost(b, budget, prompt_tokens))
                .unwrap_or(std::cmp::Ordering::Equal),
        )
    });
    chain.dedup_by(|a, b| a.tier == b.tier);

    Ok(ModelSelection {
        model_id: chosen.model_id.clone(),
        display_name: chosen.display_name.clone(),
        provider: chosen.provider.clone(),
        api_model: chosen.api_model_name().to_string(),
        tier: chosen.tier,
        minimum_tier: floor,
        estimated_cost_usd: (expected_cost(chosen, budget, prompt_tokens) * 10_000.0).round()
            / 10_000.0,
        escalation_chain: chain.iter().map(|e| e.model_id.clone()).collect(),
        capability_shortfall: shortfall,
        explanation: why,
    })
}

#[cfg(test)]
mod prompt_routing_tests {
    use super::*;

    fn all_models() -> Vec<String> {
        PriceCatalog::default_catalog()
            .entries
            .into_iter()
            .map(|e| e.model_id)
            .collect()
    }

    fn anthropic_only() -> Vec<String> {
        all_models()
            .into_iter()
            .filter(|m| m.starts_with("anthropic/"))
            .collect()
    }

    fn route(
        prompt: &str,
        vocab: Option<&ProjectVocabulary>,
    ) -> (PromptAnalysis, ContextBudget, ModelSelection) {
        let analysis = analyze_prompt(prompt, vocab);
        let budget = plan_context_budget(&analysis);
        let selection = select_model(
            &analysis,
            &budget,
            50,
            &PriceCatalog::default_catalog(),
            &anthropic_only(),
            &RoutingPolicy::default(),
        )
        .unwrap();
        (analysis, budget, selection)
    }

    fn vocab() -> ProjectVocabulary {
        ProjectVocabulary::from_paths([
            "src/components/LoginButton.tsx",
            "src/auth/session.ts",
            "src-tauri/src/commands/auth.rs",
            "src/pages/SettingsPage.tsx",
        ])
    }

    #[test]
    fn a_explain_recursion_needs_no_repository() {
        let (a, budget, sel) = route("Explain recursion.", Some(&vocab()));
        assert_eq!(a.intent, Intent::Explanation);
        assert!(a.signals.repository_dependency <= 1.0, "{a:?}");
        assert!(a.context_requirement < 1.0, "{a:?}");
        assert_eq!(budget.level, ContextLevel::None);
        assert!(!budget.allow_retrieval);
        assert_eq!(sel.tier, ModelTier::Fast);
    }

    #[test]
    fn b_creative_story_needs_creativity_not_context() {
        let (a, budget, sel) = route(
            "Write a sci-fi story about an AI predicting that its creator will cause a global catastrophe.",
            Some(&vocab()),
        );
        assert_eq!(a.intent, Intent::CreativeWriting);
        assert!(a.complexity < 4.5, "{a:?}");
        assert!(a.context_requirement < 1.0, "{a:?}");
        assert_eq!(budget.level, ContextLevel::None);
        assert!(a.required.creativity >= 7.0);
        assert_eq!(
            a.required_capabilities.first().map(String::as_str),
            Some("creativity")
        );
        // Haiku's creativity rating is below the requirement, so a creative
        // model is chosen even though complexity alone would allow Fast.
        assert!(sel.tier >= ModelTier::Balanced);
        assert!(sel.capability_shortfall.is_empty());
    }

    #[test]
    fn c_simple_label_change_is_low_complexity_small_budget_fast_model() {
        let (a, budget, sel) = route("Change the Login button text to Sign In.", Some(&vocab()));
        assert_eq!(a.intent, Intent::CodeEdit);
        assert!(a.complexity < 4.0, "{a:?}");
        assert!(a.context_requirement < 5.5, "{a:?}");
        assert_eq!(budget.level, ContextLevel::Minimal);
        assert!(budget.allow_retrieval);
        assert_eq!(sel.tier, ModelTier::Fast);
        assert_eq!(sel.model_id, "anthropic/claude-haiku-4-5");
        assert!(!sel.escalation_chain.is_empty());
    }

    #[test]
    fn d_vague_bug_fix_needs_repository_and_retrieval() {
        let (a, budget, _) = route("Fix the authentication bug.", Some(&vocab()));
        assert_eq!(a.intent, Intent::BugFix);
        assert!(a.signals.repository_dependency >= 5.0, "{a:?}");
        assert!(budget.level >= ContextLevel::Minimal);
        assert!(budget.allow_retrieval);
    }

    #[test]
    fn e_oauth_migration_is_high_everything_and_powerful() {
        let (a, budget, sel) = route(
            "Refactor authentication to OAuth, migrate existing users, update backend APIs, and update frontend session handling.",
            Some(&vocab()),
        );
        assert_eq!(a.intent, Intent::Refactor);
        assert!(a.complexity >= 7.0, "{a:?}");
        assert!(a.context_requirement >= 7.5, "{a:?}");
        assert!(a.signals.operation_count >= 4);
        assert_eq!(budget.level, ContextLevel::Broad);
        // Bounded even at the top level.
        assert!(budget.max_retrieved_tokens <= 60_000);
        assert_eq!(sel.tier, ModelTier::Powerful);
        assert!(sel.escalation_chain.is_empty());
    }

    #[test]
    fn same_complexity_can_need_different_models() {
        let story = analyze_prompt("Write a short poem about autumn.", None);
        let trivia = analyze_prompt("What is the capital of France?", None);
        assert_eq!(minimum_tier(&story), minimum_tier(&trivia));
        let catalog = PriceCatalog::default_catalog();
        let pick = |a: &PromptAnalysis| {
            select_model(
                a,
                &plan_context_budget(a),
                20,
                &catalog,
                &anthropic_only(),
                &RoutingPolicy::default(),
            )
            .unwrap()
            .tier
        };
        assert!(pick(&story) > pick(&trivia));
    }

    #[test]
    fn prefers_cheapest_sufficient_across_providers() {
        let a = analyze_prompt("Explain recursion.", None);
        let b = plan_context_budget(&a);
        let catalog = PriceCatalog::default_catalog();
        // A running local model is free and sufficient for a simple answer.
        let sel = select_model(
            &a,
            &b,
            10,
            &catalog,
            &all_models(),
            &RoutingPolicy::default(),
        )
        .unwrap();
        assert_eq!(sel.provider, "local");
        // Among cloud models the cheapest sufficient one wins, whatever the provider.
        let cloud: Vec<String> = all_models()
            .into_iter()
            .filter(|m| !m.starts_with("local/"))
            .collect();
        let sel = select_model(&a, &b, 10, &catalog, &cloud, &RoutingPolicy::default()).unwrap();
        assert_eq!(sel.model_id, "openai/gpt-4o-mini");
    }

    #[test]
    fn local_only_policy_and_missing_models_are_explicit() {
        let a = analyze_prompt("Explain recursion.", None);
        let b = plan_context_budget(&a);
        let catalog = PriceCatalog::default_catalog();
        assert!(select_model(&a, &b, 10, &catalog, &[], &RoutingPolicy::default()).is_err());
        let policy = RoutingPolicy {
            require_local_only: true,
            ..Default::default()
        };
        let sel = select_model(&a, &b, 10, &catalog, &all_models(), &policy).unwrap();
        assert_eq!(sel.provider, "local");
    }

    #[test]
    fn shortfall_is_reported_instead_of_silently_downgrading() {
        let a = analyze_prompt(
            "Refactor authentication to OAuth, migrate existing users, update backend APIs, and update frontend session handling.",
            None,
        );
        let b = plan_context_budget(&a);
        let only_fast = vec!["anthropic/claude-haiku-4-5".to_string()];
        let sel = select_model(
            &a,
            &b,
            40,
            &PriceCatalog::default_catalog(),
            &only_fast,
            &RoutingPolicy::default(),
        )
        .unwrap();
        assert!(!sel.capability_shortfall.is_empty());
    }

    #[test]
    fn free_self_hosted_models_are_preferred_when_sufficient() {
        let mut catalog = PriceCatalog::default_catalog();
        catalog.entries.push(CatalogEntry::self_hosted(
            "qwen",
            "Qwen",
            "qwen3",
            32_768,
            ModelTier::Balanced,
        ));
        catalog.entries.push(CatalogEntry::self_hosted(
            "gemma",
            "Gemma",
            "gemma4",
            32_768,
            ModelTier::Fast,
        ));
        let mut available = anthropic_only();
        available.push("custom/qwen".to_string());
        available.push("custom/gemma".to_string());
        let policy = RoutingPolicy::default();

        // A simple edit goes to the free fast model.
        let a = analyze_prompt("Change the Login button text to Sign In.", Some(&vocab()));
        let sel = select_model(
            &a,
            &plan_context_budget(&a),
            20,
            &catalog,
            &available,
            &policy,
        )
        .unwrap();
        assert_eq!(sel.model_id, "custom/gemma");
        assert_eq!(sel.api_model, "gemma4");
        assert_eq!(
            sel.escalation_chain.first().map(String::as_str),
            Some("custom/qwen")
        );

        // Creative writing needs more than a fast model: the free balanced one.
        let a = analyze_prompt("Write a short story about a lighthouse keeper.", None);
        let sel = select_model(
            &a,
            &plan_context_budget(&a),
            20,
            &catalog,
            &available,
            &policy,
        )
        .unwrap();
        assert_eq!(sel.model_id, "custom/qwen");

        // Self-hosted counts as private for local-only policies.
        let strict = RoutingPolicy {
            require_local_only: true,
            ..Default::default()
        };
        let a = analyze_prompt("Explain recursion.", None);
        let sel = select_model(
            &a,
            &plan_context_budget(&a),
            20,
            &catalog,
            &available,
            &strict,
        )
        .unwrap();
        assert_eq!(sel.provider, "custom");

        // The migration still needs a powerful model the user does not host.
        let a = analyze_prompt(
            "Refactor authentication to OAuth, migrate existing users, update backend APIs, and update frontend session handling.",
            Some(&vocab()),
        );
        let sel = select_model(
            &a,
            &plan_context_budget(&a),
            40,
            &catalog,
            &available,
            &policy,
        )
        .unwrap();
        assert_eq!(sel.model_id, "anthropic/claude-opus-5");
    }

    #[test]
    fn identifiers_split_for_vocabulary() {
        assert_eq!(split_identifier("LoginButton"), vec!["login", "button"]);
        assert_eq!(split_identifier("session_store"), vec!["session", "store"]);
        let v = vocab();
        assert!(v.contains("login") && v.contains("session"));
        assert!(!v.contains("src"));
    }

    #[test]
    fn distribution_confidence_is_zero_when_even_and_one_when_certain() {
        assert_eq!(distribution_confidence(1.0, 10), 1.0);
        assert_eq!(distribution_confidence(0.1, 10), 0.0);
        // Three options, 60% on the winner: (3·0.6 − 1) / 2 = 0.4.
        assert!((distribution_confidence(0.6, 3) - 0.4).abs() < 1e-9);
        // Two options: 50/50 is no confidence at all.
        assert_eq!(distribution_confidence(0.5, 2), 0.0);
        assert_eq!(distribution_confidence(0.7, 1), 1.0);
    }

    #[test]
    fn change_share_separates_edit_requests_from_questions() {
        let edit = analyze_prompt("Change the Login button text to Sign In.", None);
        assert!(edit.change_share > 0.6, "{}", edit.change_share);
        let question = analyze_prompt("Explain how recursion works.", None);
        assert!(question.change_share < 0.5, "{}", question.change_share);
        for a in [&edit, &question] {
            assert!((0.0..=1.0).contains(&a.intent_confidence));
        }
    }
}
