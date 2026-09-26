# Requirements traceability matrix

Backlog item 0.3. Every in-scope requirement from `Instructions.md` maps to the
code that implements it and the test that proves it. `Deferred` rows name the
phase that owns them and the ADR that fixes the design in advance.

Test ids are `file::test_name`. Run everything with `npm run check:all`.

## Functional requirements

| ID | Requirement | Implementation | Evidence |
| --- | --- | --- | --- |
| FR-01 | Choose a project directory, grant only that scope | `project::canonical_root`, `commands/projects.rs::open_project`, `capabilities/default.json` | `scan.rs::path_resolution_rejects_traversal`, ADR 0002 |
| FR-02 | Deterministic walk respecting git ignore rules | `walker::scan` (`ignore` crate, `git_ignore`/`git_global`/`git_exclude`) | `scan.rs::scan_is_deterministic_and_ordered`, `scan.rs::nested_gitignore_rules_and_negations_are_respected` |
| FR-03 | `.aiignore` only with defined precedence and explanation | `aiignore`, `policy::IgnoreSource`, Settings → Ignore precedence | `safety.rs::aiignore_is_applied_by_the_scanner`, `safety.rs::aiignore_cannot_reexpose_a_blocked_path`, ADR 0003 |
| FR-04 | Unsafe/unhelpful files visibly excluded or warned | `classify`, `walker::classify_entry` | `scan.rs::binary_detection_uses_extension_and_content_probe`, `scan.rs::unsafe_and_unhelpful_files_are_classified_with_reasons`, `scan.rs::credential_paths_are_excluded_but_examples_are_not` |
| FR-05 | Inventory keeps path, size, class, hash, exclusion reason | `inventory::FileEntry` | `scan.rs::inventory_records_hash_and_freshness_metadata` |
| FR-06 | Cancellable scan, per-path errors, responsive UI | `walker::CancelToken`, `ScanIssue`, progress events, `spawn_blocking` in `scan_project` | `scan.rs::cancellation_returns_an_error_not_partial_data`, `scan.rs::scan_emits_progress`, `scan.rs::scan_handles_permission_restricted_directories_and_files`, `scan.rs::scan_large_tree_repository_performance` |
| FR-07 | Search, select/unselect, inspect ordered selection | `FileTree.tsx`, `SelectPage.tsx`, `selection::resolve` | `FileTree.test.tsx` (13 tests incl. 10k+ virtualisation) |
| FR-08 | Folder selection never re-enables blocked files | `selection::resolve` (directory pass ignores non-selectable), `FileClass::requires_explicit_override`, `OverrideDialog` | `bundle.rs::folder_selection_never_includes_blocked_files`, `bundle.rs::blocked_files_require_an_explicit_per_file_override`, `FileTree.test.tsx::routes a blocked file through the override flow` |
| FR-09 | Deterministic bundles for same revision/options/selection | `concat::build`, sorted selection, line-ending normalisation | `bundle.rs::bundles_are_byte_identical_across_runs`, `bundle.rs::selection_order_does_not_change_the_bundle`, `bundle.rs::line_endings_are_normalized` |
| FR-10 | Path headers; reversible fences/line numbers/annotations | `concat::BundleOptions` | `bundle.rs::output_modes_are_reversible_and_snapshot_stable`, `bundle.rs::tree_preamble_lists_the_selection` |
| FR-11 | Copy/save show destination policy, return success or error | `export_preflight`, `export_bundle`, `PreviewPage` preflight dialog | `bundle.rs::manifest_records_provenance_and_warnings`; manual: `docs/manual-test-plan.md` MT-01..18 macOS recorded run |
| FR-12 | Bundle records identity, revision, selection, options, estimate kind | `manifest::BundleManifest`, `bundles` table | `bundle.rs::manifest_records_provenance_and_warnings`, `persistence.rs::retention_controls_whether_bundle_text_is_stored` |
| FR-13 | Debounced local estimate discloses tokenizer scope | `tokenizer::EstimateKind::label`, `PreviewPage` 200 ms debounce | `bundle.rs::token_estimates_disclose_their_scope`, `bundle.rs::per_file_contributions_are_reported`, ADR 0004 |
| FR-14 | Exact counts only with consent and provider support | `EstimateKind::ProviderExact`, exact token counting opt-in toggle; `commands/models.rs::estimate_provider_tokens` | `provider.rs::exact_token_counting_reports_provider_exact_provenance`, ADR 0004, ADR 0008 |
| FR-15 | Cloud run accounting and price catalog | `catalog::PriceCatalog`, `commands/models.rs::get_model_catalog`, `runs` ledger | `provider.rs::price_catalog_calculates_estimated_cost_and_detects_staleness`, `persistence.rs::exports_are_recorded_in_the_audit_log` |
| FR-16 | Never present estimated cost or savings as a guarantee | `benchmark::NOT_MEASURED`, preview caption, manifest warning | `bundle.rs::manifest_records_provenance_and_warnings`; `docs/benchmark-methodology.md` |
| FR-17 | Markdown context index with section-to-source provenance | `context::generate`, `context::render` | `context.rs::document_contains_every_required_section_in_order`, `context.rs::sections_record_provenance_and_generator` |
| FR-18 | Summaries link to source and disclose uncertainty | `ContextSection::limitations`, `SourceRef`, `ContextPage` | `context.rs::sections_disclose_their_limitations` |
| FR-19 | Changed file marks dependent sections stale | `context::apply_change_impact` | `context.rs::changed_files_invalidate_dependent_sections`, `context.rs::added_files_conservatively_invalidate_structural_sections`, `context.rs::deleted_files_are_reported` |
| FR-20 | Context loading under budget, source-on-demand | `commands/context.rs::fetch_source`, `benchmark::Strategy::Hybrid` composition order | `context.rs::generation_is_deterministic`; harness output in `docs/benchmark-methodology.md` |
| FR-21 | Local model lifecycle without shell access | `sidecar_manager::SidecarManager`, `commands/models.rs`, loopback ephemeral port allocation | `sidecar.rs::sidecar_lifecycle_with_mock_loopback_binary`, `sidecar.rs::sidecar_preflight_rejects_missing_file`, ADR 0009 |
| FR-22 | Verified model downloads with provenance | `gguf::inspect_gguf_file`, header SHA-256 fingerprint, tensor count validation | `provider.rs::gguf_inspection_validates_magic_header_and_rejects_invalid_files`, `sidecar.rs::sidecar_preflight_rejects_corrupted_file`, `sidecar.rs::sidecar_preflight_accepts_valid_synthetic_gguf` |
| FR-23 | Provider capability profiles, no assumed parity | `provider::CapabilityProfile`, `validate_request_capabilities` | `provider.rs::provider_capability_profile_validates_request_and_rejects_missing_features`, ADR 0008 |
| FR-24 | Credentials in OS secure storage, never in plaintext history | `keychain::KeychainStore` (macOS Keychain via security CLI, mock for tests), `provider_configs` stores only non-secret flags | `keychain.rs::keychain_store_roundtrip_and_delete`, `persistence.rs::no_table_stores_credentials`, ADR 0007 |
| FR-25 | Visible routing policy, fallback, budget, audit trail | `routing::route_task`, `RoutingPolicy`, `TaskClass`, `ModelsPage` simulator | `provider.rs::routing_policy_routes_tasks_and_escalates_on_context_exceeded`, ADR 0008 |
| FR-26 | Task objective, manifest, tools, budget, cancellation | `agent::TaskPreflight`, `TaskBudget`, `commands::agent::start_task_run`, `cancel_task_run` | `agent.rs::preflight_validates_objective_root_and_budgets`, `agent_workflow.rs::agent_run_and_event_ledger_roundtrip`, ADR 0010 |
| FR-27 | Agent runs read-only unless a capability is approved | `agent::ToolCapability`, `approval::ApprovalRequest`, `DecisionState`, `commands::agent::start_task_run` | `agent.rs::preflight_validates_objective_root_and_budgets`, `approval.rs::denied_approval_never_falls_through_to_action`, ADR 0010, ADR 0011 |
| FR-28 | Write-enabled runs produce a reviewable, validated diff | `approval::TransactionalPatchSession`, `PatchProposal`, `FilePatch`, `commands::agent::resolve_approval` | `approval.rs::transactional_patch_applies_and_rolls_back_cleanly`, `agent_workflow.rs::approval_persistence_and_resolution`, ADR 0011 |
| FR-29 | Structured, cited handoffs between roles | `agent::PlanArtifact`, `ContextBuilderArtifact`, `ValidatorVerdict`, `validate_proposal` | `agent.rs::context_builder_produces_fresh_provenance_citations`, `agent.rs::validator_rejects_out_of_scope_paths_and_leaked_secrets`, ADR 0011 |
| FR-30 | History keeps findings, not full transcripts | `RunRecord`, `run_events.redacted_payload`, `EpisodicMemoryEntry`, `project_memory` (Migration 4) | `agent_workflow.rs::command_allowlist_and_episodic_memory`, ADR 0006, ADR 0012 |
| FR-31 | Tester role with safe allowlisted command execution | `agent::execute_tester_command`, `TesterArtifact`, `commands::agent::run_tester_step`, TaskExecutionView | `tester.rs::*` (7 tests), `agent_workflow.rs::tester_step_event_ledger_and_allowlist`, ADR 0010, ADR 0011 |
| Retrieval | Hybrid context retrieval (pins, git, symbols, BM25) and episodic memory | `retrieval::rank_context_for_task`, `ProposedFile`, `RetrievalResult`, `commands::agent::query_task_context` | `retrieval.rs::hybrid_retrieval_ranks_by_pins_diffs_and_terms`, `retrieval.rs::disabled_retrieval_falls_back_deterministically_to_pinned_files`, ADR 0012 |

