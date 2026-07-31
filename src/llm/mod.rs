//! LLM integration: compression insights and MCP server.

pub mod compress;
#[cfg(feature = "local-model")]
pub mod local;
pub mod mcp;

pub use compress::{compress, LlmConfig};
#[cfg(feature = "local-model")]
pub use local::infer_local;
