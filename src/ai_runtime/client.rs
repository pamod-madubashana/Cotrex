use super::error::AiRuntimeError;
use cotrex_ai_contract::{CapabilityKind, CapabilityRequest, CapabilityResponse, ProviderInfo};
use cotrex_ai_runtime::CapabilityProvider;
use std::sync::Arc;

/// Client holding a reference to the AI capability provider.
#[allow(dead_code)]
pub struct AiRuntimeClient {
    provider: Arc<dyn CapabilityProvider + Send + Sync>,
    info: ProviderInfo,
}

#[allow(dead_code)]
impl AiRuntimeClient {
    /// Create a new client with the given provider.
    pub fn new(provider: Arc<dyn CapabilityProvider + Send + Sync>) -> Self {
        let info = provider.info();

        Self { provider, info }
    }

    /// Check if provider supports a capability kind.
    fn supports(&self, kind: CapabilityKind) -> bool {
        self.info.supported_capabilities.contains(&kind)
    }

    /// Execute a capability request with validation.
    pub fn execute(
        &self,
        request: CapabilityRequest,
    ) -> Result<CapabilityResponse, AiRuntimeError> {
        // Determine capability kind from request
        let kind = match &request {
            CapabilityRequest::BuildSummary(_) => CapabilityKind::BuildSummary,
            CapabilityRequest::ExplainRust(_) => CapabilityKind::ExplainRust,
        };

        // Fail fast if provider doesn't support this capability
        if !self.supports(kind) {
            return Err(AiRuntimeError::UnsupportedCapability(format!("{:?}", kind)));
        }

        self.provider.execute(request).map_err(Into::into)
    }

    /// Get provider metadata.
    pub fn info(&self) -> &ProviderInfo {
        &self.info
    }

    /// Get provider health.
    pub fn health(&self) -> cotrex_ai_contract::ProviderHealth {
        self.provider.health()
    }
}
