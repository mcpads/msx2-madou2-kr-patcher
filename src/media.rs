use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{error::Error, path::Path};

pub fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[derive(Deserialize)]
struct MediaSpec {
    files: Vec<SourceFile>,
}

#[derive(Deserialize)]
struct SourceFile {
    filename: String,
    size_bytes: usize,
    sha256: String,
}

pub fn read_source(root: &Path) -> Result<Vec<u8>, Box<dyn Error>> {
    let spec: MediaSpec = serde_json::from_str(include_str!("../config/source-media.json"))?;
    if spec.files.len() != 1 {
        return Err("Expected exactly one source disk specification".into());
    }
    let file = &spec.files[0];
    let bytes = std::fs::read(root.join(&file.filename))?;
    if bytes.len() != file.size_bytes || sha256(&bytes) != file.sha256 {
        return Err("Source size or SHA-256 mismatch".into());
    }
    Ok(bytes)
}
