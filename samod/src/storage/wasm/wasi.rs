//! WASI filesystem storage implementation
//!
//! This implementation uses standard filesystem APIs that work with WASI
//! (WebAssembly System Interface) runtimes.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::storage::{Storage, key_to_path};
use samod_core::StorageKey;

/// WASI filesystem storage implementation
#[derive(Clone, Debug)]
pub struct WasiStorage {
    base_path: PathBuf,
}

impl WasiStorage {
    /// Create a new WASI filesystem storage instance
    pub fn new<P: AsRef<Path>>(base_path: P) -> io::Result<Self> {
        let base_path = base_path.as_ref().to_path_buf();

        // Create base directory if it doesn't exist
        fs::create_dir_all(&base_path)?;

        Ok(Self { base_path })
    }

    /// Check if WASI filesystem is available
    /// In WASI environments, standard filesystem APIs should work
    pub fn is_available() -> bool {
        // Check if we can access the filesystem
        fs::metadata(".").is_ok()
    }

    /// Convert a StorageKey to a filesystem path
    fn key_to_fs_path(&self, key: &StorageKey) -> PathBuf {
        let mut path = self.base_path.clone();
        path.push(key_to_path(key));
        path
    }

    /// Recursively collect all files under a directory
    fn collect_files_recursive(
        &self,
        dir_path: &Path,
        prefix: &StorageKey,
        current_prefix: StorageKey,
        result: &mut HashMap<StorageKey, Vec<u8>>,
    ) -> io::Result<()> {
        let entries = fs::read_dir(dir_path)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy().to_string();

            let new_key = current_prefix.with_component(file_name_str);

            if path.is_dir() {
                // Recursively process subdirectory
                self.collect_files_recursive(&path, prefix, new_key, result)?;
            } else if path.is_file() {
                // Read file contents
                if let Ok(data) = fs::read(&path) {
                    if prefix.is_prefix_of(&new_key) {
                        result.insert(new_key, data);
                    }
                }
            }
        }

        Ok(())
    }
}

impl Storage for WasiStorage {
    async fn load(&self, key: StorageKey) -> Option<Vec<u8>> {
        let path = self.key_to_fs_path(&key);

        // Check if it's a file (not a directory)
        if let Ok(metadata) = fs::metadata(&path) {
            if metadata.is_file() {
                return fs::read(path).ok();
            }
        }

        None
    }

    async fn load_range(&self, prefix: StorageKey) -> HashMap<StorageKey, Vec<u8>> {
        let mut result = HashMap::new();
        let path = self.key_to_fs_path(&prefix);

        // Check if the directory exists
        if let Ok(metadata) = fs::metadata(&path) {
            if metadata.is_dir() {
                let _ = self.collect_files_recursive(&path, &prefix, prefix, &mut result);
            }
        }

        result
    }

    async fn put(&self, key: StorageKey, data: Vec<u8>) {
        let path = self.key_to_fs_path(&key);

        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        // Write the file
        let _ = fs::write(path, data);
    }

    async fn delete(&self, key: StorageKey) {
        let path = self.key_to_fs_path(&key);
        let _ = fs::remove_file(path);
    }
}
