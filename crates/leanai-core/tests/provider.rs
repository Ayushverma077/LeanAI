use leanai_core::catalog::PriceCatalog;
use leanai_core::error::CoreError;
use leanai_core::gguf::{create_synthetic_gguf, inspect_gguf_file};
use leanai_core::provider::{
    validate_request_capabilities, CapabilityProfile, CompletionRequest, MockProvider, Provider,
};
use leanai_core::routing::{route_task, RoutingPolicy, RoutingReason, TaskClass};

#[test]
fn provider_capability_profile_validates_request_and_rejects_missing_features() {
    let mut profile = CapabilityProfile {
        context_cap: 4096,
        streaming: true,
        tool_calling: false,
        structured_output: false,
        vision: false,
        exact_token_counting: false,
        cached_token_policy: leanai_core::provider::CachedTokenPolicy::None,
    };

    let mut request = CompletionRequest::default();
    assert!(validate_request_capabilities(&profile, &request, 100).is_ok());

    // Tool calling missing
    request.requires_tool_calling = true;
    let err = validate_request_capabilities(&profile, &request, 100).unwrap_err();
    assert!(matches!(err, CoreError::MissingCapability(cap) if cap == "tool_calling"));
    request.requires_tool_calling = false;

    // Structured output missing
    request.requires_structured_output = true;
    let err = validate_request_capabilities(&profile, &request, 100).unwrap_err();
    assert!(matches!(err, CoreError::MissingCapability(cap) if cap == "structured_output"));
    request.requires_structured_output = false;

    // Vision missing
    request.requires_vision = true;
    let err = validate_request_capabilities(&profile, &request, 100).unwrap_err();
    assert!(matches!(err, CoreError::MissingCapability(cap) if cap == "vision"));
    request.requires_vision = false;

    // Context cap exceeded
    let err = validate_request_capabilities(&profile, &request, 5000).unwrap_err();
    assert!(matches!(err, CoreError::LimitExceeded(_)));

    // Grant capabilities and re-verify
    profile.tool_calling = true;
    profile.structured_output = true;
    profile.vision = true;
    request.requires_tool_calling = true;
    request.requires_structured_output = true;
    request.requires_vision = true;
    assert!(validate_request_capabilities(&profile, &request, 2000).is_ok());
}

#[test]
fn exact_token_counting_reports_provider_exact_provenance() {
    let mut profile = CapabilityProfile {
        exact_token_counting: true,
        ..Default::default()
    };
    let provider = MockProvider::new("test_provider", "model_v1", profile.clone());

    let result = provider.count_tokens("one two three").unwrap();
    assert!(result.estimate_kind.is_exact());
    assert_eq!(
        result.estimate_kind.label(),
        "exact · test_provider/model_v1"
    );

    // When exact counting is not supported, label indicates local estimate
    profile.exact_token_counting = false;
    let fallback_provider = MockProvider::new("test_provider", "model_v1", profile);
    let fallback_result = fallback_provider.count_tokens("one two three").unwrap();
    assert!(!fallback_result.estimate_kind.is_exact());
    assert!(fallback_result
        .estimate_kind
        .label()
        .contains("OpenAI-family"));
}

#[test]
fn price_catalog_calculates_estimated_cost_and_detects_staleness() {
    let catalog = PriceCatalog::default_catalog();
    assert!(!catalog.entries.is_empty());

    let cost = catalog
        .estimate_cost("openai/gpt-4o-mini", 10_000, 1_000, 2_000)
        .expect("model exists");
    // (8000 / 1m) * 0.15 + (1000 / 1m) * 0.60 + (2000 / 1m) * 0.075
    // = 0.0012 + 0.0006 + 0.00015 = 0.00195
    assert!((cost - 0.00195).abs() < 1e-6);

    // Staleness check
    assert!(!catalog.is_stale(365, catalog.updated_at_ms + 1000));
    assert!(catalog.is_stale(30, catalog.updated_at_ms + (31 * 24 * 3600 * 1000)));
}

#[test]
fn routing_policy_routes_tasks_and_escalates_on_context_exceeded() {
    let catalog = PriceCatalog::default_catalog();
    let policy = RoutingPolicy {
        low_cost_model_id: "openai/gpt-4o-mini".to_string(),
        strong_model_id: "openai/gpt-4o".to_string(),
        auto_escalate_on_context_exceeded: true,
        max_budget_usd: Some(1.00),
        require_local_only: false,
    };

    // Standard task within limit routes to low cost
    let dec = route_task(TaskClass::FileInventory, 1000, &policy, &catalog).unwrap();
    assert_eq!(dec.selected_model_id, "openai/gpt-4o-mini");
    assert_eq!(dec.reason, RoutingReason::TaskDefaultLowCost);

    // Complex task routes to strong model
    let dec = route_task(TaskClass::CodeChangeProposal, 1000, &policy, &catalog).unwrap();
    assert_eq!(dec.selected_model_id, "openai/gpt-4o");
    assert_eq!(dec.reason, RoutingReason::TaskRequiresStrongReasoning);

    // Exceeding low-cost context cap (128k) auto-escalates if configured
    let mut small_catalog = catalog.clone();
    small_catalog.entries[0].context_cap = 5000;
    let dec = route_task(TaskClass::FileInventory, 6000, &policy, &small_catalog).unwrap();
    assert_eq!(dec.selected_model_id, "openai/gpt-4o");
    assert_eq!(
        dec.reason,
        RoutingReason::EscalatedContextExceededLowCostCap
    );

    // Budget ceiling violation fails safely
    let strict_budget_policy = RoutingPolicy {
        max_budget_usd: Some(0.0001),
        ..policy.clone()
    };
    let err = route_task(
        TaskClass::CodeChangeProposal,
        50_000,
        &strict_budget_policy,
        &catalog,
    )
    .unwrap_err();
    assert!(matches!(err, CoreError::BudgetExceeded(_)));

    // Local-only enforcement
    let local_policy = RoutingPolicy {
        require_local_only: true,
        ..policy
    };
    let dec = route_task(TaskClass::FileInventory, 1000, &local_policy, &catalog).unwrap();
    assert_eq!(dec.selected_provider, "local");
    assert_eq!(dec.reason, RoutingReason::LocalOnlyEnforced);
}

#[test]
fn gguf_inspection_validates_magic_header_and_rejects_invalid_files() {
    let dir = tempfile::TempDir::new().unwrap();
    let valid_path = dir.path().join("model.gguf");
    let synthetic = create_synthetic_gguf(3, 150, 10);
    std::fs::write(&valid_path, synthetic).unwrap();

    let meta = inspect_gguf_file(&valid_path).expect("valid gguf header");
    assert_eq!(meta.version, 3);
    assert_eq!(meta.tensor_count, 150);
    assert!(!meta.checksum_sha256.is_empty());

    // Corrupt magic bytes
    let invalid_path = dir.path().join("corrupt.gguf");
    std::fs::write(&invalid_path, b"NOT_A_GGUF_FILE_HEADER_12345678").unwrap();
    let err = inspect_gguf_file(&invalid_path).unwrap_err();
    assert!(matches!(err, CoreError::InvalidModel(_)));

    // Truncated file (< 24 bytes)
    let truncated_path = dir.path().join("truncated.gguf");
    std::fs::write(&truncated_path, b"GGUF123").unwrap();
    let err = inspect_gguf_file(&truncated_path).unwrap_err();
    assert!(matches!(err, CoreError::InvalidModel(_)));
}
