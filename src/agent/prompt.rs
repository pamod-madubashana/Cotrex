//! Prompts.
//!
//! A single quoted argument is a *prompt*, not a command. Free text is a **task**: the model turns
//! it into one shell command and cotrex *runs* it, returning the command's output (not the command).
//! `category: text` (or a JSON object of several) instead returns a structured answer using that
//! category's header — those aren't runnable commands.
//!
//! Two presentation modes:
//! - **User** (`cotrex "…"`): a spinner while the model thinks; between commands it narrates each
//!   step ("Let me check…") in its own words, streams the running command's output, then the answer.
//! - **Model** (`cotrex -m "…"`): no spinner, no narration — just the output on stdout.
//!
//! Every call goes through the local llama.cpp model.

use std::collections::VecDeque;
use std::io::{IsTerminal, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::agent::tool::{self, OutputLimiter, ToolContext, BUILTINS};
use crate::core::intent::Intent;
use crate::core::orchestrate::{self, Options};

/// Who's reading the output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    /// A human: spinner + live-streamed generation.
    User,
    /// Another model/agent: output only, no thinking or chatter.
    Model,
}

// Each row binds a category to its header (system prompt). Add a row to add a category.
const CATEGORIES: &[(&str, &str)] = &[
    (
        "plan-stack",
        "You recommend the single best application tech stack for a developer's task. Inspect the \
working directory first if it helps you decide. Answer with the stack name and one concise sentence \
of reasoning — nothing more.",
    ),
    (
        "theme",
        "You are a senior UI designer. Given a short style description, answer with a color palette \
(hex colors), a font, a few key effects, and one concise sentence of rationale.",
    ),
];

// Single unified system prompt used by every code path (agentic loop, text generation, category
// fallback). `build_system_prompt` appends the target-shell note after it.
const COTREX_SYSTEM_PROMPT: &str = r#"You are Cotrex, a local developer execution engine.

You operate inside the user's current working directory.

Your job:
- understand the request
- inspect the local environment when information is required
- execute requested developer actions
- return useful, concise results
- avoid guessing about files, code, configuration, or project state

Do not create subagents or plans. Do not pretend to be multiple agents.

Every response MUST be exactly one valid JSON object.
Never output markdown or explanations outside JSON.

Use {"answer":"text"} for direct answers.
Use {"run":"command","say":"short reason"} for shell commands.
Use {"tool":"name","args":{},"say":"short reason"} ONLY for these built-in file tools: read, write, edit, glob, grep.
Never invent tool names — if the action is not a file operation, use {"run":"command"} instead.
Prefer tools for file operations and shell commands for everything else."#;

/// The header (system prompt) bound to a category, if it is known.
pub fn header(category: &str) -> Option<&'static str> {
    CATEGORIES
        .iter()
        .find(|(n, _)| *n == category)
        .map(|(_, h)| *h)
}

/// Resolve a category to its agentic persona header. An empty category (a JSON object with an empty
/// key) falls back to the default; an unknown one is an error.
pub fn category_header(category: &str) -> Result<&'static str, String> {
    if category.is_empty() {
        Ok(COTREX_SYSTEM_PROMPT)
    } else {
        header(category).ok_or_else(|| format!("unknown category '{category}'"))
    }
}

/// Get the decision system prompt (used by qualification tests).
pub fn decision_system() -> &'static str {
    COTREX_SYSTEM_PROMPT
}

/// Get the decision system prompt with model-specific shell instructions.
pub fn decision_system_with_model(_model_id: &str) -> String {
    let shell = if cfg!(windows) {
        "Any command runs in Windows PowerShell — use PowerShell cmdlets and syntax."
    } else {
        "Any command runs in a POSIX bash shell — use POSIX tools."
    };
    build_system_prompt(shell)
}

fn build_system_prompt(shell: &str) -> String {
    format!("{COTREX_SYSTEM_PROMPT}\n\n{shell}")
}

/// Keep the old role hook as a stub so callers can still compile while the single-assistant flow is used.
#[allow(dead_code)]
pub fn role(_name: &str) -> Option<(&'static str, &'static str, &'static str, usize)> {
    None
}

