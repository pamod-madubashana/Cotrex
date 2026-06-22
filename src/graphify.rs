//! graphify integration: tokex keeps the code map fresh so agents only ever *read* it
//! (`graphify-out/GRAPH_REPORT.md`, `graphify-out/wiki/`) and never spend a turn updating it.
//!
//! graphify is a Python tool (`pip install graphifyy`, run via `python -m graphify`). Updates are
//! AST-only — no LLM/token cost. tokex auto-installs it once and refreshes in the background after
//! code-changing runs. Everything here is best-effort: it never blocks or fails a tokex run.

use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Does this command plausibly change source the map should re-read?
/// ponytail: a skiplist of obvious read-only commands; everything else triggers an update. Not
/// precise change detection — upgrade to a git-diff check if redundant updates ever bite.
pub fn touches_code(command: &str) -> bool {
    let mut t = command.split_whitespace();
    let first = t.next().unwrap_or("");
    let second = t.next().unwrap_or("");
    const READ_ONLY: &[&str] =
        &["ls", "tree", "cat", "echo", "pwd", "which", "find", "grep", "wc", "head", "tail", "env"];
    if READ_ONLY.contains(&first) {
        return false;
    }
    if matches!(first, "git" | "gh")
        && matches!(
            second,
            "status" | "log" | "diff" | "show" | "branch" | "remote" | "fetch" | "blame" | "ls-files"
        )
    {
        return false;
    }
    true
}

/// `python` or `python3`, whichever responds.
fn py() -> &'static str {
    if run_quiet("python", &["--version"]) {
        "python"
    } else {
        "python3"
    }
}

fn run_quiet(prog: &str, args: &[&str]) -> bool {
    Command::new(prog)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn marker() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("tokex").join(".graphify-ok"))
}

fn write_marker() {
    if let Some(m) = marker() {
        if let Some(p) = m.parent() {
            let _ = std::fs::create_dir_all(p);
        }
        let _ = std::fs::write(&m, b"ok");
    }
}

/// One-time bootstrap (cached via a marker so it runs once): make graphifyy importable
/// (auto-`pip install`), then register the graphify skill with the agent (`graphify install`) so the
/// assistant knows to read the map. Returns false if Python/pip can't provide graphify.
fn ensure(py: &str, verbose: bool) -> bool {
    if marker().map(|m| m.exists()).unwrap_or(false) {
        return true;
    }
    let mut importable = run_quiet(py, &["-c", "import graphify"]);
    if !importable {
        if verbose {
            eprintln!("tokex: installing graphifyy (one-time) …");
        }
        importable = run_quiet(py, &["-m", "pip", "install", "--quiet", "graphifyy"])
            && run_quiet(py, &["-c", "import graphify"]);
    }
    if !importable {
        return false;
    }
    // Register the graphify skill with the agent (one-time). Best-effort; inherits this process's
    // stdio (visible under `tokex graph`, silent when the bootstrap runs detached).
    if verbose {
        eprintln!("tokex: registering graphify skill with your agent …");
    }
    let _ = Command::new(py).args(["-m", "graphify", "install"]).status();
    write_marker();
    true
}

/// Best-effort refresh after a code-changing run — never blocks the run.
/// If already set up, fire a cheap incremental `graphify update .` in the background. If not, run
/// the one-time bootstrap detached (via `tokex graph`) so the install/skill-register never stalls
/// the command. ponytail: no lock — a rare double-bootstrap is idempotent.
pub fn auto_update(command: &str) {
    if !touches_code(command) {
        return;
    }
    if marker().map(|m| m.exists()).unwrap_or(false) {
        let _ = Command::new(py())
            .args(["-m", "graphify", "update", "."])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    } else if let Ok(exe) = std::env::current_exe() {
        let _ = Command::new(exe)
            .arg("graph")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }
}

/// `tokex graph`: blocking refresh with visible output; installs graphify if missing.
pub fn update_blocking() -> Result<(), String> {
    let py = py();
    if !ensure(py, true) {
        return Err("graphify unavailable — need Python + pip to install graphifyy".into());
    }
    let ok = Command::new(py)
        .args(["-m", "graphify", "update", "."])
        .status()
        .map_err(|e| e.to_string())?
        .success();
    if ok {
        Ok(())
    } else {
        Err("graphify update failed".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_commands_skip_update() {
        assert!(!touches_code("git status"));
        assert!(!touches_code("ls -la"));
        assert!(!touches_code("git log --oneline"));
    }

    #[test]
    fn building_or_vcs_writes_trigger_update() {
        assert!(touches_code("cargo build"));
        assert!(touches_code("git commit -m x"));
        assert!(touches_code("npm install"));
    }
}
