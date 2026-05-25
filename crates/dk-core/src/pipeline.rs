//! Trait-based structured-pipeline shim (ADR 0001 fallback).
//!
//! Mirrors the `aikit-sdk` structured pipeline surface (`TemplateRenderer`,
//! `AgentRunner`, `ResponseValidator`, `Pipeline`) so the dependency can be
//! swapped in when its API lands. Composition: render prompt -> run agent ->
//! extract first ```json block -> validate against output schema, retrying up
//! to `max_retries` times with the validation errors appended to the prompt.

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PipelineError {
    /// A `{{slot}}` in the template had no corresponding value (spec R1).
    #[error("unknown slot in template: {{{{{slot}}}}}")]
    UnknownSlot { slot: String },
    #[error("template not found: {path}")]
    TemplateNotFound { path: String },
    #[error("configured agent not found: {agent}")]
    AgentNotFound { agent: String },
    #[error("agent invocation failed: {message}")]
    AgentFailed { message: String },
    #[error("no ```json block found in agent response")]
    NoJsonBlock,
    #[error("agent response JSON did not parse: {message}")]
    JsonParse { message: String },
    #[error("agent output failed schema validation after {attempts} attempt(s): {}", errors.join("; "))]
    SchemaValidation { errors: Vec<String>, attempts: u32 },
    #[error("invalid output schema: {message}")]
    InvalidSchema { message: String },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl PipelineError {
    pub fn code(&self) -> &'static str {
        match self {
            PipelineError::TemplateNotFound { .. } => "DK_TEMPLATE_NOT_FOUND",
            PipelineError::AgentNotFound { .. } => "DK_AGENT_NOT_FOUND",
            PipelineError::Io(_) => "DK_IO_ERROR",
            _ => "DK_PIPELINE_ERROR",
        }
    }
}

/// Renders a `{{slot}}` template against a slot map. Used for both the prompt
/// and the report (spec `TemplateRenderer` / `ReportRenderer`).
pub trait TemplateRenderer {
    fn render(
        &self,
        template: &str,
        slots: &HashMap<String, String>,
    ) -> Result<String, PipelineError>;
}

/// Runs a coding agent against `prompt` with `working_dir` as its cwd.
pub trait AgentRunner {
    fn run(&self, prompt: &str, working_dir: &Path) -> Result<String, PipelineError>;
}

/// Extracts the first ```json block from a raw agent response and validates it
/// against the output schema.
pub trait ResponseValidator {
    fn extract_and_validate(&self, raw: &str, schema: &Value) -> Result<Value, PipelineError>;
}

/// Single-pass `{{slot}}` substitution. Unknown slots in the *template* error;
/// values are inserted literally and never rescanned (so a `{{...}}` appearing
/// inside an injected value — e.g. the verbatim methodology — is left as-is).
#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultRenderer;

impl TemplateRenderer for DefaultRenderer {
    fn render(
        &self,
        template: &str,
        slots: &HashMap<String, String>,
    ) -> Result<String, PipelineError> {
        let mut out = String::with_capacity(template.len());
        let bytes = template.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if i + 1 < bytes.len() && bytes[i] == b'{' && bytes[i + 1] == b'{' {
                if let Some(close) = template[i + 2..].find("}}") {
                    let name = template[i + 2..i + 2 + close].trim();
                    // Only treat as a slot if the name is a simple identifier.
                    if !name.is_empty()
                        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                    {
                        match slots.get(name) {
                            Some(v) => out.push_str(v),
                            None => {
                                return Err(PipelineError::UnknownSlot {
                                    slot: name.to_string(),
                                })
                            }
                        }
                        i = i + 2 + close + 2;
                        continue;
                    }
                }
            }
            // Not a slot opener: copy this character (respecting UTF-8 boundaries).
            let ch_len = utf8_len(bytes[i]);
            out.push_str(&template[i..i + ch_len]);
            i += ch_len;
        }
        Ok(out)
    }
}

fn utf8_len(first: u8) -> usize {
    if first < 0x80 {
        1
    } else if first >> 5 == 0b110 {
        2
    } else if first >> 4 == 0b1110 {
        3
    } else {
        4
    }
}

/// Extracts the first fenced block whose info string is `json` (case-insensitive)
/// and validates it against the schema (JSON Schema draft 2020-12).
#[derive(Debug, Default, Clone, Copy)]
pub struct JsonResponseValidator;

impl ResponseValidator for JsonResponseValidator {
    fn extract_and_validate(&self, raw: &str, schema: &Value) -> Result<Value, PipelineError> {
        let block = extract_json_block(raw).ok_or(PipelineError::NoJsonBlock)?;
        let value: Value = serde_json::from_str(&block).map_err(|e| PipelineError::JsonParse {
            message: e.to_string(),
        })?;
        validate_json(schema, &value).map_err(|errors| PipelineError::SchemaValidation {
            errors,
            attempts: 1,
        })?;
        Ok(value)
    }
}

