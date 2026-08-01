//! Minimal MCP server over stdio (newline-delimited JSON-RPC 2.0). Exposes Cotrex's execution core
//! as the `run` tool so agents call it natively — RTK executes, Cotrex returns it as structured,
//! agent-consumable events.
//!
//! ponytail: hand-rolled subset (initialize, tools/list, tools/call, ping) to keep the project
//! sync and tokio-free. Swap to the rmcp SDK only if we need the full spec or an async transport.

use std::io::{self, BufRead, Write};
use std::sync::{Arc, OnceLock};

use serde_json::{json, Value};

use cotrex_ai_contract::{
    BuildSummaryRequest, CapabilityRequest, CapabilityResponse, ExplainRustRequest, RequestMetadata,
};
use cotrex_ai_runtime::{OrchestrationRequest, Orchestrator};

use crate::config::Config;
use crate::core::intent::Intent;
use crate::core::orchestrate::{self, Options};

const PROTOCOL_VERSION: &str = "2024-11-05";

static ORCHESTRATOR: OnceLock<Arc<Orchestrator>> = OnceLock::new();

/// Run the stdio JSON-RPC loop until stdin closes. stdout is the protocol channel — nothing else
/// may write to it (the execution core writes to in-memory buffers instead).
pub fn serve() -> ! {
    eprintln!("cotrex MCP server (stdio) — protocol {PROTOCOL_VERSION}");
    let cfg = crate::config::load();
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(req) = serde_json::from_str::<Value>(line) else {
            continue; // ignore malformed frames
        };
        let id = req.get("id").cloned();
        let method = req.get("method").and_then(Value::as_str).unwrap_or("");
        let params = req.get("params").cloned().unwrap_or(json!({}));

        match dispatch(method, &params, &cfg) {
            Ok(None) => {} // notification: no reply
            Ok(Some(result)) => {
                if let Some(id) = id {
                    send(
                        &mut stdout,
                        json!({"jsonrpc":"2.0","id":id,"result":result}),
                    );
                }
            }
            Err((code, msg)) => {
                if let Some(id) = id {
                    send(
                        &mut stdout,
                        json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":msg}}),
                    );
                }
            }
        }
    }
    std::process::exit(0);
}

/// Run the MCP server with an orchestrator available.
/// The orchestrator is lazily initialized on first tool call.
pub fn serve_with_ai(orchestrator: Arc<Orchestrator>) -> ! {
    ORCHESTRATOR.set(orchestrator).ok();
    serve()
}

fn send(out: &mut impl Write, msg: Value) {
    writeln!(out, "{msg}").ok();
    out.flush().ok();
}

/// Returns Ok(None) for notifications, Ok(Some(result)) for replies, Err((code,msg)) for errors.
fn dispatch(method: &str, params: &Value, cfg: &Config) -> Result<Option<Value>, (i64, String)> {
    match method {
        "initialize" => Ok(Some(json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "cotrex", "version": env!("CARGO_PKG_VERSION")},
        }))),
        "notifications/initialized" => Ok(None),
        "ping" => Ok(Some(json!({}))),
        "tools/list" => Ok(Some(tools_list())),
        "tools/call" => Ok(Some(tools_call(params, cfg))),
        other => Err((-32601, format!("method not found: {other}"))),
    }
}

