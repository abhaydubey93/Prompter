//! Framework pack loading from `framework_packs/` (spec §8).
//!
//! Packs are JSON files with a Jinja template. Loads from next-to-binary
//! (dev) then app data dir, falling back to hardcoded CREATE/APE if none
//! found on disk.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tracing::info;

/// A framework pack loaded from JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameworkPack {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub variables: Vec<String>,
    pub template: String,
}

/// Load all framework packs from disk, falling back to hardcoded defaults
/// if no files are found.
pub fn load_all(app_data_dir: &std::path::Path) -> anyhow::Result<HashMap<String, FrameworkPack>> {
    let packs_dir = app_data_dir.join("framework_packs");
    let mut frameworks = HashMap::new();

    let mut load_from = |dir: &PathBuf| -> anyhow::Result<()> {
        if !dir.exists() {
            return Ok(());
        }
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "json") {
                let content = std::fs::read_to_string(&path)?;
                let pack: FrameworkPack = serde_json::from_str(&content)?;
                info!(pack_id = %pack.id, "loaded framework pack");
                frameworks.insert(pack.id.clone(), pack);
            }
        }
        Ok(())
    };

    // Try next-to-binary first (dev / `cargo run`).
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()));
    if let Some(ref dir) = exe_dir {
        let _ = load_from(&dir.join("framework_packs"));
    }
    // Then app data dir.
    load_from(&packs_dir)?;

    // Hardcoded fallbacks in case no files found on disk.
    if frameworks.is_empty() {
        for pack in hardcoded_packs() {
            frameworks.insert(pack.id.clone(), pack);
        }
    }

    Ok(frameworks)
}

/// Compile-time CREATE + APE fallback packs (verbatim from spec §8).
pub fn hardcoded_packs() -> Vec<FrameworkPack> {
    vec![
        FrameworkPack {
            id: "CREATE".into(),
            name: "CREATE".into(),
            variables: vec!["context".into(), "role".into(), "task".into(), "explanation".into(), "constraints".into(), "tone".into(), "extras".into()],
            template: r#"You are a prompt engineer. Rewrite the user's raw prompt using the CREATE framework.

CREATE = Context, Request, Explanation, Action, Tone, Extras.
Return ONLY the rewritten prompt.

Raw prompt:
{{ raw_prompt }}
{% if context %}Context: {{ context }}{% endif %}
{% if tone %}Tone: {{ tone }}{% endif %}"#
                .into(),
        },
        FrameworkPack {
            id: "APE".into(),
            name: "APE (Action, Purpose, Expectation)".into(),
            variables: vec!["action".into(), "purpose".into(), "expectation".into()],
            template: r#"You are a prompt engineer. Rewrite the user's raw prompt using the APE framework (Action, Purpose, Expectation).
Return ONLY the rewritten prompt.

Raw prompt:
{{ raw_prompt }}"#
                .into(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hardcoded_packs_present() {
        let packs = hardcoded_packs();
        let ids: Vec<&str> = packs.iter().map(|p| p.id.as_str()).collect();
        assert!(ids.contains(&"CREATE"));
        assert!(ids.contains(&"APE"));
    }

    #[test]
    fn test_hardcoded_packs_have_templates() {
        for pack in hardcoded_packs() {
            assert!(pack.template.contains("{{ raw_prompt }}"),
                "pack {} must reference raw_prompt", pack.id);
        }
    }

    #[test]
    fn test_load_all_with_empty_dir() {
        let tmp = std::env::temp_dir().join("promptopt_fw_test_empty");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        // Empty dir → should fall back to hardcoded packs.
        let packs = load_all(&tmp).unwrap();
        assert!(packs.contains_key("CREATE"));
        assert!(packs.contains_key("APE"));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_load_all_from_disk() {
        let tmp = std::env::temp_dir().join("promptopt_fw_test_disk");
        let _ = std::fs::remove_dir_all(&tmp);
        let packs_dir = tmp.join("framework_packs");
        std::fs::create_dir_all(&packs_dir).unwrap();
        std::fs::write(
            packs_dir.join("custom.json"),
            r#"{"id":"CUSTOM","name":"Custom","variables":[],"template":"{{ raw_prompt }}"}"#,
        ).unwrap();
        let packs = load_all(&tmp).unwrap();
        assert!(packs.contains_key("CUSTOM"));
        // When at least one pack found on disk, hardcoded fallbacks NOT added.
        assert!(!packs.contains_key("CREATE"));
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
