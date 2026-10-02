//! Shared PFH5 fixtures for integration tests.
#![allow(clippy::unwrap_used)]
use std::{fs, path::Path};
use wh3_core::catalog::Catalog;

/// PFH5 writer with header dependencies; payload bytes are zero-filled to the declared sizes.
pub fn fixture(path: &Path, dependencies: &[&str], files: &[(&str, u32)]) {
    let mut dependency_bytes = Vec::new();
    for dependency in dependencies {
        dependency_bytes.extend_from_slice(dependency.as_bytes());
        dependency_bytes.push(0);
    }
    let mut index = Vec::new();
    let mut payload = 0;
    for (name, size) in files {
        index.extend_from_slice(&size.to_le_bytes());
        index.push(0);
        index.extend_from_slice(name.as_bytes());
        index.push(0);
        payload += *size as usize;
    }
    let mut bytes = b"PFH5".to_vec();
    for word in [
        3,
        dependencies.len() as u32,
        dependency_bytes.len() as u32,
        files.len() as u32,
        index.len() as u32,
        0,
    ] {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    bytes.extend(dependency_bytes);
    bytes.extend(index);
    bytes.resize(bytes.len() + payload, 0);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

pub fn index_of(catalog: &Catalog, name: &str) -> usize {
    catalog.by_name[name][0]
}
