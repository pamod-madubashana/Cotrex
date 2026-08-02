//! Tool registry for the agentic prompt loop.
//!
//! OpenCode has a full tool registry with typed tools (shell, read, write, edit, glob, grep).
//! Cotrex's agentic loop currently only runs shell commands through RTK. This module adds a minimal
//! tool abstraction so the model can call structured tools directly, improving reliability and
//! reducing token waste from shell command generation.

use std::path::{Path, PathBuf};

use serde_json::Value;

/// A tool that the agentic loop can invoke.
#[derive(Copy, Clone)]
pub struct Tool {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: &'static str, // JSON Schema as string
    pub execute: fn(&ToolContext, &serde_json::Value) -> Result<String, String>,
}

impl std::fmt::Debug for Tool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tool")
            .field("name", &self.name)
            .field("description", &self.description)
            .finish_non_exhaustive()
    }
}

/// Context passed to tool execution. Deliberately minimal — no permissions, no session state.
/// Authorization is checked before the tool is called, not inside it.
pub struct ToolContext {
    pub workdir: PathBuf,
}

/// Centralized output limiting. Stateless policy — no internal counters.
pub struct OutputLimiter {
    pub max_lines: usize,
}

impl OutputLimiter {
    pub fn truncate(&self, output: &str) -> String {
        let mut out = String::new();
        for (i, line) in output.lines().enumerate() {
            if i >= self.max_lines {
                let extra = output.lines().count().saturating_sub(self.max_lines);
                out.push_str(&format!("… ({extra} more lines truncated)\n"));
                break;
            }
            out.push_str(line);
            out.push('\n');
        }
        out
    }
}

/// Static registry of built-in tools. Zero allocation, deterministic startup.
pub static BUILTINS: &[Tool] = &[
    READ_TOOL,
    WRITE_TOOL,
    EDIT_TOOL,
    GLOB_TOOL,
    GREP_TOOL,
    GRAPHIFY_REPORT_TOOL,
];

/// Resolve a tool name against the static registry.
pub fn resolve(name: &str) -> Option<&'static Tool> {
    BUILTINS.iter().find(|t| t.name == name)
}

/// Validate tool arguments against the tool's JSON Schema.
/// Returns Ok(()) if valid, or Err with a descriptive message.
pub fn validate_args(tool: &Tool, args: &Value) -> Result<(), String> {
    let schema: Value = serde_json::from_str(tool.parameters)
        .map_err(|e| format!("invalid schema for {}: {e}", tool.name))?;

    let obj = args
        .as_object()
        .ok_or(format!("{}: arguments must be a JSON object", tool.name))?;

    // Check required fields
    if let Some(required) = schema.get("required").and_then(|r| r.as_array()) {
        for req in required {
            if let Some(field) = req.as_str() {
                if !obj.contains_key(field) {
                    return Err(format!("{}: missing required field '{field}'", tool.name));
                }
            }
        }
    }

    // Check field types against properties
    if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
        for (field, schema_def) in properties {
            if let Some(value) = obj.get(field) {
                if let Some(expected_type) = schema_def.get("type").and_then(|t| t.as_str()) {
                    let actual_type = match value {
                        Value::String(_) => "string",
                        Value::Number(_) => "number",
                        Value::Bool(_) => "boolean",
                        Value::Array(_) => "array",
                        Value::Object(_) => "object",
                        Value::Null => "null",
                    };
                    if actual_type != expected_type {
                        return Err(format!(
                            "{}: field '{field}' must be {expected_type}, got {actual_type}",
                            tool.name
                        ));
                    }
                }
            }
        }
    }

    Ok(())
}

// ── Built-in tools ──────────────────────────────────────────────────────────

/// Read a file's contents.
static READ_TOOL: Tool = Tool {
    name: "read",
    description: "Read a file's contents",
    parameters: r#"{"type":"object","properties":{"path":{"type":"string","description":"File path relative to workdir"}},"required":["path"]}"#,
    execute: |ctx, args| {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or("missing 'path'")?;
        let full = resolve_path(&ctx.workdir, path);
        std::fs::read_to_string(&full).map_err(|e| format!("read {path}: {e}"))
    },
};