fn tools_list() -> Value {
    json!({"tools": [{
        "name": "run",
        "description": "Run a command via RTK.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "command": {"type": "string", "description": "Command line, e.g. \"cargo test\""},
                "llm": {"type": "boolean", "description": "Compress output into an LLM insight"},
            },
            "required": ["command"],
        },
    }, {
        "name": "set_agent",
        "description": "Set agent platform id.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "agent": {"type": "string", "description": "graphify platform id, e.g. claude, codex, cursor, gemini, opencode"},
            },
            "required": ["agent"],
        },
    }, {
        "name": "list_roles",
        "description": "List available roles.",
        "inputSchema": {
            "type": "object",
            "properties": {},
        },
    }, {
        "name": "delegate",
        "description": "Delegate task to a role.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "task": {"type": "string", "description": "The task to delegate, e.g. \"analyze the project structure\""},
                "role": {"type": "string", "description": "Role name (default: assistant). Options: planner, router, orchestrator, coder, assistant"},
            },
            "required": ["task"],
        },
    }, {
        "name": "plan",
        "description": "Plan a task.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "task": {"type": "string", "description": "The task to plan, e.g. \"build a music player app\""},
            },
            "required": ["task"],
        },
    }, {
        "name": "usage",
        "description": "Show token usage.",
        "inputSchema": {
            "type": "object",
            "properties": {},
        },
    }, {
        "name": "graphify",
        "description": "Query knowledge graph.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "question": {"type": "string", "description": "The question to search for in the graph"},
                "dfs": {"type": "boolean", "description": "Use DFS mode instead of BFS (default: false)"},
                "budget": {"type": "integer", "description": "Token budget for output (default: 2000)"},
            },
            "required": ["question"],
        },
    }, {
        "name": "graphify_path",
        "description": "Path between two nodes.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "node_a": {"type": "string", "description": "Source concept name"},
                "node_b": {"type": "string", "description": "Target concept name"},
            },
            "required": ["node_a", "node_b"],
        },
    }, {
        "name": "graphify_explain",
        "description": "Explain a graph node.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "node_name": {"type": "string", "description": "Node name to explain"},
            },
            "required": ["node_name"],
        },
    }, {
        "name": "graphify_add",
        "description": "Add URL to graph corpus.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "url": {"type": "string", "description": "URL to fetch"},
                "author": {"type": "string", "description": "Author tag"},
                "contributor": {"type": "string", "description": "Contributor tag"},
            },
            "required": ["url"],
        },
    }, {
        "name": "graphify_save_result",
        "description": "Save Q&A to graph.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "question": {"type": "string", "description": "The question text to save (not to ask - this is for storing a completed Q&A pair)"},
                "answer": {"type": "string", "description": "The answer text to save"},
                "result_type": {"type": "string", "description": "Type: query, path_query, or explain"},
                "nodes": {"type": "array", "items": {"type": "string"}, "description": "Node labels cited in the answer"},
            },
            "required": ["question", "answer"],
        },
    }, {
        "name": "graphify_export",
        "description": "Export graph (svg/graphml/neo4j).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "format": {"type": "string", "description": "Export format: svg, graphml, or neo4j"},
            },
            "required": ["format"],
        },
    }, {
        "name": "cotrex_build_summary",
        "description": "Summarize build output with AI.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "command": {"type": "string", "description": "The build command that was run"},
                "exit_code": {"type": "integer", "description": "Process exit code"},
                "stdout": {"type": "string", "description": "Standard output from the build"},
                "stderr": {"type": "string", "description": "Standard error from the build"},
                "prompt": {"type": "string", "description": "Optional custom prompt for the AI"},
            },
            "required": ["command", "exit_code", "stderr"],
        },
    }, {
        "name": "cotrex_explain_rust",
        "description": "Explain Rust code with AI.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "source": {"type": "string", "description": "The Rust source code to explain"},
                "question": {"type": "string", "description": "What to explain about the code (default: explain what it does)"},
            },
            "required": ["source"],
        },
    }, {
        "name": "workspace_context",
        "description": "Get workspace git context.",
        "inputSchema": {
            "type": "object",
            "properties": {},
        },
    }]})
}

/// Dispatch a tools/call to the right handler.
fn tools_call(params: &Value, cfg: &Config) -> Value {
    tools_call_with(params, cfg, ORCHESTRATOR.get().map(|o| o.as_ref()))
}

/// Dispatch with an optional orchestrator reference. Tests inject their own orchestrator
/// here to avoid global state leakage via OnceLock.
fn tools_call_with(params: &Value, cfg: &Config, orch: Option<&Orchestrator>) -> Value {
    match params.get("name").and_then(Value::as_str).unwrap_or("") {
        "run" => tool_run(params, cfg),
        "set_agent" => tool_set_agent(params),
        "list_roles" => tool_list_roles(),
        "delegate" => tool_delegate(params, cfg),
        "plan" => tool_plan(params, cfg),
        "usage" => tool_usage(),
        "graphify" => tool_graphify_query(params),
        "graphify_path" => tool_graphify_path(params),
        "graphify_explain" => tool_graphify_explain(params),
        "graphify_add" => tool_graphify_add(params),
        "graphify_save_result" => tool_graphify_save_result(params),
        "graphify_export" => tool_graphify_export(params),
        "cotrex_build_summary" => tool_build_summary_with(params, orch),
        "cotrex_explain_rust" => tool_explain_rust_with(params, orch),
        "workspace_context" => tool_workspace_context_with(params, orch),
        other => tool_error(format!("unknown tool: {other}")),
    }
}

