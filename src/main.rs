//! Cotrex.
//! A deterministic RTK orchestration layer: normalize agent intent, forward to RTK, normalize
//! the stream. Cotrex does not own execution; RTK does.

mod agent;
mod ai_runtime;
mod commands;
mod config;
mod core;
mod dispatch;
mod graphify;
mod llm;
mod script;
mod usage;

fn main() {
    // Suppress llama.cpp verbose logging before any backend init
    unsafe {
        extern "C" fn noop_log(
            _level: std::os::raw::c_int,
            _text: *const std::os::raw::c_char,
            _user_data: *mut std::os::raw::c_void,
        ) {
        }
        llama_cpp_sys_2::ggml_log_set(Some(noop_log), std::ptr::null_mut());
    }

    // All routing lives in dispatch — main.rs is just the module tree.
    dispatch::run();
}