fn prepare_task(task: &str) -> String {
    let lower = task.to_lowercase();
    let is_project_query = lower.contains("what is this project")
        || lower.contains("what does this project")
        || lower.contains("describe this project")
        || lower.contains("what is this code")
        || lower.contains("what does this code")
        || lower.contains("explain this repository")
        || lower.contains("describe this repository");

    if !is_project_query {
        return task.to_string();
    }

    let mut info = String::new();

    if let Ok(readme) = std::fs::read_to_string("README.md") {
        let preview: String = readme.lines().take(40).collect::<Vec<_>>().join("\n");
        info.push_str(&format!("README.md:\n{preview}\n\n"));
    }

    if let Ok(toml) = std::fs::read_to_string("Cargo.toml") {
        let preview: String = toml.lines().take(30).collect::<Vec<_>>().join("\n");
        info.push_str(&format!("Cargo.toml:\n{preview}\n\n"));
    } else if let Ok(pkg) = std::fs::read_to_string("package.json") {
        let preview: String = pkg.lines().take(30).collect::<Vec<_>>().join("\n");
        info.push_str(&format!("package.json:\n{preview}\n\n"));
    }

    if let Ok(entries) = std::fs::read_dir("src") {
        let mut names: Vec<String> = entries
            .filter_map(Result::ok)
            .filter_map(|e| {
                e.file_type()
                    .ok()
                    .filter(|t| t.is_dir())
                    .map(|_| e.file_name().to_string_lossy().into_owned())
            })
            .collect();
        names.sort();
        if !names.is_empty() {
            info.push_str(&format!("src/: {}\n\n", names.join(", ")));
        }
    }

    if let Ok(gitignore) = std::fs::read_to_string(".gitignore") {
        let preview: String = gitignore.lines().take(20).collect::<Vec<_>>().join("\n");
        info.push_str(&format!(".gitignore:\n{preview}\n\n"));
    }

    let tree = std::process::Command::new("git")
        .args(["ls-files", "--short"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    if !tree.is_empty() {
        let lines: Vec<&str> = tree.lines().take(50).collect();
        info.push_str(&format!("Project files:\n{}\n", lines.join("\n")));
    }

    if info.is_empty() {
        task.to_string()
    } else {
        format!(
            "{task}\n\n--- Project Context ---\n{info}\n--- End Context ---\n\nSummarize what this project is and how it is organized."
        )
    }
}

/// How a single bare argument should be handled.
#[derive(Debug, PartialEq)]
pub enum Dispatch {
    /// JSON object of `category -> text` (possibly several). Pass the raw string to `parse_json`.
    Json(String),
    /// `category: text` with a known category — returns a structured answer.
    Category(String, String),
    /// Free-text task for the assistant agent — it decides whether to run a command or just answer.
    Prompt(String),
    /// Project structure request — short-circuits the model and renders a tree directly.
    Structure,
}

/// Check if a prompt is a project-structure request (e.g. "show project structure", "list tree").
/// Short-circuits the model entirely — renders a tree from `git ls-files`.
fn is_structure_request(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    let has_structure_word =
        lower.contains("structure") || lower.contains("tree") || lower.contains("layout");
    let has_directory_noun = lower.contains("project")
        || lower.contains("directory")
        || lower.contains("repo")
        || lower.contains("codebase")
        || lower.contains("files");
    if has_structure_word && has_directory_noun {
        return true;
    }
    // Also match display verbs with a structure word (e.g. "show the tree", "list structure")
    if has_structure_word {
        let has_display_verb = lower.starts_with("show")
            || lower.starts_with("list")
            || lower.starts_with("display")
            || lower.starts_with("print")
            || lower.starts_with("view");
        if has_display_verb {
            return true;
        }
    }
    false
}

/// Classify one argument. A single quoted arg reaches here; multi-arg invocations are commands and
/// never get classified. Anything that isn't a JSON object or a `known-category: text` is a prompt —
/// even a lone word like `hi`, so User mode behaves like a normal AI agent. Run a raw command with
/// args (`cotrex git status`) or `cotrex run <cmd>`.
pub fn classify(arg: &str) -> Dispatch {
    let s = arg.trim();
    if s.starts_with('{') {
        return Dispatch::Json(s.to_string());
    }
    if let Some((cat, rest)) = s.split_once(':') {
        if header(cat.trim()).is_some() {
            return Dispatch::Category(cat.trim().to_string(), rest.trim().to_string());
        }
    }
    if is_structure_request(s) {
        return Dispatch::Structure;
    }
    Dispatch::Prompt(s.to_string())
}

/// Directories to skip (noise/build artifacts).
const SKIP_DIRS: &[&str] = &[
    "vendor",
    "target",
    "node_modules",
    ".git",
    "dist",
    "build",
    ".gitmodules",
];

/// Render a depth-limited project tree from `git ls-files`. Honors .gitignore, leaves submodules
/// unexpanded. Falls back to a shallow directory walk outside a git repo.
pub fn project_tree() -> String {
    // Try git ls-files first (honors .gitignore automatically).
    let git_output = std::process::Command::new("git")
        .args(["ls-files", "--cached", "--others", "--exclude-standard"])
        .output();
    if let Ok(output) = git_output {
        if output.status.success() {
            let files: Vec<String> = String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter(|l| !l.is_empty())
                .map(String::from)
                .collect();
            if !files.is_empty() {
                return build_tree_from_files(&files);
            }
        }
    }
    // Fallback: shallow walk of current directory.
    build_tree_fallback()
}

/// Build a tree string from a list of file paths (from git ls-files).
fn build_tree_from_files(files: &[String]) -> String {
    // Simple tree builder: just render the file list as a tree structure.
    let mut result = String::from(".\n");
    let mut prev_dir = String::new();

    for file in files {
        let parts: Vec<&str> = file.split('/').collect();
        let depth = parts.len().saturating_sub(1);
        let name = parts.last().unwrap_or(&"");

        if depth == 0 {
            // Root-level file
            let connector = "├── ";
            result.push_str(&format!("{connector}{name}\n"));
        } else {
            // Nested file: show directory prefix if it changed
            let dir = parts[..depth].join("/");
            if dir != prev_dir {
                for (d, dir_name) in parts.iter().enumerate().take(depth) {
                    let prefix: String = "│   ".repeat(d);
                    if d == depth - 1 {
                        result.push_str(&format!("{prefix}├── {dir_name}/\n"));
                    }
                }
                prev_dir = dir;
            }
            let prefix: String = "│   ".repeat(depth.saturating_sub(1));
            let connector = "├── ";
            result.push_str(&format!("{prefix}{connector}{name}\n"));
        }
    }
    result
}

/// Fallback tree builder when git is not available.
fn build_tree_fallback() -> String {
    let mut result = String::from(".\n");
    if let Ok(entries) = std::fs::read_dir(".") {
        let mut dirs: Vec<String> = Vec::new();
        let mut files: Vec<String> = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || SKIP_DIRS.contains(&name.as_str()) {
                continue;
            }
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                dirs.push(name);
            } else {
                files.push(name);
            }
        }
        dirs.sort();
        files.sort();
        for (i, dir) in dirs.iter().enumerate() {
            let is_last = i == dirs.len() - 1 && files.is_empty();
            let connector = if is_last { "└── " } else { "├── " };
            result.push_str(&format!("{connector}{dir}/\n"));
        }
        for (i, file) in files.iter().enumerate() {
            let is_last = i == files.len() - 1;
            let connector = if is_last { "└── " } else { "├── " };
            result.push_str(&format!("{connector}{file}\n"));
        }
    }
    result
}

/// Parse the JSON multi-category form into `(category, text)` pairs. Accepts a flat object
/// `{"plan-stack":"…","theme":"…"}` or a `{"task": { … }}` wrapper.
pub fn parse_json(s: &str) -> Result<Vec<(String, String)>, String> {
    let v: serde_json::Value =
        serde_json::from_str(s).map_err(|e| format!("invalid JSON prompt: {e}"))?;
    let obj = match v.get("task").and_then(|t| t.as_object()) {
        Some(o) => o.clone(),
        None => v
            .as_object()
            .ok_or("JSON prompt must be an object")?
            .clone(),
    };
    let pairs: Vec<(String, String)> = obj
        .iter()
        .filter_map(|(k, val)| val.as_str().map(|s| (k.clone(), s.to_string())))
        .collect();
    if pairs.is_empty() {
        return Err("JSON prompt has no category:text pairs".into());
    }
    Ok(pairs)
}

