pub mod context_source;

use std::path::PathBuf;
use std::sync::Arc;

use cotrex_ai_kernel::{
    AiContextProjection, AiContextSummary, EventStore, FileChangeProjection, ObservationPipeline,
    PersistentEventStore, ProjectionEngine, RawObservation,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum KernelError {
    #[error("kernel error: {0}")]
    Init(String),

    #[error("observation error: {0}")]
    Observation(String),

    #[error("event store error: {0}")]
    Store(#[from] cotrex_ai_kernel::EventStoreError),
}

pub struct WorkspaceKernel {
    store: PersistentEventStore,
    engine: ProjectionEngine,
    pipeline: ObservationPipeline,
    ai_context: Arc<AiContextProjection>,
}

impl WorkspaceKernel {
    pub fn open(root: PathBuf) -> Result<Self, KernelError> {
        let data_dir = root.join(".cotrex");
        let store = PersistentEventStore::open(data_dir)
            .map_err(|e| KernelError::Init(format!("failed to open event store: {}", e)))?;

        let engine = ProjectionEngine::new();

        let ai_context = Arc::new(AiContextProjection::new());
        engine
            .register(Box::new(AiContextProjectionWrapper(ai_context.clone())))
            .map_err(|e| KernelError::Init(format!("failed to register ai_context: {}", e)))?;
        engine
            .register(Box::new(FileChangeProjection::new()))
            .map_err(|e| KernelError::Init(format!("failed to register file_change: {}", e)))?;
        engine
            .rebuild_all(&store)
            .map_err(|e| KernelError::Init(format!("failed to rebuild projections: {}", e)))?;

        // Start processing on all projections so they receive new events.
        // initialize() sets status to Initialized; process_event() requires Processing.
        for name in engine
            .list()
            .map_err(|e| KernelError::Init(format!("failed to list projections: {}", e)))?
        {
            engine.start_processing(&name).map_err(|e| {
                KernelError::Init(format!("failed to start processing {}: {}", name, e))
            })?;
        }

        let pipeline = ObservationPipeline::new(root.clone());
        pipeline
            .initialize()
            .map_err(|e| KernelError::Init(format!("failed to initialize pipeline: {}", e)))?;
        pipeline
            .start_watching()
            .map_err(|e| KernelError::Init(format!("failed to start pipeline: {}", e)))?;

        Ok(Self {
            store,
            engine,
            pipeline,
            ai_context,
        })
    }

    pub fn observe(&self, observation: RawObservation) -> Result<(), KernelError> {
        let count = self
            .pipeline
            .process_observation(&observation, &self.store)
            .map_err(|e| KernelError::Observation(format!("pipeline error: {}", e)))?;

        if count > 0 {
            let start = self.store.next_sequence() - count;
            let replay = self
                .store
                .replay(start)
                .map_err(|e| KernelError::Observation(format!("replay error: {}", e)))?;
            for event in &replay.events {
                self.engine
                    .process_event(event)
                    .map_err(|e| KernelError::Observation(format!("projection error: {}", e)))?;
            }
        }

        Ok(())
    }

    pub fn summary(&self) -> AiContextSummary {
        self.ai_context.summary()
    }
}

struct AiContextProjectionWrapper(Arc<AiContextProjection>);

impl cotrex_ai_kernel::Projection for AiContextProjectionWrapper {
    fn name(&self) -> &str {
        self.0.name()
    }

    fn status(&self) -> cotrex_ai_kernel::ProjectionStatus {
        self.0.status()
    }

    fn checkpoint(&self) -> u64 {
        self.0.checkpoint()
    }

    fn process_event(
        &self,
        event: &cotrex_ai_kernel::Event,
    ) -> Result<(), cotrex_ai_kernel::EventStoreError> {
        self.0.process_event(event)
    }

    fn rebuild(
        &self,
        store: &dyn cotrex_ai_kernel::EventStore,
    ) -> Result<(), cotrex_ai_kernel::EventStoreError> {
        self.0.rebuild(store)
    }

    fn initialize(
        &self,
        store: &dyn cotrex_ai_kernel::EventStore,
    ) -> Result<(), cotrex_ai_kernel::EventStoreError> {
        self.0.initialize(store)
    }

    fn start_processing(&self) -> Result<(), cotrex_ai_kernel::EventStoreError> {
        self.0.start_processing()
    }

    fn mark_failed(&self) -> Result<(), cotrex_ai_kernel::EventStoreError> {
        self.0.mark_failed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cotrex_ai_kernel::{RawOperation, WorkspaceStatus as KernelStatus};
    use cotrex_ai_runtime::ContextSource;
    use std::sync::Arc;

    #[test]
    fn workspace_kernel_opens_and_sumsmarizes() {
        let tmp = tempfile::tempdir().unwrap();
        let kernel = WorkspaceKernel::open(tmp.path().to_path_buf()).unwrap();
        let summary = kernel.summary();
        // Empty workspace
        assert_eq!(summary.workspace_status, KernelStatus::Empty);
        assert_eq!(summary.file_count, 0);
    }

    #[test]
    fn workspace_kernel_processes_observation() {
        let tmp = tempfile::tempdir().unwrap();
        let kernel = WorkspaceKernel::open(tmp.path().to_path_buf()).unwrap();

        let obs = RawObservation {
            path: tmp.path().join("src/main.rs"),
            operation: RawOperation::Created,
        };
        kernel.observe(obs).unwrap();

        let summary = kernel.summary();
        assert_eq!(summary.workspace_status, KernelStatus::Active);
        assert_eq!(summary.total_changes, 1);
    }

    #[test]
    fn kernel_context_source_provides_context() {
        let tmp = tempfile::tempdir().unwrap();
        let kernel = Arc::new(WorkspaceKernel::open(tmp.path().to_path_buf()).unwrap());
        let source = crate::kernel::context_source::KernelContextSource::new(kernel);

        let ctx = source.context().unwrap();
        assert_eq!(
            ctx.workspace_status,
            cotrex_ai_runtime::WorkspaceStatus::Unknown
        );
        assert_eq!(ctx.file_count, 0);
    }

    #[test]
    fn kernel_context_source_after_observation() {
        let tmp = tempfile::tempdir().unwrap();
        let kernel = Arc::new(WorkspaceKernel::open(tmp.path().to_path_buf()).unwrap());

        let obs = RawObservation {
            path: tmp.path().join("src/lib.rs"),
            operation: RawOperation::Modified,
        };
        kernel.observe(obs).unwrap();

        let source = crate::kernel::context_source::KernelContextSource::new(kernel);
        let ctx = source.context().unwrap();
        assert_eq!(
            ctx.workspace_status,
            cotrex_ai_runtime::WorkspaceStatus::Modified
        );
        assert!(ctx.file_count > 0);
    }
}
