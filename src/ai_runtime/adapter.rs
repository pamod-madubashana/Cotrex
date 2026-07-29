use super::error::AiRuntimeError;
use super::intent::AiCapabilityIntent;
use super::result::{AiResult, AiStatus};
use cotrex_ai_contract::{BuildSummaryRequest, CapabilityKind, ExplainRustRequest};

/// Get the CapabilityKind for an intent.
#[allow(dead_code)]
pub fn intent_kind(intent: &AiCapabilityIntent) -> CapabilityKind {
    match intent {
        AiCapabilityIntent::BuildSummary { .. } => CapabilityKind::BuildSummary,
        AiCapabilityIntent::ExplainRust { .. } => CapabilityKind::ExplainRust,
    }
}

/// Convert AiCapabilityIntent to CapabilityRequest.
#[allow(dead_code)]
pub fn intent_to_request(
    intent: &AiCapabilityIntent,
) -> Result<cotrex_ai_contract::CapabilityRequest, AiRuntimeError> {
    match intent {
        AiCapabilityIntent::BuildSummary {
            command,
            stdout,
            stderr,
            exit_code,
        } => Ok(cotrex_ai_contract::CapabilityRequest::BuildSummary(
            BuildSummaryRequest {
                metadata: cotrex_ai_contract::RequestMetadata::new(),
                command: command.clone(),
                exit_code: *exit_code,
                stdout: stdout.clone(),
                stderr: stderr.clone(),
            },
        )),
        AiCapabilityIntent::ExplainRust { source, question } => Ok(
            cotrex_ai_contract::CapabilityRequest::ExplainRust(ExplainRustRequest {
                metadata: cotrex_ai_contract::RequestMetadata::new(),
                source: source.clone(),
                question: question.clone(),
            }),
        ),
    }
}

/// Convert CapabilityResponse to AiResult.
#[allow(dead_code)]
pub fn response_to_result(
    response: cotrex_ai_contract::CapabilityResponse,
) -> Result<AiResult, AiRuntimeError> {
    match response {
        cotrex_ai_contract::CapabilityResponse::BuildSummary(resp) => {
            let status = if resp.success {
                AiStatus::Success
            } else {
                AiStatus::Failed
            };
            let mut result = AiResult::new(
                status,
                format!(
                    "Build {}: {}",
                    if resp.success { "succeeded" } else { "failed" },
                    resp.summary
                ),
            );
            if let Some(rec) = resp.recommendation {
                result = result.with_details(rec);
            }
            Ok(result)
        }
        cotrex_ai_contract::CapabilityResponse::ExplainRust(resp) => {
            Ok(AiResult::success(resp.explanation))
        }
    }
}