/// Whether `COTREX_PROFILE` is enabled.
fn agent_profiling() -> bool {
    std::env::var("COTREX_PROFILE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Step-by-step profile of an entire agent execution.
/// Each inference call, tool call, and shell command is recorded with its
/// wall-clock duration so you can see exactly where time goes.
#[derive(Debug, Default)]
struct AgentProfile {
    steps: Vec<StepProfile>,
    total: Duration,
}

/// One step in an agent execution.
#[derive(Debug)]
enum StepProfile {
    Inference {
        label: String,
        duration: Duration,
        profile: Option<cotrex_ai_runtime::InferProfile>,
    },
    Tool {
        name: &'static str,
        duration: Duration,
    },
    Shell {
        cmd: String,
        duration: Duration,
    },
}

impl AgentProfile {
    fn print(&self) {
        eprintln!("  ┌─ Agent Profile ─────────────────────────────┐");
        for step in &self.steps {
            match step {
                StepProfile::Inference { label, duration, profile } => {
                    eprintln!("  │ {:<20} {:>7.1} ms", label, duration.as_secs_f64() * 1000.0);
                    if let Some(p) = profile {
                        eprintln!("  │   ├─ chat_template  {:>5.1} ms",
                            p.chat_template.as_secs_f64() * 1000.0);
                        eprintln!("  │   ├─ tokenize       {:>5.1} ms",
                            p.tokenize.as_secs_f64() * 1000.0);
                        eprintln!("  │   ├─ new_context    {:>5.1} ms",
                            p.new_context.as_secs_f64() * 1000.0);
                        eprintln!("  │   ├─ prompt_decode  {:>5.1} ms ({} tok, {:.0} tok/s)",
                            p.prompt_decode.as_secs_f64() * 1000.0,
                            p.prompt_tokens,
                            p.prompt_tok_s());
                        eprintln!("  │   └─ generation    {:>5.1} ms ({} tok, {:.1} tok/s)",
                            p.generation.as_secs_f64() * 1000.0,
                            p.generated_tokens,
                            p.gen_tok_s());
                    }
                }
                StepProfile::Tool { name, duration } => {
                    eprintln!("  │ {:<20} {:>7.1} ms", format!("tool:{}", name), duration.as_secs_f64() * 1000.0);
                }
                StepProfile::Shell { cmd, duration } => {
                    let short = if cmd.len() > 25 { format!("{}…", &cmd[..24]) } else { cmd.clone() };
                    eprintln!("  │ {:<20} {:>7.1} ms", format!("sh:{}", short), duration.as_secs_f64() * 1000.0);
                }
            }
        }
        eprintln!("  │ {:<20} {:>7.1} ms", "TOTAL", self.total.as_secs_f64() * 1000.0);
        eprintln!("  └────────────────────────────────────────────┘");
    }
}

/// Fulfill a task: ask the model to decide between running a command or answering, then do it.
/// `max_steps` limits how many command iterations the agent can run
/// before forced to answer. Returns the exit code (0 for an answered task). Prints the real command
/// output, or the answer, to stdout.
pub fn fulfill(
    task: &str,
    mode: Mode,
    opts: &Options,
    max_steps: usize,
) -> Result<i32, String> {
    let task = prepare_task(task);
    let task = &task;
    // Generate for the shell we actually run on (see `exec_capture`): PowerShell on Windows, POSIX
    // bash elsewhere. Mismatching them is what makes a Windows run try to execute Linux commands.
    let shell = if cfg!(windows) {
        "Any command runs in Windows PowerShell — use PowerShell cmdlets and syntax (Get-ChildItem, \
Select-String, Measure-Object, Select-Object, Where-Object). Do NOT use bash/POSIX tools (no sed, \
awk, grep, or `find` with -printf)."
    } else {
        "Any command runs in a POSIX bash shell — use POSIX tools (find, grep, sed, awk, wc, ls, \
git); never PowerShell or cmd syntax."
    };
    let system = build_system_prompt(shell);

    // Step loop: the model runs commands to gather info (each output fed back, capped), then
    // finishes with an ANALYZED answer — never a raw command dump. A failure is fed back to fix.
    let mut transcript_events: Vec<TranscriptEvent> = Vec::new();
    let mut agent_profile = if agent_profiling() { Some(AgentProfile::default()) } else { None };
    let agent_start = Instant::now();
    let mut seen: Vec<String> = Vec::new();
    let mut failed: Vec<(String, String)> = Vec::new(); // (cmd, error) pairs
    let perms = crate::agent::permission::Permissions::default();
    let limiter = OutputLimiter { max_lines: 500 };
    for step in 0..max_steps {
        let transcript = render_transcript(&transcript_events);
        let user = if transcript.is_empty() {
            task.to_string()
        } else {
            let fix_hint = if let Some((_last_cmd, last_err)) = failed.last() {
                format!("\nThe last command failed: {last_err}\nTry a different approach to avoid the same error.")
            } else {
                String::new()
            };
            format!("Request: {task}\n\nCommands run so far:\n{transcript}{fix_hint}\nGather more if needed, else answer.")
        };
        let (decision_text, infer_profile) = one_call(&system, &user, mode, false)?;
        if let Some(ref mut ap) = agent_profile {
            ap.steps.push(StepProfile::Inference {
                label: format!("step-{} infer", step),
                duration: infer_profile.as_ref().map_or(Duration::ZERO, |p| p.total),
                profile: infer_profile,
            });
        }
        match parse_decision(&decision_text) {
            Decision::Answer(text) => {
                if let Some(ref mut ap) = agent_profile {
                    ap.total = agent_start.elapsed();
                    ap.print();
                }
                print_answer(&text, mode);
                return Ok(0);
            }
            Decision::Retry(error) => {
                // Unknown tool — feed error back so the model can correct itself.
                transcript_events.push(TranscriptEvent::ToolResult {
                    name: "tool",
                    output: error,
                    error: true,
                });
                continue;
            }
            Decision::Tool { tool, args, say } => {
                say_step(say.as_deref(), mode);
                let tool_key = format!("tool:{}", tool.name);
                // Check for duplicate tool+args (loop prevention)
                let call_sig = format!(
                    "{}:{}",
                    tool_key,
                    serde_json::to_string(&args).unwrap_or_default()
                );
                if seen.contains(&call_sig) {
                    break;
                }
                seen.push(call_sig);

                transcript_events.push(TranscriptEvent::ToolCall {
                    name: tool.name,
                    args: args.clone(),
                });

                // Validate args against JSON Schema before permission check
                if let Err(e) = tool::validate_args(tool, &args) {
                    transcript_events.push(TranscriptEvent::ToolResult {
                        name: tool.name,
                        output: e,
                        error: true,
                    });
                    continue;
                }

                // Permission check
                match perms.evaluate(tool.name, None) {
                    crate::agent::permission::Action::Deny => {
                        transcript_events.push(TranscriptEvent::ToolResult {
                            name: tool.name,
                            output: "Permission denied by policy.".to_string(),
                            error: true,
                        });
                    }
                    crate::agent::permission::Action::Ask => {
                        // In User mode, prompt; in Model mode, allow (MCP gates externally)
                        if matches!(mode, Mode::User) {
                            eprint!("  {}? (y/n) ", tool.description);
                            let _ = std::io::stderr().flush();
                            let mut input = String::new();
                            if std::io::stdin().read_line(&mut input).unwrap_or(0) == 0
                                || !is_yes(&input)
                            {
                                transcript_events.push(TranscriptEvent::ToolResult {
                                    name: tool.name,
                                    output: "Permission denied by user.".to_string(),
                                    error: true,
                                });
                                continue;
                            }
                        }
                        // Fall through to execute
                    }
                    crate::agent::permission::Action::Allow => {}
                }
                // Execute (Ask confirmed or Allow)
                let ctx = ToolContext {
                    workdir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
                };
                let t_tool = Instant::now();
                let result = (tool.execute)(&ctx, &args);
                let tool_elapsed = t_tool.elapsed();
                if let Some(ref mut ap) = agent_profile {
                    ap.steps.push(StepProfile::Tool { name: tool.name, duration: tool_elapsed });
                }
                match result {
                    Ok(output) => {
                        let truncated = limiter.truncate(&output);
                        transcript_events.push(TranscriptEvent::ToolResult {
                            name: tool.name,
                            output: truncated,
                            error: false,
                        });
                    }
                    Err(e) => {
                        transcript_events.push(TranscriptEvent::ToolResult {
                            name: tool.name,
                            output: e,
                            error: true,
                        });
                    }
                }
            }
            // A weak model loops, re-running a command it already ran. That yields no new info, so
            // break to the forced answer instead of spinning.
            Decision::Run { cmd, .. } if seen.contains(&cmd) => {
                // If the command failed before, give the model one more chance with a hint.
                if failed.iter().any(|(c, _)| *c == cmd) && step < max_steps - 1 {
                    transcript_events.push(TranscriptEvent::Shell {
                        cmd,
                        exit_code: -1,
                        output: "already failed — try a different command".to_string(),
                    });
                    continue;
                }
                break;
            }
            Decision::Run { say, cmd } => {
                say_step(say.as_deref(), mode); // the model's own "let me check…" narration
                seen.push(cmd.clone());
                if is_risky(&cmd) {
                    if !confirm(&cmd) {
                        let _ = writeln!(std::io::stderr(), "aborted (not confirmed).");
                        return Ok(130); // 128 + SIGINT
                    }
                } else {
                    let _ = writeln!(std::io::stderr(), "$ {cmd}"); // safe → show what runs
                }
                let t_shell = Instant::now();
                let (code, out) = exec_capture(&cmd, opts, mode)?;
                let shell_elapsed = t_shell.elapsed();
                if let Some(ref mut ap) = agent_profile {
                    ap.steps.push(StepProfile::Shell { cmd: cmd.clone(), duration: shell_elapsed });
                }
                if code != 0 {
                    failed.push((cmd.clone(), format!("exit {code}: {}", trunc(&out, 200))));
                }
                transcript_events.push(TranscriptEvent::Shell {
                    cmd,
                    exit_code: code,
                    output: limiter.truncate(&trunc(&out, 1500)),
                });
            }
        }
    }
    // Out of steps: force a final answer from what we've gathered.
    let transcript = render_transcript(&transcript_events);
    let user = format!(
        "Request: {task}\n\nCommands run so far:\n{transcript}\nGive your final answer now as {{\"answer\":\"...\"}}."
    );
    let (decision_text, infer_profile) = one_call(&system, &user, mode, false)?;
    if let Some(ref mut ap) = agent_profile {
        ap.steps.push(StepProfile::Inference {
            label: "forced-answer".into(),
            duration: infer_profile.as_ref().map_or(Duration::ZERO, |p| p.total),
            profile: infer_profile,
        });
    }
    if let Decision::Answer(text) = parse_decision(&decision_text) {
        if let Some(ref mut ap) = agent_profile {
            ap.total = agent_start.elapsed();
            ap.print();
        }
        print_answer(&text, mode);
    }
    Ok(0)
}

/// Like `fulfill()` but returns the answer text instead of printing it. Used by MCP tools where
/// the result needs to go back as tool content, not to stdout/stderr.
pub fn fulfill_and_capture(
    task: &str,
    opts: &Options,
    max_steps: usize,
) -> Result<String, String> {
    let shell = if cfg!(windows) {
        "Any command runs in Windows PowerShell — use PowerShell cmdlets and syntax (Get-ChildItem, \
        Select-String, Measure-Object, Select-Object, Where-Object). Do NOT use bash/POSIX tools (no sed, \
        awk, grep, or `find` with -printf)."
    } else {
        "Any command runs in a POSIX bash shell — use POSIX tools (find, grep, sed, awk, wc, ls, \
        git); never PowerShell or cmd syntax."
    };
    let system = build_system_prompt(shell);

    let mut transcript_events: Vec<TranscriptEvent> = Vec::new();
    let mut agent_profile = if agent_profiling() { Some(AgentProfile::default()) } else { None };
    let agent_start = Instant::now();
    let mut seen: Vec<String> = Vec::new();
    let mut failed: Vec<(String, String)> = Vec::new();
    let _perms = crate::agent::permission::Permissions::default();
    let limiter = OutputLimiter { max_lines: 500 };
    for step in 0..max_steps {
        let transcript = render_transcript(&transcript_events);
        let user = if transcript.is_empty() {
            task.to_string()
        } else {
            let fix_hint = if let Some((_last_cmd, last_err)) = failed.last() {
                format!("\nThe last command failed: {last_err}\nTry a different approach to avoid the same error.")
            } else {
                String::new()
            };
            format!("Request: {task}\n\nCommands run so far:\n{transcript}{fix_hint}\nGather more if needed, else answer.")
        };
        let (decision_text, infer_profile) = one_call(&system, &user, Mode::Model, false)?;
        if let Some(ref mut ap) = agent_profile {
            ap.steps.push(StepProfile::Inference {
                label: format!("step-{} infer", step),
                duration: infer_profile.as_ref().map_or(Duration::ZERO, |p| p.total),
                profile: infer_profile,
            });
        }
        match parse_decision(&decision_text) {
            Decision::Answer(text) => {
                if let Some(ref mut ap) = agent_profile {
                    ap.total = agent_start.elapsed();
                    ap.print();
                }
                return Ok(text);
            }
            Decision::Retry(error) => {
                transcript_events.push(TranscriptEvent::ToolResult {
                    name: "tool",
                    output: error,
                    error: true,
                });
                continue;
            }
            Decision::Tool { tool, args, say: _ } => {
                let call_sig = format!(
                    "tool:{}:{}",
                    tool.name,
                    serde_json::to_string(&args).unwrap_or_default()
                );
                if seen.contains(&call_sig) {
                    break;
                }
                seen.push(call_sig);

                transcript_events.push(TranscriptEvent::ToolCall {
                    name: tool.name,
                    args: args.clone(),
                });

                // Validate args before permission check
                if let Err(e) = tool::validate_args(tool, &args) {
                    transcript_events.push(TranscriptEvent::ToolResult {
                        name: tool.name,
                        output: e,
                        error: true,
                    });
                    continue;
                }

                // Model mode: always execute (MCP gates externally)
                let ctx = ToolContext {
                    workdir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
                };
                let t_tool = Instant::now();
                let result = (tool.execute)(&ctx, &args);
                let tool_elapsed = t_tool.elapsed();
                if let Some(ref mut ap) = agent_profile {
                    ap.steps.push(StepProfile::Tool { name: tool.name, duration: tool_elapsed });
                }
                match result {
                    Ok(output) => {
                        let truncated = limiter.truncate(&output);
                        transcript_events.push(TranscriptEvent::ToolResult {
                            name: tool.name,
                            output: truncated,
                            error: false,
                        });
                    }
                    Err(e) => {
                        transcript_events.push(TranscriptEvent::ToolResult {
                            name: tool.name,
                            output: e,
                            error: true,
                        });
                    }
                }
            }
            Decision::Run { cmd, .. } if seen.contains(&cmd) => {
                if failed.iter().any(|(c, _)| *c == cmd) && step < max_steps - 1 {
                    transcript_events.push(TranscriptEvent::Shell {
                        cmd,
                        exit_code: -1,
                        output: "already failed — try a different command".to_string(),
                    });
                    continue;
                }
                break;
            }
            Decision::Run { cmd, .. } => {
                seen.push(cmd.clone());
                let t_shell = Instant::now();
                let (code, out) = exec_capture(&cmd, opts, Mode::Model)?;
                let shell_elapsed = t_shell.elapsed();
                if let Some(ref mut ap) = agent_profile {
                    ap.steps.push(StepProfile::Shell { cmd: cmd.clone(), duration: shell_elapsed });
                }
                if code != 0 {
                    failed.push((cmd.clone(), format!("exit {code}: {}", trunc(&out, 200))));
                }
                transcript_events.push(TranscriptEvent::Shell {
                    cmd,
                    exit_code: code,
                    output: limiter.truncate(&trunc(&out, 1500)),
                });
            }
        }
    }
    // Out of steps: force a final answer.
    let transcript = render_transcript(&transcript_events);
    let user = format!(
        "Request: {task}\n\nCommands run so far:\n{transcript}\nGive your final answer now as {{\"answer\":\"...\"}}."
    );
    let (decision_text, infer_profile) = one_call(&system, &user, Mode::Model, false)?;
    if let Some(ref mut ap) = agent_profile {
        ap.steps.push(StepProfile::Inference {
            label: "forced-answer".into(),
            duration: infer_profile.as_ref().map_or(Duration::ZERO, |p| p.total),
            profile: infer_profile,
        });
    }
    if let Decision::Answer(text) = parse_decision(&decision_text) {
        if let Some(ref mut ap) = agent_profile {
            ap.total = agent_start.elapsed();
            ap.print();
        }
        return Ok(text);
    }
    Ok("No answer produced.".to_string())
}

/// Render transcript events to a string for the prompt.
fn render_transcript(events: &[TranscriptEvent]) -> String {
    events.iter().map(|e| e.render()).collect()
}

/// Show the model's one-line narration of a step ("Let me check the routes.") to a human, in User
/// mode only — a light color so it reads as the agent talking, distinct from command output.
fn say_step(say: Option<&str>, mode: Mode) {
    if let (Mode::User, Some(s)) = (mode, say) {
        let _ = writeln!(std::io::stderr(), "{SAY_COLOR}{s}\x1b[0m");
    }
}

/// Light steel-blue for the agent's narration — clearly lighter than the dim it replaced.
const SAY_COLOR: &str = "\x1b[38;5;153m";

// Max commands the model may run to gather info before it must give a final answer.
pub const MAX_STEPS: usize = 6;

/// Print an answer to stdout. User mode renders markdown to ANSI (headers, lists, syntax-highlighted
/// code blocks) so it reads in a terminal instead of showing raw ``` fences; Model mode prints the
/// raw text, since an agent wants plain markdown, not escape codes.
fn print_answer(text: &str, mode: Mode) {
    match mode {
        Mode::User => {
            let opts = markdown_to_ansi::Options {
                syntax_highlight: true,
                // Wrap to the terminal width when known; otherwise let the terminal wrap.
                width: std::env::var("COLUMNS").ok().and_then(|c| c.parse().ok()),
                code_bg: true,
            };
            println!("{}", markdown_to_ansi::render(text, &opts));
        }
        Mode::Model => println!("{text}"),
    }
}

#[derive(Debug)]
pub enum Decision {
    /// Run a command to gather info (the loop continues and feeds the output back). `say` is the
    /// model's own one-line narration of what it's about to do, shown to a human before the command.
    Run {
        say: Option<String>,
        cmd: String,
    },
    /// Call a built-in tool. Resolved to a `&'static Tool` during parsing.
    Tool {
        tool: &'static crate::agent::tool::Tool,
        args: serde_json::Value,
        say: Option<String>,
    },
    Answer(String),
    /// Unknown tool name — feed error back and let the model retry.
    Retry(String),
}

/// Structured transcript events for tool execution.
#[allow(dead_code)]
enum TranscriptEvent {
    Assistant(String),
    ToolCall {
        name: &'static str,
        args: serde_json::Value,
    },
    ToolResult {
        name: &'static str,
        output: String,
        error: bool,
    },
    Shell {
        cmd: String,
        exit_code: i32,
        output: String,
    },
}

impl TranscriptEvent {
    /// Render this event to a string for the prompt.
    fn render(&self) -> String {
        match self {
            TranscriptEvent::Assistant(text) => format!("Assistant: {text}\n"),
            TranscriptEvent::ToolCall { name, args } => {
                let args_str = format_tool_args(name, args);
                format!("> {name}({args_str})\n")
            }
            TranscriptEvent::ToolResult {
                name: _,
                output,
                error,
            } => {
                if *error {
                    format!("[error] {output}\n")
                } else {
                    format!("{output}\n")
                }
            }
            TranscriptEvent::Shell {
                cmd,
                exit_code,
                output,
            } => {
                format!("$ {cmd}\n(exit {exit_code})\n{output}\n")
            }
        }
    }
}

/// Format tool args for display in the transcript.
fn format_tool_args(name: &str, args: &serde_json::Value) -> String {
    match name {
        "read" | "write" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("?");
            if name == "write" {
                let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
                format!("\"{path}\", \"{}\"", truncate_str(content, 40))
            } else {
                format!("\"{path}\"")
            }
        }
        "edit" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("?");
            let old = args.get("old").and_then(|v| v.as_str()).unwrap_or("");
            format!("\"{path}\", \"{}\"", truncate_str(old, 30))
        }
        "glob" => {
            let pattern = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("?");
            format!("\"{pattern}\"")
        }
        "grep" => {
            let pattern = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("?");
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            format!("\"{pattern}\", \"{path}\"")
        }
        _ => format!("{args}"),
    }
}

