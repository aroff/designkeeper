//! Post-validation module — removed in favour of Pack-schema ownership.
//!
//! V1–V4 rules (score reconciliation, blocker/verdict correlation) were
//! default-rubric assumptions and have been deleted. The Pack's `schemas/review.json`
//! is responsible for consistency constraints. The embedded core contract
//! (`contract.rs`) enforces the minimum required fields.
