//! OS-keychain wrapper for cloud provider API keys (spec FR-S1).
//!
//! Each provider's key lives under service `promptopt.<provider_id>`,
//! account `api_key`. Never written to disk, never logged.

use keyring::Entry;

const SERVICE_PREFIX: &str = "promptopt.";
const ACCOUNT: &str = "api_key";

fn service(provider_id: &str) -> String {
    format!("{SERVICE_PREFIX}{provider_id}")
}

/// Read a key from the OS keychain. Returns None if absent or inaccessible.
pub fn get(provider_id: &str) -> Option<String> {
    let entry = Entry::new(&service(provider_id), ACCOUNT).ok()?;
    entry.get_password().ok()
}

/// Write a key to the OS keychain. Overwrites any prior value.
pub fn set(provider_id: &str, value: &str) -> anyhow::Result<()> {
    let entry = Entry::new(&service(provider_id), ACCOUNT)
        .map_err(|e| anyhow::anyhow!("keyring entry create failed: {e}"))?;
    entry
        .set_password(value)
        .map_err(|e| anyhow::anyhow!("keyring set failed: {e}"))
}

/// Delete a key. Silently succeeds if absent.
pub fn delete(provider_id: &str) -> anyhow::Result<()> {
    let entry = Entry::new(&service(provider_id), ACCOUNT)
        .map_err(|e| anyhow::anyhow!("keyring entry create failed: {e}"))?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(anyhow::anyhow!("keyring delete failed: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Roundtrip a throwaway key. Skipped silently if the host has no keychain
    /// backend (CI/sandbox stores may accept writes but not return them).
    #[test]
    fn test_keyring_roundtrip() {
        let id = "promptopt_unit_test_throwaway";
        let val = "sk-test-DO-NOT-USE-12345";
        if set(id, val).is_err() {
            eprintln!("skipping keyring test — no write backend on this host");
            return;
        }
        match get(id) {
            Some(v) => assert_eq!(v, val),
            None => {
                eprintln!("skipping keyring read check — credential store not readable in this environment");
            }
        }
        let _ = delete(id); // best-effort cleanup
    }
}