fn truncate_str(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max).collect::<String>())
    }
}

/// Read the model's JSON decision: `{"run":…}` → gather via a command, `{"tool":…,…}` → call a
/// built-in tool, `{"answer":…}` → final text. An optional `say` narrates the step. Anything that
/// isn't our JSON is treated as a plain answer (graceful when a model strays).
pub fn parse_decision(content: &str) -> Decision {
    // The model is told to emit ONE JSON object, but a weak one sometimes emits several (or trailing
    // prose). Parse the FIRST complete object from the first `{` — a span of first-`{`..last-`}`
    // would glue multiple objects into invalid JSON and lose the decision entirely.
    if let Some(a) = content.find('{') {
        let mut objs =
            serde_json::Deserializer::from_str(&content[a..]).into_iter::<serde_json::Value>();
        if let Some(Ok(v)) = objs.next() {
            let say = v
                .get("say")
                .and_then(|x| x.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from);
            // Check for tool call — resolve against static registry immediately.
            if let Some(name) = v.get("tool").and_then(|x| x.as_str()) {
                if let Some(tool) = tool::resolve(name) {
                    let args = v.get("args").cloned().unwrap_or(serde_json::json!({}));
                    return Decision::Tool { tool, args, say };
                }
                // Unknown tool name — feed error back so the model can retry.
                return Decision::Retry(format!(
                    "Unknown tool \"{name}\"\n\nAvailable tools:\n{}",
                    tool_list_with_descriptions()
                ));
            }
            if let Some(cmd) = v.get("run").and_then(|x| x.as_str()) {
                if !cmd.trim().is_empty() {
                    return Decision::Run {
                        say,
                        cmd: cmd.trim().to_string(),
                    };
                }
            }
            if let Some(ans) = v.get("answer").and_then(|x| x.as_str()) {
                return Decision::Answer(ans.trim().to_string());
            }
        }
    }
    Decision::Answer(content.trim().to_string())
}