/// Write content to a file (creates or overwrites).
static WRITE_TOOL: Tool = Tool {
    name: "write",
    description: "Write content to a file",
    parameters: r#"{"type":"object","properties":{"path":{"type":"string","description":"File path relative to workdir"},"content":{"type":"string","description":"Content to write"}},"required":["path","content"]}"#,
    execute: |ctx, args| {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or("missing 'path'")?;
        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or("missing 'content'")?;
        let full = resolve_path(&ctx.workdir, path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
        }
        std::fs::write(&full, content).map_err(|e| format!("write {path}: {e}"))?;
        Ok(format!("wrote {} bytes to {path}", content.len()))
    },
};

/// Edit a file by replacing a string.
static EDIT_TOOL: Tool = Tool {
    name: "edit",
    description: "Edit a file by replacing an exact string match",
    parameters: r#"{"type":"object","properties":{"path":{"type":"string","description":"File path relative to workdir"},"old":{"type":"string","description":"Exact string to find (must match uniquely)"},"new":{"type":"string","description":"Replacement string"}},"required":["path","old","new"]}"#,
    execute: |ctx, args| {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or("missing 'path'")?;
        let old = args
            .get("old")
            .and_then(|v| v.as_str())
            .ok_or("missing 'old'")?;
        let new = args
            .get("new")
            .and_then(|v| v.as_str())
            .ok_or("missing 'new'")?;
        let full = resolve_path(&ctx.workdir, path);
        let content = std::fs::read_to_string(&full).map_err(|e| format!("read {path}: {e}"))?;
        let count = content.matches(old).count();
        if count == 0 {
            return Err(format!("'{old}' not found in {path}"));
        }
        if count > 1 {
            return Err(format!(
                "'{old}' matches {count} times in {path} — provide more context"
            ));
        }
        let updated = content.replacen(old, new, 1);
        std::fs::write(&full, &updated).map_err(|e| format!("write {path}: {e}"))?;
        Ok(format!("edited {path}"))
    },
};

/// Glob for files matching a pattern.
static GLOB_TOOL: Tool = Tool {
    name: "glob",
    description: "Find files matching a glob pattern",
    parameters: r#"{"type":"object","properties":{"pattern":{"type":"string","description":"Glob pattern relative to workdir"}},"required":["pattern"]}"#,
    execute: |ctx, args| {
        let pattern = args
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or("missing 'pattern'")?;
        let full = resolve_path(&ctx.workdir, pattern);
        let mut results = Vec::new();
        if let Ok(entries) = glob::glob(&full.to_string_lossy()) {
            for entry in entries.flatten() {
                if let Ok(rel) = entry.strip_prefix(&ctx.workdir) {
                    results.push(rel.to_string_lossy().to_string());
                } else {
                    results.push(entry.to_string_lossy().to_string());
                }
            }
        }
        if results.is_empty() {
            Ok("no files found".to_string())
        } else {
            results.sort();
            Ok(results.join("\n"))
        }
    },
};

/// Search file contents with a regex pattern.
static GREP_TOOL: Tool = Tool {
    name: "grep",
    description: "Search file contents with a regex pattern",
    parameters: r#"{"type":"object","properties":{"pattern":{"type":"string","description":"Regex pattern to search for"},"path":{"type":"string","description":"Directory or file to search in (relative to workdir, default: .)"}},"required":["pattern"]}"#,
    execute: |ctx, args| {
        let pattern = args
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or("missing 'pattern'")?;
        let search_path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let full = resolve_path(&ctx.workdir, search_path);
        let re = regex::Regex::new(pattern).map_err(|e| format!("invalid regex: {e}"))?;
        let mut results = Vec::new();
        let walk = if full.is_dir() {
            walk_dir(&full, &mut results, &re, &ctx.workdir)
        } else {
            search_file(&full, &re, &ctx.workdir, &mut results)
        };
        if let Err(e) = walk {
            return Err(format!("grep error: {e}"));
        }
        if results.is_empty() {
            Ok("no matches found".to_string())
        } else {
            results.sort();
            Ok(results.join("\n"))
        }
    },
};

// ── Graphify tools ──────────────────────────────────────────────────────────

