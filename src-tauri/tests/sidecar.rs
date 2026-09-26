use std::path::PathBuf;
use tempfile::NamedTempFile;

use leanai_core::gguf::create_synthetic_gguf;
use leanai_desktop_lib::sidecar_manager::{SidecarManager, SidecarStatus};

#[test]
fn sidecar_preflight_rejects_missing_file() {
    let manager = SidecarManager::new();
    let missing_path = PathBuf::from("/path/to/definitely/nonexistent/model.gguf");

    let err = manager
        .start("m1", "Test Model", &missing_path, None)
        .unwrap_err();

    assert_eq!(err.code, "model_not_found");
    assert!(matches!(manager.status(), SidecarStatus::Error { .. }));
}

#[test]
fn sidecar_preflight_rejects_corrupted_file() {
    let manager = SidecarManager::new();
    let temp_file = NamedTempFile::new().unwrap();
    // Write corrupted bytes (not GGUF)
    std::fs::write(temp_file.path(), b"NOT_A_GGUF_FILE_HEADER_DATA").unwrap();

    let err = manager
        .start("m1", "Corrupt Model", temp_file.path(), None)
        .unwrap_err();

    assert_eq!(err.code, "invalid_model");
    assert!(matches!(manager.status(), SidecarStatus::Error { .. }));
}

#[test]
fn sidecar_preflight_accepts_valid_synthetic_gguf() {
    let temp_file = NamedTempFile::new().unwrap();
    let bytes = create_synthetic_gguf(3, 32, 1);
    std::fs::write(temp_file.path(), bytes).unwrap();

    // Verify leanai_core inspects it cleanly
    let header = leanai_core::gguf::inspect_gguf_file(temp_file.path()).unwrap();
    assert_eq!(header.architecture, "llama");
    assert_eq!(header.tensor_count, 32);
}

#[test]
fn sidecar_lifecycle_with_mock_loopback_binary() {
    let model_file = NamedTempFile::new().unwrap();
    let bytes = create_synthetic_gguf(3, 10, 1);
    std::fs::write(model_file.path(), bytes).unwrap();

    // Create a mock executable script that simulates the llama-server interface
    let temp_dir = tempfile::tempdir().unwrap();
    let script_path = temp_dir.path().join("mock_llama_server");
    let script_content = r#"#!/usr/bin/env python3
import sys, time
# Simulates long-running background sidecar process
while True:
    time.sleep(1)
"#;
    std::fs::write(&script_path, script_content).unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&script_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script_path, perms).unwrap();
    }

    let manager = SidecarManager::new();
    manager.set_binary_override(Some(script_path));
    manager.set_mock_healthy(true);

    assert_eq!(manager.status(), SidecarStatus::Stopped);

    // Start sidecar
    let status = manager
        .start(
            "test-model",
            "Synthetic Llama",
            model_file.path(),
            Some(2048),
        )
        .unwrap();

    let pid = match status {
        SidecarStatus::Ready {
            port,
            pid,
            ref model_id,
            ref display_name,
        } => {
            assert!(port > 1024);
            assert!(pid > 0);
            assert_eq!(model_id, "test-model");
            assert_eq!(display_name, "Synthetic Llama");
            pid
        }
        other => panic!("Expected Ready status, got {other:?}"),
    };

    assert_eq!(manager.status(), status);

    // Stop sidecar
    let stopped = manager.stop().unwrap();
    assert_eq!(stopped, SidecarStatus::Stopped);
    assert_eq!(manager.status(), SidecarStatus::Stopped);

    // Verify process was terminated and no orphan was left
    #[cfg(unix)]
    {
        let check = std::process::Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .status();
        if let Ok(c) = check {
            assert!(
                !c.success(),
                "Process {pid} should not be running after stop()"
            );
        }
    }
}
