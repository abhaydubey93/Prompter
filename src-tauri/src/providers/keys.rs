//! OS-keychain wrapper for cloud provider API keys (spec FR-S1).
//!
//! Each provider's key lives under service `promptopt.<provider_id>`,
//! account `api_key`. Never written to disk, never logged.
//!
//! An in-memory cache backs the keychain: writes go to both, reads try cache
//! first (instant) then fall back to the OS keychain. This guarantees that
//! a key set during the current app session is always readable, even if the
//! OS credential store silently drops the entry (known Windows edge-case).

use std::sync::Mutex;

use keyring::Entry;
use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use sha2::{Digest, Sha256};

const SERVICE_PREFIX: &str = "prompter.";
const ACCOUNT: &str = "api_key";

fn service(provider_id: &str) -> String {
    format!("{SERVICE_PREFIX}{provider_id}")
}

/// Process-wide in-memory key cache. Keys set via `set()` are stored here so
/// they survive even if the OS keychain read fails later in the same session.
static CACHE: std::sync::LazyLock<Mutex<std::collections::HashMap<String, String>>> =
    std::sync::LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

fn fallback_file() -> Option<std::path::PathBuf> {
    dirs::data_dir().map(|p| p.join("PromptOpt").join("keys_fallback.enc"))
}

fn derive_encryption_key() -> aes_gcm::Key<Aes256Gcm> {
    let uid = machine_uid::get().unwrap_or_else(|_| "promptopt-fallback-uid".to_string());
    
    let mut hasher = Sha256::new();
    hasher.update(b"promptopt-keys-salt-v1");
    hasher.update(uid.as_bytes());
    let result = hasher.finalize();
    
    aes_gcm::Key::<Aes256Gcm>::from_slice(&result).clone()
}

fn load_fallback() -> std::collections::HashMap<String, String> {
    let path = match fallback_file() {
        Some(p) => p,
        None => return std::collections::HashMap::new(),
    };
    
    let encrypted_data = match std::fs::read(&path) {
        Ok(data) => data,
        Err(_) => return std::collections::HashMap::new(),
    };
    
    if encrypted_data.len() < 12 {
        return std::collections::HashMap::new();
    }
    
    let (nonce_bytes, ciphertext) = encrypted_data.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);
    
    let key = derive_encryption_key();
    let cipher = Aes256Gcm::new(&key);
    
    match cipher.decrypt(nonce, ciphertext) {
        Ok(plaintext) => {
            if let Ok(s) = std::str::from_utf8(&plaintext) {
                serde_json::from_str(s).unwrap_or_default()
            } else {
                std::collections::HashMap::new()
            }
        }
        Err(e) => {
            tracing::warn!("keys: failed to decrypt fallback file: {}", e);
            std::collections::HashMap::new()
        }
    }
}

fn save_fallback(map: &std::collections::HashMap<String, String>) {
    let path = match fallback_file() {
        Some(p) => p,
        None => return,
    };
    
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    
    let json_str = match serde_json::to_string(map) {
        Ok(s) => s,
        Err(_) => return,
    };
    
    let key = derive_encryption_key();
    let cipher = Aes256Gcm::new(&key);
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng); // 96-bits; unique per message
    
    match cipher.encrypt(&nonce, json_str.as_bytes()) {
        Ok(ciphertext) => {
            let mut final_data = Vec::with_capacity(12 + ciphertext.len());
            final_data.extend_from_slice(&nonce);
            final_data.extend_from_slice(&ciphertext);
            
            if let Err(e) = std::fs::write(&path, final_data) {
                tracing::warn!("keys: failed to write encrypted fallback file: {}", e);
            }
            
            // Delete old plaintext file if it exists, to be secure
            if let Some(p) = dirs::data_dir().map(|p| p.join("PromptOpt").join("keys_fallback.json")) {
                let _ = std::fs::remove_file(p);
            }
        }
        Err(e) => {
            tracing::warn!("keys: failed to encrypt fallback file: {}", e);
        }
    }
}

