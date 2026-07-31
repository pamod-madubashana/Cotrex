//! LLM integration: local inference, compression insights, and MCP server.

#[cfg(feature = "local-model")]
pub mod local;
pub mod compress;
pub mod mcp;

pub use compress::{compress, LlmConfig};

#[cfg(feature = "local-model")]
pub use local::infer_local;
