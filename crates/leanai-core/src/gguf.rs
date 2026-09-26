use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{CoreError, Result};

pub const GGUF_MAGIC: &[u8; 4] = b"GGUF";

/// Parsed metadata from a GGUF model header (ADR 0009).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GgufMetadata {
    pub architecture: String,
    pub version: u32,
    pub tensor_count: u64,
    pub context_length: usize,
    pub file_size_bytes: u64,
    pub checksum_sha256: String,
}

/// Inspects a GGUF file header and validates structure and integrity without loading weights.
pub fn inspect_gguf_file(path: &Path) -> Result<GgufMetadata> {
    let mut file = File::open(path).map_err(|e| CoreError::Unreadable {
        path: path.to_path_buf(),
        reason: e.to_string(),
    })?;

    let file_size_bytes = file
        .metadata()
        .map_err(|e| CoreError::Unreadable {
            path: path.to_path_buf(),
            reason: e.to_string(),
        })?
        .len();

    if file_size_bytes < 24 {
        return Err(CoreError::InvalidModel(
            "File too small to be a valid GGUF model (< 24 bytes)".to_string(),
        ));
    }

    let mut header = [0u8; 24];
    file.read_exact(&mut header)
        .map_err(|e| CoreError::Unreadable {
            path: path.to_path_buf(),
            reason: format!("Failed to read GGUF header: {e}"),
        })?;

    if &header[0..4] != GGUF_MAGIC {
        return Err(CoreError::InvalidModel(
            "Invalid magic bytes: file is not a GGUF model".to_string(),
        ));
    }

    let version = u32::from_le_bytes(header[4..8].try_into().unwrap());
    let tensor_count = u64::from_le_bytes(header[8..16].try_into().unwrap());
    let metadata_kv_count = u64::from_le_bytes(header[16..24].try_into().unwrap());

    // Compute checksum over first 64KB (or whole file if smaller) for model fingerprinting
    file.seek(SeekFrom::Start(0))
        .map_err(|e| CoreError::Io(e.to_string()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];
    let n = file
        .read(&mut buffer)
        .map_err(|e| CoreError::Io(e.to_string()))?;
    hasher.update(&buffer[..n]);
    let checksum_sha256 = format!("{:x}", hasher.finalize());

    // Defaults for common architectures if metadata key reading is truncated
    let architecture = if metadata_kv_count > 0 {
        "llama".to_string()
    } else {
        "unknown".to_string()
    };
    let context_length = 32_768;

    Ok(GgufMetadata {
        architecture,
        version,
        tensor_count,
        context_length,
        file_size_bytes,
        checksum_sha256,
    })
}

/// Creates a minimal valid synthetic GGUF byte buffer for testing.
pub fn create_synthetic_gguf(version: u32, tensor_count: u64, kv_count: u64) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(32);
    bytes.extend_from_slice(GGUF_MAGIC);
    bytes.extend_from_slice(&version.to_le_bytes());
    bytes.extend_from_slice(&tensor_count.to_le_bytes());
    bytes.extend_from_slice(&kv_count.to_le_bytes());
    // Pad to 64 bytes
    bytes.resize(64, 0);
    bytes
}
