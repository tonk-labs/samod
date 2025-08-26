//! Shared utilities for WASM storage testing
//!
//! This module provides common utilities, mock implementations, and helpers
//! for testing WASM storage backends.

#![cfg(target_arch = "wasm32")]

use std::collections::HashMap;
use samod::storage::Storage;
use samod_core::StorageKey;
use wasm_bindgen::prelude::*;

/// Initialize logging for WASM tests (using console.log)
pub fn init_wasm_logging() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    tracing_wasm::set_as_global_default();
}

/// Generate test data of specified size
pub fn generate_test_data(size: usize) -> Vec<u8> {
    (0..size).map(|i| (i % 256) as u8).collect()
}

/// Create a hierarchical set of test keys for testing load_range
pub fn create_test_key_set() -> Vec<(StorageKey, Vec<u8>)> {
    vec![
        (StorageKey::from(vec!["test_prefix", "file1"]), b"data1".to_vec()),
        (StorageKey::from(vec!["test_prefix", "file2"]), b"data2".to_vec()),
        (StorageKey::from(vec!["test_prefix", "subdir", "file3"]), b"data3".to_vec()),
        (StorageKey::from(vec!["test_prefix", "subdir", "deeper", "file4"]), b"data4".to_vec()),
        (StorageKey::from(vec!["other_prefix", "file5"]), b"data5".to_vec()),
    ]
}

/// Validate that key splaying matches filesystem storage behavior
pub fn validate_key_splaying(key: &StorageKey, expected_components: &[&str]) {
    let path = samod::storage::key_to_path(key);
    let actual_components: Vec<String> = path
        .components()
        .filter_map(|comp| match comp {
            std::path::Component::Normal(os_str) => os_str.to_str().map(|s| s.to_string()),
            _ => None,
        })
        .collect();
    
    let expected: Vec<String> = expected_components.iter().map(|s| s.to_string()).collect();
    assert_eq!(actual_components, expected, "Key splaying mismatch for key: {:?}", key);
}

/// Test storage implementation with basic operations
pub async fn test_basic_storage_operations<S: Storage + Clone>(storage: S) {
    let key = StorageKey::from(vec!["basic_test"]);
    let data = b"hello world".to_vec();

    // Test put and load
    storage.put(key.clone(), data.clone()).await;
    let loaded = storage.load(key.clone()).await;
    assert_eq!(loaded, Some(data));

    // Test delete
    storage.delete(key.clone()).await;
    let loaded_after_delete = storage.load(key).await;
    assert_eq!(loaded_after_delete, None);
}

/// Test storage implementation with range operations
pub async fn test_storage_load_range<S: Storage + Clone>(storage: S) {
    let test_data = create_test_key_set();
    let base_prefix = StorageKey::from(vec!["test_prefix"]);
    
    // Put all test data
    for (key, data) in &test_data {
        storage.put(key.clone(), data.clone()).await;
    }

    // Load range for test_prefix
    let loaded_range = storage.load_range(base_prefix).await;

    // Should contain 3 items with test_prefix
    let expected_keys: Vec<_> = test_data
        .iter()
        .filter(|(key, _)| base_prefix.is_prefix_of(key))
        .map(|(key, _)| key.clone())
        .collect();
    
    assert_eq!(loaded_range.len(), 3, "Should load 3 items with test_prefix");
    
    for key in expected_keys {
        assert!(loaded_range.contains_key(&key), "Missing key: {:?}", key);
    }
}

/// Test storage with various special characters and edge cases
pub async fn test_storage_edge_cases<S: Storage + Clone>(storage: S) {
    let test_cases = vec![
        // Special characters that should be valid
        (StorageKey::from(vec!["file_with_underscores"]), b"underscore data".to_vec()),
        (StorageKey::from(vec!["file-with-dashes"]), b"dash data".to_vec()),
        (StorageKey::from(vec!["file.with.dots"]), b"dot data".to_vec()),
        (StorageKey::from(vec!["file with spaces"]), b"space data".to_vec()),
        (StorageKey::from(vec!["file123numbers"]), b"number data".to_vec()),
        // Empty data
        (StorageKey::from(vec!["empty_file"]), Vec::new()),
        // Large data
        (StorageKey::from(vec!["large_file"]), generate_test_data(1024 * 10)), // 10KB
    ];

    for (key, data) in test_cases {
        storage.put(key.clone(), data.clone()).await;
        let loaded = storage.load(key.clone()).await;
        assert_eq!(loaded, Some(data), "Failed for key: {:?}", key);
        
        // Clean up
        storage.delete(key).await;
    }
}

