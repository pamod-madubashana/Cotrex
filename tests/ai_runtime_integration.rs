use cotrex::ai_runtime::adapter;
use cotrex::ai_runtime::client::AiRuntimeClient;
use cotrex::ai_runtime::error::AiRuntimeError;
use cotrex::ai_runtime::result::{AiResult, AiStatus};
use cotrex_ai_contract::*;
use cotrex_ai_runtime::CapabilityProvider;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Test providers
// ---------------------------------------------------------------------------

struct TestProvider;

impl CapabilityProvider for TestProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            name: "test".into(),
            version: "0.1.0".into(),
            supported_capabilities: vec![CapabilityKind::BuildSummary, CapabilityKind::ExplainRust],
        }
    }

    fn health(&self) -> ProviderHealth {
        ProviderHealth::Healthy
    }

    fn execute(
        &self,
        request: CapabilityRequest,
    ) -> Result<CapabilityResponse, cotrex_ai_runtime::RuntimeError> {
        match request {
            CapabilityRequest::BuildSummary(req) => {
                Ok(CapabilityResponse::BuildSummary(BuildSummaryResponse {
                    success: req.exit_code == 0,
                    summary: format!("exit {}", req.exit_code),
                    recommendation: None,
                }))
            }
            CapabilityRequest::ExplainRust(req) => {
                Ok(CapabilityResponse::ExplainRust(ExplainRustResponse {
                    explanation: format!("About: {}", req.question),
                }))
            }
        }
    }
}

struct BuildOnlyProvider;

impl CapabilityProvider for BuildOnlyProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            name: "build-only".into(),
            version: "0.1.0".into(),
            supported_capabilities: vec![CapabilityKind::BuildSummary],
        }
    }

    fn health(&self) -> ProviderHealth {
        ProviderHealth::Healthy
    }

    fn execute(
        &self,
        request: CapabilityRequest,
    ) -> Result<CapabilityResponse, cotrex_ai_runtime::RuntimeError> {
        match request {
            CapabilityRequest::BuildSummary(req) => {
                Ok(CapabilityResponse::BuildSummary(BuildSummaryResponse {
                    success: req.exit_code == 0,
                    summary: format!("exit {}", req.exit_code),
                    recommendation: None,
                }))
            }
            _ => Err(cotrex_ai_runtime::RuntimeError::Capability(
                CapabilityError::InvalidRequest,
            )),
        }
    }
}

struct FailingProvider;

impl CapabilityProvider for FailingProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            name: "failing".into(),
            version: "0.1.0".into(),
            supported_capabilities: vec![CapabilityKind::BuildSummary],
        }
    }

    fn health(&self) -> ProviderHealth {
        ProviderHealth::Unhealthy { reason: "test" }
    }

    fn execute(
        &self,
        _request: CapabilityRequest,
    ) -> Result<CapabilityResponse, cotrex_ai_runtime::RuntimeError> {
        Err(cotrex_ai_runtime::RuntimeError::Provider(
            "simulated failure".into(),
        ))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn test_build_summary_success() {
    let client = AiRuntimeClient::new(Arc::new(TestProvider));
    let request = CapabilityRequest::BuildSummary(BuildSummaryRequest {
        metadata: RequestMetadata::new(),
        command: "cargo build".into(),
        exit_code: 0,
        stdout: String::new(),
        stderr: String::new(),
        prompt: String::new(),
        temperature: 0.1,
        max_tokens: 512,
    });
    let response = client.execute(request).unwrap();
    let result = adapter::response_to_result(response).unwrap();

    assert_eq!(result.status, AiStatus::Success);
    assert!(result.summary.contains("succeeded"));
}

#[test]
fn test_build_summary_failure() {
    let client = AiRuntimeClient::new(Arc::new(TestProvider));
    let request = CapabilityRequest::BuildSummary(BuildSummaryRequest {
        metadata: RequestMetadata::new(),
        command: "cargo build".into(),
        exit_code: 1,
        stdout: String::new(),
        stderr: "error".into(),
        prompt: String::new(),
        temperature: 0.1,
        max_tokens: 512,
    });
    let response = client.execute(request).unwrap();
    let result = adapter::response_to_result(response).unwrap();

    assert_eq!(result.status, AiStatus::Failed);
    assert!(result.summary.contains("failed"));
}

#[test]
fn test_explain_rust_success() {
    let client = AiRuntimeClient::new(Arc::new(TestProvider));
    let request = CapabilityRequest::ExplainRust(ExplainRustRequest {
        metadata: RequestMetadata::new(),
        source: "let x = 1;".into(),
        question: "what is x?".into(),
        prompt: String::new(),
        temperature: 0.2,
        max_tokens: 1024,
    });
    let response = client.execute(request).unwrap();
    let result = adapter::response_to_result(response).unwrap();

    assert_eq!(result.status, AiStatus::Success);
    assert!(!result.summary.is_empty());
}

#[test]
fn test_unsupported_capability() {
    let client = AiRuntimeClient::new(Arc::new(BuildOnlyProvider));
    let request = CapabilityRequest::ExplainRust(ExplainRustRequest {
        metadata: RequestMetadata::new(),
        source: "let x = 1;".into(),
        question: "what is x?".into(),
        prompt: String::new(),
        temperature: 0.2,
        max_tokens: 1024,
    });
    let result = client.execute(request);

    assert!(result.is_err());
    let err_str = format!("{}", result.unwrap_err());
    assert!(err_str.contains("Unsupported AI capability"));
}

#[test]
fn test_provider_failure() {
    let client = AiRuntimeClient::new(Arc::new(FailingProvider));
    let request = CapabilityRequest::BuildSummary(BuildSummaryRequest {
        metadata: RequestMetadata::new(),
        command: "cargo build".into(),
        exit_code: 1,
        stdout: String::new(),
        stderr: String::new(),
        prompt: String::new(),
        temperature: 0.1,
        max_tokens: 512,
    });
    let result = client.execute(request);

    assert!(result.is_err());
    let err_str = format!("{}", result.unwrap_err());
    // Verify error is clean, no internal details
    assert!(!err_str.contains("simulated failure"));
    assert!(err_str.contains("provider failure"));
}

#[test]
fn test_error_mapping_from_runtime_error() {
    let runtime_err = cotrex_ai_runtime::RuntimeError::Provider("CUDA panic".into());
    let ai_err: AiRuntimeError = runtime_err.into();

    // No message field - just type
    assert!(matches!(ai_err, AiRuntimeError::ProviderFailure));

    // Clean display
    let display = format!("{ai_err}");
    assert!(!display.contains("CUDA"));
    assert!(display.contains("provider failure"));
}

#[test]
fn test_client_info_cached() {
    let client = AiRuntimeClient::new(Arc::new(TestProvider));
    let info = client.info();

    assert_eq!(info.name, "test");
    assert_eq!(info.version, "0.1.0");
    assert_eq!(info.supported_capabilities.len(), 2);
}

#[test]
fn test_client_health() {
    let client = AiRuntimeClient::new(Arc::new(TestProvider));
    let health = client.health();

    assert!(matches!(health, ProviderHealth::Healthy));
}
