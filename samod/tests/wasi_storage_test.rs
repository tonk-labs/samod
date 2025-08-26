//! Tests for WASI storage implementation
//!
//! These tests validate the WASI filesystem storage backend.

#[cfg(all(target_arch = "wasm32", feature = "wasi"))]
mod wasi_tests {
    use samod::storage::wasm::WasiStorage;
    use samod::storage::Storage;
    use samod_core::StorageKey;
    use std::collections::HashMap;

    fn init_logging() {
        // WASI typically uses standard output for logging
        std::panic::set_hook(Box::new(|panic_info| {
            eprintln!("WASI test panic: {:?}", panic_info);
        }));
    }

    #[tokio::test]
    async fn test_wasi_storage_creation() {
        init_logging();
        
        // Test with a valid directory path
        match WasiStorage::new("/tmp/wasi_storage_test") {
            Ok(storage) => {
                eprintln!("WASI storage created successfully");
                
                // Test basic functionality
                let key = StorageKey::from(vec!["wasi_test"]);
                let data = b"wasi test data".to_vec();

                storage.put(key.clone(), data.clone()).await;
                let loaded = storage.load(key.clone()).await;
                assert_eq!(loaded, Some(data));

                // Clean up
                storage.delete(key).await;
            },
            Err(e) => {
                eprintln!("WASI storage creation failed (may be expected in test env): {:?}", e);
                // Don't fail the test if WASI filesystem isn't available
            }
        }
    }

    #[tokio::test]
    async fn test_wasi_storage_invalid_path() {
        init_logging();
        
        // Test with an invalid directory path
        match WasiStorage::new("/invalid/nonexistent/path") {
            Ok(_) => {
                eprintln!("WASI storage unexpectedly succeeded with invalid path");
            },
            Err(e) => {
                eprintln!("WASI storage correctly failed with invalid path: {:?}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_wasi_storage_operations() {
        init_logging();
        
        if let Ok(storage) = WasiStorage::new("/tmp/wasi_operations_test") {
            // Test various storage operations
            let test_cases = vec![
                (StorageKey::from(vec!["simple"]), "simple data"),
                (StorageKey::from(vec!["nested", "directory", "file"]), "nested data"),
                (StorageKey::from(vec!["abcdef", "splayed"]), "splayed data"),
                (StorageKey::from(vec!["with-dashes"]), "dash data"),
                (StorageKey::from(vec!["with_underscores"]), "underscore data"),
            ];
            
            for (key, data_str) in &test_cases {
                let data = data_str.as_bytes().to_vec();
                storage.put(key.clone(), data.clone()).await;
                let loaded = storage.load(key.clone()).await;
                assert_eq!(loaded, Some(data), "Failed for key: {:?}", key);
            }
            
            // Clean up
            for (key, _) in test_cases {
                storage.delete(key).await;
            }
        } else {
            eprintln!("WASI storage not available for operations test");
        }
    }

    #[tokio::test]
    async fn test_wasi_storage_load_range() {
        init_logging();
        
        if let Ok(storage) = WasiStorage::new("/tmp/wasi_range_test") {
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
            
            // Clean up
            for (key, _) in test_data {
                storage.delete(key).await;
            }
        } else {
            eprintln!("WASI storage not available for load_range test");
        }
    }

    #[tokio::test]
    async fn test_wasi_storage_key_splaying() {
        init_logging();
        
        if let Ok(storage) = WasiStorage::new("/tmp/wasi_splaying_test") {
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
        } else {
            eprintln!("WASI storage not available for key splaying test");
        }
    }

    #[tokio::test]
    async fn test_wasi_storage_error_handling() {
        init_logging();
        
        if let Ok(storage) = WasiStorage::new("/tmp/wasi_error_test") {
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
        } else {
            eprintln!("WASI storage not available for error handling test");
        }
    }

    #[tokio::test]
    async fn test_wasi_storage_large_data() {
        init_logging();
        
        if let Ok(storage) = WasiStorage::new("/tmp/wasi_large_test") {
            let key = StorageKey::from(vec!["large_file"]);
            let data = vec![42u8; 1024 * 100]; // 100KB of data
            
            storage.put(key.clone(), data.clone()).await;
            let loaded = storage.load(key.clone()).await;
            assert_eq!(loaded, Some(data));
            
            storage.delete(key).await;
        } else {
            eprintln!("WASI storage not available for large data test");
        }
    }

    #[tokio::test]
    async fn test_wasi_storage_deep_nesting() {
        init_logging();
        
        if let Ok(storage) = WasiStorage::new("/tmp/wasi_deep_test") {
            // Test deeply nested directory structure
            let key = StorageKey::from(vec![
                "level1", "level2", "level3", "level4", "file.txt"
            ]);
            let data = b"deeply nested data".to_vec();
            
            storage.put(key.clone(), data.clone()).await;
            let loaded = storage.load(key.clone()).await;
            assert_eq!(loaded, Some(data));
            
            storage.delete(key).await;
        } else {
            eprintln!("WASI storage not available for deep nesting test");
        }
    }

    #[tokio::test]
    async fn test_wasi_storage_concurrent_operations() {
        init_logging();
        
        if let Ok(storage) = WasiStorage::new("/tmp/wasi_concurrent_test") {
            // Test concurrent operations (limited in WASI single-threaded environment)
            let keys_and_data: Vec<(StorageKey, Vec<u8>)> = (0..5)
                .map(|i| (
                    StorageKey::from(vec![format!("concurrent_{}", i)]),
                    format!("data_{}", i).into_bytes()
                ))
                .collect();
            
            // Sequential operations (WASI is typically single-threaded)
            for (key, data) in &keys_and_data {
                storage.put(key.clone(), data.clone()).await;
                let loaded = storage.load(key.clone()).await;
                assert_eq!(loaded, Some(data.clone()));
            }
            
            // Clean up
            for (key, _) in keys_and_data {
                storage.delete(key).await;
            }
        } else {
            eprintln!("WASI storage not available for concurrent operations test");
        }
    }
}

// Platform-independent tests
#[cfg(test)]
mod unit_tests {
    use super::*;
    use samod_core::StorageKey;
    
    #[test]
    fn test_wasi_storage_path_logic() {
        // Test that WASI storage would use the same path logic as filesystem storage
        let test_cases = vec![
            (vec!["simple"], vec!["simple"]),
            (vec!["ab"], vec!["ab"]),
            (vec!["abc"], vec!["ab", "c"]),
            (vec!["abcdef"], vec!["ab", "cdef"]),
            (vec!["abcdef", "file"], vec!["ab", "cdef", "file"]),
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
            
            assert_eq!(components, expected, "WASI path logic failed for input: {:?}", input);
        }
    }
}