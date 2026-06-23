//! Stack planner: use the configured LLM when available, with a deterministic keyword fallback.
//!
//! The LLM call is streamed so the model's thinking shows live on stderr while the user waits, and
//! the prompt is scoped to a stack name + one-line reason — never code — so a slow reasoning model
//! doesn't burn time generating init scripts nobody asked for.

use std::io::{BufRead, BufReader, Write};

use serde::{Deserialize, Serialize};

use crate::llm::LlmConfig;

#[derive(Debug, Serialize)]
pub struct StackPlan {
    pub task: String,
    pub stack: String,
    pub reason: String,
}

#[derive(Debug, Deserialize)]
struct LlmStackPlan {
    stack: String,
    reason: String,
}

struct Rule {
    keywords: &'static [&'static str],
    stack: &'static str,
    reason: &'static str,
}

const RULES: &[Rule] = &[
    Rule {
        keywords: &[
            "website",
            "site",
            "web app",
            "webapp",
            "dashboard",
            "landing",
            "portfolio",
            "e-commerce",
            "ecommerce",
            "weather app",
        ],
        stack: "next.js",
        reason: "web UI with SSR; largest ecosystem and fastest to ship",
    },
    Rule {
        keywords: &["desktop", "music player", "player", "native app", "tray"],
        stack: "tauri",
        reason: "cross-platform desktop with Rust core + web UI; small binaries",
    },
    Rule {
        keywords: &["mobile", "ios", "android", "cross-platform app"],
        stack: "flutter",
        reason: "single codebase for iOS + Android with native performance",
    },
    Rule {
        keywords: &["cli", "tool", "daemon", "parser", "fast", "systems"],
        stack: "rust",
        reason: "deterministic CLI/systems work; single static binary",
    },
    Rule {
        keywords: &["script", "data", "ml", "scrape", "api", "automation"],
        stack: "python",
        reason: "quickest path for scripting/data/automation; rich libraries",
    },
];

// Scoped tight on purpose: a stack name + one reason, explicitly no code, so the model doesn't
// waste a long reasoning pass writing init scripts we throw away.
const SYSTEM: &str = "You name the single best application tech stack for a developer's task. \
Output ONLY minified JSON with exactly two keys: stack (short lowercase stack name) and reason \
(one concise sentence). Do NOT output code, commands, file contents, install steps, markdown, \
or any other field. Just the stack and why.";

pub fn plan(task: &str, llm: Option<&LlmConfig>) -> Result<StackPlan, String> {
    match llm {
        Some(cfg) => llm_plan(task, cfg),
        None => Ok(heuristic_plan(task)),
    }
}

pub fn heuristic_plan(task: &str) -> StackPlan {
    let t = task.to_ascii_lowercase();
    for r in RULES {
        if r.keywords.iter().any(|k| t.contains(k)) {
            return StackPlan {
                task: task.to_string(),
                stack: r.stack.to_string(),
                reason: r.reason.to_string(),
            };
        }
    }
    // Default: when nothing matches, Python is the lowest-friction starting point.
    StackPlan {
        task: task.to_string(),
        stack: "python".to_string(),
        reason: "no strong signal in the task; Python is the lowest-friction default".to_string(),
    }
}

fn llm_plan(task: &str, cfg: &LlmConfig) -> Result<StackPlan, String> {
    let body = serde_json::json!({
        "model": cfg.model,
        "temperature": 0.1,
        "stream": true,
        "messages": [
            {"role": "system", "content": SYSTEM},
            {"role": "user", "content": format!("Task: {task}")},
        ],
    });
    let resp = ureq::post(&cfg.url)
        .set("Authorization", &format!("Bearer {}", cfg.key))
        .set("Content-Type", "application/json")
        .send_json(body)
        .map_err(|e| format!("request failed: {e}"))?;

    let content = stream_content(resp)?;
    let raw = parse_llm_plan(&content)?;
    if raw.stack.trim().is_empty() {
        return Err("response missing stack".into());
    }
    if raw.reason.trim().is_empty() {
        return Err("response missing reason".into());
    }
    Ok(StackPlan {
        task: task.to_string(),
        stack: raw.stack.trim().to_string(),
        reason: raw.reason.trim().to_string(),
    })
}

/// Read an OpenAI-compatible SSE stream: print the model's reasoning to stderr live (so the user
/// sees thinking while waiting) and accumulate the answer `content` for parsing. stdout stays clean
/// for the final JSON.
fn stream_content(resp: ureq::Response) -> Result<String, String> {
    let mut err = std::io::stderr();
    let reader = BufReader::new(resp.into_reader());
    let mut content = String::new();
    let mut thinking = false;
    for line in reader.lines() {
        let line = line.map_err(|e| format!("stream read: {e}"))?;
        let data = match line.strip_prefix("data:") {
            Some(d) => d.trim(),
            None => continue,
        };
        if data == "[DONE]" {
            break;
        }
        let v: serde_json::Value = match serde_json::from_str(data) {
            Ok(v) => v,
            Err(_) => continue, // keep-alive or partial line; skip
        };
        let delta = &v["choices"][0]["delta"];
        // Reasoning models stream their chain-of-thought in `reasoning_content`; show it live.
        if let Some(r) = delta["reasoning_content"].as_str() {
            if !r.is_empty() {
                if !thinking {
                    let _ = write!(err, "thinking: ");
                    thinking = true;
                }
                let _ = write!(err, "{r}");
                let _ = err.flush();
            }
        }
        if let Some(c) = delta["content"].as_str() {
            content.push_str(c);
        }
    }
    if thinking {
        let _ = writeln!(err);
    }
    Ok(content)
}

fn parse_llm_plan(content: &str) -> Result<LlmStackPlan, String> {
    let start = content.find('{').ok_or("no JSON object in llm output")?;
    let end = content.rfind('}').ok_or("no JSON object in llm output")?;
    serde_json::from_str(&content[start..=end]).map_err(|e| format!("JSON parse: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_keyword_picks_stack() {
        assert_eq!(heuristic_plan("build a music player app").stack, "tauri");
        assert_eq!(heuristic_plan("portfolio site").stack, "next.js");
        assert_eq!(heuristic_plan("e-commerce site").stack, "next.js");
        assert_eq!(heuristic_plan("a fast CLI tool").stack, "rust");
        assert_eq!(heuristic_plan("scrape some data").stack, "python");
    }

    #[test]
    fn unknown_defaults_to_python() {
        assert_eq!(heuristic_plan("zzzqqq").stack, "python");
    }

    #[test]
    fn parses_fenced_llm_plan() {
        let p = parse_llm_plan(
            "```json\n{\"stack\":\"next.js\",\"reason\":\"good web default\"}\n```",
        )
        .unwrap();
        assert_eq!(p.stack, "next.js");
        assert_eq!(p.reason, "good web default");
    }
}
