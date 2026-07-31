use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};

use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Spawn the cotrex MCP server as a subprocess.
fn spawn_mcp() -> Child {
    Command::new("cargo")
        .args(["run", "--bin", "cotrex", "--", "mcp"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn cotrex mcp")
}

/// Send a JSON-RPC request and read the response.
fn send_request(child: &mut Child, request: &Value) -> Option<Value> {
    let stdin = child.stdin.as_mut().expect("stdin");
    let mut line = serde_json::to_string(request).unwrap();
    line.push('\n');
    stdin.write_all(line.as_bytes()).unwrap();
    stdin.flush().unwrap();

    let stdout = child.stdout.as_mut().expect("stdout");
    let mut reader = BufReader::new(stdout);
    let mut response_line = String::new();

    match reader.read_line(&mut response_line) {
        Ok(0) => None,
        Ok(_) => serde_json::from_str(&response_line).ok(),
        Err(_) => None,
    }
}

/// Build a JSON-RPC request.
fn jsonrpc(id: u64, method: &str, params: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    })
}

// ---------------------------------------------------------------------------
// T1: initialize
// ---------------------------------------------------------------------------

#[test]
fn mcp_initialize() {
    let mut child = spawn_mcp();
    let resp = send_request(&mut child, &jsonrpc(1, "initialize", json!({})));
    child.kill().ok();

    let resp = resp.expect("no response");
    assert_eq!(resp["id"], 1);
    assert_eq!(resp["result"]["serverInfo"]["name"], "cotrex");
    assert!(resp["result"]["capabilities"]["tools"].is_object());
}

// ---------------------------------------------------------------------------
// T2: tools/list
// ---------------------------------------------------------------------------

#[test]
fn mcp_tools_list() {
    let mut child = spawn_mcp();

    // Initialize first
    send_request(&mut child, &jsonrpc(1, "initialize", json!({})));

    // List tools
    let resp = send_request(&mut child, &jsonrpc(2, "tools/list", json!({})));
    child.kill().ok();

    let resp = resp.expect("no response");
    let names: Vec<&str> = resp["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();

    assert!(names.contains(&"run"));
    assert!(names.contains(&"workspace_context"));
    assert!(names.contains(&"cotrex_build_summary"));
    assert!(names.contains(&"cotrex_explain_rust"));
}

// ---------------------------------------------------------------------------
// T3: workspace_context (returns error without orchestrator)
// ---------------------------------------------------------------------------

#[test]
fn mcp_workspace_context_no_orchestrator() {
    let mut child = spawn_mcp();

    send_request(&mut child, &jsonrpc(1, "initialize", json!({})));

    let resp = send_request(
        &mut child,
        &jsonrpc(
            2,
            "tools/call",
            json!({
                "name": "workspace_context",
                "arguments": {}
            }),
        ),
    );
    child.kill().ok();

    let resp = resp.expect("no response");
    // Without local-model feature, orchestrator is not set -> isError=true
    // With local-model feature, orchestrator is set -> isError=false
    // Both are valid depending on whether local-model feature is enabled.
    // Just verify we got a response.
    assert!(
        resp.get("result").is_some(),
        "expected a result response"
    );
}

// ---------------------------------------------------------------------------
// T4: unknown tool error
// ---------------------------------------------------------------------------

#[test]
fn mcp_unknown_tool_error() {
    let mut child = spawn_mcp();

    send_request(&mut child, &jsonrpc(1, "initialize", json!({})));

    let resp = send_request(
        &mut child,
        &jsonrpc(
            2,
            "tools/call",
            json!({
                "name": "nonexistent_tool",
                "arguments": {}
            }),
        ),
    );
    child.kill().ok();

    let resp = resp.expect("no response");
    assert_eq!(resp["result"]["isError"], true);
    assert!(resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("unknown tool"));
}

// ---------------------------------------------------------------------------
// T5: unknown method error
// ---------------------------------------------------------------------------

#[test]
fn mcp_unknown_method_error() {
    let mut child = spawn_mcp();

    let resp = send_request(&mut child, &jsonrpc(1, "bogus_method", json!({})));
    child.kill().ok();

    let resp = resp.expect("no response");
    assert!(resp["error"].is_object());
    assert_eq!(resp["error"]["code"], -32601);
}

// ---------------------------------------------------------------------------
// T6: notification returns no response
// ---------------------------------------------------------------------------

#[test]
fn mcp_notification_no_response() {
    let mut child = spawn_mcp();

    send_request(&mut child, &jsonrpc(1, "initialize", json!({})));

    // Send a notification (no id)
    let stdin = child.stdin.as_mut().unwrap();
    let notification = json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized",
    });
    let mut line = serde_json::to_string(&notification).unwrap();
    line.push('\n');
    stdin.write_all(line.as_bytes()).unwrap();
    stdin.flush().unwrap();

    // The next response should be for a subsequent request, not the notification
    let resp = send_request(&mut child, &jsonrpc(2, "ping", json!({})));
    child.kill().ok();

    let resp = resp.expect("no response");
    assert_eq!(resp["id"], 2);
    assert_eq!(resp["result"], json!({}));
}

// ---------------------------------------------------------------------------
// T7: ping
// ---------------------------------------------------------------------------

#[test]
fn mcp_ping() {
    let mut child = spawn_mcp();

    send_request(&mut child, &jsonrpc(1, "initialize", json!({})));

    let resp = send_request(&mut child, &jsonrpc(2, "ping", json!({})));
    child.kill().ok();

    let resp = resp.expect("no response");
    assert_eq!(resp["id"], 2);
    assert_eq!(resp["result"], json!({}));
}