/// Validate `instance` against a JSON Schema (draft auto-detected from `$schema`;
/// draft 2020-12 supported). Returns the list of validation error strings.
pub fn validate_json(schema: &Value, instance: &Value) -> Result<(), Vec<String>> {
    let compiled = jsonschema::JSONSchema::compile(schema)
        .map_err(|e| vec![format!("invalid schema: {e}")])?;
    if let Err(errors) = compiled.validate(instance) {
        return Err(errors.map(|e| e.to_string()).collect());
    }
    Ok(())
}

/// Extract the contents of the first ```json fenced block (case-insensitive
/// info string). Returns the inner text without the fences.
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

/// Real agent runner: invokes the agent binary as `<agent> [-m model] -p <prompt>`
/// with `working_dir` as cwd. Tests inject a mock instead.
#[derive(Debug, Clone)]
pub struct SubprocessAgent {
    pub agent: String,
    pub model: Option<String>,
}

impl AgentRunner for SubprocessAgent {
    fn run(&self, prompt: &str, working_dir: &Path) -> Result<String, PipelineError> {
        let mut cmd = Command::new(&self.agent);
        cmd.current_dir(working_dir);
        if let Some(model) = &self.model {
            cmd.arg("-m").arg(model);
        }
        cmd.arg("-p").arg(prompt);
        let output = cmd.output().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                PipelineError::AgentNotFound {
                    agent: self.agent.clone(),
                }
            } else {
                PipelineError::AgentFailed {
                    message: e.to_string(),
                }
            }
        })?;
        if !output.status.success() {
            return Err(PipelineError::AgentFailed {
                message: format!(
                    "agent exited with {}: {}",
                    output.status,
                    String::from_utf8_lossy(&output.stderr)
                ),
            });
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

/// Composes renderer + agent + validator with retry-on-validation-failure.
pub struct Pipeline<'a> {
    pub renderer: &'a dyn TemplateRenderer,
    pub agent: &'a dyn AgentRunner,
    pub validator: &'a dyn ResponseValidator,
    /// Number of *retries* after the first attempt (spec: up to 2).
    pub max_retries: u32,
}

impl<'a> Pipeline<'a> {
    pub fn new(
        renderer: &'a dyn TemplateRenderer,
        agent: &'a dyn AgentRunner,
        validator: &'a dyn ResponseValidator,
    ) -> Self {
        Self {
            renderer,
            agent,
            validator,
            max_retries: 2,
        }
    }

