use std::sync::Arc;

use cotrex_ai_contract::ProviderInfo;
use cotrex_ai_runtime::model_manager::{load_registry, ModelResolver};
use cotrex_ai_runtime::{
    CapabilityProvider, LocalModel, ProviderFactory, ResolvedConfig, RuntimeError,
};

pub struct LocalModelFactory;

impl ProviderFactory for LocalModelFactory {
    fn create(&self) -> Result<Arc<dyn CapabilityProvider + Send + Sync>, RuntimeError> {
        let model_id = "qwen2.5-0.5b";

        let registry = load_registry().map_err(|e| RuntimeError::Model(e.to_string()))?;
        let resolver = ModelResolver::new(registry);

        let model_path = resolver.resolve(model_id).map_err(|_| {
            RuntimeError::Model(format!(
                "model '{model_id}' not installed. Run: cotrex model install {model_id}"
            ))
        })?;

        let config = ResolvedConfig {
            model_path,
            model_name: model_id.into(),
            ..ResolvedConfig::default()
        };

        let mut model = llama_cpp_provider::LlamaCppModel::new();
        model
            .load(&config)
            .map_err(|e| RuntimeError::Model(e.to_string()))?;

        let info = ProviderInfo {
            name: "llama.cpp".into(),
            version: "0.1.0".into(),
            supported_capabilities: vec![
                cotrex_ai_contract::CapabilityKind::BuildSummary,
                cotrex_ai_contract::CapabilityKind::ExplainRust,
            ],
        };
        let mut provider = cotrex_ai_runtime::LocalProvider::new(model, config, info);
        provider
            .load()
            .map_err(|e| RuntimeError::Model(e.to_string()))?;

        Ok(Arc::new(provider))
    }
}
