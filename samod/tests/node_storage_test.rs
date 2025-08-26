//! Tests for Node.js filesystem storage implementation
//!
//! These tests can run on any platform and validate the Node.js storage logic.

#[cfg(all(target_arch = "wasm32", feature = "wasm-node"))]
mod node_tests {
    use samod::storage::wasm::NodeFsStorage;
    use samod::storage::Storage;
    use samod_core::StorageKey;
    use std::collections::HashMap;

    fn init_logging() {
        // For Node.js WASM, we'd use console logging
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    }

    #[tokio::test]
    async fn test_node_storage_creation() {
        init_logging();
        
        let storage = NodeFsStorage::new("/tmp/node_storage_test");
        
        // Test basic functionality
        let key = StorageKey::from(vec!["node_test"]);
        let data = b"node test data".to_vec();

        storage.put(key.clone(), data.clone()).await;
        let loaded = storage.load(key.clone()).await;
        assert_eq!(loaded, Some(data));

        // Clean up
        storage.delete(key).await;
    }

    #[tokio::test]
    async fn test_node_storage_path_validation() {
        init_logging();
        
        let storage = NodeFsStorage::new("/tmp/node_path_test");
        
        // Test various path scenarios that should work in Node.js
        let test_cases = vec![
            (StorageKey::from(vec!["simple"]), "simple file"),
            (StorageKey::from(vec!["with", "nested", "directories"]), "nested data"),
            (StorageKey::from(vec!["abcdef", "splayed"]), "splayed data"), // First component splayed
            (StorageKey::from(vec!["file-with-dashes"]), "dash data"),
            (StorageKey::from(vec!["file_with_underscores"]), "underscore data"),
            (StorageKey::from(vec!["file.with.extensions.dat"]), "extension data"),
        ];
        
        for (key, data_str) in test_cases {
            let data = data_str.as_bytes().to_vec();
            storage.put(key.clone(), data.clone()).await;
            let loaded = storage.load(key.clone()).await;
            assert_eq!(loaded, Some(data), "Failed for key: {:?}", key);
            storage.delete(key).await;
        }
    }

    #[tokio::test]
    async fn test_node_storage_load_range() {
        init_logging();
        
        let storage = NodeFsStorage::new("/tmp/node_range_test");
        
        // Create test data
        let test_data = vec![
            (StorageKey::from(vec!["prefix", "file1"]), b"data1".to_vec()),
            (StorageKey::from(vec!["prefix", "file2"]), b"data2".to_vec()),
            (StorageKey::from(vec!["prefix", "subdir", "file3"]), b"data3".to_vec()),
            (StorageKey::from(vec!["other", "file4"]), b"data4".to_vec()),
        ];
        
        // Put all data
        for (key, data) in &test_data {
            storage.put(key.clone(), data.clone()).await;
        }
        
        // Test load_range
        let prefix = StorageKey::from(vec!["prefix"]);
        let loaded_range = storage.load_range(prefix.clone()).await;
        
        // Should contain 3 items with "prefix"
        let expected_count = test_data.iter()
            .filter(|(key, _)| prefix.is_prefix_of(key))
            .count();
        assert_eq!(loaded_range.len(), expected_count);
        
        // Verify contents
        for (key, data) in &test_data {
            if prefix.is_prefix_of(key) {
                assert_eq!(loaded_range.get(key), Some(data));
            }
        }
        
        // Clean up
        for (key, _) in test_data {
            storage.delete(key).await;
        }
    }

    #[tokio::test]
    async fn test_node_storage_key_splaying() {
        init_logging();
        
        let storage = NodeFsStorage::new("/tmp/node_splaying_test");
        
        // Test key splaying behavior
        let key = StorageKey::from(vec!["abcdef", "file.txt"]);
        let data = b"splayed data".to_vec();
        
        // Verify the key gets splayed correctly
        let path = samod::storage::key_to_path(&key);
        let components: Vec<String> = path.components()
            .filter_map(|comp| match comp {
                std::path::Component::Normal(os_str) => os_str.to_str().map(|s| s.to_string()),
                _ => None,
            })
            .collect();
        
        assert_eq!(components, vec!["ab", "cdef", "file.txt"]);
        
        // Test that storage works with splayed keys
        storage.put(key.clone(), data.clone()).await;
        let loaded = storage.load(key.clone()).await;
        assert_eq!(loaded, Some(data));
        
        storage.delete(key).await;
    }

