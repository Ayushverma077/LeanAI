# LeanAI Desktop - Complete Development Changelog

This document chronicles the complete development history and feature set built into LeanAI Desktop from the very beginning up to the current state.

## 1. Core Architecture & Foundations
* **Stack Initialization**: Set up a hybrid native/web application using **Tauri 2.11.5** (Rust backend) and **React + Vite** (Frontend).
* **State Management**: Implemented a centralized, reactive store using **Zustand** (`useAppStore.ts`) to manage projects, bundle configurations, file selections, and git tracking.
* **Local Persistence**: Integrated a local SQLite database to persist immutable audit logs, context bundle snapshots, and user preferences with zero cloud egress.
* **Styling Engine**: Configured **Tailwind CSS v4** for utility-first styling, mapping native design tokens to CSS variables.

## 2. GitHub & Git Integration
* **Secure Authentication**: Built `GitAuthSettings` to securely store GitHub Personal Access Tokens inside the native OS keychain (macOS Keychain) rather than plaintext files.
* **Remote Repository Management**: Added Rust backend commands using the `git2` crate and `reqwest` to list GitHub repositories, clone them locally, and test SSH connectivity.
* **Version Control Awareness**: Integrated local repository tracking, allowing the app to detect untracked/unstaged changes and filter context selection strictly by active git diffs.
* **Commit & Push Flow**: Built the `GitSyncModal` to allow agents or users to push generated changes directly to a remote branch.

## 3. Context Bundling & Scanning Engine
* **Intelligent File Indexing**: Created a scanning engine that reads directories while strictly respecting `.gitignore`, `.git/info/exclude`, and custom `.aiignore` rules.
* **Safety & Exclusion Policy**: Implemented default policy filters to exclude dependency lockfiles, minified/vendored outputs, hidden files, and files exceeding safe byte limits.
* **Markdown Synthesis**: Built the formatting engine to synthesize context into LLM-optimized Markdown, supporting:
  * Directory tree preambles
  * File path headers and line numbers for precise citations
  * Code fences with language hints
  * Token size and token share annotations
* **Pre-Export Secret Scanner**: Built a credential scanner that analyzes code for high/medium/low risk secrets prior to export. Integrated an `ExportPreflightDialog` that blocks export until the user explicitly acknowledges credential risks.

## 4. Local & Cloud LLM Integration
* **GGUF Local Sidecar (Phase 6)**: Integrated the `llama-server` as a local sidecar process. It binds strictly to `127.0.0.1` on an ephemeral port, allowing 100% offline, zero-egress inference without exposing network listeners.
* **Cloud Providers & OS Keychain (Phase 7)**: Added support for major cloud providers (OpenAI, Anthropic, Gemini). API keys are securely persisted in the OS keychain and never touch the SQLite database or logs (ADR 0007).
* **Cost-Aware Routing Simulator**: Implemented deterministic task-based model selection. The system evaluates task complexity, estimates token cost, applies hard budget ceilings, and routes prompts to the most efficient model (local or cloud).
* **Token Estimation**: Built exact token counting against provider endpoints (opt-in) alongside a fast local BPE tokenizer fallback.

## 5. Multi-Agent Orchestration Pipeline
* **Specialized Agent Fleet**: Designed a roster of specialized agents (Orchestrator, Researcher, Planner, Coder, Validator, Documenter) with unique prompts and tool capabilities.
* **Task Execution Workspace**: Built the `TaskExecutionView` where users assign objectives. The Orchestrator deconstructs requests, formulates plans, and coordinates sub-agents.
* **Mechanical Validation ("Tester")**: Empowered agents to run verification commands (e.g., `cargo test`, `npm run build`) against proposed code changes directly in the UI.
* **High-Friction Approval Workflow (ADR 0010)**: Ensured that agents *propose* diffs in memory. No files are actually written to the disk until the user manually reviews and approves the diff in the UI.

## 6. Complete UI Redesign & Brand Alignment
* **New Logo & Iconography**: Replaced the initial dark logo with a polished, clean gradient "L" squircle logo. Injected the new logo directly into the macOS dock via native Cocoa FFI to bypass OS caching.
* **Theme Overhaul**: Replaced the default harsh obsidian black backgrounds with a softer, premium **deep navy base** (`#07071a`) combined with a primary brand blue (`#4B7BFF`).
* **De-cluttering & Simplification**:
  * Cleaned up the `TopBar`, `Sidebar`, and `StatusBar` by removing unnecessary technical noise, verbose labels, and heavy borders.
  * Overhauled the `OverviewPage` to remove heavy marketing banners in favor of a sleek, two-column layout focusing purely on metrics and file classification.
  * Shortened descriptive text and simplified panel structures across the `AgentsPage`, `SettingsPage`, `HistoryPage`, and `PreviewPage` to achieve a highly professional, modern aesthetic.
