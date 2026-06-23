//! Category-driven prompts.
//!
//! A single quoted argument is a *prompt*, not a command: `tokex "plan-stack: media player"`.
//! The part before the colon is a **category**; each category binds to a *header* (a system prompt)
//! that's prepended to the agent's text. With no known category the text is sent under a default
//! header. Several categories at once go through JSON: `tokex '{"plan-stack":"…","theme":"…"}'`.
//!
//! Every call is streamed so the model's thinking shows live on stderr while the user waits; stdout
//! stays machine-clean for the final JSON answer.

use std::io::{BufRead, BufReader, Write};

use crate::llm::LlmConfig;

// Each row binds a category to its header (system prompt). Add a row to add a category.
const CATEGORIES: &[(&str, &str)] = &[
    (
        "plan-stack",
        "You name the single best application tech stack for a developer's task. Output ONLY \
minified JSON with exactly two keys: stack (short lowercase stack name) and reason (one concise \
sentence). Do NOT output code, commands, file contents, install steps, markdown, or any other field.",
    ),
    (
        "theme",
        "You are a senior UI designer. Given a short style description, output ONLY minified JSON \
with keys: palette (array of hex colors), font (string), effects (array of short phrases), \
rationale (one concise sentence). No code, no markdown, no other fields.",
    ),
];

// Used when a prompt has no recognized category.
const DEFAULT_HEADER: &str = "You are a concise senior software engineer. Answer the developer's \
question briefly and practically. No preamble, no markdown headings.";

/// The header (system prompt) bound to a category, if it is known.
pub fn header(category: &str) -> Option<&'static str> {
    CATEGORIES.iter().find(|(n, _)| *n == category).map(|(_, h)| *h)
}

/// How a single bare argument should be handled.
#[derive(Debug, PartialEq)]
pub enum Dispatch {
    /// JSON object of `category -> text` (possibly several). Pass the raw string to `parse_json`.
    Json(String),
    /// `category: text` with a known category.
    Category(String, String),
    /// Free-text prompt with no category.
    Prompt(String),
    /// A single bare token — run it as a command, not a prompt.
    Command(String),
}

/// Classify one argument. Quotes (i.e. a single arg) reach here; multi-arg invocations are commands
/// and never get classified.
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
    // Whitespace means it reads as a sentence → a prompt; a lone token is a command (e.g. `ls`).
    if s.split_whitespace().count() > 1 {
        Dispatch::Prompt(s.to_string())
    } else {
        Dispatch::Command(s.to_string())
    }
}

/// Parse the JSON multi-category form into `(category, text)` pairs. Accepts a flat object
/// `{"plan-stack":"…","theme":"…"}` or a `{"task": { … }}` wrapper.
pub fn parse_json(s: &str) -> Result<Vec<(String, String)>, String> {
    let v: serde_json::Value =
        serde_json::from_str(s).map_err(|e| format!("invalid JSON prompt: {e}"))?;
    let obj = match v.get("task").and_then(|t| t.as_object()) {
        Some(o) => o.clone(),
        None => v.as_object().ok_or("JSON prompt must be an object")?.clone(),
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

/// Run every `(category, text)` pair through the LLM and collect the answers into one JSON object
/// keyed by category (`answer` when there's no category). Streams thinking to stderr as it goes.
pub fn run(pairs: &[(String, String)], cfg: &LlmConfig) -> Result<serde_json::Value, String> {
    let mut results = serde_json::Map::new();
    for (cat, text) in pairs {
        let system = if cat.is_empty() {
            DEFAULT_HEADER
        } else {
            header(cat).ok_or_else(|| format!("unknown category '{cat}'"))?
        };
        let answer = one_call(cfg, system, text, cat)?;
        let key = if cat.is_empty() { "answer" } else { cat.as_str() };
        results.insert(key.to_string(), as_value(&answer));
    }
    Ok(serde_json::Value::Object(results))
}

/// Embed the model's answer as parsed JSON when it returned an object, else as a plain string.
fn as_value(answer: &str) -> serde_json::Value {
    if let (Some(a), Some(b)) = (answer.find('{'), answer.rfind('}')) {
        if a < b {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&answer[a..=b]) {
                return v;
            }
        }
    }
    serde_json::Value::String(answer.trim().to_string())
}

fn one_call(cfg: &LlmConfig, system: &str, user: &str, label: &str) -> Result<String, String> {
    let body = serde_json::json!({
        "model": cfg.model,
        "temperature": 0.2,
        "stream": true,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user},
        ],
    });
    let resp = ureq::post(&cfg.url)
        .set("Authorization", &format!("Bearer {}", cfg.key))
        .set("Content-Type", "application/json")
        .send_json(body)
        .map_err(|e| format!("request failed: {e}"))?;
    stream_content(resp, label)
}

/// Read an OpenAI-compatible SSE stream: print the model's reasoning to stderr live (thinking while
/// waiting) and accumulate the answer `content` for the caller. stdout is never touched here.
fn stream_content(resp: ureq::Response, label: &str) -> Result<String, String> {
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
                    let tag = if label.is_empty() {
                        "thinking: ".to_string()
                    } else {
                        format!("thinking [{label}]: ")
                    };
                    let _ = write!(err, "{tag}");
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
            classify("find python lib for web scrap"),
            Dispatch::Prompt("find python lib for web scrap".into())
        );
        assert_eq!(classify("ls"), Dispatch::Command("ls".into()));
        // Quoted command string reads as a prompt (no known category).
        assert_eq!(
            classify("git status"),
            Dispatch::Prompt("git status".into())
        );
        match classify("{\"plan-stack\":\"x\"}") {
            Dispatch::Json(_) => {}
            other => panic!("expected Json, got {other:?}"),
        }
        // Unknown category before the colon is just prose, not a category.
        assert_eq!(
            classify("note: refactor later"),
            Dispatch::Prompt("note: refactor later".into())
        );
    }

    #[test]
    fn parse_json_flat_and_wrapped() {
        let flat = parse_json(r#"{"plan-stack":"media player","theme":"glass"}"#).unwrap();
        assert_eq!(flat.len(), 2);
        let wrapped = parse_json(r#"{"task":{"plan-stack":"media player"}}"#).unwrap();
        assert_eq!(wrapped, vec![("plan-stack".to_string(), "media player".to_string())]);
        assert!(parse_json("[]").is_err());
        assert!(parse_json("{}").is_err());
    }

    #[test]
    fn as_value_parses_json_or_keeps_string() {
        assert!(as_value(r#"here: {"stack":"rust","reason":"fast"}"#).is_object());
        assert_eq!(as_value("just text"), serde_json::Value::String("just text".into()));
    }

    #[test]
    fn known_categories_have_headers() {
        assert!(header("plan-stack").is_some());
        assert!(header("theme").is_some());
        assert!(header("nope").is_none());
    }
}
