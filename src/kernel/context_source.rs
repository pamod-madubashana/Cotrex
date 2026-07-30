use std::sync::Arc;

use cotrex_ai_runtime::{ContextSource, InferenceContext, RuntimeError};

use super::snapshot::WorkspaceSnapshot;
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
        let snapshot = self.kernel.snapshot();
        Ok(snapshot_to_context(&snapshot))
    }
}

fn snapshot_to_context(snapshot: &WorkspaceSnapshot) -> InferenceContext {
    use cotrex_ai_kernel::WorkspaceStatus as KStatus;
    use cotrex_ai_runtime::WorkspaceStatus;

    let workspace_status = match snapshot.ai.workspace_status {
        KStatus::Empty => WorkspaceStatus::Unknown,
        KStatus::Active => WorkspaceStatus::Modified,
        KStatus::Idle => WorkspaceStatus::Clean,
    };

    let mut ctx = InferenceContext {
        recent_changes: snapshot.ai.recent_changes.clone(),
        workspace_status,
        file_count: snapshot.ai.file_count,
        hash: 0,
        git_branch: snapshot.git.branch.clone(),
        git_dirty: snapshot.git.working_tree_dirty,
        git_modified_count: snapshot.git.modified_files.len(),
    };
    ctx.hash = ctx.compute_hash();
    ctx
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::git::GitSnapshot;
    use cotrex_ai_kernel::AiContextSummary;
    use cotrex_ai_kernel::WorkspaceStatus as KStatus;

    fn make_snapshot(ai: AiContextSummary) -> WorkspaceSnapshot {
        WorkspaceSnapshot {
            ai,
            git: GitSnapshot::default(),
        }
    }

    #[test]
    fn summary_empty_maps_to_unknown() {
        let snapshot = make_snapshot(AiContextSummary {
            workspace_status: KStatus::Empty,
            recent_changes: Vec::new(),
            file_count: 0,
            total_changes: 0,
        });
        let ctx = snapshot_to_context(&snapshot);
        assert_eq!(
            ctx.workspace_status,
            cotrex_ai_runtime::WorkspaceStatus::Unknown
        );
        assert_eq!(ctx.file_count, 0);
        assert!(ctx.recent_changes.is_empty());
    }

    #[test]
    fn summary_active_maps_to_modified() {
        let snapshot = make_snapshot(AiContextSummary {
            workspace_status: KStatus::Active,
            recent_changes: vec!["src/main.rs".to_string()],
            file_count: 5,
            total_changes: 10,
        });
        let ctx = snapshot_to_context(&snapshot);
        assert_eq!(
            ctx.workspace_status,
            cotrex_ai_runtime::WorkspaceStatus::Modified
        );
        assert_eq!(ctx.file_count, 5);
        assert_eq!(ctx.recent_changes, vec!["src/main.rs".to_string()]);
    }

    #[test]
    fn summary_idle_maps_to_clean() {
        let snapshot = make_snapshot(AiContextSummary {
            workspace_status: KStatus::Idle,
            recent_changes: Vec::new(),
            file_count: 3,
            total_changes: 7,
        });
        let ctx = snapshot_to_context(&snapshot);
        assert_eq!(
            ctx.workspace_status,
            cotrex_ai_runtime::WorkspaceStatus::Clean
        );
        assert_eq!(ctx.file_count, 3);
    }

    #[test]
    fn context_hash_is_computed() {
        let snapshot = make_snapshot(AiContextSummary {
            workspace_status: KStatus::Active,
            recent_changes: vec!["a.rs".to_string(), "b.rs".to_string()],
            file_count: 2,
            total_changes: 2,
        });
        let ctx = snapshot_to_context(&snapshot);
        assert_ne!(ctx.hash, 0);
        assert_eq!(ctx.hash, ctx.compute_hash());
    }

    #[test]
    fn git_fields_populated_from_snapshot() {
        let snapshot = WorkspaceSnapshot {
            ai: AiContextSummary {
                workspace_status: KStatus::Active,
                recent_changes: Vec::new(),
                file_count: 10,
                total_changes: 1,
            },
            git: GitSnapshot {
                branch: Some("main".to_string()),
                working_tree_dirty: true,
                modified_files: vec!["src/main.rs".to_string()],
                ..Default::default()
            },
        };
        let ctx = snapshot_to_context(&snapshot);
        assert_eq!(ctx.git_branch.as_deref(), Some("main"));
        assert!(ctx.git_dirty);
        assert_eq!(ctx.git_modified_count, 1);
    }
}
