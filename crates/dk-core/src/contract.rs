//! Embedded core-contract validator and Pack schema utilities.

const CONTRACT_SCHEMA: &str = include_str!("../schemas/dk-core-contract-v1.json");

/// Validates agent output against the embedded `dk-core-contract-v1.json`.
pub struct ContractValidator {
    schema: serde_json::Value,
}

impl ContractValidator {
    /// Load the embedded contract schema (compile-time `include_str!`).
    pub fn new() -> Self {
        let schema: serde_json::Value =
            serde_json::from_str(CONTRACT_SCHEMA).expect("embedded contract schema is valid JSON");
        Self { schema }
    }

    /// Validate a JSON value against the contract.
    /// Returns `Ok(())` or `Err(Vec<error_message>)`.
    pub fn validate(&self, doc: &serde_json::Value) -> Result<(), Vec<String>> {
        let validator = jsonschema::validator_for(&self.schema)
            .map_err(|e| vec![format!("invalid contract schema: {e}")])?;
        let errors: Vec<String> = validator.iter_errors(doc).map(|e| e.to_string()).collect();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Extract the severity enum order from a Pack output schema.
    ///
    /// Looks at `$defs/finding/properties/severity/enum` or
    /// `definitions/finding/properties/severity/enum`.
    /// Returns `None` if not found or not an array of strings.
    pub fn severity_order(pack_output_schema: &serde_json::Value) -> Option<Vec<String>> {
        let order = pack_output_schema
            .pointer("/$defs/finding/properties/severity/enum")
            .or_else(|| {
                pack_output_schema.pointer("/definitions/finding/properties/severity/enum")
            })?;
        let arr = order.as_array()?;
        let strings: Vec<String> = arr
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();
        if strings.is_empty() {
            None
        } else {
            Some(strings)
        }
    }
}

impl Default for ContractValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_doc() -> serde_json::Value {
        serde_json::json!({
            "summary": {
                "verdict": "approve",
                "overall_score": 8,
                "one_paragraph": "Looks good."
            },
            "grades": {
                "alpha": { "score": 8, "rationale": "solid" }
            },
            "overall_score": 8,
            "good_things": ["clean code"],
            "findings": [],
            "limitations": [],
            "suggested_next_steps": ["none needed"]
        })
    }

    #[test]
    fn valid_document_passes() {
        ContractValidator::new()
            .validate(&valid_doc())
            .expect("should pass");
    }

    #[test]
    fn missing_verdict_fails() {
        let mut doc = valid_doc();
        doc["summary"].as_object_mut().unwrap().remove("verdict");
        assert!(ContractValidator::new().validate(&doc).is_err());
    }

    #[test]
    fn invalid_verdict_string_fails() {
        let mut doc = valid_doc();
        doc["summary"]["verdict"] = serde_json::json!("unknown_verdict");
        assert!(ContractValidator::new().validate(&doc).is_err());
    }

    #[test]
    fn missing_suggested_next_steps_fails() {
        let mut doc = valid_doc();
        doc.as_object_mut().unwrap().remove("suggested_next_steps");
        assert!(ContractValidator::new().validate(&doc).is_err());
    }

    #[test]
    fn empty_suggested_next_steps_fails() {
        let mut doc = valid_doc();
        doc["suggested_next_steps"] = serde_json::json!([]);
        assert!(ContractValidator::new().validate(&doc).is_err());
    }

    #[test]
    fn empty_grades_fails() {
        let mut doc = valid_doc();
        doc["grades"] = serde_json::json!({});
        assert!(ContractValidator::new().validate(&doc).is_err());
    }

    #[test]
    fn extra_summary_fields_allowed() {
        let mut doc = valid_doc();
        doc["summary"]["structure_score"] = serde_json::json!(8.5);
        ContractValidator::new()
            .validate(&doc)
            .expect("extra summary fields should be allowed");
    }

    #[test]
    fn severity_order_from_defs() {
        let schema = serde_json::json!({
            "$defs": {
                "finding": {
                    "properties": {
                        "severity": {
                            "enum": ["high", "low"]
                        }
                    }
                }
            }
        });
        let order = ContractValidator::severity_order(&schema);
        assert_eq!(order, Some(vec!["high".to_string(), "low".to_string()]));
    }

    #[test]
    fn severity_order_missing_returns_none() {
        let schema = serde_json::json!({ "type": "object" });
        assert_eq!(ContractValidator::severity_order(&schema), None);
    }
}
