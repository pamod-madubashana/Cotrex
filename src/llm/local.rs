//! Local model inference adapter.
//!
//! Wraps the llama.cpp provider to provide a simple `generate(system, prompt)` interface
//! for the prompt system. No remote API needed.

use cotrex_ai_runtime::{ChatMessage, InferProfile, LocalModel, ResolvedConfig};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, OnceLock};

/// Local model backend used by Cotrex prompt execution.
#[derive(Clone)]
pub struct LocalBackend;

impl Default for LocalBackend {
    fn default() -> Self {
        Self
    }
}

impl LocalBackend {
    pub fn generate(&self, system: &str, prompt: &str) -> Result<String, String> {
        self.generate_profiled(system, prompt).map(|(text, _)| text)
    }

    /// Like `generate` but also returns the inference profile.
    pub fn generate_profiled(
        &self,
        system: &str,
        prompt: &str,
    ) -> Result<(String, Option<InferProfile>), String> {
        let inf = LOCAL_INFERENCE.get_or_init(|| Mutex::new(LocalInference::new()));
        let mut guard = inf.lock().map_err(|e| format!("lock: {e}"))?;
        guard.infer_with_system_profiled(system, prompt)
    }

    /// Streaming inference: runs on a worker thread, emits tokens via the
    /// provided channel sender as they are generated, then sends a final
    /// `None` sentinel to signal completion.
    pub fn generate_stream(
        &self,
        system: String,
        prompt: String,
        tx: Sender<String>,
    ) -> thread::JoinHandle<Result<(String, Option<InferProfile>), String>> {
        thread::spawn(move || {
            let inf = LOCAL_INFERENCE.get_or_init(|| Mutex::new(LocalInference::new()));
            let mut guard = inf.lock().map_err(|e| format!("lock: {e}"))?;
            let callback: Arc<Mutex<dyn FnMut(&str) + Send + 'static>> =
                Arc::new(Mutex::new(move |token: &str| {
                    let _ = tx.send(token.to_string());
                }));
            guard.infer_with_system_stream(&system, &prompt, Some(callback))
        })
    }
}

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

        let model_id = crate::config::settings::active_model();
        let model_path = resolver.resolve(&model_id).map_err(|e| format!("{e}"))?;

        let config = ResolvedConfig {
            model_path,
            model_name: model_id.into(),
            ..ResolvedConfig::default()
        };

        let mut model = llama_cpp_provider::LlamaCppModel::new();
        model
            .load(&config)
            .map_err(|e| format!("failed to load model: {e}"))?;

        drop(_guard);

        self.model = Some(Box::new(model));
        self.config = Some(config);
        Ok(())
    }

    /// Infer with structured chat messages (system + user).
    /// The provider applies the GGUF's chat template (e.g. ChatML) before tokenization.
    #[allow(dead_code)]
    pub fn infer_messages(&mut self, messages: Vec<ChatMessage>) -> Result<String, String> {
        self.infer_messages_profiled(messages).map(|(text, _)| text)
    }

    /// Like `infer_messages` but also returns the inference profile when
    /// `COTREX_PROFILE=1` is set.  Callers that need per-phase timings use
    /// this; everyone else uses `infer_messages`.
    pub fn infer_messages_profiled(
        &mut self,
        messages: Vec<ChatMessage>,
    ) -> Result<(String, Option<InferProfile>), String> {
        self.ensure_loaded()?;

        let model = self.model.as_ref().unwrap();
        let request = cotrex_ai_runtime::InferenceRequest {
            prompt: cotrex_ai_runtime::Prompt::new(""),
            messages,
            temperature: 0.7,
            max_tokens: 512,
            token_callback: None,
        };

        let _guard = StderrSuppress::new();
        let response = model
            .infer(request)
            .map_err(|e| format!("inference failed: {e}"))?;
        drop(_guard);

        Ok((response.text, response.profile))
    }

    /// Infer with a system prompt plus a user prompt.
    #[allow(dead_code)]
    pub fn infer_with_system(&mut self, system: &str, prompt: &str) -> Result<String, String> {
        let messages = vec![ChatMessage::system(system), ChatMessage::user(prompt)];
        self.infer_messages(messages)
    }

    /// Like `infer_with_system` but also returns the inference profile.
    pub fn infer_with_system_profiled(
        &mut self,
        system: &str,
        prompt: &str,
    ) -> Result<(String, Option<InferProfile>), String> {
        let messages = vec![ChatMessage::system(system), ChatMessage::user(prompt)];
        self.infer_messages_profiled(messages)
    }

    /// Streaming variant: attaches a token callback to the inference request
    /// so the provider emits each generated text piece as it is produced.
    pub fn infer_with_system_stream(
        &mut self,
        system: &str,
        prompt: &str,
        token_callback: Option<Arc<Mutex<dyn FnMut(&str) + Send + 'static>>>,
    ) -> Result<(String, Option<InferProfile>), String> {
        let messages = vec![ChatMessage::system(system), ChatMessage::user(prompt)];
        self.infer_messages_stream(messages, token_callback)
    }

    /// Streaming variant of `infer_messages`.
    /// Suppresses C-level stderr during inference so llama.cpp's context-
    /// construction messages (~50 lines) are silenced.  Tokens are delivered
    /// through the callback; the caller uses `console_write` to display them
    /// via a direct console handle that bypasses the fd 2 suppression.
    fn infer_messages_stream(
        &mut self,
        messages: Vec<ChatMessage>,
        token_callback: Option<Arc<Mutex<dyn FnMut(&str) + Send + 'static>>>,
    ) -> Result<(String, Option<InferProfile>), String> {
        self.ensure_loaded()?;

        let model = self.model.as_ref().unwrap();
        let request = cotrex_ai_runtime::InferenceRequest {
            prompt: cotrex_ai_runtime::Prompt::new(""),
            messages,
            temperature: 0.7,
            max_tokens: 512,
            token_callback,
        };

        let _guard = StderrSuppress::new();
        let response = model
            .infer(request)
            .map_err(|e| format!("inference failed: {e}"))?;
        drop(_guard);

        Ok((response.text, response.profile))
    }
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

