use cotrex_ai_kernel::AiContextSummary;

use super::git::GitSnapshot;

// ---------------------------------------------------------------------------
// WorkspaceSnapshot
//
// Aggregated workspace state. Combines kernel AI context with git
// workspace information into a single snapshot for downstream
// translation into InferenceContext.
//
// This is the stable aggregation point for future snapshot types
// (build, diagnostics, etc.) — Milestone K territory.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct WorkspaceSnapshot {
    pub ai: AiContextSummary,
    pub git: GitSnapshot,
}
