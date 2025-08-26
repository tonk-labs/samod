//! IndexedDB storage implementation for browsers
//!
//! IndexedDB is a low-level API for client-side storage of significant amounts of structured data.
//! This implementation provides a fallback for browsers that don't support OPFS.

use js_sys::{Array, Uint8Array};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;
use web_sys::{IdbDatabase, IdbKeyRange, IdbRequest};

use crate::storage::{Storage, key_to_path};
use samod_core::StorageKey;

const DB_NAME: &str = "samod_storage";
const DB_VERSION: f64 = 1.0;
const STORE_NAME: &str = "documents";

/// IndexedDB-based storage implementation
#[derive(Clone)]
pub struct IndexedDbStorage {
    db_name: String,
}

impl IndexedDbStorage {
    /// Create a new IndexedDB storage instance
    pub async fn new() -> Result<Self, JsValue> {
        Self::with_name(DB_NAME).await
    }

    /// Create a new IndexedDB storage instance with a custom database name
    pub async fn with_name(name: &str) -> Result<Self, JsValue> {
        let storage = Self {
            db_name: name.to_string(),
        };

        // Initialize the database
        storage.ensure_database().await?;

        Ok(storage)
    }

    /// Check if IndexedDB is available in the current browser
    pub fn is_available() -> bool {
        if let Some(window) = web_sys::window() {
            window.indexed_db().ok().flatten().is_some()
        } else {
            false
        }
    }

    /// Ensure the database and object store exist
    async fn ensure_database(&self) -> Result<IdbDatabase, JsValue> {
        let window =
            web_sys::window().ok_or_else(|| JsValue::from_str("No window object found"))?;

        let idb_factory = window
            .indexed_db()?
            .ok_or_else(|| JsValue::from_str("IndexedDB not available"))?;

        // Open database with version  
        let open_request = idb_factory.open_with_u32(&self.db_name, DB_VERSION as u32)?;

        // Handle database upgrade (create object store if needed)
        let closure = Closure::once(move |event: web_sys::Event| {
            let target = event.target().unwrap();
            let request: IdbRequest = target.dyn_into().unwrap();
            let result = request.result().unwrap();
            let db: IdbDatabase = result.dyn_into().unwrap();

            // Create object store if it doesn't exist
            if !db.object_store_names().contains(&STORE_NAME.to_string()) {
                db.create_object_store(STORE_NAME).unwrap();
            }
        });

        open_request.set_onupgradeneeded(Some(closure.as_ref().unchecked_ref()));
        closure.forget(); // Prevent closure from being dropped

        // Wait for the database to open
        let db = Self::open_request_to_future(open_request).await?;
        let db: IdbDatabase = db.into();

        Ok(db)
    }

    /// Convert a StorageKey to a string key for IndexedDB
    fn storage_key_to_string(&self, key: &StorageKey) -> String {
        // Use the same path structure as filesystem storage for consistency
        let path_buf = key_to_path(key);
        path_buf.to_string_lossy().to_string()
    }

    /// Convert a string key back to a StorageKey
    fn string_to_storage_key(&self, key_str: &str) -> StorageKey {
        // Parse the path back into components
        let path = std::path::Path::new(key_str);
        let mut components = Vec::new();

        // Track if we're processing the first component (which may be splayed)
        let mut is_first = true;
        let mut first_part = String::new();

        for component in path.components() {
            if let std::path::Component::Normal(os_str) = component {
                if let Some(s) = os_str.to_str() {
                    if is_first {
                        // First component might be the first part of a splayed key
                        if s.len() == 2 {
                            first_part = s.to_string();
                            continue;
                        } else {
                            components.push(s.to_string());
                            is_first = false;
                        }
                    } else if !first_part.is_empty() {
                        // This is the second part of a splayed first component
                        components.push(format!("{}{}", first_part, s));
                        first_part.clear();
                        is_first = false;
                    } else {
                        components.push(s.to_string());
                    }
                }
            }
        }

        StorageKey::from(components)
    }

    /// Read data from IndexedDB
    async fn read_data(&self, key: &str) -> Result<Option<Vec<u8>>, JsValue> {
        let db = self.ensure_database().await?;

        // Create a read transaction
        let transaction = db.transaction_with_str(STORE_NAME)?;
        let store = transaction.object_store(STORE_NAME)?;

        // Get the data
        let request = store.get(&JsValue::from_str(key))?;
        let result = Self::request_to_future(request).await?;

        if result.is_undefined() || result.is_null() {
            Ok(None)
        } else {
            // Convert from Uint8Array to Vec<u8>
            let uint8_array: Uint8Array = result.into();
            let mut data = vec![0u8; uint8_array.length() as usize];
            uint8_array.copy_to(&mut data);
            Ok(Some(data))
        }
    }

    /// Write data to IndexedDB
    async fn write_data(&self, key: &str, data: &[u8]) -> Result<(), JsValue> {
        let db = self.ensure_database().await?;

        // Create a readwrite transaction
        let transaction =
            db.transaction_with_str_and_mode(STORE_NAME, web_sys::IdbTransactionMode::Readwrite)?;
        let store = transaction.object_store(STORE_NAME)?;

        // Convert data to Uint8Array
        let uint8_array = Uint8Array::from(data);

        // Put the data
        let request = store.put_with_key(&uint8_array, &JsValue::from_str(key))?;

        Self::request_to_future(request).await?;
        Ok(())
    }