/// Read the graphify code map overview (GRAPH_REPORT.md).
static GRAPHIFY_REPORT_TOOL: Tool = Tool {
    name: "graphify_report",
    description: "Read the codebase knowledge graph overview (GRAPH_REPORT.md)",
    parameters: r#"{"type":"object","properties":{},"required":[]}"#,
    execute: |_ctx, _args| {
        // Try GRAPH_REPORT.md first (fast path — pre-built overview)
        let report_path = std::path::Path::new("graphify-out").join("GRAPH_REPORT.md");
        if let Ok(content) = std::fs::read_to_string(&report_path) {
            // Truncate to key sections to save context
            let truncated = truncate_report(&content, 150);
            return Ok(truncated);
        }
        // Fallback: try graph.json for basic stats
        let graph_path = std::path::Path::new("graphify-out").join("graph.json");
        if let Ok(content) = std::fs::read_to_string(&graph_path) {
            let len = content.len();
            return Ok(format!(
                "graphify-out/graph.json exists ({len} bytes). \
                 Run 'graphify update .' to regenerate GRAPH_REPORT.md."
            ));
        }
        Err("No graphify graph found. Run 'graphify update .' to build the code map first.".into())
    },
};

/// Truncate a graphify report to the most important sections.
/// Keeps: Summary, God Nodes, Surprising Connections, Import Cycles, Communities overview.
/// Skips: Community Hubs (verbose wiki links), verbose node lists per community.
fn truncate_report(content: &str, _max_lines: usize) -> String {
    let mut out = Vec::new();
    let mut in_skip_section = false;
    let mut in_communities_detail = false;
    let mut communities_count = 0;

    for line in content.lines() {
        // Detect section headers
        if line.starts_with("## ") || line.starts_with("### ") {
            in_skip_section = false;
            in_communities_detail = false;

            let header = line.trim_start_matches('#').trim();

            // Skip the Community Hubs section (huge list of wiki links, not useful)
            if header.starts_with("Community Hubs") {
                in_skip_section = true;
                continue;
            }

            // For Communities detail (### Community N - "..."), only keep header + cohesion + first 3 nodes
            if line.starts_with("### ") && header.starts_with("Community ") {
                in_communities_detail = true;
                communities_count += 1;
                // Only show first 15 communities in detail
                if communities_count > 15 {
                    in_skip_section = true;
                    continue;
                }
            }

            out.push(line.to_string());
            continue;
        }

        // Skip lines in skipped sections
        if in_skip_section {
            continue;
        }

        // For community details, only keep cohesion and first few nodes
        if in_communities_detail {
            if line.contains("Cohesion:") || line.starts_with("Nodes ") {
                // Truncate node list at 5 entries
                if let Some(open) = line.find('(') {
                    if let Some(close) = line.find(')') {
                        let nodes_str = &line[open + 1..close];
                        let nodes: Vec<&str> = nodes_str.split(", ").collect();
                        if nodes.len() > 5 {
                            let total = nodes.len();
                            let truncated: Vec<&str> = nodes.into_iter().take(5).collect();
                            let prefix = &line[..open + 1];
                            out.push(format!(
                                "{}{} (+{} more)",
                                prefix,
                                truncated.join(", "),
                                total - 5
                            ));
                        } else {
                            out.push(line.to_string());
                        }
                    } else {
                        out.push(line.to_string());
                    }
                } else {
                    out.push(line.to_string());
                }
            }
            // Skip individual node lines in community sections
            continue;
        }

        out.push(line.to_string());
    }

    if out.len() >= _max_lines {
        out.truncate(_max_lines);
        out.push("… (report truncated)".to_string());
    }

    out.join("\n")
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn resolve_path(workdir: &Path, relative: &str) -> PathBuf {
    let p = Path::new(relative);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        workdir.join(p)
    }
}

fn walk_dir(
    dir: &Path,
    results: &mut Vec<String>,
    re: &regex::Regex,
    workdir: &Path,
) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name == "target" || name == "node_modules" || name == ".git" || name == "vendor" {
                continue;
            }
            walk_dir(&path, results, re, workdir)?;
        } else if let Ok(content) = std::fs::read_to_string(&path) {
            search_file_content(&path, &content, re, workdir, results);
        }
    }
    Ok(())
}

