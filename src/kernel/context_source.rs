use std::sync::Arc;

use cotrex_ai_kernel::AiContextSummary;
use cotrex_ai_runtime::{ContextSource, InferenceContext, RuntimeError};

use super::WorkspaceKernel;

pub struct KernelContextSource {
    kernel: Arc<WorkspaceKernel>,
}

impl KernelContextSource {
    pub fn new(kernel: Arc<WorkspaceKernel>) -> Self {
        Self { kernel }
    }
}

impl ContextSource for KernelContextSource {
    fn context(&self) -> Result<InferenceContext, RuntimeError> {
        let summary = self.kernel.summary();
        Ok(summary_to_context(&summary))
    }
}

fn summary_to_context(summary: &AiContextSummary) -> InferenceContext {
    use cotrex_ai_kernel::WorkspaceStatus as KStatus;
    use cotrex_ai_runtime::WorkspaceStatus;

    let workspace_status = match summary.workspace_status {
        KStatus::Empty => WorkspaceStatus::Unknown,
        KStatus::Active => WorkspaceStatus::Modified,
        KStatus::Idle => WorkspaceStatus::Clean,
    };

    let mut ctx = InferenceContext {
        recent_changes: summary.recent_changes.clone(),
        workspace_status,
        file_count: summary.file_count,
        hash: 0,
    };
    ctx.hash = ctx.compute_hash();
    ctx
}

#[cfg(test)]
mod tests {
    use super::*;
    use cotrex_ai_kernel::WorkspaceStatus as KStatus;

    #[test]
    fn summary_empty_maps_to_unknown() {
        let summary = AiContextSummary {
            workspace_status: KStatus::Empty,
            recent_changes: Vec::new(),
            file_count: 0,
            total_changes: 0,
        };
        let ctx = summary_to_context(&summary);
        assert_eq!(
            ctx.workspace_status,
            cotrex_ai_runtime::WorkspaceStatus::Unknown
        );
        assert_eq!(ctx.file_count, 0);
        assert!(ctx.recent_changes.is_empty());
    }

    #[test]
    fn summary_active_maps_to_modified() {
        let summary = AiContextSummary {
            workspace_status: KStatus::Active,
            recent_changes: vec!["src/main.rs".to_string()],
            file_count: 5,
            total_changes: 10,
        };
        let ctx = summary_to_context(&summary);
        assert_eq!(
            ctx.workspace_status,
            cotrex_ai_runtime::WorkspaceStatus::Modified
        );
        assert_eq!(ctx.file_count, 5);
        assert_eq!(ctx.recent_changes, vec!["src/main.rs".to_string()]);
    }

    #[test]
    fn summary_idle_maps_to_clean() {
        let summary = AiContextSummary {
            workspace_status: KStatus::Idle,
            recent_changes: Vec::new(),
            file_count: 3,
            total_changes: 7,
        };
        let ctx = summary_to_context(&summary);
        assert_eq!(
            ctx.workspace_status,
            cotrex_ai_runtime::WorkspaceStatus::Clean
        );
        assert_eq!(ctx.file_count, 3);
    }

    #[test]
    fn context_hash_is_computed() {
        let summary = AiContextSummary {
            workspace_status: KStatus::Active,
            recent_changes: vec!["a.rs".to_string(), "b.rs".to_string()],
            file_count: 2,
            total_changes: 2,
        };
        let ctx = summary_to_context(&summary);
        assert_ne!(ctx.hash, 0);
        assert_eq!(ctx.hash, ctx.compute_hash());
    }
}
