/// Cotrex-facing presentation result.
/// Not a replacement for cotrex-ai CapabilityResponse.
///
/// This is an adapter result for RFC-0005. Different consumers
/// (CLI, MCP, agent context) format this differently.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AiResult {
    pub status: AiStatus,
    pub summary: String,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum AiStatus {
    Success,
    Failed,
}

#[allow(dead_code)]
impl AiResult {
    pub fn success(summary: impl Into<String>) -> Self {
        Self {
            status: AiStatus::Success,
            summary: summary.into(),
            details: None,
        }
    }

    pub fn failed(summary: impl Into<String>) -> Self {
        Self {
            status: AiStatus::Failed,
            summary: summary.into(),
            details: None,
        }
    }

    pub fn new(status: AiStatus, summary: impl Into<String>) -> Self {
        Self {
            status,
            summary: summary.into(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }
}
