//! Framework pack loading (spec §8, FR-E1).
//!
//! Packs = JSON files with Jinja template. Load order: next-to-binary
//! (`cargo run`) → bundled resource dir (post-build) → app data dir (user
//! imports) → hardcoded 10 built-ins as final fallback.
//!
//! Built-ins: APE, TAG, RACE, CARE, RISE, ERA, CREATE, TRACE, ROSES, SPARK.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

/// A framework pack loaded from JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameworkPack {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub variables: Vec<String>,
    pub template: String,
}

/// IDs that cannot be deleted by the user (built-ins).
pub const BUILTIN_IDS: &[&str] = &[
    "APE", "TAG", "RACE", "CARE", "RISE", "ERA", "CREATE", "TRACE", "ROSES", "SPARK",
];

pub fn is_builtin(id: &str) -> bool {
    BUILTIN_IDS.iter().any(|b| b.eq_ignore_ascii_case(id))
}

/// Load all packs from disk; merge built-ins underneath so user packs can
/// override built-ins by id. Returns a map keyed by id (case-sensitive).
pub fn load_all(
    app_data_dir: &std::path::Path,
    resource_dir: Option<&std::path::Path>,
) -> anyhow::Result<HashMap<String, FrameworkPack>> {
    let mut frameworks: HashMap<String, FrameworkPack> = HashMap::new();

    let mut load_from = |dir: &PathBuf, source: &str| -> anyhow::Result<()> {
        if !dir.exists() {
            return Ok(());
        }
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "json") {
                let content = match std::fs::read_to_string(&path) {
                    Ok(c) => c,
                    Err(e) => {
                        warn!(?path, %e, "skipping unreadable framework pack");
                        continue;
                    }
                };
                match serde_json::from_str::<FrameworkPack>(&content) {
                    Ok(mut pack) => {
                        pack.id = pack.id.to_uppercase();
                        info!(pack_id = %pack.id, source, "loaded framework pack");
                        frameworks.insert(pack.id.clone(), pack);
                    }
                    Err(e) => {
                        warn!(?path, %e, "skipping malformed framework pack");
                    }
                }
            }
        }
        Ok(())
    };

    // 1. Next-to-binary (dev / `cargo run`).
    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
    {
        let _ = load_from(&exe_dir.join("framework_packs"), "exe_dir");
    }
    // 2. Bundled resource dir (post-build).
    if let Some(res) = resource_dir {
        let _ = load_from(&res.join("framework_packs"), "resource_dir");
    }
    // 3. App data dir (user imports live here).
    let _ = load_from(&app_data_dir.join("framework_packs"), "app_data_dir");

    // 4. Built-in fallbacks (merge under — user/on-disk packs override).
    for pack in builtin_packs() {
        frameworks.entry(pack.id.clone()).or_insert(pack);
    }

    if frameworks.is_empty() {
        warn!("no framework packs loaded — built-ins failed?");
    }
    Ok(frameworks)
}