/// `set_agent`: the model tells cotrex its own platform (no TTY needed). Persists it and kicks off
/// the graphify skill install in the background — never writes to stdout (the JSON-RPC channel).
fn tool_set_agent(params: &Value) -> Value {
    let agent = params
        .get("arguments")
        .and_then(|a| a.get("agent"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if agent.is_empty() {
        return tool_error("missing required argument 'agent'".into());
    }
    let mut cfg = crate::config::load();
    cfg.agent = agent.clone();
    if let Err(e) = crate::config::save(&cfg) {
        return tool_error(format!("could not save config: {e}"));
    }
    crate::graphify::clear_skill_marker();
    crate::graphify::bootstrap_detached();
    json!({
        "content": [{"type": "text", "text": format!("Agent set to '{agent}'. Installing the graphify skill for it in the background.")}],
        "isError": false,
    })
}

/// `list_roles`: return the single available assistant role.
fn tool_list_roles() -> Value {
    let role = json!({
        "name": "assistant",
        "model": "local",
        "description": "Single local execution assistant"
    });
    json!({
        "content": [{"type": "text", "text": serde_json::to_string_pretty(&vec![role]).unwrap_or_default()}],
        "isError": false,
    })
}

/// `delegate`: invoke the single assistant with a task and return the answer.
fn tool_delegate(params: &Value, cfg: &Config) -> Value {
    let args = params.get("arguments").cloned().unwrap_or(json!({}));
    let task = args
        .get("task")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if task.is_empty() {
        return tool_error("missing required argument 'task'".into());
    }
    let _role_name = args
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or("assistant")
        .trim();

    let opts = Options {
        raw: cfg.compression == "off",
        ultra_compact: cfg.rtk_verbosity == "ultra-compact",
        llm_on_failure: false,
        footer: false,
        quiet: false,
    };
    let max_steps = crate::agent::prompt::MAX_STEPS;

    match crate::agent::prompt::fulfill_and_capture(&task, &opts, max_steps) {
        Ok(answer) => json!({
            "content": [{"type": "text", "text": answer}],
            "isError": false,
        }),
        Err(e) => tool_error(e),
    }
}

/// `plan`: shorthand for delegate with the planner role.
fn tool_plan(params: &Value, cfg: &Config) -> Value {
    let args = params.get("arguments").cloned().unwrap_or(json!({}));
    let task = args
        .get("task")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if task.is_empty() {
        return tool_error("missing required argument 'task'".into());
    }

    let opts = Options {
        raw: cfg.compression == "off",
        ultra_compact: cfg.rtk_verbosity == "ultra-compact",
        llm_on_failure: false,
        footer: false,
        quiet: false,
    };
    let max_steps = crate::agent::prompt::MAX_STEPS;

    match crate::agent::prompt::fulfill_and_capture(&task, &opts, max_steps) {
        Ok(answer) => json!({
            "content": [{"type": "text", "text": answer}],
            "isError": false,
        }),
        Err(e) => tool_error(e),
    }
}

/// Execute the `run` tool via the shared core, returning MCP tool-result content.
fn tool_run(params: &Value, cfg: &Config) -> Value {
    let args = params.get("arguments").cloned().unwrap_or(json!({}));
    let command = args
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if command.is_empty() {
        return tool_error("missing required argument 'command'".into());
    }
    let mut intent = Intent::from_command(command);
    intent.llm = false;
    let opts = Options {
        raw: cfg.compression == "off",
        ultra_compact: cfg.rtk_verbosity == "ultra-compact",
        llm_on_failure: false,
        footer: true,
        quiet: false,
    };

    // Capture the machine channel; discard the human summary. stdout stays the protocol channel.
    let mut machine: Vec<u8> = Vec::new();
    let mut human = io::sink();
    match orchestrate::run(&intent, &mut machine, &mut human, &opts) {
        Ok(code) => {
            if cfg.graph_auto {
                crate::graphify::auto_update(&intent.command);
            }
            let text = String::from_utf8_lossy(&machine).to_string();
            let input_bytes = intent.command.len();
            let output_bytes = text.len();
            crate::usage::record(&intent.command, input_bytes, output_bytes, code, "mcp");
            let usage_footer =
                crate::usage::footer(&intent.command, input_bytes, output_bytes, code);
            let tokens_in = input_bytes / 4;
            let tokens_out = output_bytes / 4;
            let mut content = vec![
                json!({"type": "text", "text": text}),
                json!({"type": "text", "text": usage_footer}),
            ];
            if cfg.graph_auto && crate::graphify::current_agent().is_none() {
                content.push(json!({"type": "text", "text": "note: cotrex couldn't detect your agent, so the graphify code-map skill isn't installed. Call the set_agent tool with your platform id (e.g. claude, codex, cursor, gemini) to enable it."}));
            }
            json!({
                "content": content,
                "isError": code != 0,
                "usage": {
                    "command": intent.command,
                    "tokens_in": tokens_in,
                    "tokens_out": tokens_out,
                    "input_bytes": input_bytes,
                    "output_bytes": output_bytes,
                    "exit_code": code,
                    "status": if code == 0 { "ok" } else { "failed" },
                }
            })
        }
        Err(e) => tool_error(e),
    }
}

fn tool_error(msg: String) -> Value {
    json!({"content": [{"type": "text", "text": format!("error: {msg}")}], "isError": true})
}

fn tool_usage() -> Value {
    let usage = crate::usage::summary();
    let json = crate::usage::summary_json();
    json!({
        "content": [
            {"type": "text", "text": usage},
            {"type": "text", "text": format!("\n{json}")},
        ],
        "isError": false,
    })
}

fn tool_graphify_query(params: &Value) -> Value {
    let args = params.get("arguments").cloned().unwrap_or(json!({}));
    let question = args
        .get("question")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if question.is_empty() {
        return tool_error("missing required argument 'question'".into());
    }
    let dfs = args.get("dfs").and_then(Value::as_bool).unwrap_or(false);
    let budget = args.get("budget").and_then(Value::as_u64).unwrap_or(2000) as u32;
    match crate::graphify::query_graph(question, dfs, budget) {
        Ok(output) => json!({
            "content": [{"type": "text", "text": output}],
            "isError": false,
        }),
        Err(e) => tool_error(e),
    }
}

fn tool_graphify_path(params: &Value) -> Value {
    let args = params.get("arguments").cloned().unwrap_or(json!({}));
    let node_a = args
        .get("node_a")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let node_b = args
        .get("node_b")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if node_a.is_empty() || node_b.is_empty() {
        return tool_error("missing required arguments 'node_a' and 'node_b'".into());
    }
    match crate::graphify::path_between(node_a, node_b) {
        Ok(output) => json!({
            "content": [{"type": "text", "text": output}],
            "isError": false,
        }),
        Err(e) => tool_error(e),
    }
}

fn tool_graphify_explain(params: &Value) -> Value {
    let args = params.get("arguments").cloned().unwrap_or(json!({}));
    let node_name = args
        .get("node_name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if node_name.is_empty() {
        return tool_error("missing required argument 'node_name'".into());
    }
    match crate::graphify::explain_node(node_name) {
        Ok(output) => json!({
            "content": [{"type": "text", "text": output}],
            "isError": false,
        }),
        Err(e) => tool_error(e),
    }
}

fn tool_graphify_add(params: &Value) -> Value {
    let args = params.get("arguments").cloned().unwrap_or(json!({}));
    let url = args.get("url").and_then(Value::as_str).unwrap_or("").trim();
    if url.is_empty() {
        return tool_error("missing required argument 'url'".into());
    }
    let author = args
        .get("author")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let contributor = args
        .get("contributor")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    match crate::graphify::add_url(url, author, contributor) {
        Ok(output) => json!({
            "content": [{"type": "text", "text": output}],
            "isError": false,
        }),
        Err(e) => tool_error(e),
    }
}

fn tool_graphify_save_result(params: &Value) -> Value {
    let args = params.get("arguments").cloned().unwrap_or(json!({}));
    let question = args
        .get("question")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let answer = args
        .get("answer")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if question.is_empty() || answer.is_empty() {
        return tool_error("missing required arguments 'question' and 'answer'".into());
    }
    let result_type = args
        .get("result_type")
        .and_then(Value::as_str)
        .unwrap_or("query")
        .trim();
    let nodes: Vec<&str> = args
        .get("nodes")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    match crate::graphify::save_result(question, answer, result_type, &nodes) {
        Ok(output) => json!({
            "content": [{"type": "text", "text": output}],
            "isError": false,
        }),
        Err(e) => tool_error(e),
    }
}

fn tool_graphify_export(params: &Value) -> Value {
    let args = params.get("arguments").cloned().unwrap_or(json!({}));
    let format = args
        .get("format")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let result = match format {
        "svg" => crate::graphify::export_svg(),
        "graphml" => crate::graphify::export_graphml(),
        "neo4j" => crate::graphify::export_neo4j(),
        _ => {
            return tool_error(format!(
                "unknown export format: {format}. Supported: svg, graphml, neo4j"
            ))
        }
    };
    match result {
        Ok(output) => json!({
            "content": [{"type": "text", "text": output}],
            "isError": false,
        }),
        Err(e) => tool_error(e),
    }
}

fn tool_build_summary_with(params: &Value, orch: Option<&Orchestrator>) -> Value {
    let Some(orch) = orch else {
        return tool_error("Orchestrator not initialized.".into());
    };

    let args = &params["arguments"];
    let request = OrchestrationRequest {
        capability: CapabilityRequest::BuildSummary(BuildSummaryRequest {
            metadata: RequestMetadata::new(),
            command: args["command"].as_str().unwrap_or("").into(),
            exit_code: args["exit_code"].as_i64().unwrap_or(0) as i32,
            stdout: args["stdout"].as_str().unwrap_or("").into(),
            stderr: args["stderr"].as_str().unwrap_or("").into(),
            prompt: args
                .get("prompt")
                .and_then(Value::as_str)
                .unwrap_or("Summarize this build output concisely.")
                .into(),
            temperature: 0.1,
            max_tokens: 256,
        }),
        context: None,
    };

    match orch.execute(request) {
        Ok(resp) => {
            let success = match &resp.capability {
                CapabilityResponse::BuildSummary(r) => r.success,
                _ => true,
            };
            json!({
                "content": [{"type": "text", "text": resp.text().to_string()}],
                "isError": !success,
            })
        }
        Err(e) => tool_error(e.to_string()),
    }
}

fn tool_explain_rust_with(params: &Value, orch: Option<&Orchestrator>) -> Value {
    let Some(orch) = orch else {
        return tool_error("Orchestrator not initialized.".into());
    };

    let args = &params["arguments"];
    let source = args["source"].as_str().unwrap_or("");
    let question = args
        .get("question")
        .and_then(Value::as_str)
        .unwrap_or("Explain what this code does");

    let prompt = format!(
        "Explain this Rust code:\n{}\n\nQuestion: {}",
        source, question
    );

    let request = OrchestrationRequest {
        capability: CapabilityRequest::ExplainRust(ExplainRustRequest {
            metadata: RequestMetadata::new(),
            source: source.into(),
            question: question.into(),
            prompt,
            temperature: 0.1,
            max_tokens: 256,
        }),
        context: None,
    };

    match orch.execute(request) {
        Ok(resp) => json!({
            "content": [{"type": "text", "text": resp.text().to_string()}],
            "isError": false,
        }),
        Err(e) => tool_error(e.to_string()),
    }
}

fn tool_workspace_context_with(_params: &Value, orch: Option<&Orchestrator>) -> Value {
    let Some(orch) = orch else {
        return tool_error("Orchestrator not initialized.".into());
    };

    // Route through Orchestrator to prove the full context flow:
    // MCP → Orchestrator → ContextSource → InferenceContext → PromptAssembler → Provider
    let request = OrchestrationRequest {
        capability: CapabilityRequest::BuildSummary(BuildSummaryRequest {
            metadata: RequestMetadata::new(),
            command: "workspace_context".into(),
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
            prompt: "Return workspace context".into(),
            temperature: 0.0,
            max_tokens: 0,
        }),
        context: None,
    };

    match orch.execute(request) {
        Ok(resp) => json!({
            "content": [{"type": "text", "text": resp.text().to_string()}],
            "isError": false,
        }),
        Err(e) => tool_error(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// Local mock provider for MCP tests.
    struct MockProvider;

    impl cotrex_ai_runtime::CapabilityProvider for MockProvider {
        fn info(&self) -> cotrex_ai_contract::ProviderInfo {
            cotrex_ai_contract::ProviderInfo {
                name: "mock".into(),
                version: "0.1.0".into(),
                supported_capabilities: vec![
                    cotrex_ai_contract::CapabilityKind::BuildSummary,
                    cotrex_ai_contract::CapabilityKind::ExplainRust,
                ],
            }
        }

        fn health(&self) -> cotrex_ai_contract::ProviderHealth {
            cotrex_ai_contract::ProviderHealth::Healthy
        }

        fn execute(
            &self,
            request: cotrex_ai_contract::CapabilityRequest,
        ) -> Result<cotrex_ai_contract::CapabilityResponse, cotrex_ai_runtime::RuntimeError>
        {
            match request {
                cotrex_ai_contract::CapabilityRequest::BuildSummary(req) => {
                    Ok(cotrex_ai_contract::CapabilityResponse::BuildSummary(
                        cotrex_ai_contract::BuildSummaryResponse {
                            success: req.exit_code == 0,
                            summary: "Build completed successfully.".into(),
                            recommendation: None,
                        },
                    ))
                }
                cotrex_ai_contract::CapabilityRequest::ExplainRust(req) => {
                    Ok(cotrex_ai_contract::CapabilityResponse::ExplainRust(
                        cotrex_ai_contract::ExplainRustResponse {
                            explanation: format!("mock: {}", req.question),
                        },
                    ))
                }
            }
        }
    }

    fn test_orchestrator() -> Orchestrator {
        use cotrex_ai_runtime::{
            DefaultCapabilityResponseParser, DefaultOutputParser, DefaultPromptAssembler,
        };

        let tmp = tempfile::tempdir().unwrap();
        let kernel = cotrex::kernel::WorkspaceKernel::open(tmp.path().to_path_buf()).unwrap();
        let context_source = Arc::new(cotrex::kernel::context_source::KernelContextSource::new(
            Arc::new(kernel),
        ));

        Orchestrator::new(
            Arc::new(MockProvider),
            context_source,
            Arc::new(DefaultPromptAssembler),
            Arc::new(DefaultOutputParser),
            Arc::new(DefaultCapabilityResponseParser),
        )
    }

    #[test]
    fn initialize_reports_protocol_and_name() {
        let r = dispatch("initialize", &json!({}), &Config::default())
            .unwrap()
            .unwrap();
        assert_eq!(r["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(r["serverInfo"]["name"], "cotrex");
    }

    #[test]
    fn tools_list_exposes_run() {
        let r = dispatch("tools/list", &json!({}), &Config::default())
            .unwrap()
            .unwrap();
        assert_eq!(r["tools"][0]["name"], "run");
    }

    #[test]
    fn tools_list_exposes_set_agent() {
        let r = dispatch("tools/list", &json!({}), &Config::default())
            .unwrap()
            .unwrap();
        let names: Vec<&str> = r["tools"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|t| t["name"].as_str())
            .collect();
        assert!(names.contains(&"set_agent"));
    }

    #[test]
    fn set_agent_requires_agent() {
        let r = tools_call(
            &json!({"name": "set_agent", "arguments": {}}),
            &Config::default(),
        );
        assert_eq!(r["isError"], true);
    }

    #[test]
    fn initialized_is_a_notification() {
        assert_eq!(
            dispatch("notifications/initialized", &json!({}), &Config::default()),
            Ok(None)
        );
    }

    #[test]
    fn unknown_method_errors() {
        assert!(dispatch("bogus", &json!({}), &Config::default()).is_err());
    }

    #[test]
    fn call_without_command_is_tool_error() {
        let r = tools_call(&json!({"name": "run", "arguments": {}}), &Config::default());
        assert_eq!(r["isError"], true);
    }

    #[test]
    fn tools_list_exposes_workspace_context() {
        let r = dispatch("tools/list", &json!({}), &Config::default())
            .unwrap()
            .unwrap();
        let names: Vec<&str> = r["tools"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|t| t["name"].as_str())
            .collect();
        assert!(names.contains(&"workspace_context"));
    }

    #[test]
    fn workspace_context_without_orchestrator_errors() {
        let r = tools_call_with(
            &json!({"name": "workspace_context", "arguments": {}}),
            &Config::default(),
            None,
        );
        assert_eq!(r["isError"], true);
        assert!(r["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Orchestrator not initialized"));
    }

    #[test]
    fn workspace_context_through_orchestrator() {
        let orch = test_orchestrator();
        let r = tools_call_with(
            &json!({"name": "workspace_context", "arguments": {}}),
            &Config::default(),
            Some(&orch),
        );
        assert_eq!(r["isError"], false);
        assert!(!r["content"][0]["text"].as_str().unwrap().is_empty());
    }

    #[test]
    fn build_summary_through_mock_provider() {
        let orch = test_orchestrator();
        let r = tools_call_with(
            &json!({"name": "cotrex_build_summary", "arguments": {
                "command": "cargo test",
                "exit_code": 0,
                "stderr": ""
            }}),
            &Config::default(),
            Some(&orch),
        );
        assert_eq!(r["isError"], false);
        assert!(r["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Build completed successfully"));
    }

    #[test]
    fn unknown_tool_returns_error() {
        let r = tools_call_with(
            &json!({"name": "bogus_tool", "arguments": {}}),
            &Config::default(),
            None,
        );
        assert_eq!(r["isError"], true);
        assert!(r["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("unknown tool"));
    }
}
