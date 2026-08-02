//! LLM integration: local inference and MCP server.

pub mod local;
pub mod mcp;

pub use local::console_write;
pub use local::infer_local;
pub use local::infer_local_profiled;
pub use local::infer_local_stream;
