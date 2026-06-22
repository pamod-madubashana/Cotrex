//! Optional LLM compression. RTK already filters logs; this squeezes them further into a tiny
//! structured insight so the agent reads ~4 fields instead of raw output. Opt-in via `--llm`.
//! Endpoint + key come from the environment (or a local `.env`) — never from code.

use serde::{Deserialize, Serialize};
use std::env;

pub struct LlmConfig {
    pub url: String,
    pub key: String,
    pub model: String,
}

impl LlmConfig {
    /// Read config from the environment (after loading `.env`). Returns None if URL or KEY is unset.
    pub fn from_env() -> Option<Self> {
        load_dotenv();
        let url = env::var("AEM_LLM_URL").ok()?;
        let key = env::var("AEM_LLM_KEY").ok()?;
        if url.trim().is_empty() || key.trim().is_empty() {
            return None;
        }
        let model =
            env::var("AEM_LLM_MODEL").unwrap_or_else(|_| "meta-llama/llama-3.1-8b-instruct".into());
        Some(LlmConfig { url, key, model })
    }
}

/// The compact result the agent consumes instead of full logs.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Insight {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub root_cause: String,
    #[serde(default)]
    pub important_errors: Vec<String>,
    #[serde(default)]
    pub suggested_fix: String,
}

const SYSTEM: &str = "You compress command/build/test logs into a tiny JSON object for another AI \
agent. Output ONLY minified JSON with keys: status (\"ok\" or \"failed\"), root_cause (short \
string), important_errors (array of short strings, max 5), suggested_fix (short string). No \
markdown, no prose.";

/// POST the captured output to an OpenAI-compatible chat endpoint and parse the insight.
pub fn compress(cfg: &LlmConfig, command: &str, exit_code: i32, raw: &str) -> Result<Insight, String> {
    let user = format!("command: {command}\nexit_code: {exit_code}\n--- output ---\n{raw}");
    let body = serde_json::json!({
        "model": cfg.model,
        "temperature": 0,
        "messages": [
            {"role": "system", "content": SYSTEM},
            {"role": "user", "content": user},
        ],
    });
    let resp = ureq::post(&cfg.url)
        .set("Authorization", &format!("Bearer {}", cfg.key))
        .set("Content-Type", "application/json")
        .send_json(body)
        .map_err(|e| format!("request failed: {e}"))?;
    let v: serde_json::Value = resp.into_json().map_err(|e| format!("bad response: {e}"))?;
    let content = v["choices"][0]["message"]["content"]
        .as_str()
        .ok_or("response missing message content")?;
    parse_insight(content)
}

/// Extract the JSON object from model text, tolerating ```json fences or surrounding prose.
fn parse_insight(content: &str) -> Result<Insight, String> {
    let start = content.find('{').ok_or("no JSON object in llm output")?;
    let end = content.rfind('}').ok_or("no JSON object in llm output")?;
    serde_json::from_str(&content[start..=end]).map_err(|e| format!("JSON parse: {e}"))
}

/// Minimal `.env` loader: `K=V` lines in cwd; only sets vars not already in the environment.
/// ponytail: ~12 lines instead of the dotenvy crate; no escaping/multiline — add if ever needed.
fn load_dotenv() {
    let Ok(text) = std::fs::read_to_string(".env") else {
        return;
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, val)) = line.split_once('=') {
            let (k, val) = (k.trim(), val.trim().trim_matches('"'));
            if env::var(k).is_err() {
                env::set_var(k, val);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fenced_json() {
        let out = "```json\n{\"status\":\"failed\",\"root_cause\":\"missing crate serde\",\
\"important_errors\":[\"E0432\"],\"suggested_fix\":\"add serde\"}\n```";
        let i = parse_insight(out).unwrap();
        assert_eq!(i.status, "failed");
        assert_eq!(i.important_errors, vec!["E0432"]);
        assert_eq!(i.suggested_fix, "add serde");
    }

    #[test]
    fn rejects_non_json() {
        assert!(parse_insight("sorry, I can't help with that").is_err());
    }
}
