//! Local model inference adapter.
//!
//! Wraps the llama.cpp provider to provide a simple `infer(prompt) -> response` interface
//! for the prompt system. No remote API needed.

use cotrex_ai_runtime::{LocalModel, ResolvedConfig};
use std::sync::{Mutex, OnceLock};

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

    /// Load the model if not already loaded. Suppresses C stderr during loading.
    fn ensure_loaded(&mut self) -> Result<(), String> {
        if self.model.is_some() {
            return Ok(());
        }

        let _guard = StderrSuppress::new();

        let registry = cotrex_ai_runtime::model_manager::load_registry()
            .map_err(|e| format!("failed to load registry: {e}"))?;
        let resolver = cotrex_ai_runtime::model_manager::ModelResolver::new(registry);

        let model_id = "qwen2.5-1.5b";
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

/// RAII guard that suppresses C-level stderr (fd 2). Used to hide llama.cpp loading logs.
struct StderrSuppress {
    saved_fd: i32,
}

impl StderrSuppress {
    fn new() -> Self {
        unsafe {
            let saved_fd = _dup(2);
            let nul_fd = _open(b"NUL\0".as_ptr() as *const i8, 0x0002);
            if nul_fd >= 0 {
                _dup2(nul_fd, 2);
                _close(nul_fd);
            }
            Self { saved_fd }
        }
    }
}

impl Drop for StderrSuppress {
    fn drop(&mut self) {
        unsafe {
            if self.saved_fd >= 0 {
                _dup2(self.saved_fd, 2);
                _close(self.saved_fd);
            }
        }
    }
}

extern "C" {
    fn _dup(fd: i32) -> i32;
    fn _dup2(src_fd: i32, dst_fd: i32) -> i32;
    fn _open(filename: *const i8, flags: i32, ...) -> i32;
    fn _close(fd: i32) -> i32;
}

/// Global local inference instance (lazily initialized).
static LOCAL_INFERENCE: OnceLock<Mutex<LocalInference>> = OnceLock::new();

pub fn local_infer(prompt: &str) -> Result<String, String> {
    let inf = LOCAL_INFERENCE.get_or_init(|| Mutex::new(LocalInference::new()));
    let mut guard = inf.lock().map_err(|e| format!("lock: {e}"))?;
    guard.infer(prompt)
}

pub fn infer_local(system: &str, user: &str) -> Result<String, String> {
    let prompt = format!("{system}\n\n{user}");
    local_infer(&prompt)
}
