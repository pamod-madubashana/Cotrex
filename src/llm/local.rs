//! Local model inference adapter.
//!
//! Wraps the llama.cpp provider to provide a simple `infer(prompt) -> response` interface
//! for the prompt system. No remote API needed.

use cotrex_ai_runtime::{ChatMessage, LocalModel, ResolvedConfig};
use serde_json::json;
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

    fn ensure_loaded(&mut self) -> Result<(), String> {
        if self.model.is_some() {
            return Ok(());
        }

        let _guard = StderrSuppress::new();

        let registry = cotrex_ai_runtime::model_manager::load_registry()
            .map_err(|e| format!("failed to load registry: {e}"))?;
        let resolver = cotrex_ai_runtime::model_manager::ModelResolver::new(registry);

        let model_id = "qwen2.5-1.5b";
        let model_path = resolver.resolve(model_id).map_err(|e| format!("{e}"))?;

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

    /// Infer with structured chat messages (system + user).
    /// The provider applies the GGUF's chat template (e.g. ChatML) before tokenization.
    pub fn infer_messages(&mut self, messages: Vec<ChatMessage>) -> Result<String, String> {
        if let Ok(response) = infer_ollama_messages(&messages) {
            return Ok(response);
        }

        self.ensure_loaded()?;

        let model = self.model.as_ref().unwrap();
        let request = cotrex_ai_runtime::InferenceRequest {
            prompt: cotrex_ai_runtime::Prompt::new(""),
            messages,
            temperature: 0.7,
            max_tokens: 512,
        };

        let _guard = StderrSuppress::new();
        let response = model
            .infer(request)
            .map_err(|e| format!("inference failed: {e}"))?;
        drop(_guard);

        Ok(response.text)
    }

    /// Infer with raw prompt text (fallback, no chat template applied).
    #[allow(dead_code)]
    pub fn infer(&mut self, prompt: &str) -> Result<String, String> {
        if let Ok(response) = infer_ollama_prompt(prompt) {
            return Ok(response);
        }

        self.ensure_loaded()?;

        let model = self.model.as_ref().unwrap();
        let request = cotrex_ai_runtime::InferenceRequest {
            prompt: cotrex_ai_runtime::Prompt::new(prompt),
            messages: vec![],
            temperature: 0.7,
            max_tokens: 512,
        };

        let _guard = StderrSuppress::new();
        let response = model
            .infer(request)
            .map_err(|e| format!("inference failed: {e}"))?;
        drop(_guard);

        Ok(response.text)
    }
}

fn ollama_url(path: &str) -> String {
    let base = std::env::var("COTREX_OLLAMA_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:11434".into())
        .trim_end_matches('/')
        .to_string();
    format!("{base}{path}")
}

fn ollama_model() -> String {
    std::env::var("COTREX_OLLAMA_MODEL").unwrap_or_else(|_| "cotrex-eval".into())
}

fn infer_ollama_messages(messages: &[ChatMessage]) -> Result<String, String> {
    let messages: Vec<_> = messages
        .iter()
        .map(|m| json!({ "role": m.role.as_str(), "content": m.content.as_str() }))
        .collect();

    let body = json!({
        "model": ollama_model(),
        "messages": messages,
        "stream": false,
        "options": {
            "temperature": 0.0
        }
    });

    let response: serde_json::Value = ureq::post(&ollama_url("/api/chat"))
        .send_json(body)
        .map_err(|e| format!("ollama chat failed: {e}"))?
        .into_json()
        .map_err(|e| format!("ollama chat response: {e}"))?;

    response
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "ollama chat response did not contain message.content".into())
}

fn infer_ollama_prompt(prompt: &str) -> Result<String, String> {
    let body = json!({
        "model": ollama_model(),
        "prompt": prompt,
        "stream": false,
        "options": {
            "temperature": 0.0
        }
    });

    let response: serde_json::Value = ureq::post(&ollama_url("/api/generate"))
        .send_json(body)
        .map_err(|e| format!("ollama generate failed: {e}"))?
        .into_json()
        .map_err(|e| format!("ollama generate response: {e}"))?;

    response
        .get("response")
        .and_then(|c| c.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "ollama generate response did not contain response text".into())
}

/// RAII guard that suppresses C-level stderr (fd 2).
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

#[allow(dead_code)]
pub fn local_infer(prompt: &str) -> Result<String, String> {
    let inf = LOCAL_INFERENCE.get_or_init(|| Mutex::new(LocalInference::new()));
    let mut guard = inf.lock().map_err(|e| format!("lock: {e}"))?;
    guard.infer(prompt)
}

/// Infer with structured system + user messages.
/// The provider applies the GGUF's chat template before tokenization.
pub fn infer_local(system: &str, user: &str) -> Result<String, String> {
    let inf = LOCAL_INFERENCE.get_or_init(|| Mutex::new(LocalInference::new()));
    let mut guard = inf.lock().map_err(|e| format!("lock: {e}"))?;
    let messages = vec![ChatMessage::system(system), ChatMessage::user(user)];
    guard.infer_messages(messages)
}