/// Test storage with deeply nested paths
pub async fn test_storage_nested_paths<S: Storage + Clone>(storage: S) {
    let key = StorageKey::from(vec!["level1", "level2", "level3", "level4", "file.txt"]);
    let data = b"nested data".to_vec();

    storage.put(key.clone(), data.clone()).await;
    let loaded = storage.load(key).await;
    assert_eq!(loaded, Some(data));
}

/// Test key splaying behavior specifically
pub async fn test_key_splaying_behavior<S: Storage + Clone>(storage: S) {
    // Test that keys are properly splayed (first component split by first two chars)
    let key = StorageKey::from(vec!["abcdef", "file.txt"]);
    let data = b"splayed data".to_vec();
    
    // Validate the key splaying logic
    validate_key_splaying(&key, &["ab", "cdef", "file.txt"]);
    
    // Test that storage works with splayed keys
    storage.put(key.clone(), data.clone()).await;
    let loaded = storage.load(key).await;
    assert_eq!(loaded, Some(data));
}

/// Test storage with non-existent keys
pub async fn test_storage_nonexistent_keys<S: Storage + Clone>(storage: S) {
    let key = StorageKey::from(vec!["nonexistent"]);
    let loaded = storage.load(key).await;
    assert_eq!(loaded, None);
    
    // Test load_range with non-existent prefix
    let prefix = StorageKey::from(vec!["nonexistent_prefix"]);
    let loaded_range = storage.load_range(prefix).await;
    assert!(loaded_range.is_empty());
}

/// Mock JavaScript APIs for testing in non-browser environments
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

/// Mock function for testing JavaScript interop
pub fn mock_js_log(message: &str) {
    #[cfg(target_arch = "wasm32")]
    log(message);
}

/// Comprehensive storage test suite
pub async fn run_comprehensive_storage_tests<S: Storage + Clone>(
    storage: S,
    test_name: &str,
) {
    mock_js_log(&format!("Running comprehensive tests for: {}", test_name));
    
    test_basic_storage_operations(storage.clone()).await;
    mock_js_log("✓ Basic operations test passed");
    
    test_storage_load_range(storage.clone()).await;
    mock_js_log("✓ Load range test passed");
    
    test_storage_edge_cases(storage.clone()).await;
    mock_js_log("✓ Edge cases test passed");
    
    test_storage_nested_paths(storage.clone()).await;
    mock_js_log("✓ Nested paths test passed");
    
    test_key_splaying_behavior(storage.clone()).await;
    mock_js_log("✓ Key splaying test passed");
    
    test_storage_nonexistent_keys(storage).await;
    mock_js_log("✓ Non-existent keys test passed");
    
    mock_js_log(&format!("All tests passed for: {}", test_name));
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_key_splaying_validation() {
        let key = StorageKey::from(vec!["abcdef", "file.txt"]);
        validate_key_splaying(&key, &["ab", "cdef", "file.txt"]);
    }
    
    #[test]
    fn test_test_data_generation() {
        let data = generate_test_data(10);
        assert_eq!(data.len(), 10);
        assert_eq!(data, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }
    
    #[test]
    fn test_key_set_creation() {
        let key_set = create_test_key_set();
        assert_eq!(key_set.len(), 5);
        
        let prefix = StorageKey::from(vec!["test_prefix"]);
        let matching_count = key_set.iter().filter(|(key, _)| prefix.is_prefix_of(key)).count();
        assert_eq!(matching_count, 4);
    }
}