## Non-functional requirements

| Area | Requirement | Implementation | Evidence |
| --- | --- | --- | --- |
| 7.1 | Repository content is untrusted data | No shell/network capability; context body is rendered as text, never executed | ADR 0002, ADR 0010 |
| 7.1 | Default-deny export; review before anything leaves | `export_preflight` runs for clipboard *and* file | `PreviewPage` preflight dialog; `bundle.rs` manifest warnings |
| 7.1 | Secret paths excluded by default; override is high friction | `SENSITIVE_PATH_PATTERNS`, `OverrideDialog` typed phrase | `scan.rs::credential_paths_are_excluded_but_examples_are_not`, `bundle.rs::blocked_files_require_an_explicit_per_file_override` |
| 7.1 | Secret scanning is a warning, not a boundary | `SecretReport::DISCLAIMER` shown in every preflight | `safety.rs::secret_findings_are_graded_and_redacted` |
| 7.1 | Root boundary checks; symlinks not followed | `project::resolve_within_root`, `Policy::follow_symlinks = false` | `scan.rs::symlinks_never_escape_the_project_root` |
| 7.1 | Audit record for every consequential decision | `record_audit` on export and `.aiignore` write | `persistence.rs::exports_are_recorded_in_the_audit_log` |
| 7.2 | No partial data loss on cancellation | Cancellation returns `Cancelled` and discards the partial walk | `scan.rs::cancellation_returns_an_error_not_partial_data` |
| 7.2 | Versioned, transactional migrations | `db::migrations` (one transaction per migration) | `persistence.rs::migrations_apply_and_are_idempotent`, `persistence.rs::a_newer_database_is_refused_with_a_clear_message` |
| 7.3 | Keyboard navigable tree and dialogs | `FileTree` roving focus, arrow/space/enter/home/end | `FileTree.test.tsx::is keyboard navigable and toggles with the space key` |
| 7.3 | Screen-reader labels; colour never alone | ARIA tree with level/setsize, labelled checkboxes, text in every `Chip` | `FileTree.test.tsx::blocks a credential-sensitive file and explains why`, `::announces each row's depth` |
| 7.3 | Clear empty/loading/error/denied states | `EmptyState`, `ErrorBoundary`, `AppError.recovery` | `App.tsx`, `error.rs` |
| 7.4 | UI stays responsive on large repositories | Tree virtualisation; scan and build on `spawn_blocking` with progress events | `FileTree.tsx` windowing (`FileTree.test.tsx` 10k+ virtualisation); `scan.rs::scan_emits_progress`, `scan.rs::scan_large_tree_repository_performance`; `docs/performance-matrix.md` |
| 7.4 | Every limit has a visible reason and an override | `policy::LIMIT_REASONS` rendered in Settings | `bundle.rs::selection_limits_are_enforced` |
| 12.2 | Benchmarks report quality caveats, not token counts alone | `benchmark::NOT_MEASURED` in every report | `docs/benchmark-methodology.md`, `docs/performance-matrix.md` |
