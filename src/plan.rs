//! Lightweight stack planner: keyword -> recommended stack + init commands.
//! ponytail: a static keyword table covers the common asks; `--llm` upgrade is a v2 swap.

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct StackPlan {
    pub task: String,
    pub stack: &'static str,
    pub reason: &'static str,
    pub init_commands: Vec<&'static str>,
}

struct Rule {
    keywords: &'static [&'static str],
    stack: &'static str,
    reason: &'static str,
    init: &'static [&'static str],
}

const RULES: &[Rule] = &[
    Rule {
        keywords: &["website", "web app", "webapp", "dashboard", "landing"],
        stack: "next.js",
        reason: "web UI with SSR; largest ecosystem and fastest to ship",
        init: &["npx create-next-app@latest"],
    },
    Rule {
        keywords: &["desktop", "music player", "player", "native app", "tray"],
        stack: "tauri",
        reason: "cross-platform desktop with Rust core + web UI; small binaries",
        init: &["npm create tauri-app@latest"],
    },
    Rule {
        keywords: &["mobile", "ios", "android", "cross-platform app"],
        stack: "flutter",
        reason: "single codebase for iOS + Android with native performance",
        init: &["flutter create app"],
    },
    Rule {
        keywords: &["cli", "tool", "daemon", "parser", "fast", "systems"],
        stack: "rust",
        reason: "deterministic CLI/systems work; single static binary",
        init: &["cargo init"],
    },
    Rule {
        keywords: &["script", "data", "ml", "scrape", "api", "automation"],
        stack: "python",
        reason: "quickest path for scripting/data/automation; rich libraries",
        init: &["python -m venv .venv"],
    },
];

pub fn plan(task: &str) -> StackPlan {
    let t = task.to_ascii_lowercase();
    for r in RULES {
        if r.keywords.iter().any(|k| t.contains(k)) {
            return StackPlan {
                task: task.to_string(),
                stack: r.stack,
                reason: r.reason,
                init_commands: r.init.to_vec(),
            };
        }
    }
    // Default: when nothing matches, Python is the lowest-friction starting point.
    StackPlan {
        task: task.to_string(),
        stack: "python",
        reason: "no strong signal in the task; Python is the lowest-friction default",
        init_commands: vec!["python -m venv .venv"],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_keyword_picks_stack() {
        assert_eq!(plan("build a music player app").stack, "tauri");
        assert_eq!(plan("a fast CLI tool").stack, "rust");
        assert_eq!(plan("scrape some data").stack, "python");
    }

    #[test]
    fn unknown_defaults_to_python() {
        assert_eq!(plan("zzzqqq").stack, "python");
    }
}
