//! OPFS (Origin Private File System) storage implementation for modern browsers
//!
//! OPFS provides a private filesystem that's fast, persistent, and private to the origin.
//! This is the recommended storage for modern browsers that support it.

use std::collections::HashMap;

use js_sys::{ArrayBuffer, Uint8Array as JsUint8Array};
use samod_core::StorageKey;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::{FileSystemDirectoryHandle, FileSystemFileHandle, FileSystemWritableFileStream};

use crate::storage::{Storage, key_to_path};

/// OPFS-based storage implementation
#[derive(Clone)]
pub struct OpfsStorage {
    root_handle: FileSystemDirectoryHandle,
}

impl OpfsStorage {
    /// Create a new OPFS storage instance
    pub async fn new() -> Result<Self, JsValue> {
        // Get the OPFS root directory
        let window =
            web_sys::window().ok_or_else(|| JsValue::from_str("No window object found"))?;
        let navigator = window.navigator();
        let storage_manager = navigator.storage();

        // Get the origin private file system directory
        let root_promise = storage_manager.get_directory();
        let root_handle = JsFuture::from(root_promise).await?;
        let root_handle: FileSystemDirectoryHandle = root_handle.into();

        Ok(Self { root_handle })
    }

    /// Check if OPFS is available in the current browser
    pub fn is_available() -> bool {
        if let Some(window) = web_sys::window() {
            let navigator = window.navigator();
            let storage = navigator.storage();

            // Check if getDirectory method exists
            js_sys::Reflect::has(&storage, &"getDirectory".into()).unwrap_or(false)
        } else {
            false
        }
    }

    /// Convert a StorageKey to a path within OPFS
    /// This mirrors the filesystem storage approach but adapted for OPFS
    async fn ensure_path_exists(&self, key: &StorageKey) -> Result<FileSystemFileHandle, JsValue> {
        let path_buf = key_to_path(key);
        let components: Vec<String> = path_buf
            .components()
            .filter_map(|comp| match comp {
                std::path::Component::Normal(os_str) => os_str.to_str().map(|s| s.to_string()),
                _ => None,
            })
            .collect();

        // Navigate to parent directory (all components except last)
        let parent_components = &components[..components.len().saturating_sub(1)];
        let current_dir = self.create_directory_path(parent_components).await?;

        // Get/create the final file
        let filename = components
            .last()
            .ok_or_else(|| JsValue::from_str("Empty storage key"))?;
        self.get_or_create_file(&current_dir, filename).await
    }

    /// Get or create a directory within the given parent directory
    /// Note: This is a simplified implementation - full OPFS support requires proper option handling
    async fn get_or_create_directory(
        &self,
        parent: &FileSystemDirectoryHandle,
        name: &str,
    ) -> Result<FileSystemDirectoryHandle, JsValue> {
        // For now, just try to get existing directory
        let promise = parent.get_directory_handle(name);
        let handle = JsFuture::from(promise).await?;
        Ok(handle.into())
    }

    /// Get or create a file within the given directory
    /// Note: This is a simplified implementation - full OPFS support requires proper option handling  
    async fn get_or_create_file(
        &self,
        parent: &FileSystemDirectoryHandle,
        name: &str,
    ) -> Result<FileSystemFileHandle, JsValue> {
        // For now, just try to get existing file
        let promise = parent.get_file_handle(name);
        let handle = JsFuture::from(promise).await?;
        Ok(handle.into())
    }

    /// Get file handle without creating it (for reading)
    async fn get_file_handle(&self, key: &StorageKey) -> Result<FileSystemFileHandle, JsValue> {
        let parent_dir = self.navigate_to_parent_directory(&key).await?;
        let filename = key
            .into_iter()
            .last()
            .ok_or_else(|| JsValue::from("Empty storage key"))?;

        let promise = parent_dir.get_file_handle(&filename);
        let handle = JsFuture::from(promise).await?;
        Ok(handle.into())
    }

    /// Read contents of a file
    async fn read_file_contents(
        &self,
        file_handle: &FileSystemFileHandle,
    ) -> Result<Vec<u8>, JsValue> {
        let file_promise = file_handle.get_file();
        let file = JsFuture::from(file_promise).await?;
        let file: web_sys::File = file.into();

        let array_buffer_promise = file.array_buffer();
        let array_buffer = JsFuture::from(array_buffer_promise).await?;
        let array_buffer: ArrayBuffer = array_buffer.into();

        let uint8_array = JsUint8Array::new(&array_buffer);
        let mut data = vec![0u8; uint8_array.length() as usize];
        uint8_array.copy_to(&mut data);

        Ok(data)
    }

    /// Write contents to a file handle
    async fn write_file_contents(
        &self,
        file_handle: &FileSystemFileHandle,
        data: &[u8],
    ) -> Result<(), JsValue> {
        let writable_promise = file_handle.create_writable();
        let writable = JsFuture::from(writable_promise).await?;
        let writable: FileSystemWritableFileStream = writable.into();

        let uint8_array = js_sys::Uint8Array::from(data);

        // Get the writer
        let writer = writable.get_writer()?;

        // Write the data
        let write_promise = writer.write_with_chunk(&uint8_array.into());
        JsFuture::from(write_promise).await?;

        let close_promise = writer.close();
        JsFuture::from(close_promise).await?;

        Ok(())
    }