    #[tokio::test]
    async fn test_node_storage_error_handling() {
        init_logging();
        
        let storage = NodeFsStorage::new("/tmp/node_error_test");
        
        // Test loading non-existent file
        let key = StorageKey::from(vec!["nonexistent"]);
        let loaded = storage.load(key).await;
        assert_eq!(loaded, None);
        
        // Test deleting non-existent file (should not panic)
        let key = StorageKey::from(vec!["nonexistent_delete"]);
        storage.delete(key).await; // Should complete without error
        
        // Test load_range with non-existent prefix
        let prefix = StorageKey::from(vec!["nonexistent_prefix"]);
        let range = storage.load_range(prefix).await;
        assert!(range.is_empty());
    }

    #[tokio::test]
    async fn test_node_storage_large_data() {
        init_logging();
        
        let storage = NodeFsStorage::new("/tmp/node_large_test");
        
        let key = StorageKey::from(vec!["large_file"]);
        let data = vec![42u8; 1024 * 100]; // 100KB of data
        
        storage.put(key.clone(), data.clone()).await;
        let loaded = storage.load(key.clone()).await;
        assert_eq!(loaded, Some(data));
        
        storage.delete(key).await;
    }

    #[tokio::test]
    async fn test_node_storage_empty_data() {
        init_logging();
        
        let storage = NodeFsStorage::new("/tmp/node_empty_test");
        
        let key = StorageKey::from(vec!["empty_file"]);
        let data = Vec::new();
        
        storage.put(key.clone(), data.clone()).await;
        let loaded = storage.load(key.clone()).await;
        assert_eq!(loaded, Some(data));
        
        storage.delete(key).await;
    }

    #[tokio::test]
    async fn test_node_storage_concurrent_operations() {
        init_logging();
        
        let storage = NodeFsStorage::new("/tmp/node_concurrent_test");
        
        // Test concurrent operations
        let mut handles = Vec::new();
        
        for i in 0..5 {
            let storage_clone = storage.clone();
            let key = StorageKey::from(vec![format!("concurrent_{}", i)]);
            let data = format!("data_{}", i).into_bytes();
            
            let handle = tokio::spawn(async move {
                storage_clone.put(key.clone(), data.clone()).await;
                let loaded = storage_clone.load(key.clone()).await;
                assert_eq!(loaded, Some(data));
                storage_clone.delete(key).await;
            });
            
            handles.push(handle);
        }
        
        // Wait for all operations to complete
        for handle in handles {
            handle.await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_node_storage_special_characters() {
        init_logging();
        
        let storage = NodeFsStorage::new("/tmp/node_special_test");
        
        // Test with various characters that should work in Node.js filesystems
        let test_cases = vec![
            "file_with_underscores",
            "file-with-dashes", 
            "file.with.dots",
            "file123numbers",
            // Note: spaces might be problematic in some Node.js environments
            // depending on how paths are handled
        ];
        
        for filename in test_cases {
            let key = StorageKey::from(vec![filename]);
            let data = format!("data for {}", filename).into_bytes();
            
            storage.put(key.clone(), data.clone()).await;
            let loaded = storage.load(key.clone()).await;
            assert_eq!(loaded, Some(data), "Failed for filename: {}", filename);
            storage.delete(key).await;
        }
    }

    #[tokio::test]
    async fn test_node_storage_deep_nesting() {
        init_logging();
        
        let storage = NodeFsStorage::new("/tmp/node_deep_test");
        
        // Test deeply nested directory structure
        let key = StorageKey::from(vec![
            "level1", "level2", "level3", "level4", "level5", "file.txt"
        ]);
        let data = b"deeply nested data".to_vec();
        
        storage.put(key.clone(), data.clone()).await;
        let loaded = storage.load(key.clone()).await;
        assert_eq!(loaded, Some(data));
        
        storage.delete(key).await;
    }
}

// Tests that can run on any platform (non-WASM)
#[cfg(test)]
mod unit_tests {
    use super::*;
    use samod_core::StorageKey;
    
    #[test]
    fn test_storage_key_to_path_consistency() {
        // Test that our key splaying logic is consistent
        let test_cases = vec![
            (vec!["ab"], vec!["ab"]),
            (vec!["abc"], vec!["ab", "c"]),
            (vec!["abcdef"], vec!["ab", "cdef"]),
            (vec!["abcdef", "file"], vec!["ab", "cdef", "file"]),
            (vec!["abcdef", "nested", "file.txt"], vec!["ab", "cdef", "nested", "file.txt"]),
        ];
        
        for (input, expected) in test_cases {
            let key = StorageKey::from(input.clone());
            let path = samod::storage::key_to_path(&key);
            let components: Vec<String> = path.components()
                .filter_map(|comp| match comp {
                    std::path::Component::Normal(os_str) => os_str.to_str().map(|s| s.to_string()),
                    _ => None,
                })
                .collect();
            
            assert_eq!(components, expected, "Key splaying failed for input: {:?}", input);
        }
    }
}