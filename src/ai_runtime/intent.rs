/// Explicit AI capability intent from Cotrex agent layer.
///
/// The agent decides when AI is needed and constructs the appropriate intent.
/// RFC-0005 only translates. No string parsing.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum AiCapabilityIntent {
    /// Analyze build/test output and provide summary
    BuildSummary {
        command: String,
        stdout: String,
        stderr: String,
        exit_code: i32,
    },

    /// Explain Rust code or answer questions about it
    ExplainRust { source: String, question: String },
}