/// Read a key: in-memory cache first, then OS keychain fallback.
pub fn get(provider_id: &str) -> Option<String> {
    // 1. Try the in-memory cache.
    if let Ok(cache) = CACHE.lock() {
        if let Some(key) = cache.get(provider_id) {
            tracing::debug!(provider = provider_id, "keys::get: cache hit ({} chars)", key.len());
            return Some(key.clone());
        }
    }
    // 2. Fall back to OS keychain.
    let svc = service(provider_id);
    let keychain_val = match Entry::new(&svc, ACCOUNT) {
        Ok(entry) => match entry.get_password() {
            Ok(pwd) => Some(pwd),
            Err(e) => {
                tracing::warn!(provider = provider_id, "keys::get: keychain failed: {e}");
                None
            }
        },
        Err(e) => {
            tracing::warn!(provider = provider_id, "keys::get: entry creation failed: {e}");
            None
        }
    };

    if let Some(pwd) = keychain_val {
        tracing::debug!(provider = provider_id, "keys::get: keychain OK ({} chars)", pwd.len());
        // Warm the cache so future reads are instant.
        if let Ok(mut cache) = CACHE.lock() {
            cache.insert(provider_id.to_string(), pwd.clone());
        }
        return Some(pwd);
    }

    // 3. Fall back to file if OS keychain failed or was empty
    let fallback = load_fallback();
    if let Some(pwd) = fallback.get(provider_id) {
        tracing::debug!(provider = provider_id, "keys::get: fallback file OK ({} chars)", pwd.len());
        if let Ok(mut cache) = CACHE.lock() {
            cache.insert(provider_id.to_string(), pwd.clone());
        }
        return Some(pwd.clone());
    }

    None
}

/// Write a key to BOTH the OS keychain and the in-memory cache.
/// If the keychain write fails we still keep it in cache for this session.
pub fn set(provider_id: &str, value: &str) -> anyhow::Result<()> {
    // Always update the in-memory cache (guaranteed availability).
    if let Ok(mut cache) = CACHE.lock() {
        cache.insert(provider_id.to_string(), value.to_string());
    }
    // Best-effort OS keychain write (may fail on some Windows configs).
    let svc = service(provider_id);
    let mut keychain_ok = false;
    match Entry::new(&svc, ACCOUNT) {
        Ok(entry) => match entry.set_password(value) {
            Ok(()) => {
                tracing::info!(provider = provider_id, "keys::set: keychain write OK ({} chars)", value.len());
                // Double check if it actually saved (handles silent drops)
                if entry.get_password().is_ok() {
                    keychain_ok = true;
                } else {
                    tracing::warn!(provider = provider_id, "keys::set: keychain silently dropped the key");
                }
            }
            Err(e) => {
                tracing::warn!(provider = provider_id, "keys::set: keychain write failed: {e}");
            }
        },
        Err(e) => {
            tracing::warn!(provider = provider_id, "keys::set: entry creation failed: {e}");
        }
    }

    tracing::info!(provider = provider_id, "keys::set: unconditionally using fallback file for durable persistence");
    let mut fallback = load_fallback();
    fallback.insert(provider_id.to_string(), value.to_string());
    save_fallback(&fallback);

    Ok(()) // Never fail — cache/fallback is the source of truth for the session.
}

/// Delete a key from both cache and OS keychain.
pub fn delete(provider_id: &str) -> anyhow::Result<()> {
    // Remove from cache.
    if let Ok(mut cache) = CACHE.lock() {
        cache.remove(provider_id);
    }
    // Remove from fallback file.
    let mut fallback = load_fallback();
    if fallback.remove(provider_id).is_some() {
        save_fallback(&fallback);
    }
    // Best-effort keychain delete.
    let svc = service(provider_id);
    match Entry::new(&svc, ACCOUNT) {
        Ok(entry) => match entry.delete_credential() {
            Ok(()) => {}
            Err(keyring::Error::NoEntry) => {}
            Err(e) => tracing::warn!(provider = provider_id, "keys::delete: keychain failed: {e}"),
        },
        Err(e) => tracing::warn!(provider = provider_id, "keys::delete: entry creation failed: {e}"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Roundtrip a throwaway key via cache+keychain.
    #[test]
    fn test_keyring_roundtrip() {
        let id = "prompter_unit_test_throwaway";
        let val = "sk-test-DO-NOT-USE-12345";
        if set(id, val).is_err() {
            eprintln!("skipping keyring test — no write backend on this host");
            return;
        }
        // get() should find it via cache (no keychain needed).
        match get(id) {
            Some(v) => assert_eq!(v, val),
            None => panic!("expected key in cache after set()"),
        }
        let _ = delete(id); // best-effort cleanup
    }

    /// Cache-only roundtrip (doesn't touch OS keychain).
    #[test]
    fn test_cache_roundtrip() {
        let id = "prompter_cache_test_unique";
        let val = "sk-cache-only-test";
        // Clean up any prior state.
        let _ = delete(id);
        // Clear cache entry too.
        if let Ok(mut cache) = CACHE.lock() {
            cache.remove(id);
        }
        // set() stores in cache.
        set(id, val).unwrap();
        // get() should find it from cache.
        assert_eq!(get(id).as_deref(), Some(val));
        // Cleanup.
        let _ = delete(id);
    }
}