use std::thread;

/// Global local inference instance (lazily initialized).
static LOCAL_INFERENCE: OnceLock<Mutex<LocalInference>> = OnceLock::new();

#[allow(dead_code)]
pub fn local_infer(prompt: &str) -> Result<String, String> {
    let backend = LocalBackend::default();
    backend.generate("", prompt)
}

/// Infer with structured system + user messages.
/// The provider applies the GGUF's chat template before tokenization.
pub fn infer_local(system: &str, user: &str) -> Result<String, String> {
    let backend = LocalBackend::default();
    backend.generate(system, user)
}

/// Like `infer_local` but also returns the inference profile.
pub fn infer_local_profiled(
    system: &str,
    user: &str,
) -> Result<(String, Option<InferProfile>), String> {
    let backend = LocalBackend::default();
    backend.generate_profiled(system, user)
}

/// Streaming inference: spawns a worker thread, returns `(JoinHandle, Receiver)`.
/// The receiver yields tokens as they are generated.
pub fn infer_local_stream(
    system: &str,
    user: &str,
) -> (
    thread::JoinHandle<Result<(String, Option<InferProfile>), String>>,
    std::sync::mpsc::Receiver<String>,
) {
    let (tx, rx) = std::sync::mpsc::channel();
    let backend = LocalBackend::default();
    let handle = backend.generate_stream(system.to_string(), user.to_string(), tx);
    (handle, rx)
}

