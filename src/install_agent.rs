use std::fs;
use std::path::{Path, PathBuf};

const SUPPORTED_AGENTS: &[(&str, &str)] = &[
    ("opencode", "opencode"),
    ("claude", "claude"),
    ("codex", "codex"),
    ("cursor", "cursor"),
    ("gemini", "gemini"),
    ("windsurf", "windsurf"),
    ("aider", "aider"),
    ("continue", "continue"),
    ("cline", "cline"),
];

fn is_project_dir(dir: &Path) -> bool {
    const MARKERS: &[&str] = &[
        ".git",
        "Cargo.toml",
        "package.json",
        "pyproject.toml",
        "go.mod",
        "pom.xml",
        "build.gradle",
        "build.gradle.kts",
        "deno.json",
        "composer.json",
        "Gemfile",
    ];
    MARKERS.iter().any(|m| dir.join(m).exists())
}

fn current_project_dir() -> Option<PathBuf> {
    std::env::current_dir().ok().filter(|d| is_project_dir(d))
}

fn graphify_skill(agent: &str, project_name: &str) -> String {
    format!(
        r#"---
name: graphify
description: "any input (code, docs, papers, images) -> knowledge graph -> clustered communities -> HTML + JSON + audit report. Use when user asks any question about a codebase, project content, architecture, or file relationships -- especially if graphify-out/ exists."
trigger: /graphify
---

# /graphify

Turn any folder of files into a navigable knowledge graph with community detection, an honest audit trail, and three outputs: interactive HTML, GraphRAG-ready JSON, and a plain-language GRAPH_REPORT.md.

## Usage

```
/graphify                                             # full pipeline on current directory
/graphify <path>                                      # full pipeline on specific path
/graphify <path> --mode deep                          # thorough extraction
/graphify <path> --update                             # incremental re-extraction
```

## Installed for: {project_name}

Installed by `tokex install {agent}`. Reinstall with `tokex install {agent}`.
"#,
        agent = agent,
        project_name = project_name
    )
}

fn tokex_skill(agent: &str, project_name: &str) -> String {
    format!(
        r#"---
name: tokex
description: "Tokex RTK orchestration skills for {project_name}. Run commands, inspect projects, and get normalized output."
---

# Tokex Skills

**IMPORTANT:** You are an AI agent. Always use `tokex -m` (model mode) to get clean output
without spinners, live streaming, or ANSI decorations. Never use `tokex run` or bare `tokex`
without `-m` — that mode is for humans and will produce noisy, hard-to-parse output.

## Rules

1. Always prefix commands with `tokex -m` — never `tokex run` or bare `tokex`.
2. One command at a time. Feed the result back before running the next.
3. Skip vendor/, target/, .git/ — they're noise.

## Run a command

```bash
tokex -m "cargo test"
tokex -m cargo test           # shorthand
tokex -m "git status"
```

## Show project structure

```bash
tokex -m "show the project tree"
tokex -m "give me the directory layout"
```

## Install skills for this project

```bash
tokex install {agent}         # reinstall/update skills
```

## Installed for: {project_name}

Skills are in `.tokex/skills/`. Your agent detects them automatically.
"#,
        agent = agent,
        project_name = project_name
    )
}

pub fn install_agent(agent: &str) -> Result<(), String> {
    let agent_id = SUPPORTED_AGENTS
        .iter()
        .find(|(name, _)| *name == agent)
        .map(|(_, id)| *id)
        .ok_or_else(|| {
            let names: Vec<&str> = SUPPORTED_AGENTS.iter().map(|(n, _)| *n).collect();
            format!(
                "unsupported agent '{agent}'. Supported: {}",
                names.join(", ")
            )
        })?;

    let project_dir = current_project_dir()
        .ok_or("not in a project directory (no Cargo.toml, package.json, etc.)")?;

    let project_name = project_dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "project".into());

    let tokex_dir = project_dir.join(".tokex");
    let skills_dir = tokex_dir.join("skills");
    fs::create_dir_all(&skills_dir)
        .map_err(|e| format!("failed to create {}: {e}", skills_dir.display()))?;

    fs::write(
        skills_dir.join("graphify.md"),
        graphify_skill(&agent_id, &project_name),
    )
    .map_err(|e| format!("failed to write graphify skill: {e}"))?;

    fs::write(
        skills_dir.join("tokex.md"),
        tokex_skill(&agent_id, &project_name),
    )
    .map_err(|e| format!("failed to write tokex skill: {e}"))?;

    eprintln!(
        "Installed Tokex skills for '{agent_id}' in {}",
        skills_dir.display()
    );
    Ok(())
}

pub fn list_installed() -> Result<(), String> {
    let project_dir = current_project_dir().ok_or("not in a project directory")?;

    let skills_dir = project_dir.join(".tokex").join("skills");
    if !skills_dir.exists() {
        eprintln!("No Tokex skills installed in this project.");
        return Ok(());
    }

    eprintln!("Tokex skills in {}:", project_dir.display());
    if let Ok(entries) = fs::read_dir(&skills_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let s = name.to_string_lossy();
            if let Some(skill_name) = s.strip_suffix(".md") {
                eprintln!("  - {skill_name}");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_agents_list() {
        assert!(SUPPORTED_AGENTS.iter().any(|(n, _)| *n == "opencode"));
        assert!(SUPPORTED_AGENTS.iter().any(|(n, _)| *n == "claude"));
    }

    #[test]
    fn unsupported_agent_errors() {
        assert!(install_agent("nonexistent").is_err());
    }
}
