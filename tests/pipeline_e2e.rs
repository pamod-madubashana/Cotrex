use std::sync::Arc;

use cotrex::kernel::context_source::KernelContextSource;
use cotrex::kernel::WorkspaceKernel;
use cotrex_ai_contract::{
    BuildSummaryRequest, BuildSummaryResponse, CapabilityRequest, CapabilityResponse,
    ExplainRustResponse, RequestMetadata,
};
use cotrex_ai_runtime::{
    CapabilityProvider, ContextSource, DefaultCapabilityResponseParser, DefaultOutputParser,
    DefaultPromptAssembler, InferenceContext, OrchestrationRequest, Orchestrator, RuntimeError,
    WorkspaceStatus,
};

// ---------------------------------------------------------------------------
// CapturingProvider
//
// Records the CapabilityRequest it receives, then returns a deterministic
// response. This proves the orchestrator actually carries the assembled
// context into the provider boundary.
// ---------------------------------------------------------------------------

struct CapturingProvider {
    captured: std::sync::Mutex<Option<CapabilityRequest>>,
}

impl CapturingProvider {
    fn new() -> Self {
        Self {
            captured: std::sync::Mutex::new(None),
        }
    }

    fn captured_request(&self) -> Option<CapabilityRequest> {
        self.captured.lock().unwrap().take()
    }
}

impl CapabilityProvider for CapturingProvider {
    fn info(&self) -> cotrex_ai_contract::ProviderInfo {
        cotrex_ai_contract::ProviderInfo {
            name: "capturing".into(),
            version: "0.1.0".into(),
            supported_capabilities: vec![
                cotrex_ai_contract::CapabilityKind::BuildSummary,
                cotrex_ai_contract::CapabilityKind::ExplainRust,
            ],
        }
    }

    fn health(&self) -> cotrex_ai_contract::ProviderHealth {
        cotrex_ai_contract::ProviderHealth::Healthy
    }

    fn execute(&self, request: CapabilityRequest) -> Result<CapabilityResponse, RuntimeError> {
        *self.captured.lock().unwrap() = Some(request.clone());
        match request {
            CapabilityRequest::BuildSummary(req) => {
                Ok(CapabilityResponse::BuildSummary(BuildSummaryResponse {
                    success: req.exit_code == 0,
                    summary: format!("mock: exit {}", req.exit_code),
                    recommendation: None,
                }))
            }
            CapabilityRequest::ExplainRust(req) => {
                Ok(CapabilityResponse::ExplainRust(ExplainRustResponse {
                    explanation: format!("mock: {}", req.question),
                }))
            }
        }
    }
}

fn build_orchestrator(
    provider: Arc<dyn CapabilityProvider + Send + Sync>,
    context_source: Arc<dyn ContextSource>,
) -> Orchestrator {
    Orchestrator::new(
        provider,
        context_source,
        Arc::new(DefaultPromptAssembler),
        Arc::new(DefaultOutputParser),
        Arc::new(DefaultCapabilityResponseParser),
    )
}

fn build_summary_request() -> CapabilityRequest {
    CapabilityRequest::BuildSummary(BuildSummaryRequest {
        metadata: RequestMetadata::new(),
        command: "cargo build".into(),
        exit_code: 0,
        stdout: "Finished dev profile".into(),
        stderr: String::new(),
        prompt: "Summarize build: cargo build".into(),
        temperature: 0.1,
        max_tokens: 512,
    })
}

// ---------------------------------------------------------------------------
// T1: Pipeline with mock provider (no git repo)
// ---------------------------------------------------------------------------

#[test]
fn pipeline_with_mock_provider() {
    let tmp = tempfile::tempdir().unwrap();
    let kernel = Arc::new(WorkspaceKernel::open(tmp.path().to_path_buf()).unwrap());
    let context_source = Arc::new(KernelContextSource::new(kernel));
    let provider = Arc::new(CapturingProvider::new());

    let orch = build_orchestrator(provider.clone(), context_source);
    let request = OrchestrationRequest {
        capability: build_summary_request(),
        context: None,
    };

    let response = orch.execute(request).unwrap();
    assert!(response.text().contains("mock: exit 0"));

    // Verify the prompt that reached the provider contains expected fields
    let captured = provider.captured_request().unwrap();
    if let CapabilityRequest::BuildSummary(req) = captured {
        assert!(req.prompt.contains("Status:"));
        assert!(req.prompt.contains("Files tracked:"));
        assert!(req.prompt.contains("Working tree dirty:"));
        assert!(req.prompt.contains("Context hash:"));
    } else {
        panic!("Expected BuildSummary request");
    }
}

// ---------------------------------------------------------------------------
// T2: Pipeline with git context
// ---------------------------------------------------------------------------

