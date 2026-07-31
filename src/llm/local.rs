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

    /// Load the model if not already loaded (stderr suppressed).
    fn ensure_loaded(&mut self) -> Result<(), String> {
        if self.model.is_some() {
            return Ok(());
        }

        let _guard = StderrSuppress::new();

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

/// RAII guard that suppresses C-level stderr (fd 2) during model loading.
struct StderrSuppress {
    saved_fd: i32,
}

impl StderrSuppress {
    /// Redirect C stderr (fd 2) to NUL. Restores on drop.
    fn new() -> Self {
        unsafe {
            // Duplicate fd 2 so we can restore it later
            let saved_fd = libc_dup2(2, 100); // save original stderr to fd 100

            // Open NUL device and dup2 it to fd 2
            let nul_fd = libc_open(
                b"NUL\0".as_ptr() as *const i8,
                0x0002, // O_WRONLY
            );
            if nul_fd >= 0 {
                libc_dup2(nul_fd, 2);
                libc_close(nul_fd);
            }

            Self { saved_fd }
        }
    }
}

impl Drop for StderrSuppress {
    fn drop(&mut self) {
        unsafe {
            // Restore original stderr from saved fd
            if self.saved_fd >= 0 {
                libc_dup2(self.saved_fd, 2);
                libc_close(self.saved_fd);
            }
        }
    }
}

// C runtime FFI for fd-level I/O
extern "C" {
    fn _dup(fd: i32) -> i32;
    fn _dup2(src_fd: i32, dst_fd: i32) -> i32;
    fn _open(filename: *const i8, flags: i32, ...) -> i32;
    fn _close(fd: i32) -> i32;
}

// Inline wrappers matching the underscore-prefixed CRT names
unsafe fn libc_dup2(src: i32, dst: i32) -> i32 {
    _dup2(src, dst)
}

unsafe fn libc_open(path: *const i8, flags: i32) -> i32 {
    _open(path, flags)
}

unsafe fn libc_close(fd: i32) {
    _close(fd);
}

/// Global local inference instance (lazily initialized).
static LOCAL_INFERENCE: OnceLock<Mutex<LocalInference>> = OnceLock::new();

/// Get or initialize the local inference instance.
pub fn local_infer(prompt: &str) -> Result<String, String> {
    let inf = LOCAL_INFERENCE.get_or_init(|| Mutex::new(LocalInference::new()));
    let mut guard = inf.lock().map_err(|e| format!("lock: {e}"))?;
    guard.infer(prompt)
}

/// Simple local inference without retry logic (replaces remote API calls).
pub fn infer_local(system: &str, user: &str) -> Result<String, String> {
    let prompt = format!("{system}\n\n{user}");
    local_infer(&prompt)
}
