//! Node.js filesystem storage implementation for WASM
//!
//! This implementation uses JavaScript filesystem bindings to provide
//! persistent storage when running in Node.js environments.

use js_sys::{Array, Promise, Uint8Array};
use std::collections::HashMap;
use std::path::PathBuf;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use crate::storage::{Storage, key_to_path};
use samod_core::StorageKey;

// JavaScript filesystem bindings
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["require", "fs", "promises"], js_name = readFile)]
    fn read_file_node(path: &str) -> Promise;

    #[wasm_bindgen(js_namespace = ["require", "fs", "promises"], js_name = writeFile)]
    fn write_file_node(path: &str, data: &Uint8Array) -> Promise;

    #[wasm_bindgen(js_namespace = ["require", "fs", "promises"], js_name = mkdir)]
    fn mkdir_node(path: &str, options: &JsValue) -> Promise;

    #[wasm_bindgen(js_namespace = ["require", "fs", "promises"], js_name = unlink)]
    fn unlink_node(path: &str) -> Promise;

    #[wasm_bindgen(js_namespace = ["require", "fs", "promises"], js_name = readdir)]
    fn readdir_node(path: &str, options: &JsValue) -> Promise;

    #[wasm_bindgen(js_namespace = ["require", "fs", "promises"], js_name = stat)]
    fn stat_node(path: &str) -> Promise;

    #[wasm_bindgen(js_namespace = ["require", "path"], js_name = join)]
    fn path_join(parts: &Array) -> String;

    #[wasm_bindgen(js_namespace = ["require", "path"], js_name = dirname)]
    fn path_dirname(path: &str) -> String;
}

// Helper to check if we're in a Node.js environment
#[wasm_bindgen(inline_js = "
    export function is_node() {
        return typeof process !== 'undefined' && process.versions && process.versions.node;
    }
")]
extern "C" {
    fn is_node() -> bool;
}

/// Node.js filesystem storage implementation
#[derive(Clone)]
pub struct NodeFsStorage {
    base_path: String,
}

impl NodeFsStorage {
    /// Create a new Node.js filesystem storage instance
    pub fn new(base_path: impl Into<String>) -> Self {
        Self {
            base_path: base_path.into(),
        }
    }

    /// Check if we're running in a Node.js environment
    pub fn is_available() -> bool {
        is_node()
    }

    /// Convert a StorageKey to a filesystem path
    fn key_to_fs_path(&self, key: &StorageKey) -> String {
        let path_buf = key_to_path(key);
        let components = Array::new();

        // Add base path as first component
        components.push(&JsValue::from_str(&self.base_path));

        // Add all path components
        for component in path_buf.components() {
            if let std::path::Component::Normal(os_str) = component {
                if let Some(s) = os_str.to_str() {
                    components.push(&JsValue::from_str(s));
                }
            }
        }

        path_join(&components)
    }

    /// Create parent directories if they don't exist
    async fn ensure_parent_dir(&self, path: &str) -> Result<(), JsValue> {
        let parent = path_dirname(path);

        // Create directory with recursive option
        let options = js_sys::Object::new();
        js_sys::Reflect::set(&options, &"recursive".into(), &true.into())?;

        JsFuture::from(mkdir_node(&parent, &options)).await?;
        Ok(())
    }

    /// Check if a path is a directory
    async fn is_directory(&self, path: &str) -> Result<bool, JsValue> {
        match JsFuture::from(stat_node(path)).await {
            Ok(stat) => {
                // Call isDirectory() on the stat object
                let is_dir_fn = js_sys::Reflect::get(&stat, &"isDirectory".into())?;
                let is_dir_fn = is_dir_fn.dyn_into::<js_sys::Function>()?;
                let is_dir = is_dir_fn.call0(&stat)?;
                Ok(is_dir.as_bool().unwrap_or(false))
            }
            Err(_) => Ok(false),
        }
    }

