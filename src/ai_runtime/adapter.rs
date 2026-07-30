use super::error::AiRuntimeError;
use super::result::{AiResult, AiStatus};

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