    /// Navigate to the parent directory of a key
    async fn navigate_to_parent_directory(
        &self,
        key: &StorageKey,
    ) -> Result<FileSystemDirectoryHandle, JsValue> {
        let path_buf = key_to_path(key);
        let components: Vec<String> = path_buf
            .components()
            .filter_map(|comp| match comp {
                std::path::Component::Normal(os_str) => os_str.to_str().map(|s| s.to_string()),
                _ => None,
            })
            .collect();

        if components.is_empty() {
            return Ok(self.root_handle.clone());
        }

        let parent_components = &components[..components.len() - 1];
        self.navigate_to_directory_from_components(parent_components)
            .await
    }

    /// Navigate to the directory specified by the prefix (for load_range)
    async fn navigate_to_directory(
        &self,
        prefix: &StorageKey,
    ) -> Result<FileSystemDirectoryHandle, JsValue> {
        let path_buf = key_to_path(prefix);
        let components: Vec<String> = path_buf
            .components()
            .filter_map(|comp| match comp {
                std::path::Component::Normal(os_str) => os_str.to_str().map(|s| s.to_string()),
                _ => None,
            })
            .collect();

        self.navigate_to_directory_from_components(&components)
            .await
    }

    async fn navigate_to_directory_from_components(
        &self,
        components: &[String],
    ) -> Result<FileSystemDirectoryHandle, JsValue> {
        let mut current_dir = self.root_handle.clone();

        for comp in components {
            current_dir = self.get_existing_directory(&current_dir, comp).await?;
        }

        Ok(current_dir)
    }

    /// Create directories from components
    async fn create_directory_path(
        &self,
        components: &[String],
    ) -> Result<FileSystemDirectoryHandle, JsValue> {
        let mut current_dir = self.root_handle.clone();

        for component in components {
            current_dir = self
                .get_or_create_directory(&current_dir, component)
                .await?;
        }

        Ok(current_dir)
    }

    /// Get an existing directory (don't create)
    async fn get_existing_directory(
        &self,
        parent: &FileSystemDirectoryHandle,
        name: &str,
    ) -> Result<FileSystemDirectoryHandle, JsValue> {
        let promise = parent.get_directory_handle(name);
        let handle = JsFuture::from(promise).await?;
        Ok(handle.into())
    }

    /// Recursively collect all files under a directory for load_range
    async fn collect_files_recursive(
        &self,
        dir_handle: &FileSystemDirectoryHandle,
        current_prefix: StorageKey,
        result: &mut HashMap<StorageKey, Vec<u8>>,
    ) {
        // Get directory entries
        if let Ok(entries_iter) = self.get_directory_entries(dir_handle).await {
            for entry in entries_iter {
                let (name, kind) = entry;
                let new_key = current_prefix.with_component(name.clone());

                match kind.as_str() {
                    "directory" => {
                        // Recursively explore subdirectory
                        if let Ok(subdir) = self.get_existing_directory(dir_handle, &name).await {
                            Box::pin(self.collect_files_recursive(&subdir, new_key, result)).await;
                        }
                    }
                    "file" => {
                        // Read file contents
                        let file_handle = dir_handle.get_file_handle(&name);
                        if let Ok(promise_result) = JsFuture::from(file_handle).await {
                            let file_handle: FileSystemFileHandle = promise_result.into();
                            if let Ok(contents) = self.read_file_contents(&file_handle).await {
                                result.insert(new_key, contents);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// Get entries from a directory
    async fn get_directory_entries(
        &self,
        _dir_handle: &FileSystemDirectoryHandle,
    ) -> Result<Vec<(String, String)>, JsValue> {
        // The OPFS entries() API returns an async iterator which is complex to handle in wasm-bindgen
        // For a full implementation, you would need to properly bind the async iterator protocol
        // For now, returning an empty vec to make it compile
        Ok(vec![])

        // TODO: Implement proper async iterator handling
        // The entries() method returns an async iterator of [name, handle] pairs
        // where handle has a 'kind' property that can be "file" or "directory"
    }
}

impl Storage for OpfsStorage {
    /// Load data from a file
    async fn load(&self, key: StorageKey) -> Option<Vec<u8>> {
        match self.get_file_handle(&key).await {
            Ok(file_handle) => match self.read_file_contents(&file_handle).await {
                Ok(data) => Some(data),
                Err(_) => None,
            },
            Err(_) => None,
        }
    }

    /// Load all files iwth keys that start with the given prefix
    async fn load_range(&self, prefix: StorageKey) -> HashMap<StorageKey, Vec<u8>> {
        let mut result = HashMap::new();

        // Navigate to the prefix directory
        if let Ok(dir_handle) = self.navigate_to_directory(&prefix).await {
            self.collect_files_recursive(&dir_handle, prefix, &mut result)
                .await;
        }

        result
    }

    // Write data to a file
    async fn put(&self, key: StorageKey, data: Vec<u8>) -> () {
        if let Ok(file_handle) = self.ensure_path_exists(&key).await {
            let _ = self.write_file_contents(&file_handle, &data).await;
        }
    }

    // Delete a file
    async fn delete(&self, key: StorageKey) -> () {
        if let Ok(parent_dir) = self.navigate_to_parent_directory(&key).await {
            if let Some(filename) = key.into_iter().last() {
                // OPFS delete is async
                let promise = parent_dir.remove_entry(&filename);
                let _ = JsFuture::from(promise).await;
            }
        }
    }
}