#[test]
fn pipeline_with_git_context() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_path_buf();

    // Initialize git repo with a tracked file, then modify it
    let git_init = std::process::Command::new("git")
        .args(["init"])
        .current_dir(&root)
        .output();
    if git_init.is_err() || !git_init.unwrap().status.success() {
        // Git not available — skip this test
        return;
    }

    // Configure git identity (required for commits on Windows)
    let _ = std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(&root)
        .output();
    let _ = std::process::Command::new("git")
        .args(["config", "user.name", "Cotrex Test"])
        .current_dir(&root)
        .output();

    // Create and commit a file
    std::fs::write(root.join("file.txt"), "initial").unwrap();
    let _ = std::process::Command::new("git")
        .args(["add", "file.txt"])
        .current_dir(&root)
        .output();
    let commit = std::process::Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(&root)
        .output();
    if commit.is_err() || !commit.unwrap().status.success() {
        return;
    }

    // Modify the tracked file to create dirty state
    std::fs::write(root.join("file.txt"), "modified").unwrap();

    let kernel = Arc::new(WorkspaceKernel::open(root).unwrap());
    let context_source = Arc::new(KernelContextSource::new(kernel));
    let provider = Arc::new(CapturingProvider::new());

    let orch = build_orchestrator(provider.clone(), context_source);
    let request = OrchestrationRequest {
        capability: build_summary_request(),
        context: None,
    };

    let response = orch.execute(request).unwrap();
    assert!(response.text().contains("mock: exit 0"));

    // Verify git context flows through to the provider
    let captured = provider.captured_request().unwrap();
    if let CapabilityRequest::BuildSummary(req) = captured {
        assert!(
            req.prompt.contains("Branch:"),
            "Expected Branch: in prompt, got:\n{}",
            req.prompt
        );
        assert!(
            req.prompt.contains("Working tree dirty: true"),
            "Expected Working tree dirty: true, got:\n{}",
            req.prompt
        );
        assert!(
            req.prompt.contains("Modified files:"),
            "Expected Modified files: in prompt, got:\n{}",
            req.prompt
        );
        assert!(
            req.prompt.contains("Context hash:"),
            "Expected Context hash: in prompt, got:\n{}",
            req.prompt
        );
    } else {
        panic!("Expected BuildSummary request");
    }
}

// ---------------------------------------------------------------------------
// T3: Prompt contains workspace status and file count
// ---------------------------------------------------------------------------

#[test]
fn pipeline_prompt_contains_workspace_status() {
    let tmp = tempfile::tempdir().unwrap();
    let kernel = Arc::new(WorkspaceKernel::open(tmp.path().to_path_buf()).unwrap());

    // Observe a file creation to set workspace status to Active
    kernel
        .observe(cotrex_ai_kernel::RawObservation {
            path: tmp.path().join("src/main.rs"),
            operation: cotrex_ai_kernel::RawOperation::Created,
        })
        .unwrap();

    let context_source = Arc::new(KernelContextSource::new(kernel));
    let provider = Arc::new(CapturingProvider::new());

    let orch = build_orchestrator(provider.clone(), context_source);
    let request = OrchestrationRequest {
        capability: build_summary_request(),
        context: None,
    };

    let _ = orch.execute(request).unwrap();

    let captured = provider.captured_request().unwrap();
    if let CapabilityRequest::BuildSummary(req) = captured {
        assert!(
            req.prompt.contains("Status: Modified"),
            "Expected Status: Modified in prompt, got:\n{}",
            req.prompt
        );
        assert!(
            req.prompt.contains("Files tracked:"),
            "Expected Files tracked: in prompt, got:\n{}",
            req.prompt
        );
    } else {
        panic!("Expected BuildSummary request");
    }
}

// ---------------------------------------------------------------------------
// T4: Null context source fallback
// ---------------------------------------------------------------------------

#[test]
fn pipeline_null_context_fallback() {
    let context_source = Arc::new(cotrex_ai_runtime::NullContextSource);
    let provider = Arc::new(CapturingProvider::new());

    let orch = build_orchestrator(provider.clone(), context_source);
    let request = OrchestrationRequest {
        capability: build_summary_request(),
        context: None,
    };

    let response = orch.execute(request).unwrap();
    assert!(response.text().contains("mock: exit 0"));

    let captured = provider.captured_request().unwrap();
    if let CapabilityRequest::BuildSummary(req) = captured {
        // NullContextSource defaults — no git info
        assert!(req.prompt.contains("Working tree dirty: false"));
        assert!(req.prompt.contains("Context hash:"));
    } else {
        panic!("Expected BuildSummary request");
    }
}

// ---------------------------------------------------------------------------
// T5: Hash changes with git state
// ---------------------------------------------------------------------------

#[test]
fn pipeline_hash_changes_with_git_state() {
    let ctx_clean = InferenceContext {
        recent_changes: vec![],
        workspace_status: WorkspaceStatus::Clean,
        file_count: 5,
        hash: 0,
        git_branch: None,
        git_dirty: false,
        git_modified_count: 0,
        tracked_files: 10,
    };
    let ctx_dirty = InferenceContext {
        recent_changes: vec![],
        workspace_status: WorkspaceStatus::Clean,
        file_count: 5,
        hash: 0,
        git_branch: Some("main".into()),
        git_dirty: true,
        git_modified_count: 3,
        tracked_files: 10,
    };

    let h1 = ctx_clean.compute_hash();
    let h2 = ctx_dirty.compute_hash();
    assert_ne!(h1, h2, "Hashes should differ with different git state");
}