    /// Recursively collect all files under a directory
    async fn collect_files_recursive(
        &self,
        dir_path: &str,
        prefix: &StorageKey,
        current_components: Vec<String>,
        result: &mut HashMap<StorageKey, Vec<u8>>,
    ) -> Result<(), JsValue> {
        // Read directory with file types
        let options = js_sys::Object::new();
        js_sys::Reflect::set(&options, &"withFileTypes".into(), &true.into())?;

        let entries = JsFuture::from(readdir_node(dir_path, &options)).await?;
        let entries_array: Array = entries.into();

        for i in 0..entries_array.length() {
            let entry = entries_array.get(i);

            // Get name and check if it's a directory
            let name = js_sys::Reflect::get(&entry, &"name".into())?
                .as_string()
                .unwrap_or_default();

            let is_dir_fn = js_sys::Reflect::get(&entry, &"isDirectory".into())?;
            let is_dir_fn = is_dir_fn.dyn_into::<js_sys::Function>()?;
            let is_dir = is_dir_fn.call0(&entry)?.as_bool().unwrap_or(false);

            let mut new_components = current_components.clone();
            new_components.push(name.clone());

            let entry_array = Array::new();
            entry_array.push(&JsValue::from_str(dir_path));
            entry_array.push(&JsValue::from_str(&name));
            let entry_path = path_join(&entry_array);

            if is_dir {
                // Recursively process subdirectory
                Box::pin(self.collect_files_recursive(&entry_path, prefix, new_components, result))
                    .await?;
            } else {
                // Read file contents
                if let Ok(data) = JsFuture::from(read_file_node(&entry_path)).await {
                    let uint8_array: Uint8Array = data.into();
                    let mut file_data = vec![0u8; uint8_array.length() as usize];
                    uint8_array.copy_to(&mut file_data);

                    // Reconstruct the StorageKey from components
                    let storage_key = self.components_to_storage_key(&new_components);
                    if prefix.is_prefix_of(&storage_key) {
                        result.insert(storage_key, file_data);
                    }
                }
            }
        }

        Ok(())
    }

    /// Convert path components back to a StorageKey
    fn components_to_storage_key(&self, components: &[String]) -> StorageKey {
        // The first component might be splayed (e.g., "ab" and "cdef")
        // We need to reconstruct the original key
        let mut key_components = Vec::new();

        if components.len() >= 2 && components[0].len() == 2 {
            // Looks like a splayed first component
            key_components.push(format!("{}{}", components[0], components[1]));
            key_components.extend_from_slice(&components[2..]);
        } else {
            key_components.extend_from_slice(components);
        }

        StorageKey::from(key_components)
    }
}

impl Storage for NodeFsStorage {
    async fn load(&self, key: StorageKey) -> Option<Vec<u8>> {
        let path = self.key_to_fs_path(&key);

        match JsFuture::from(read_file_node(&path)).await {
            Ok(data) => {
                let uint8_array: Uint8Array = data.into();
                let mut vec_data = vec![0u8; uint8_array.length() as usize];
                uint8_array.copy_to(&mut vec_data);
                Some(vec_data)
            }
            Err(_) => None,
        }
    }

    async fn load_range(&self, prefix: StorageKey) -> HashMap<StorageKey, Vec<u8>> {
        let mut result = HashMap::new();
        let dir_path = self.key_to_fs_path(&prefix);

        // Check if the directory exists
        if self.is_directory(&dir_path).await.unwrap_or(false) {
            let _ = self
                .collect_files_recursive(&dir_path, &prefix, Vec::new(), &mut result)
                .await;
        }

        result
    }

    async fn put(&self, key: StorageKey, data: Vec<u8>) {
        let path = self.key_to_fs_path(&key);

        // Ensure parent directory exists
        if self.ensure_parent_dir(&path).await.is_ok() {
            let uint8_array = Uint8Array::from(&data[..]);
            let _ = JsFuture::from(write_file_node(&path, &uint8_array)).await;
        }
    }

    async fn delete(&self, key: StorageKey) {
        let path = self.key_to_fs_path(&key);
        let _ = JsFuture::from(unlink_node(&path)).await;
    }
}