/// Format the available tools list with descriptions for error messages.
fn tool_list_with_descriptions() -> String {
    BUILTINS
        .iter()
        .map(|t| format!("  {:<8} {}", t.name, t.description))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Run `cmd` via rtk and capture its combined output. Generated commands target the native shell —
/// PowerShell on Windows, bash on Unix — and `rtk run -c` uses the OS shell (cmd.exe on Windows),
/// which mangles pipes/quoting. So write the command to a temp script and invoke the native
/// interpreter on it by path: no inline quoting, no cross-shell mount surprises.
fn exec_capture(cmd: &str, opts: &Options, mode: Mode) -> Result<(i32, String), String> {
    let pid = std::process::id();
    // Fail hard on error so a bad command gets a non-zero exit (PowerShell non-terminating errors
    // and bash mid-pipeline failures otherwise pass silently) — that's what drives the fix retry.
    let (tmp, run_line, content) = if cfg!(windows) {
        let p = std::env::temp_dir().join(format!("cotrex-task-{pid}.ps1"));
        let line = format!(
            "powershell -NoProfile -ExecutionPolicy Bypass -File {}",
            p.display()
        );
        (p, line, format!("$ErrorActionPreference = 'Stop'\n{cmd}\n"))
    } else {
        let p = std::env::temp_dir().join(format!("cotrex-task-{pid}.sh"));
        let line = format!("bash {}", p.display());
        (p, line, format!("set -e\n{cmd}\n"))
    };
    std::fs::write(&tmp, content).map_err(|e| format!("temp script: {e}"))?;
    // ponytail: temp paths have no spaces, so the unquoted path is safe; quote only if that breaks.
    let exec = Options {
        raw: true,
        footer: false,
        llm_on_failure: false,
        ..*opts
    };
    // Capture every line, and in User mode show the last 5 live in a ```bash viewport (see TailView).
    let mut view = TailView::new(mode);
    let result = orchestrate::run(
        &Intent::from_command(run_line),
        &mut view,
        &mut std::io::sink(),
        &exec,
    );
    let buf = view.finish();
    let _ = std::fs::remove_file(&tmp);
    let code = result?;
    Ok((code, cap_lines(&String::from_utf8_lossy(&buf), OUTPUT_CAP)))
}

/// Live tail of a running command. Captures every byte (returned to the caller) while redrawing the
/// last `TAIL_ROWS` lines in place on stderr as a markdown ```bash block (rendered to ANSI, so it
/// shows as a styled code box, not raw backticks), so a human watches output stream by without it
/// scrolling the terminal. The final tail is left on screen when the command finishes, so the
/// output stays visible. Off in Model mode or when stderr isn't a TTY (no cursor control).
/// ponytail: track the rows we printed and clear-to-end before each repaint, so a variable-height
/// render stays aligned; throttle repaints so a fast command doesn't flicker.
const TAIL_ROWS: usize = 5;
const REDRAW_INTERVAL: Duration = Duration::from_millis(70);

struct TailView {
    full: Vec<u8>,
    pending: String,
    tail: VecDeque<String>,
    rows: usize, // rows the last repaint printed (to move back up over them)
    shown: bool,
    live: bool,
    last_draw: std::time::Instant,
}

impl TailView {
    fn new(mode: Mode) -> Self {
        let live = mode == Mode::User && std::io::stderr().is_terminal();
        TailView {
            full: Vec::new(),
            pending: String::new(),
            tail: VecDeque::new(),
            rows: 0,
            shown: false,
            live,
            last_draw: std::time::Instant::now(),
        }
    }

    fn push_line(&mut self, line: String) {
        if self.tail.len() == TAIL_ROWS {
            self.tail.pop_front();
        }
        self.tail.push_back(line);
        // Repaint on the first line for instant feedback, then at most every REDRAW_INTERVAL.
        if !self.shown || self.last_draw.elapsed() >= REDRAW_INTERVAL {
            self.redraw();
        }
    }

    /// Render the current tail as an ANSI ```bash block and repaint it in place.
    fn redraw(&mut self) {
        let mut err = std::io::stderr();
        if self.shown {
            let _ = write!(err, "\r\x1b[{}A\x1b[J", self.rows); // up over the last paint, clear down
        }
        let block = render_block(&self.tail);
        let _ = write!(err, "{block}");
        let _ = err.flush();
        self.rows = block.matches('\n').count();
        self.shown = true;
        self.last_draw = std::time::Instant::now();
    }

    /// Return the full captured output, leaving the final tail on screen (so the command's output
    /// stays visible after it finishes) and dropping the cursor below it for what prints next.
    fn finish(mut self) -> Vec<u8> {
        if self.live && self.shown {
            self.redraw(); // ensure the last batch of lines is displayed
            let _ = writeln!(std::io::stderr()); // move below the retained block, don't erase it
        }
        self.full
    }
}

/// Build the ANSI-rendered ```bash block for the tail (oldest→newest, padded to `TAIL_ROWS`), with no
/// trailing newline so the caller can count rows by counting `\n`.
// Columns left blank on the right of the output box, so it doesn't run to the terminal edge.
const RIGHT_MARGIN: usize = 4;

fn render_block(tail: &VecDeque<String>) -> String {
    let w = term_width().saturating_sub(1 + RIGHT_MARGIN);
    let mut md = String::from("```bash\n");
    for line in tail {
        md.push_str(&clip(line, w));
        md.push('\n');
    }
    md.push_str("```");
    let opts = markdown_to_ansi::Options {
        syntax_highlight: true,
        width: Some(w + 1),
        code_bg: true,
    };
    markdown_to_ansi::render(&md, &opts)
        .trim_end_matches('\n')
        .to_string()
}

impl Write for TailView {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.full.extend_from_slice(b);
        if self.live {
            for ch in String::from_utf8_lossy(b).chars() {
                match ch {
                    '\n' => {
                        let line = std::mem::take(&mut self.pending);
                        self.push_line(line);
                    }
                    '\r' => {}
                    _ => self.pending.push(ch),
                }
            }
        }
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn term_width() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|c| c.parse().ok())
        .filter(|w| *w > 0)
        .unwrap_or(80)
}