fn search_file(
    file: &Path,
    re: &regex::Regex,
    workdir: &Path,
    results: &mut Vec<String>,
) -> std::io::Result<()> {
    let content = std::fs::read_to_string(file)?;
    search_file_content(file, &content, re, workdir, results);
    Ok(())
}

fn search_file_content(
    file: &Path,
    content: &str,
    re: &regex::Regex,
    workdir: &Path,
    results: &mut Vec<String>,
) {
    let rel = file.strip_prefix(workdir).unwrap_or(file);
    for (i, line) in content.lines().enumerate() {
        if re.is_match(line) {
            results.push(format!("{}:{}:{}", rel.display(), i + 1, line));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_tool_requires_path() {
        let ctx = ToolContext {
            workdir: std::env::temp_dir(),
        };
        let args = serde_json::json!({});
        assert!((READ_TOOL.execute)(&ctx, &args).is_err());
    }

    #[test]
    fn write_tool_creates_file() {
        let dir = std::env::temp_dir().join("cotrex-tool-test");
        let _ = std::fs::create_dir_all(&dir);
        let ctx = ToolContext {
            workdir: dir.clone(),
        };
        let args = serde_json::json!({"path": "test.txt", "content": "hello"});
        let result = (WRITE_TOOL.execute)(&ctx, &args);
        assert!(result.is_ok());
        let _ = std::fs::remove_file(dir.join("test.txt"));
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn edit_tool_finds_and_replaces() {
        let dir = std::env::temp_dir().join("cotrex-edit-test");
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("test.rs"), "fn main() {}").unwrap();
        let ctx = ToolContext {
            workdir: dir.clone(),
        };
        let args = serde_json::json!({"path": "test.rs", "old": "fn main() {}", "new": "fn main() { println!(\"hi\"); }"});
        let result = (EDIT_TOOL.execute)(&ctx, &args);
        assert!(result.is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_args_valid() {
        let args = serde_json::json!({"path": "foo.rs"});
        assert!(validate_args(&READ_TOOL, &args).is_ok());
    }

    #[test]
    fn validate_args_missing_required() {
        let args = serde_json::json!({});
        let err = validate_args(&READ_TOOL, &args).unwrap_err();
        assert!(err.contains("missing required field 'path'"));
    }

    #[test]
    fn validate_args_wrong_type() {
        let args = serde_json::json!({"path": 123});
        let err = validate_args(&READ_TOOL, &args).unwrap_err();
        assert!(err.contains("must be string, got number"));
    }

    #[test]
    fn validate_args_not_object() {
        let args = serde_json::json!("just a string");
        let err = validate_args(&READ_TOOL, &args).unwrap_err();
        assert!(err.contains("must be a JSON object"));
    }

    #[test]
    fn resolve_finds_builtin() {
        assert!(resolve("read").is_some());
        assert!(resolve("write").is_some());
        assert!(resolve("edit").is_some());
        assert!(resolve("glob").is_some());
        assert!(resolve("grep").is_some());
        assert!(resolve("graphify_report").is_some());
        assert!(resolve("writee").is_none());
        assert!(resolve("unknown").is_none());
    }

    #[test]
    fn output_limiter_truncates() {
        let limiter = OutputLimiter { max_lines: 2 };
        let output = "line1\nline2\nline3\nline4\n";
        let result = limiter.truncate(output);
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines.len(), 3); // 2 content + 1 truncation message
        assert!(lines[2].contains("2 more lines"));
    }

    #[test]
    fn output_limiter_no_truncation() {
        let limiter = OutputLimiter { max_lines: 10 };
        let output = "line1\nline2\n";
        let result = limiter.truncate(output);
        assert_eq!(result, "line1\nline2\n");
    }

    #[test]
    fn unknown_tool_does_not_reach_executor() {
        // Static registry resolution is a security boundary.
        // A typo like "writee" should fail before any execution path.
        assert!(resolve("writee").is_none());
        assert!(resolve("").is_none());
        assert!(resolve("rm").is_none());
        assert!(resolve("exec").is_none());
    }
}
