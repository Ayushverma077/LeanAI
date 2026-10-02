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

    // Build a native mock so the same lifecycle is exercised on Windows and
    // Unix. Bind the requested port to exercise the real readiness check.
    let temp_dir = tempfile::tempdir().unwrap();
    let source_path = temp_dir.path().join("mock_llama_server.rs");
    let binary_path = temp_dir
        .path()
        .join(format!("mock_llama_server{}", std::env::consts::EXE_SUFFIX));
    let source = r#"
fn main() {
    let mut args = std::env::args();
    let port = loop {
        if args.next().as_deref() == Some("--port") {
            break args.next().unwrap().parse::<u16>().unwrap();
        }
    };
    let listener = std::net::TcpListener::bind(("127.0.0.1", port)).unwrap();
    for stream in listener.incoming() {
        drop(stream.unwrap());
    }
}
"#;
    std::fs::write(&source_path, source).unwrap();
    let build = std::process::Command::new("rustc")
        .arg(&source_path)
        .arg("-o")
        .arg(&binary_path)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "mock compilation failed: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    let manager = SidecarManager::new();
    manager.set_binary_override(Some(binary_path));

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

    let port = match status {
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
            port
        }
        other => panic!("Expected Ready status, got {other:?}"),
    };

    assert_eq!(manager.status(), status);

    // Stop sidecar
    let stopped = manager.stop().unwrap();
    assert_eq!(stopped, SidecarStatus::Stopped);
    assert_eq!(manager.status(), SidecarStatus::Stopped);

    // The listener belongs to the child, so a successful connection would
    // mean stop left the mock running. This check works on both platforms.
    assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_err());
}