    /// Delete data from IndexedDB
    async fn delete_data(&self, key: &str) -> Result<(), JsValue> {
        let db = self.ensure_database().await?;

        // Create a readwrite transaction
        let transaction =
            db.transaction_with_str_and_mode(STORE_NAME, web_sys::IdbTransactionMode::Readwrite)?;
        let store = transaction.object_store(STORE_NAME)?;

        // Delete the data
        let request = store.delete(&JsValue::from_str(key))?;
        Self::request_to_future(request).await?;
        Ok(())
    }

    /// Get all keys that start with a prefix
    async fn get_keys_with_prefix(&self, prefix: &str) -> Result<Vec<String>, JsValue> {
        let db = self.ensure_database().await?;

        // Create a read transaction
        let transaction = db.transaction_with_str(STORE_NAME)?;
        let store = transaction.object_store(STORE_NAME)?;

        // Create a key range that starts with our prefix
        // For prefix matching, we use a range from prefix to prefix + '\u{FFFF}'
        let lower = JsValue::from_str(prefix);
        let upper = JsValue::from_str(&format!("{}\u{FFFF}", prefix));
        let key_range = IdbKeyRange::bound(&lower, &upper)?;

        // Get all keys in this range
        let request = store.get_all_keys_with_key(&key_range)?;
        let result = Self::request_to_future(request).await?;

        let array: Array = result.into();
        let mut keys = Vec::new();

        for i in 0..array.length() {
            if let Some(key) = array.get(i).as_string() {
                if key.starts_with(prefix) {
                    keys.push(key);
                }
            }
        }

        Ok(keys)
    }
    
    /// Convert an IdbRequest to a Future  
    async fn request_to_future(request: IdbRequest) -> Result<JsValue, JsValue> {
        Self::idb_request_to_future_impl(request.into()).await
    }
    
    /// Convert an IdbOpenDbRequest to a Future
    async fn open_request_to_future(request: web_sys::IdbOpenDbRequest) -> Result<JsValue, JsValue> {
        Self::idb_request_to_future_impl(request.into()).await  
    }
    
    /// Internal implementation for converting any IDB request to Future
    async fn idb_request_to_future_impl(request: IdbRequest) -> Result<JsValue, JsValue> {
        use wasm_bindgen::closure::Closure;
        use wasm_bindgen::JsCast;
        use std::rc::Rc;
        use std::cell::RefCell;
        
        let (sender, receiver) = futures::channel::oneshot::channel();
        let sender = Rc::new(RefCell::new(Some(sender)));
        
        let sender_clone = sender.clone();
        let success_callback = Closure::once(Box::new(move |event: web_sys::Event| {
            let target = event.target().unwrap();
            let request: web_sys::IdbRequest = target.dyn_into().unwrap();
            let result = request.result().unwrap();
            if let Some(sender) = sender_clone.borrow_mut().take() {
                let _ = sender.send(Ok(result));
            }
        }) as Box<dyn FnOnce(web_sys::Event)>);
        
        let sender_clone = sender.clone();
        let error_callback = Closure::once(Box::new(move |event: web_sys::Event| {
            let _target = event.target().unwrap();
            let error = JsValue::from_str("IndexedDB operation failed");
            if let Some(sender) = sender_clone.borrow_mut().take() {
                let _ = sender.send(Err(error));
            }
        }) as Box<dyn FnOnce(web_sys::Event)>);
        
        request.set_onsuccess(Some(success_callback.as_ref().unchecked_ref()));
        request.set_onerror(Some(error_callback.as_ref().unchecked_ref()));
        
        success_callback.forget();
        error_callback.forget();
        
        receiver.await.map_err(|_| JsValue::from_str("Request cancelled"))?
    }
}

impl Storage for IndexedDbStorage {
    async fn load(&self, key: StorageKey) -> Option<Vec<u8>> {
        let key_str = self.storage_key_to_string(&key);
        self.read_data(&key_str).await.ok().flatten()
    }

    async fn load_range(&self, prefix: StorageKey) -> HashMap<StorageKey, Vec<u8>> {
        let mut result = HashMap::new();
        let prefix_str = self.storage_key_to_string(&prefix);

        // Get all keys with the prefix
        if let Ok(keys) = self.get_keys_with_prefix(&prefix_str).await {
            for key_str in keys {
                // Convert back to StorageKey
                let storage_key = self.string_to_storage_key(&key_str);

                // Check if this key actually has our prefix
                if prefix.is_prefix_of(&storage_key) {
                    // Load the data
                    if let Ok(Some(data)) = self.read_data(&key_str).await {
                        result.insert(storage_key, data);
                    }
                }
            }
        }

        result
    }

    async fn put(&self, key: StorageKey, data: Vec<u8>) {
        let key_str = self.storage_key_to_string(&key);
        let _ = self.write_data(&key_str, &data).await;
    }

    async fn delete(&self, key: StorageKey) {
        let key_str = self.storage_key_to_string(&key);
        let _ = self.delete_data(&key_str).await;
    }
}
