use std::fmt;

/// Cotrex-specific AI runtime errors.
/// No internal provider details exposed to agent.
#[derive(Debug)]
#[allow(dead_code)]
pub enum AiRuntimeError {
    /// Provider failed to execute capability
    ProviderFailure,

    /// Response didn't match expected type
    InvalidResponse,

    /// Requested capability not supported by provider
    UnsupportedCapability(String),
}

impl fmt::Display for AiRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProviderFailure => write!(f, "AI capability unavailable: provider failure"),
            Self::InvalidResponse => write!(f, "Invalid response from AI provider"),
            Self::UnsupportedCapability(cap) => write!(f, "Unsupported AI capability: {cap}"),
        }
    }
}

impl std::error::Error for AiRuntimeError {}

impl From<cotrex_ai_runtime::RuntimeError> for AiRuntimeError {
    fn from(err: cotrex_ai_runtime::RuntimeError) -> Self {
        match err {
            cotrex_ai_runtime::RuntimeError::Provider(_) => Self::ProviderFailure,
            cotrex_ai_runtime::RuntimeError::InvalidResponse => Self::InvalidResponse,
            cotrex_ai_runtime::RuntimeError::Capability(e) => Self::from(e),
        }
    }
}

impl From<cotrex_ai_contract::CapabilityError> for AiRuntimeError {
    fn from(err: cotrex_ai_contract::CapabilityError) -> Self {
        match err {
            cotrex_ai_contract::CapabilityError::InvalidRequest => Self::ProviderFailure,
            cotrex_ai_contract::CapabilityError::UnsupportedProtocolVersion => {
                Self::ProviderFailure
            }
        }
    }
}
