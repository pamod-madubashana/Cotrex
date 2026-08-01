//! Persistent config + the interactive `cotrex setup` flow.
//!
//! The config lives in the user's config dir (e.g. %APPDATA%\cotrex\config.toml), set *after*
//! install via `cotrex setup` — not a project `.env`. Env vars still override for power users / CI.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    /// off | heuristic — default compression for command output.
    pub compression: String,
    /// normal | ultra-compact — rtk output verbosity.
    pub rtk_verbosity: String,
    /// Keep the graphify code map fresh automatically after code-changing runs.
    pub graph_auto: bool,
    /// graphify platform id for skill registration (e.g. claude, codex, cursor). Blank = auto-detect.
    pub agent: String,
    /// Active model ID for local inference (e.g. qwen2.5-1.5b). Blank = default.
    pub model: String,
    /// Sampling temperature for local inference (0.0 = deterministic, 1.0 = max randomness).
    pub temperature: f64,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            compression: "heuristic".into(),
            rtk_verbosity: "normal".into(),
            graph_auto: true,
            agent: String::new(),
            model: String::new(),
            temperature: 0.7,
        }
    }
}

pub fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("cotrex").join("config.toml"))
}

/// Load config from disk (or defaults), then apply env overrides.
pub fn load() -> Config {
    let mut cfg = config_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| toml::from_str::<Config>(&s).ok())
        .unwrap_or_default();
    if let Ok(v) = std::env::var("COTREX_COMPRESSION") {
        cfg.compression = v;
    }
    if let Ok(v) = std::env::var("COTREX_RTK_VERBOSITY") {
        cfg.rtk_verbosity = v;
    }
    if let Ok(v) = std::env::var("COTREX_GRAPH_AUTO") {
        cfg.graph_auto = v == "true" || v == "1" || v == "yes";
    }
    if let Ok(v) = std::env::var("COTREX_MODEL") {
        cfg.model = v;
    }
    if let Ok(v) = std::env::var("COTREX_TEMPERATURE") {
        if let Ok(t) = v.parse::<f64>() {
            cfg.temperature = t.clamp(0.0, 1.0);
        }
    }
    cfg
}

pub fn save(cfg: &Config) -> Result<PathBuf, String> {
    let path = config_path().ok_or("cannot determine config dir")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let s = toml::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    std::fs::write(&path, s).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Return the active model ID from config, or the default if unset.
pub fn active_model() -> String {
    let cfg = load();
    if cfg.model.is_empty() {
        "qwen3-8b".into()
    } else {
        cfg.model
    }
}

/// Return the active sampling temperature from config.
pub fn active_temperature() -> f64 {
    load().temperature
}

/// Format bytes as human-readable size for display in setup prompts.
fn format_size_display(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.0} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

/// Interactive setup. Pretty prompts via `inquire`; writes the config file.
pub fn run_setup() -> Result<(), String> {
    use inquire::Select;

    let compression = Select::new(
        "Default compression",
        vec!["heuristic (rtk filter)", "off (raw output)"],
    )
    .prompt()
    .map_err(|e| e.to_string())?
    .split_whitespace()
    .next()
    .unwrap()
    .to_string();

    let rtk_verbosity = Select::new("RTK output", vec!["normal", "ultra-compact"])
        .prompt()
        .map_err(|e| e.to_string())?
        .to_string();

    let graph_auto = Select::new(
        "Auto-update the graphify code map after code changes?",
        vec!["Yes", "No"],
    )
    .prompt()
    .map_err(|e| e.to_string())?
        == "Yes";

    let agent = if graph_auto {
        let choice = Select::new(
            "Agent for graphify skill",
            vec![
                "opencode",
                "claude",
                "codex",
                "cursor",
                "gemini",
                "windsurf",
                "aider",
                "continue",
                "cline",
                "custom (type your own)",
                "auto-detect",
            ],
        )
        .prompt()
        .map_err(|e| e.to_string())?;

        match choice {
            "custom (type your own)" => inquire::Text::new("Agent name")
                .prompt()
                .map_err(|e| e.to_string())?
                .trim()
                .to_string(),
            "auto-detect" => String::new(),
            other => other.to_string(),
        }
    } else {
        String::new()
    };

    let model_options = {
        let registry = cotrex_ai_runtime::model_manager::registry::ModelRegistry::built_in();
        let mut opts: Vec<String> = registry
            .models
            .iter()
            .map(|m| {
                format!("{} ({})", m.id, format_size_display(m.size))
            })
            .collect();
        if opts.is_empty() {
            opts.push("qwen3-8b".into());
        }
        opts.push("(keep current)".into());
        opts
    };
    let model_choice = Select::new("Active model for inference", model_options)
        .prompt()
        .map_err(|e| e.to_string())?;
    let model = if model_choice == "(keep current)" {
        load().model
    } else {
        // Extract just the model ID (first token)
        model_choice.split_whitespace().next().unwrap_or("qwen3-8b").to_string()
    };

    let cfg = Config {
        compression,
        rtk_verbosity,
        graph_auto,
        agent,
        model,
        temperature: load().temperature,
    };
    let path = save(&cfg)?;
    eprintln!("Saved config to {}", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_round_trips_through_toml() {
        let cfg = Config {
            compression: "heuristic".into(),
            rtk_verbosity: "ultra-compact".into(),
            graph_auto: true,
            agent: "codex".into(),
            model: String::new(),
            temperature: 0.7,
        };
        let s = toml::to_string_pretty(&cfg).unwrap();
        let back: Config = toml::from_str(&s).unwrap();
        assert_eq!(cfg, back);
    }

    #[test]
    fn defaults_are_safe() {
        let c = Config::default();
        assert_eq!(c.compression, "heuristic");
    }
}