/// Clip a line to `max` display chars (UTF-8 safe) so it can't wrap and break the cursor math.
fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!(
            "{}…",
            s.chars().take(max.saturating_sub(1)).collect::<String>()
        )
    }
}

/// Flood stop: a weak model sometimes emits a recurse-everything command (10k+ lines). Keep the
/// first `OUTPUT_CAP` lines so a bad command can't bury the terminal/context; normal output (well
/// under the cap) is untouched.
const OUTPUT_CAP: usize = 500;

fn cap_lines(s: &str, max: usize) -> String {
    let mut out = String::new();
    for (i, line) in s.lines().enumerate() {
        if i >= max {
            let extra = s.lines().count() - max;
            out.push_str(&format!(
                "… ({extra} more lines truncated — narrow the command)\n"
            ));
            break;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// A command is risky (→ confirm) if it can delete, overwrite, fetch+run, escalate, or mutate the
/// repo/system. Read-only inspection (Get-ChildItem/find/ls/cat/grep/git status…) is safe and runs
/// unprompted. Uses the permission module for pattern-based evaluation.
fn is_risky(cmd: &str) -> bool {
    let perms = crate::agent::permission::Permissions::default();
    matches!(
        perms.evaluate("shell", Some(cmd)),
        crate::agent::permission::Action::Ask | crate::agent::permission::Action::Deny
    )
}

/// Confirm a risky command before running. Default No: empty line, EOF (no TTY / no input), or a
/// read error all decline.
fn confirm(cmd: &str) -> bool {
    let mut err = std::io::stderr();
    let _ = writeln!(err, "$ {cmd}");
    let _ = write!(err, "Run this command? [y/N] ");
    let _ = err.flush();
    let mut line = String::new();
    if std::io::stdin().read_line(&mut line).unwrap_or(0) == 0 {
        return false; // EOF / no input
    }
    is_yes(&line)
}

fn is_yes(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

/// Truncate long error output (by chars, UTF-8 safe) before feeding it back to the model.
fn trunc(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max).collect::<String>())
    }
}

/// Run one inference call through the local llama.cpp model.
///
/// In **User mode** the function shows a spinner while the model loads, then
/// streams the answer text to stderr in real-time as tokens arrive.  C-level
/// llama.cpp context-construction noise may appear briefly during setup, but
/// cursor save/restore (`\x1b[s` / `\x1b[u\x1b[J`) erases everything after
/// generation finishes.  The full JSON is returned so `parse_decision` +
/// `print_answer` can render the final markdown-formatted answer on stdout.
///
/// In **Model mode** inference runs silently and the raw JSON is returned.
fn one_call(
    system: &str,
    user: &str,
    mode: Mode,
    _live: bool,
) -> Result<(String, Option<cotrex_ai_runtime::InferProfile>), String> {
    let phrases = [
        "cooking",
        "brewing",
        "pondering",
        "crunching",
        "consulting the oracle",
        "sparking neurons",
        "weaving words",
    ];
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let random_index = (nanos % phrases.len() as u128) as usize;
    let label = phrases[random_index];

    if mode == Mode::User {
        // Save cursor via console_write (bypasses fd 2, works even while the
        // worker thread has StderrSuppress active).
        crate::llm::console_write(b"\x1b[s");

        let mut spinner = Some(Spinner::start(label));
        let (handle, rx) = crate::llm::infer_local_stream(system, user);

        let mut buffer = String::new();
        let mut displayed: usize = 0;

        while let Ok(token) = rx.recv() {
            // First token → stop the spinner.
            drop(spinner.take());

            buffer.push_str(&token);

            // Incrementally extract the answer-text value from the growing
            // JSON and stream it via console_write (bypasses fd 2 suppression).
            if let Some(text) = extract_answer_value(&buffer) {
                if text.len() > displayed {
                    crate::llm::console_write(text[displayed..].as_bytes());
                    displayed = text.len();
                }
            }
        }

        drop(spinner.take());

        // Restore cursor + erase spinner, C noise, and streamed text.
        crate::llm::console_write(b"\x1b[u\x1b[J");

        let (full_text, profile) = handle.join().unwrap_or_else(|_| Err("inference thread panicked".into()))
            ?;
        Ok((full_text, profile))
    } else {
        // Model mode — silent, no output
        crate::llm::infer_local_profiled(system, user)
    }
}

/// Incrementally extract the *value* of the `"answer"` key from a partial JSON
/// string.  Searches for `"answer":"` anywhere in the buffer (handles any key
/// ordering, e.g. `{"say":"…","answer":"…"}`).  Returns `Some(text)` with the
/// unescaped text seen so far, or `None` if the buffer does not yet contain an
/// answer value.
fn extract_answer_value(buf: &str) -> Option<String> {
    // Find the answer key anywhere in the (possibly partial) JSON.
    let key = r#""answer":""#;
    let start = buf.find(key)? + key.len();
    let value_part = &buf[start..];

    // Walk the JSON string value, handling backslash escapes.
    let mut result = String::new();
    let mut chars = value_part.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => break,          // closing quote — value complete
            '\\' => {
                // Unescape the next character.
                if let Some(esc) = chars.next() {
                    result.push(match esc {
                        'n'  => '\n',
                        't'  => '\t',
                        'r'  => '\r',
                        '"'  => '"',
                        '\\' => '\\',
                        '/'  => '/',
                        other => other,   // best-effort for \uXXXX etc.
                    });
                }
            }
            other => result.push(other),
        }
    }
    Some(result)
}

