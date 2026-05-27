//! Slim pipeline utilities retained after aikit-sdk migration (ADR 0001).
//!
//! The structured pipeline (render → agent → validate → retry) now lives in
//! `aikit-sdk`. This module keeps only the Progress event type used by the CLI
//! and the jsonschema input-validation helper.

use serde_json::Value;

// ---------------------------------------------------------------------------
// Progress
// ---------------------------------------------------------------------------

/// Progress events emitted during review orchestration.
#[derive(Debug, Clone, Copy)]
pub enum Progress {
    /// About to invoke the agent for attempt `attempt` of `total`.
    AgentRunning { attempt: u32, total: u32 },
    /// Agent responded; validating its output.
    Validating { attempt: u32, total: u32 },
    /// Validation failed; about to retry. Reserved for a future aikit progress
    /// callback — not currently emitted since retries are handled inside aikit.
    Retrying {
        attempt: u32,
        total: u32,
        errors: usize,
    },
}

/// Callback that receives [`Progress`] events. Use `&|_| {}` for none.
pub type ProgressFn<'a> = dyn Fn(Progress) + 'a;

// ---------------------------------------------------------------------------
// Input-validation helper
// ---------------------------------------------------------------------------

/// Validate `instance` against a JSON Schema (jsonschema 0.46 API).
/// Returns the list of validation error strings on failure.
pub fn validate_json(schema: &Value, instance: &Value) -> Result<(), Vec<String>> {
    let validator = jsonschema::validator_for(schema)
        .map_err(|e| vec![format!("invalid schema: {e}")])?;
    let errors: Vec<String> = validator
        .iter_errors(instance)
        .map(|e| e.to_string())
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

// ---------------------------------------------------------------------------
// JSON-block extraction (kept as a test utility)
// ---------------------------------------------------------------------------

/// Extract the contents of the first ```json fenced block (case-insensitive).
/// Returns the inner text without the fences.
pub fn extract_json_block(raw: &str) -> Option<String> {
    let mut lines = raw.lines();
    let mut collecting = false;
    let mut buf: Vec<&str> = Vec::new();
    for line in lines.by_ref() {
        let trimmed = line.trim();
        if !collecting {
            if let Some(rest) = trimmed.strip_prefix("```") {
                if rest.trim().eq_ignore_ascii_case("json") {
                    collecting = true;
                }
            }
        } else if trimmed.starts_with("```") {
            return Some(buf.join("\n"));
        } else {
            buf.push(line);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn validate_json_passes_valid_instance() {
        let schema = json!({"type": "object", "required": ["ok"], "properties": {"ok": {"type": "boolean"}}});
        let instance = json!({"ok": true});
        assert!(validate_json(&schema, &instance).is_ok());
    }

    #[test]
    fn validate_json_fails_invalid_instance() {
        let schema = json!({"type": "object", "required": ["ok"], "properties": {"ok": {"type": "boolean"}}});
        let instance = json!({"ok": "not-a-bool"});
        assert!(validate_json(&schema, &instance).is_err());
    }

    #[test]
    fn validate_json_bad_schema_returns_error() {
        let not_a_schema = json!("this is a string not a schema");
        let instance = json!({});
        let result = validate_json(&not_a_schema, &instance);
        assert!(result.is_err());
    }

    #[test]
    fn extracts_first_json_block() {
        let raw = "intro\n```json\n{\"a\": 1}\n```\ntrailing\n```json\n{\"b\":2}\n```";
        assert_eq!(extract_json_block(raw).unwrap(), "{\"a\": 1}");
    }

    #[test]
    fn extract_case_insensitive() {
        assert_eq!(extract_json_block("```JSON\n{\"a\":1}\n```").unwrap(), "{\"a\":1}");
    }

    #[test]
    fn no_block_returns_none() {
        assert!(extract_json_block("no code here").is_none());
    }
}
