//! Local model inference adapter.
//!
//! Wraps the llama.cpp provider to provide a simple `infer(prompt) -> response` interface
//! for the prompt system. No remote API needed.

use crate::dispatch::factory::LocalModelFactory;
use cotrex_ai_runtime::{LocalModel, ProviderFactory, ResolvedConfig};

/// Local model inference using llama.cpp.
pub struct LocalInference {
    model: Option<Box<dyn LocalModel>>,
    config: Option<ResolvedConfig>,
}

impl LocalInference {
    pub fn new() -> Self {
        Self {
            model: None,
            config: None,
        }
    }

    /// Load the model if not already loaded.
    fn ensure_loaded(&mut self) -> Result<(), String> {
        if self.model.is_some() {
            return Ok(());
        }

        let factory = LocalModelFactory;
        let registry = cotrex_ai_runtime::model_manager::load_registry()
            .map_err(|e| format!("failed to load registry: {e}"))?;
        let resolver = cotrex_ai_runtime::model_manager::ModelResolver::new(registry);

        let model_id = "qwen2.5-0.5b";
        let model_path = resolver
            .resolve(model_id)
            .map_err(|e| format!("{e}"))?;

        let config = ResolvedConfig {
            model_path,
            model_name: model_id.into(),
            ..ResolvedConfig::default()
        };

        let mut model = llama_cpp_provider::LlamaCppModel::new();
        model
            .load(&config)
            .map_err(|e| format!("failed to load model: {e}"))?;

        self.model = Some(Box::new(model));
        self.config = Some(config);
        Ok(())
    }

    /// Run inference on a prompt and return the response text.
    pub fn infer(&mut self, prompt: &str) -> Result<String, String> {
        self.ensure_loaded()?;

        let model = self.model.as_ref().unwrap();
        let request = cotrex_ai_runtime::InferenceRequest {
            prompt: cotrex_ai_runtime::Prompt::new(prompt),
            temperature: 0.7,
            max_tokens: 512,
        };

        let response = model
            .infer(request)
            .map_err(|e| format!("inference failed: {e}"))?;

        Ok(response.text)
    }
}

/// Global local inference instance (lazily initialized).
use std::sync::OnceLock;
static LOCAL_INFERENCE: OnceLock<std::sync::Mutex<LocalInference>> = OnceLock::new();

/// Get or initialize the local inference instance.
pub fn local_infer(prompt: &str) -> Result<String, String> {
    let inf = LOCAL_INFERENCE.get_or_init(|| {
        std::sync::Mutex::new(LocalInference::new())
    });
    let mut guard = inf.lock().map_err(|e| format!("lock: {e}"))?;
    guard.infer(prompt)
}

/// Simple local inference without retry logic (replaces remote API calls).
pub fn infer_local(system: &str, user: &str) -> Result<String, String> {
    let prompt = format!("{system}\n\n{user}");
    local_infer(&prompt)
}