// ---------------------------------------------------------------------------
// Console writer with stderr fallback
//
// In the streaming path the worker thread redirects fd 2 to NUL via
// `StderrSuppress` to silence llama.cpp's C-level noise.  Rust's
// `std::io::stderr()` also goes through fd 2, so writing to stderr would be
// swallowed.
//
// `console_write` solves this in two ways:
//
//  1. **Real console window** (Windows Terminal, cmd.exe):
//     Opens `CONOUT$` via `CreateFileA` and writes with `WriteConsoleA`,
//     which bypasses fd 2 entirely.  Virtual terminal processing is enabled
//     on first use so ANSI escape codes work.
//
//  2. **Pipe-based terminal** (IDE terminals, `cargo run 2>&1`):
//     `WriteConsoleA` fails because `CONOUT$` isn't a console handle.
//     Falls back to `WriteFile` on the same handle, which works for pipes.
//     StderrSuppress will swallow this output, but at least we don't panic.
//     In this environment the user sees the spinner briefly during context
//     setup (before StderrSuppress kicks in) and the final answer after
//     cursor restore.
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
mod console {
    use std::sync::OnceLock;

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateFileA(
            lpfilename: *const i8,
            dwdesiredaccess: u32,
            dwsharemode: u32,
            lpsecurityattributes: *const std::ffi::c_void,
            dwcreationdisposition: u32,
            dwflagsandattributes: u32,
            htemplatefile: isize,
        ) -> isize;
        #[allow(dead_code)]
        fn WriteConsoleA(
            hconsoleoutput: isize,
            lpbuffer: *const u8,
            nnumberofcharstowrite: u32,
            lpnumberofcharswritten: *mut u32,
            lpreserved: *const std::ffi::c_void,
        ) -> i32;
        fn WriteConsoleW(
            hconsoleoutput: isize,
            lpbuffer: *const u16,
            nnumberofcharstowrite: u32,
            lpnumberofcharswritten: *mut u32,
            lpreserved: *const std::ffi::c_void,
        ) -> i32;
        fn WriteFile(
            hfile: isize,
            lpbuffer: *const u8,
            nnumberofbytestowrite: u32,
            lpnumberofbyteswritten: *mut u32,
            lpoverlapped: *const std::ffi::c_void,
        ) -> i32;
        fn GetConsoleMode(hconsolehandle: isize, lpmode: *mut u32) -> i32;
        fn SetConsoleMode(hconsolehandle: isize, dwmode: u32) -> i32;
    }

    const GENERIC_WRITE: u32 = 0x40000000;
    const FILE_SHARE_WRITE: u32 = 0x00000002;
    const OPEN_EXISTING: u32 = 3;
    const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;

    static CONOUT: OnceLock<isize> = OnceLock::new();

    fn get_handle() -> Option<isize> {
        let handle = *CONOUT.get_or_init(|| unsafe {
            let h = CreateFileA(
                b"CONOUT$\0".as_ptr() as *const i8,
                GENERIC_WRITE,
                FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_EXISTING,
                0,
                0,
            );
            if h != -1isize {
                let mut mode: u32 = 0;
                if GetConsoleMode(h, &mut mode) != 0 {
                    SetConsoleMode(h, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
                }
            }
            h
        });
        if handle == -1isize {
            None
        } else {
            Some(handle)
        }
    }

    /// Write bytes to the console.  Converts UTF-8 to UTF-16 and uses
    /// `WriteConsoleW` so Unicode characters (braille spinner, etc.) render
    /// correctly.  Falls back to `WriteFile` when the handle is a pipe.
    pub fn write(data: &[u8]) {
        if let Some(handle) = get_handle() {
            // Convert UTF-8 bytes to UTF-16 for WriteConsoleW.
            if let Ok(s) = std::str::from_utf8(data) {
                let wide: Vec<u16> = s.encode_utf16().collect();
                let mut written: u32 = 0;
                let ok = unsafe {
                    WriteConsoleW(
                        handle,
                        wide.as_ptr(),
                        wide.len() as u32,
                        &mut written,
                        std::ptr::null(),
                    )
                };
                if ok != 0 {
                    return;
                }
            }
            // Fallback: not a real console (pipe/redirect) — write raw bytes.
            let mut written: u32 = 0;
            unsafe {
                WriteFile(
                    handle,
                    data.as_ptr(),
                    data.len() as u32,
                    &mut written,
                    std::ptr::null(),
                );
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod console {
    /// On non-Windows, write directly to stderr (no fd 2 suppression conflict).
    pub fn write(data: &[u8]) {
        use std::io::Write;
        let mut err = std::io::stderr();
        let _ = err.write_all(data);
    }
}

pub use console::write as console_write;