/// A tiny stderr spinner that animates until stopped. Shows a green checkmark on completion.
pub struct Spinner {
    stop: Arc<AtomicBool>,
    done: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

const SPIN_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
const SPIN_FRAME: Duration = Duration::from_millis(80);
const GREEN: &str = "\x1b[32m";
const RESET: &str = "\x1b[0m";

impl Spinner {
    pub fn start(label: &str) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let done = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let done_flag = done.clone();
        let label = label.to_string();

        let handle = thread::spawn(move || {
            // Use console_write for all visible output so it bypasses fd 2
            // (StderrSuppress redirects fd 2 to NUL during inference).
            crate::llm::console_write(b"\x1b[?25l");

            let mut i = 0;
            while !flag.load(Ordering::Relaxed) {
                let start = std::time::Instant::now();
                crate::llm::console_write(
                    format!("\r{SAY_COLOR}{}\x1b[0m {label}...", SPIN_FRAMES[i % SPIN_FRAMES.len()]).as_bytes(),
                );
                i += 1;

                while !flag.load(Ordering::Relaxed) && start.elapsed() < SPIN_FRAME {
                    thread::sleep(Duration::from_millis(10));
                }
            }

            // Show green checkmark on completion
            if done_flag.load(Ordering::Relaxed) {
                crate::llm::console_write(format!("\r{GREEN}✓{RESET} {label}\n").as_bytes());
            } else {
                crate::llm::console_write(b"\r\x1b[K");
            }
            crate::llm::console_write(b"\x1b[?25h");
        });

        Spinner {
            stop,
            done,
            handle: Some(handle),
        }
    }