/// All 10 built-in framework packs (FR-E1).
pub fn builtin_packs() -> Vec<FrameworkPack> {
    vec![
        FrameworkPack {
            id: "CREATE".into(),
            name: "CREATE (Context, Request, Explanation, Action, Tone, Extras)".into(),
            variables: vec!["context".into(), "role".into(), "task".into(), "explanation".into(), "constraints".into(), "tone".into(), "extras".into()],
            template: r#"You are a prompt engineer. Rewrite the user's raw prompt in  detail using the CREATE framework.

CREATE = Context, Request, Explanation, Action, Tone, Extras.
Return ONLY the rewritten prompt.

Raw prompt:
{{ raw_prompt }}
{% if context %}Context: {{ context }}{% endif %}
{% if role %}Role: {{ role }}{% endif %}
{% if tone %}Tone: {{ tone }}{% endif %}"#
                .into(),
        },
        FrameworkPack {
            id: "APE".into(),
            name: "APE (Action, Purpose, Expectation)".into(),
            variables: vec!["action".into(), "purpose".into(), "expectation".into()],
            template: r#"You are a prompt engineer. Rewrite the user's raw prompt in  detail using the APE framework (Action, Purpose, Expectation).
Return ONLY the rewritten prompt.

Raw prompt:
{{ raw_prompt }}"#
                .into(),
        },
        FrameworkPack {
            id: "TAG".into(),
            name: "TAG (Task, Action, Goal)".into(),
            variables: vec!["task".into(), "action".into(), "goal".into(), "context".into(), "role".into(), "tone".into()],
            template: r#"You are a prompt engineer. Rewrite the user's raw prompt in  detail using the TAG framework.

TAG = Task (what to do), Action (how to do it), Goal (desired outcome).
Return ONLY the rewritten prompt.

Raw prompt:
{{ raw_prompt }}
{% if context %}Context: {{ context }}{% endif %}
{% if role %}Role: {{ role }}{% endif %}
{% if tone %}Tone: {{ tone }}{% endif %}"#
                .into(),
        },
        FrameworkPack {
            id: "RACE".into(),
            name: "RACE (Role, Action, Context, Expectation)".into(),
            variables: vec!["role".into(), "action".into(), "context".into(), "expectation".into(), "audience".into(), "tone".into()],
            template: r#"You are a prompt engineer. Rewrite the user's raw prompt in  detail using the RACE framework.

RACE = Role (who acts), Action (what to do), Context (background), Expectation (output format/quality).
Return ONLY the rewritten prompt.

Raw prompt:
{{ raw_prompt }}
{% if role %}Role: {{ role }}{% endif %}
{% if context %}Context: {{ context }}{% endif %}
{% if audience %}Audience: {{ audience }}{% endif %}
{% if tone %}Tone: {{ tone }}{% endif %}"#
                .into(),
        },
        FrameworkPack {
            id: "CARE".into(),
            name: "CARE (Context, Action, Result, Example)".into(),
            variables: vec!["context".into(), "action".into(), "result".into(), "example".into(), "role".into(), "tone".into()],
            template: r#"You are a prompt engineer. Rewrite the user's raw prompt in  detail using the CARE framework.

CARE = Context (situation), Action (task), Result (expected output), Example (reference).
Return ONLY the rewritten prompt.

Raw prompt:
{{ raw_prompt }}
{% if context %}Context: {{ context }}{% endif %}
{% if role %}Role: {{ role }}{% endif %}
{% if tone %}Tone: {{ tone }}{% endif %}"#
                .into(),
        },
        FrameworkPack {
            id: "RISE".into(),
            name: "RISE (Role, Instruction, Steps, End)".into(),
            variables: vec!["role".into(), "instruction".into(), "steps".into(), "end".into(), "context".into(), "tone".into()],
            template: r#"You are a prompt engineer. Rewrite the user's raw prompt in  detail using the RISE framework.

RISE = Role (persona), Instruction (main task), Steps (sub-tasks), End (goal/output format).
Return ONLY the rewritten prompt.

Raw prompt:
{{ raw_prompt }}
{% if role %}Role: {{ role }}{% endif %}
{% if context %}Context: {{ context }}{% endif %}
{% if tone %}Tone: {{ tone }}{% endif %}"#
                .into(),
        },
        FrameworkPack {
            id: "ERA".into(),
            name: "ERA (Expectation, Role, Action)".into(),
            variables: vec!["expectation".into(), "role".into(), "action".into(), "context".into(), "tone".into()],
            template: r#"You are a prompt engineer. Rewrite the user's raw prompt in  detail using the ERA framework.

ERA = Expectation (desired result), Role (persona), Action (task steps).
Return ONLY the rewritten prompt.

Raw prompt:
{{ raw_prompt }}
{% if role %}Role: {{ role }}{% endif %}
{% if context %}Context: {{ context }}{% endif %}
{% if tone %}Tone: {{ tone }}{% endif %}"#
                .into(),
        },
        FrameworkPack {
            id: "TRACE".into(),
            name: "TRACE (Task, Request, Action, Context, Example)".into(),
            variables: vec!["task".into(), "request".into(), "action".into(), "context".into(), "example".into(), "role".into(), "tone".into()],
            template: r#"You are a prompt engineer. Rewrite the user's raw prompt in  detail using the TRACE framework.

TRACE = Task (goal), Request (specific ask), Action (method), Context (background), Example (reference output).
Return ONLY the rewritten prompt.

Raw prompt:
{{ raw_prompt }}
{% if role %}Role: {{ role }}{% endif %}
{% if context %}Context: {{ context }}{% endif %}
{% if tone %}Tone: {{ tone }}{% endif %}"#
                .into(),
        },
        FrameworkPack {
            id: "ROSES".into(),
            name: "ROSES (Role, Objective, Steps, End state, Style)".into(),
            variables: vec!["role".into(), "objective".into(), "steps".into(), "end_state".into(), "style".into(), "context".into(), "tone".into()],
            template: r#"You are a prompt engineer. Rewrite the user's raw prompt in  detail using the ROSES framework.

ROSES = Role (persona), Objective (goal), Steps (method), End state (success criteria), Style (tone/format).
Return ONLY the rewritten prompt.

Raw prompt:
{{ raw_prompt }}
{% if role %}Role: {{ role }}{% endif %}
{% if context %}Context: {{ context }}{% endif %}
{% if tone %}Tone: {{ tone }}{% endif %}"#
                .into(),
        },
        FrameworkPack {
            id: "SPARK".into(),
            name: "SPARK (Situation, Problem, Aspiration, Results, Knew)".into(),
            variables: vec!["situation".into(), "problem".into(), "aspiration".into(), "results".into(), "knew".into(), "context".into(), "tone".into()],
            template: r#"You are a prompt engineer. Rewrite the user's raw prompt in  detail using the SPARK framework.

SPARK = Situation (context), Problem (pain point), Aspiration (goal), Results (success metrics), Knew (constraints/known facts).
Return ONLY the rewritten prompt.

Raw prompt:
{{ raw_prompt }}
{% if context %}Context: {{ context }}{% endif %}
{% if tone %}Tone: {{ tone }}{% endif %}"#
                .into(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ten_builtins_present() {
        let packs = builtin_packs();
        let ids: Vec<&str> = packs.iter().map(|p| p.id.as_str()).collect();
        for expected in [
            "CREATE", "APE", "TAG", "RACE", "CARE", "RISE", "ERA", "TRACE", "ROSES", "SPARK",
        ] {
            assert!(ids.contains(&expected), "missing built-in {expected}");
        }
        assert_eq!(packs.len(), 10, "expected exactly 10 built-ins");
    }

    #[test]
    fn test_builtins_have_raw_prompt() {
        for pack in builtin_packs() {
            assert!(
                pack.template.contains("{{ raw_prompt }}"),
                "pack {} must reference raw_prompt",
                pack.id
            );
        }
    }

    #[test]
    fn test_is_builtin() {
        assert!(is_builtin("CREATE"));
        assert!(is_builtin("create")); // case-insensitive
        assert!(!is_builtin("custom_xyz"));
    }

    #[test]
    fn test_load_all_empty_dir_falls_back() {
        let tmp = std::env::temp_dir().join("promptopt_fw_r3_empty");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let packs = load_all(&tmp, None).unwrap();
        assert_eq!(packs.len(), 10, "empty disk → 10 built-ins");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_load_all_user_override() {
        let tmp = std::env::temp_dir().join("promptopt_fw_r3_override");
        let _ = std::fs::remove_dir_all(&tmp);
        let packs_dir = tmp.join("framework_packs");
        std::fs::create_dir_all(&packs_dir).unwrap();
        std::fs::write(
            packs_dir.join("custom.json"),
            r#"{"id":"CUSTOM","name":"Custom","variables":[],"template":"{{ raw_prompt }}"}"#,
        )
        .unwrap();
        let packs = load_all(&tmp, None).unwrap();
        assert!(packs.contains_key("CUSTOM"), "user pack present");
        assert!(
            packs.contains_key("CREATE"),
            "built-ins still merged underneath"
        );
        assert_eq!(packs.len(), 11, "10 built-ins + 1 custom");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_load_all_user_overrides_builtin_by_id() {
        let tmp = std::env::temp_dir().join("promptopt_fw_r3_replace");
        let _ = std::fs::remove_dir_all(&tmp);
        let packs_dir = tmp.join("framework_packs");
        std::fs::create_dir_all(&packs_dir).unwrap();
        // Same id CREATE, different template — should win.
        std::fs::write(
            packs_dir.join("create.json"),
            r#"{"id":"CREATE","name":"CREATE Customized","variables":[],"template":"CUSTOM TEMPLATE {{ raw_prompt }}"}"#,
        ).unwrap();
        let packs = load_all(&tmp, None).unwrap();
        let create = packs.get("CREATE").unwrap();
        assert_eq!(create.name, "CREATE Customized");
        assert!(create.template.contains("CUSTOM TEMPLATE"));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_load_all_skips_malformed() {
        let tmp = std::env::temp_dir().join("promptopt_fw_r3_malformed");
        let _ = std::fs::remove_dir_all(&tmp);
        let packs_dir = tmp.join("framework_packs");
        std::fs::create_dir_all(&packs_dir).unwrap();
        std::fs::write(packs_dir.join("bad.json"), "{not valid json").unwrap();
        std::fs::write(
            packs_dir.join("good.json"),
            r#"{"id":"GOOD","name":"Good","variables":[],"template":"{{ raw_prompt }}"}"#,
        )
        .unwrap();
        let packs = load_all(&tmp, None).unwrap();
        assert!(packs.contains_key("GOOD"));
        assert!(!packs.contains_key("bad"));
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
