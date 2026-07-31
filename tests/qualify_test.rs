//! Tests for the model qualification system.
//!
//! Tests the data structures and serialization without running actual qualification
//! (which requires the full agent runtime).

use std::collections::HashMap;

/// Capability status for a qualification test.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
enum CapabilityStatus {
    Passed,
    Failed,
    Unknown,
}

impl std::fmt::Display for CapabilityStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CapabilityStatus::Passed => write!(f, "passed"),
            CapabilityStatus::Failed => write!(f, "failed"),
            CapabilityStatus::Unknown => write!(f, "unknown"),
        }
    }
}

/// A single test result with transcript.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct TestResult {
    name: String,
    passed: bool,
    reason: Option<String>,
    transcript: Vec<TranscriptEntry>,
}

/// Transcript entry for qualification tests.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct TranscriptEntry {
    role: String,
    content: String,
}

/// Full qualification result for a model.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct QualificationResult {
    model_id: String,
    timestamp: String,
    runtime_version: String,
    prompt_hash: String,
    capabilities: HashMap<String, CapabilityStatus>,
    tests: Vec<TestResult>,
}

#[test]
fn qualification_serializes() {
    let mut capabilities = HashMap::new();
    capabilities.insert("chat".into(), CapabilityStatus::Passed);
    capabilities.insert("tool_calling".into(), CapabilityStatus::Failed);

    let result = QualificationResult {
        model_id: "test-model".into(),
        timestamp: "1234567890".into(),
        runtime_version: "3.0.0".into(),
        prompt_hash: "abc123".into(),
        capabilities,
        tests: vec![TestResult {
            name: "test1".into(),
            passed: true,
            reason: None,
            transcript: vec![TranscriptEntry {
                role: "user".into(),
                content: "hello".into(),
            }],
        }],
    };

    let json = serde_json::to_string_pretty(&result).unwrap();
    assert!(json.contains("test-model"));
    assert!(json.contains("abc123"));

    let deserialized: QualificationResult = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.model_id, "test-model");
    assert_eq!(
        deserialized.capabilities.get("chat"),
        Some(&CapabilityStatus::Passed)
    );
}

#[test]
fn capability_status_display() {
    assert_eq!(CapabilityStatus::Passed.to_string(), "passed");
    assert_eq!(CapabilityStatus::Failed.to_string(), "failed");
    assert_eq!(CapabilityStatus::Unknown.to_string(), "unknown");
}

#[test]
fn test_result_with_failure() {
    let result = TestResult {
        name: "tool_calling.read".into(),
        passed: false,
        reason: Some("expected tool 'read', got 'answer'".into()),
        transcript: vec![
            TranscriptEntry {
                role: "user".into(),
                content: "Read README.md".into(),
            },
            TranscriptEntry {
                role: "assistant".into(),
                content: r#"{"answer":"Hi! What would you like to do?"}"#.into(),
            },
        ],
    };

    let json = serde_json::to_string_pretty(&result).unwrap();
    assert!(json.contains("expected tool 'read'"));
    assert!(!result.passed);
}

#[test]
fn qualification_roundtrip() {
    let mut capabilities = HashMap::new();
    capabilities.insert("chat".into(), CapabilityStatus::Passed);
    capabilities.insert("tool_calling".into(), CapabilityStatus::Passed);
    capabilities.insert("multi_step".into(), CapabilityStatus::Failed);

    let result = QualificationResult {
        model_id: "qwen2.5-1.5b".into(),
        timestamp: "1700000000".into(),
        runtime_version: "3.0.0".into(),
        prompt_hash: "def456".into(),
        capabilities,
        tests: vec![],
    };

    // Serialize to JSON
    let json = serde_json::to_string(&result).unwrap();

    // Deserialize back
    let restored: QualificationResult = serde_json::from_str(&json).unwrap();

    // Verify roundtrip
    assert_eq!(restored.model_id, "qwen2.5-1.5b");
    assert_eq!(restored.prompt_hash, "def456");
    assert_eq!(restored.capabilities.len(), 3);
    assert_eq!(
        restored.capabilities.get("multi_step"),
        Some(&CapabilityStatus::Failed)
    );
}

#[test]
fn model_test_cli_variant() {
    // Verify the CLI recognizes the test subcommand
    use clap::Subcommand;

    // This tests that the ModelAction enum has the Test variant
    // by checking the help text includes it
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_cotrex"))
        .args(["model", "--help"])
        .output()
        .expect("failed to run cotrex model --help");

    let help = String::from_utf8_lossy(&output.stdout);
    assert!(
        help.contains("test") || help.contains("Test"),
        "model --help should mention 'test' subcommand"
    );
}