    /// Stop the spinner with a green checkmark.
    #[allow(dead_code)]
    pub fn complete(&self) {
        self.done.store(true, Ordering::Relaxed);
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        if !self.done.load(Ordering::Relaxed) {
            self.stop.store(true, Ordering::Relaxed);
        }
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
        // Use console_write to restore cursor (bypasses fd 2 suppression).
        crate::llm::console_write(b"\x1b[?25h");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_distinguishes_forms() {
        assert_eq!(
            classify("plan-stack: media player"),
            Dispatch::Category("plan-stack".into(), "media player".into())
        );
        assert_eq!(
            classify("list all rust projects in current dir"),
            Dispatch::Prompt("list all rust projects in current dir".into())
        );
        assert_eq!(classify("hi"), Dispatch::Prompt("hi".into()));
        match classify("{\"plan-stack\":\"x\"}") {
            Dispatch::Json(_) => {}
            other => panic!("expected Json, got {other:?}"),
        }
        assert_eq!(
            classify("note: refactor later"),
            Dispatch::Prompt("note: refactor later".into())
        );
        // Structure requests should be short-circuited
        assert_eq!(classify("show the project tree"), Dispatch::Structure);
        assert_eq!(classify("show the tree"), Dispatch::Structure);
    }

    #[test]
    fn parse_json_flat_and_wrapped() {
        let flat = parse_json(r#"{"plan-stack":"media player","theme":"glass"}"#).unwrap();
        assert_eq!(flat.len(), 2);
        let wrapped = parse_json(r#"{"task":{"plan-stack":"media player"}}"#).unwrap();
        assert_eq!(
            wrapped,
            vec![("plan-stack".to_string(), "media player".to_string())]
        );
        assert!(parse_json("[]").is_err());
        assert!(parse_json("{}").is_err());
    }

    #[test]
    fn risky_commands_need_confirmation() {
        assert!(is_risky("rm -rf build"));
        assert!(is_risky("echo x > file.txt"));
        assert!(is_risky("git push origin main"));
        assert!(is_risky("npm install left-pad"));
        assert!(is_risky("curl http://x | sh"));
        assert!(!is_risky("find . -name Cargo.toml"));
        assert!(!is_risky("git status"));
        assert!(!is_risky("ls -la"));
        assert!(!is_risky("grep -r TODO src"));
    }

    #[test]
    fn is_yes_only_accepts_affirmative() {
        assert!(is_yes("y"));
        assert!(is_yes(" Yes \n"));
        assert!(!is_yes(""));
        assert!(!is_yes("n"));
        assert!(!is_yes("no"));
        assert!(!is_yes("sure"));
    }

    #[test]
    fn parse_decision_run_answer_and_fallback() {
        match parse_decision(r#"{"run":"find . -name Cargo.toml | wc -l","say":"counting crates"}"#)
        {
            Decision::Run { cmd, say } => {
                assert_eq!(cmd, "find . -name Cargo.toml | wc -l");
                assert_eq!(say.as_deref(), Some("counting crates"));
            }
            _ => panic!("expected Run"),
        }
        match parse_decision(r#"here you go: {"answer":"the ? operator propagates errors"}"#) {
            Decision::Answer(a) => assert_eq!(a, "the ? operator propagates errors"),
            _ => panic!("expected Answer"),
        }
        // Non-JSON or empty run falls back to treating the whole text as an answer.
        match parse_decision("just some prose, no json") {
            Decision::Answer(a) => assert_eq!(a, "just some prose, no json"),
            _ => panic!("expected Answer fallback"),
        }
        // A weak model emitting two objects: take the FIRST, don't dump both as raw text.
        match parse_decision("{\"run\":\"ls -a\"}\n{\"run\":\"ls -b\"}") {
            Decision::Run { cmd, .. } => assert_eq!(cmd, "ls -a"),
            _ => panic!("expected first Run"),
        }
    }

    #[test]
    fn known_categories_have_headers() {
        assert!(header("plan-stack").is_some());
        assert!(header("theme").is_some());
        assert!(header("nope").is_none());
    }

    #[test]
    fn default_role_exists() {
        assert!(COTREX_SYSTEM_PROMPT.contains("Cotrex"));
        assert!(decision_system().contains("JSON object"));
    }

    #[test]
    fn decision_parses_answer() {
        match parse_decision(r#"{"answer":"hello there"}"#) {
            Decision::Answer(text) => assert_eq!(text, "hello there"),
            other => panic!("expected Answer, got {other:?}"),
        }
    }

    #[test]
    fn decision_parses_tool() {
        match parse_decision(r#"{"tool":"read","args":{"path":"README.md"}}"#) {
            Decision::Tool { tool, args, .. } => {
                assert_eq!(tool.name, "read");
                assert_eq!(args.get("path").unwrap().as_str().unwrap(), "README.md");
            }
            other => panic!("expected Tool, got {other:?}"),
        }
    }

    #[test]
    fn decision_parses_run() {
        match parse_decision(r#"{"run":"cargo test","say":"Running tests."}"#) {
            Decision::Run { cmd, say } => {
                assert_eq!(cmd, "cargo test");
                assert_eq!(say.as_deref(), Some("Running tests."));
            }
            other => panic!("expected Run, got {other:?}"),
        }
    }

    #[test]
    fn project_question_requires_context() {
        let prepared = prepare_task("what is this project");
        assert!(
            prepared.contains("Project Context")
                || prepared.contains("README.md")
                || prepared.contains("Cargo.toml")
        );
    }

    #[test]
    fn structure_request_matches_display_verbs() {
        // Existing: both structure word + directory noun
        assert!(is_structure_request("show the project tree"));
        assert!(is_structure_request("list project structure"));
        assert!(is_structure_request("display the codebase layout"));
        // New: display verb + structure word (no directory noun needed)
        assert!(is_structure_request("show the tree"));
        assert!(is_structure_request("list structure"));
        assert!(is_structure_request("display layout"));
        assert!(is_structure_request("print the tree"));
        assert!(is_structure_request("view the project tree"));
        // Should NOT match
        assert!(!is_structure_request("add a tree data structure"));
        assert!(!is_structure_request("build the project"));
        assert!(!is_structure_request("hello world"));
    }

    // ── N1: Tool execution loop tests ───────────────────────────────────────

    #[test]
    fn parse_decision_tool_call() {
        let input = r#"{"tool":"read","args":{"path":"src/main.rs"},"say":"Reading main.rs."}"#;
        match parse_decision(input) {
            Decision::Tool { tool, args, say } => {
                assert_eq!(tool.name, "read");
                assert_eq!(args.get("path").unwrap().as_str().unwrap(), "src/main.rs");
                assert_eq!(say.as_deref(), Some("Reading main.rs."));
            }
            other => panic!("expected Tool, got {other:?}"),
        }
    }

    #[test]
    fn parse_decision_tool_with_prose_prefix() {
        let input = r#"Let me check: {"tool":"grep","args":{"pattern":"fn main","path":"src"}}"#;
        match parse_decision(input) {
            Decision::Tool { tool, .. } => {
                assert_eq!(tool.name, "grep");
            }
            other => panic!("expected Tool, got {other:?}"),
        }
    }

    #[test]
    fn parse_decision_unknown_tool_returns_retry_with_descriptions() {
        let input = r#"{"tool":"writee","args":{}}"#;
        match parse_decision(input) {
            Decision::Retry(text) => {
                assert!(text.contains("Unknown tool \"writee\""));
                assert!(text.contains("Available tools:"));
                assert!(text.contains("read"));
                assert!(text.contains("write"));
                assert!(text.contains("grep"));
            }
            other => panic!("expected Retry with error, got {other:?}"),
        }
    }

    #[test]
    fn parse_decision_tool_defaults_empty_args() {
        let input = r#"{"tool":"read"}"#;
        match parse_decision(input) {
            Decision::Tool { tool, args, .. } => {
                assert_eq!(tool.name, "read");
                // Should default to empty object
                assert!(args.is_object());
                assert!(args.as_object().unwrap().is_empty());
            }
            other => panic!("expected Tool, got {other:?}"),
        }
    }

    #[test]
    fn transcript_render_tool_call() {
        let events = vec![
            TranscriptEvent::ToolCall {
                name: "read",
                args: serde_json::json!({"path": "src/main.rs"}),
            },
            TranscriptEvent::ToolResult {
                name: "read",
                output: "fn main() {}".to_string(),
                error: false,
            },
        ];
        let rendered = render_transcript(&events);
        assert!(rendered.contains("> read(\"src/main.rs\")"));
        assert!(rendered.contains("fn main() {}"));
    }

    #[test]
    fn transcript_render_tool_error() {
        let events = vec![TranscriptEvent::ToolResult {
            name: "read",
            output: "file not found".to_string(),
            error: true,
        }];
        let rendered = render_transcript(&events);
        assert!(rendered.contains("[error] file not found"));
    }

    #[test]
    fn transcript_render_shell() {
        let events = vec![TranscriptEvent::Shell {
            cmd: "cargo check".to_string(),
            exit_code: 0,
            output: "Finished dev profile".to_string(),
        }];
        let rendered = render_transcript(&events);
        assert!(rendered.contains("$ cargo check"));
        assert!(rendered.contains("(exit 0)"));
        assert!(rendered.contains("Finished dev profile"));
    }

    #[test]
    fn tool_list_with_descriptions_includes_all() {
        let list = tool_list_with_descriptions();
        assert!(list.contains("read"));
        assert!(list.contains("write"));
        assert!(list.contains("edit"));
        assert!(list.contains("glob"));
        assert!(list.contains("grep"));
    }

    #[test]
    fn extract_answer_value_partial() {
        // No prefix yet
        assert_eq!(extract_answer_value("{"), None);
        // Prefix only
        assert_eq!(extract_answer_value(r#"{"answer":""#), Some("".into()));
        // Partial value
        assert_eq!(
            extract_answer_value(r#"{"answer":"hello"#),
            Some("hello".into())
        );
        // Complete value
        assert_eq!(
            extract_answer_value(r#"{"answer":"hello"}"#),
            Some("hello".into())
        );
        // Escaped characters
        assert_eq!(
            extract_answer_value(r#"{"answer":"line1\nline2"}"#),
            Some("line1\nline2".into())
        );
        // Non-answer decision
        assert_eq!(extract_answer_value(r#"{"run":"ls"}"#), None);
        // Answer key NOT first (different key ordering)
        assert_eq!(
            extract_answer_value(r#"{"say":"thinking","answer":"A function"#),
            Some("A function".into())
        );
        // Answer key NOT first, complete
        assert_eq!(
            extract_answer_value(r#"{"say":"done","answer":"hello"}"#),
            Some("hello".into())
        );
    }
}
