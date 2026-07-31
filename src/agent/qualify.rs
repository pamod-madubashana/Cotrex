//! Model qualification system for Cotrex.
//!
//! Verifies whether a local model can reliably drive the Cotrex agent runtime.
//! This is NOT a benchmark — it's a runtime acceptance gate.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::agent::prompt::Decision;
use crate::agent::tool;

/// Capability status for a qualification test.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CapabilityStatus {
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub name: String,
    pub passed: bool,
    pub reason: Option<String>,
    pub transcript: Vec<TranscriptEntry>,
}

/// Transcript entry for qualification tests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptEntry {
    pub role: String,
    pub content: String,
}

/// Full qualification result for a model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualificationResult {
    pub model_id: String,
    pub timestamp: String,
    pub runtime_version: String,
    pub prompt_hash: String,
    pub capabilities: HashMap<String, CapabilityStatus>,
    pub tests: Vec<TestResult>,
}

/// Qualification test definition.
struct QualTest {
    name: String,
    capability: String,
    prompt: String,
    validator: fn(&Decision) -> Result<(), String>,
}

/// Compute a deterministic hash of the qualification prompts and system prompt.
pub fn compute_prompt_hash() -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();

    // Hash the decision system prompt
    let system = crate::agent::prompt::decision_system();
    system.hash(&mut hasher);

    // Hash tool schemas
    for tool in tool::BUILTINS {
        tool.name.hash(&mut hasher);
        tool.parameters.hash(&mut hasher);
    }

    // Hash qualification test prompts
    for test in qualification_tests() {
        test.prompt.hash(&mut hasher);
    }

    format!("{:016x}", hasher.finish())
}

/// Get the qualification storage directory.
fn qual_dir() -> Result<PathBuf, String> {
    let home = std::env::var("COTREX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .unwrap_or_else(|_| ".".into());
            PathBuf::from(home).join(".cotrex")
        });
    let dir = home.join("qualifications");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// Get the qualification file path for a model.
fn qual_path(model_id: &str) -> Result<PathBuf, String> {
    Ok(qual_dir()?.join(format!("{model_id}.json")))
}