    /// Render the prompt, run the agent, extract + validate. On schema/parse
    /// failure, append the errors to the prompt and retry up to `max_retries`.
    pub fn run(
        &self,
        prompt_template: &str,
        slots: &HashMap<String, String>,
        working_dir: &Path,
        schema: &Value,
    ) -> Result<Value, PipelineError> {
        let base_prompt = self.renderer.render(prompt_template, slots)?;
        let mut prompt = base_prompt.clone();
        let total_attempts = self.max_retries + 1;
        let mut last_errors: Vec<String> = Vec::new();
        for attempt in 1..=total_attempts {
            let raw = self.agent.run(&prompt, working_dir)?;
            match self.validator.extract_and_validate(&raw, schema) {
                Ok(value) => return Ok(value),
                Err(PipelineError::SchemaValidation { errors, .. }) => {
                    last_errors = errors;
                }
                Err(PipelineError::NoJsonBlock) => {
                    last_errors = vec!["no ```json block found in response".to_string()];
                }
                Err(PipelineError::JsonParse { message }) => {
                    last_errors = vec![format!("JSON parse error: {message}")];
                }
                Err(other) => return Err(other),
            }
            if attempt < total_attempts {
                prompt = format!(
                    "{base_prompt}\n\n## Previous attempt failed validation\n\
                     Your last response did not validate. Fix these errors and reply again with \
                     a single ```json block:\n- {}",
                    last_errors.join("\n- ")
                );
            }
        }
        Err(PipelineError::SchemaValidation {
            errors: last_errors,
            attempts: total_attempts,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn renders_known_slots() {
        let r = DefaultRenderer;
        let mut slots = HashMap::new();
        slots.insert("name".to_string(), "world".to_string());
        let out = r.render("hi {{name}}!", &slots).unwrap();
        assert_eq!(out, "hi world!");
    }

    #[test]
    fn unknown_slot_errors() {
        let r = DefaultRenderer;
        let err = r.render("hi {{missing}}", &HashMap::new()).unwrap_err();
        assert!(matches!(err, PipelineError::UnknownSlot { .. }));
        assert_eq!(err.code(), "DK_PIPELINE_ERROR");
    }

    #[test]
    fn value_braces_not_rescanned() {
        let r = DefaultRenderer;
        let mut slots = HashMap::new();
        // Injected value itself contains a {{token}} which must be left literal.
        slots.insert("body".to_string(), "cap at {{max_findings}}".to_string());
        let out = r.render("{{body}}", &slots).unwrap();
        assert_eq!(out, "cap at {{max_findings}}");
    }

    #[test]
    fn extracts_first_json_block() {
        let raw = "intro\n```json\n{\"a\": 1}\n```\ntrailing\n```json\n{\"b\":2}\n```";
        let block = extract_json_block(raw).unwrap();
        assert_eq!(block, "{\"a\": 1}");
    }

    #[test]
    fn extract_case_insensitive() {
        let raw = "```JSON\n{\"a\":1}\n```";
        assert_eq!(extract_json_block(raw).unwrap(), "{\"a\":1}");
    }

    #[test]
    fn no_block_returns_none() {
        assert!(extract_json_block("no code here").is_none());
    }

    #[test]
    fn subprocess_agent_missing_binary_is_agent_not_found() {
        let agent = SubprocessAgent {
            agent: "dk-nonexistent-agent-binary-xyz".to_string(),
            model: None,
        };
        let err = agent.run("prompt", Path::new(".")).unwrap_err();
        assert!(matches!(err, PipelineError::AgentNotFound { .. }));
        assert_eq!(err.code(), "DK_AGENT_NOT_FOUND");
    }

    #[test]
    fn io_variant_maps_to_io_code() {
        let err = PipelineError::Io(std::io::Error::other("boom"));
        assert_eq!(err.code(), "DK_IO_ERROR");
    }

    #[test]
    fn template_not_found_code() {
        let err = PipelineError::TemplateNotFound {
            path: "x".to_string(),
        };
        assert_eq!(err.code(), "DK_TEMPLATE_NOT_FOUND");
    }

    struct MockAgent {
        responses: std::cell::RefCell<Vec<String>>,
    }
    impl AgentRunner for MockAgent {
        fn run(&self, _prompt: &str, _wd: &Path) -> Result<String, PipelineError> {
            Ok(self.responses.borrow_mut().remove(0))
        }
    }

    fn tiny_schema() -> Value {
        json!({
            "type": "object",
            "required": ["ok"],
            "properties": { "ok": { "type": "boolean" } },
            "additionalProperties": false
        })
    }

    #[test]
    fn pipeline_succeeds_first_try() {
        let agent = MockAgent {
            responses: std::cell::RefCell::new(vec!["```json\n{\"ok\":true}\n```".to_string()]),
        };
        let pipe = Pipeline::new(&DefaultRenderer, &agent, &JsonResponseValidator);
        let v = pipe
            .run(
                "{{p}}",
                &HashMap::from([("p".to_string(), "x".to_string())]),
                Path::new("."),
                &tiny_schema(),
            )
            .unwrap();
        assert_eq!(v, json!({"ok": true}));
    }

    #[test]
    fn pipeline_retries_then_succeeds() {
        let agent = MockAgent {
            responses: std::cell::RefCell::new(vec![
                "```json\n{\"ok\":\"nope\"}\n```".to_string(), // invalid
                "```json\n{\"ok\":true}\n```".to_string(),     // valid
            ]),
        };
        let pipe = Pipeline::new(&DefaultRenderer, &agent, &JsonResponseValidator);
        let v = pipe
            .run(
                "{{p}}",
                &HashMap::from([("p".to_string(), "x".to_string())]),
                Path::new("."),
                &tiny_schema(),
            )
            .unwrap();
        assert_eq!(v, json!({"ok": true}));
    }

    #[test]
    fn pipeline_exhausts_retries() {
        let agent = MockAgent {
            responses: std::cell::RefCell::new(vec![
                "```json\n{\"ok\":1}\n```".to_string(),
                "```json\n{\"ok\":2}\n```".to_string(),
                "```json\n{\"ok\":3}\n```".to_string(),
            ]),
        };
        let pipe = Pipeline::new(&DefaultRenderer, &agent, &JsonResponseValidator);
        let err = pipe
            .run(
                "{{p}}",
                &HashMap::from([("p".to_string(), "x".to_string())]),
                Path::new("."),
                &tiny_schema(),
            )
            .unwrap_err();
        match err {
            PipelineError::SchemaValidation { attempts, .. } => assert_eq!(attempts, 3),
            other => panic!("expected SchemaValidation, got {other:?}"),
        }
    }
}