/// Load existing qualification for a model.
pub fn load_qualification(model_id: &str) -> Option<QualificationResult> {
    let path = qual_path(model_id).ok()?;
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Save qualification result.
pub fn save_qualification(result: &QualificationResult) -> Result<(), String> {
    let path = qual_path(&result.model_id)?;
    let json = serde_json::to_string_pretty(result).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

/// Define the three qualification tests.
fn qualification_tests() -> Vec<QualTest> {
    vec![
        QualTest {
            name: "tool_calling.read".into(),
            capability: "tool_calling".into(),
            prompt: "Read README.md".into(),
            validator: |d| match d {
                Decision::Tool { tool, args, .. } => {
                    if tool.name != "read" {
                        return Err(format!("expected tool 'read', got '{}'", tool.name));
                    }
                    let path = args
                        .get("path")
                        .and_then(|v| v.as_str())
                        .ok_or("missing 'path' argument")?;
                    if path != "README.md" {
                        return Err(format!("expected path 'README.md', got '{path}'"));
                    }
                    Ok(())
                }
                Decision::Run { cmd, .. } => {
                    Err(format!("expected tool call, got shell command: {cmd}"))
                }
                Decision::Answer(a) => Err(format!("expected tool call, got answer: {a}")),
            },
        },
        QualTest {
            name: "tool_calling.glob".into(),
            capability: "tool_calling".into(),
            prompt: "Find all Rust source files in this project.".into(),
            validator: |d| match d {
                Decision::Tool { tool, args, .. } => {
                    if tool.name != "glob" {
                        return Err(format!("expected tool 'glob', got '{}'", tool.name));
                    }
                    let pattern = args
                        .get("pattern")
                        .and_then(|v| v.as_str())
                        .ok_or("missing 'pattern' argument")?;
                    if !pattern.contains("*.rs") {
                        return Err(format!(
                            "expected pattern containing '*.rs', got '{pattern}'"
                        ));
                    }
                    Ok(())
                }
                Decision::Run { cmd, .. } => {
                    Err(format!("expected tool call, got shell command: {cmd}"))
                }
                Decision::Answer(a) => Err(format!("expected tool call, got answer: {a}")),
            },
        },
        QualTest {
            name: "multi_step".into(),
            capability: "multi_step".into(),
            prompt: "Explain what this project is. Use tools if needed.".into(),
            validator: |d| {
                // For multi-step, we validate that the first decision is a tool call
                // (the actual multi-step loop is tested by the runner)
                match d {
                    Decision::Tool { .. } | Decision::Run { .. } => Ok(()),
                    Decision::Answer(a) => {
                        if a.to_lowercase().contains("current working directory")
                            || a.to_lowercase().contains("hi!")
                            || a.to_lowercase().contains("hello")
                        {
                            Err(format!(
                                "model returned greeting instead of using tools: {a}"
                            ))
                        } else {
                            // Model answered directly — might be OK if it's a real answer
                            Ok(())
                        }
                    }
                }
            },
        },
    ]
}

/// Run all qualification tests for a model.
pub fn run_qualification(model_id: &str) -> QualificationResult {
    let timestamp = chrono_free_timestamp();
    let runtime_version = env!("CARGO_PKG_VERSION").to_string();
    let prompt_hash = compute_prompt_hash();

    let mut capabilities = HashMap::new();
    let mut tests = Vec::new();

    // Test 1 & 2: Tool calling (single-step)
    for test in &qualification_tests()[0..2] {
        let result = run_single_test(test, model_id);
        let status = if result.passed {
            CapabilityStatus::Passed
        } else {
            CapabilityStatus::Failed
        };

        // Update capability status
        let cap = capabilities
            .entry(test.capability.clone())
            .or_insert(CapabilityStatus::Passed);
        if status == CapabilityStatus::Failed {
            *cap = CapabilityStatus::Failed;
        }

        tests.push(result);
    }

    // Test 3: Multi-step (needs the agentic loop)
    let multi_test = &qualification_tests()[2];
    let multi_result = run_multi_step_test(multi_test, model_id);
    let multi_status = if multi_result.passed {
        CapabilityStatus::Passed
    } else {
        CapabilityStatus::Failed
    };
    capabilities.insert(multi_test.capability.clone(), multi_status);
    tests.push(multi_result);

    QualificationResult {
        model_id: model_id.to_string(),
        timestamp,
        runtime_version,
        prompt_hash,
        capabilities,
        tests,
    }
}

/// Run a single-step qualification test.
fn run_single_test(test: &QualTest, model_id: &str) -> TestResult {
    let mut transcript = vec![TranscriptEntry {
        role: "user".into(),
        content: test.prompt.clone(),
    }];

    // Build system prompt
    let system = crate::agent::prompt::decision_system_with_model(model_id);

    // Call the model
    let raw_output = match crate::llm::infer_local(&system, &test.prompt) {
        Ok(output) => output,
        Err(e) => {
            return TestResult {
                name: test.name.clone(),
                passed: false,
                reason: Some(format!("model inference failed: {e}")),
                transcript,
            };
        }
    };

    transcript.push(TranscriptEntry {
        role: "assistant".into(),
        content: raw_output.clone(),
    });

    // Parse decision
    let decision = crate::agent::prompt::parse_decision(&raw_output);

    // Validate
    match (test.validator)(&decision) {
        Ok(()) => TestResult {
            name: test.name.clone(),
            passed: true,
            reason: None,
            transcript,
        },
        Err(reason) => TestResult {
            name: test.name.clone(),
            passed: false,
            reason: Some(reason),
            transcript,
        },
    }
}

/// Run the multi-step qualification test.
fn run_multi_step_test(test: &QualTest, model_id: &str) -> TestResult {
    let mut transcript = vec![TranscriptEntry {
        role: "user".into(),
        content: test.prompt.clone(),
    }];

    let system = crate::agent::prompt::decision_system_with_model(model_id);
    let _perms = crate::agent::permission::Permissions::default();
    let limiter = crate::agent::tool::OutputLimiter { max_lines: 100 };
    let workdir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    let mut seen = Vec::new();
    let max_steps = 4;

    for step in 0..max_steps {
        let user = if step == 0 {
            test.prompt.clone()
        } else {
            format!(
                "Request: {}\n\nSteps taken so far:\n{}\nGather more if needed, else answer.",
                test.prompt,
                transcript
                    .iter()
                    .filter(|e| e.role == "tool_call" || e.role == "tool_result")
                    .map(|e| format!("{}: {}", e.role, e.content))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        };

        let raw_output = match crate::llm::infer_local(&system, &user) {
            Ok(output) => output,
            Err(e) => {
                return TestResult {
                    name: test.name.clone(),
                    passed: false,
                    reason: Some(format!("model inference failed at step {step}: {e}")),
                    transcript,
                };
            }
        };

        transcript.push(TranscriptEntry {
            role: "assistant".into(),
            content: raw_output.clone(),
        });

        let decision = crate::agent::prompt::parse_decision(&raw_output);

        match &decision {
            Decision::Answer(_) => {
                // Model gave a final answer — multi-step succeeded
                return TestResult {
                    name: test.name.clone(),
                    passed: true,
                    reason: None,
                    transcript,
                };
            }
            Decision::Tool { tool, args, .. } => {
                let call_sig = format!("{}:{}", tool.name, args);
                if seen.contains(&call_sig) {
                    return TestResult {
                        name: test.name.clone(),
                        passed: false,
                        reason: Some("model repeated the same tool call (loop detected)".into()),
                        transcript,
                    };
                }
                seen.push(call_sig);

                transcript.push(TranscriptEntry {
                    role: "tool_call".into(),
                    content: format!("{}({})", tool.name, args),
                });

                // Execute tool
                let ctx = crate::agent::tool::ToolContext {
                    workdir: workdir.clone(),
                };
                match (tool.execute)(&ctx, args) {
                    Ok(output) => {
                        let truncated = limiter.truncate(&output);
                        transcript.push(TranscriptEntry {
                            role: "tool_result".into(),
                            content: truncated,
                        });
                    }
                    Err(e) => {
                        transcript.push(TranscriptEntry {
                            role: "tool_result".into(),
                            content: format!("[error] {e}"),
                        });
                    }
                }
            }
            Decision::Run { cmd, .. } => {
                let call_sig = format!("run:{cmd}");
                if seen.contains(&call_sig) {
                    return TestResult {
                        name: test.name.clone(),
                        passed: false,
                        reason: Some("model repeated the same command (loop detected)".into()),
                        transcript,
                    };
                }
                seen.push(call_sig);

                transcript.push(TranscriptEntry {
                    role: "tool_call".into(),
                    content: format!("run: {cmd}"),
                });

                // Execute command
                match std::process::Command::new("sh")
                    .arg("-c")
                    .arg(&cmd)
                    .output()
                {
                    Ok(output) => {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        let combined = format!("{stdout}{stderr}");
                        let truncated = limiter.truncate(&combined);
                        transcript.push(TranscriptEntry {
                            role: "tool_result".into(),
                            content: truncated,
                        });
                    }
                    Err(e) => {
                        transcript.push(TranscriptEntry {
                            role: "tool_result".into(),
                            content: format!("[error] {e}"),
                        });
                    }
                }
            }
        }
    }

    // Ran out of steps without a final answer
    TestResult {
        name: test.name.clone(),
        passed: false,
        reason: Some(format!(
            "model did not produce a final answer within {max_steps} steps"
        )),
        transcript,
    }
}

/// Simple timestamp without chrono dependency.
fn chrono_free_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{secs}")
}
