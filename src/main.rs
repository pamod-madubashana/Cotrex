//! Tokex.
//! A deterministic RTK orchestration layer: normalize agent intent, forward to RTK, normalize
//! the stream. Tokex does not own execution; RTK does.

mod config;
mod graphify;
mod install;
mod intent;
mod llm;
mod mcp;
mod normalize;
mod orchestrate;
mod prompt;

use std::io::{self, IsTerminal, Read};
use std::process::exit;

use clap::{CommandFactory, Parser, Subcommand};

use intent::Intent;

#[derive(Parser)]
#[command(
    name = "tokex",
    version,
    about = "Deterministic RTK orchestration layer for AI agents",
    after_help = "Stdin mode: pipe a JSON intent instead of a subcommand, e.g.\n  echo '{\"tool\":\"rtk\",\"cmd\":\"git status\"}' | tokex"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run a command through RTK and stream normalized events.
    Run {
        /// Force the LLM insight on for this run (overrides the configured compression mode).
        #[arg(long)]
        llm: bool,
        /// The command line, e.g. "cargo test".
        command: String,
    },
    /// Interactive setup: choose provider, enter API key, pick modes.
    Setup,
    /// Run as an MCP server over stdio (for agents that call tools natively).
    Mcp,
    /// Pre-fetch the pinned rtk release for this OS (also happens automatically on first run).
    InstallRtk,
    /// Refresh the graphify code map now (`graphify update .`).
    Graph,
}

/// Top-level subcommands. Anything else as the first arg is treated as a command to run, so
/// `tokex git status` works like `tokex run "git status"` (mirrors how rtk itself is invoked).
const SUBCOMMANDS: &[&str] = &["run", "setup", "mcp", "install-rtk", "graph", "help"];

fn main() {
    // Two bare forms when the first arg isn't a subcommand/flag:
    //   tokex git status         -> several args -> a command, run through rtk
    //   tokex "plan-stack: foo"  -> one arg      -> a prompt (see prompt::classify)
    // Quoting is the signal: a quoted string is one arg, so `tokex "git status"` is a prompt.
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).is_some_and(|f| is_passthrough(f)) {
        let rest = &args[1..];
        if rest.len() >= 2 {
            run_intent(Intent::from_command(rest.join(" ")));
        } else {
            dispatch_one(&rest[0]);
        }
        return;
    }

    let cli = Cli::parse();

    let intent = match cli.cmd {
        Some(Cmd::Run { llm, command }) => {
            let mut i = Intent::from_command(command);
            i.llm = llm;
            i
        }
        Some(Cmd::Setup) => {
            if let Err(e) = config::run_setup() {
                eprintln!("tokex: setup failed: {e}");
                exit(1);
            }
            // Bootstrap graphify (install + register skill for the chosen agent + build map) now.
            if config::load().graph_auto {
                if let Err(e) = graphify::update_blocking_after_setup() {
                    eprintln!("tokex: graphify setup skipped: {e}");
                }
            }
            return;
        }
        Some(Cmd::Mcp) => mcp::serve(),
        Some(Cmd::InstallRtk) => {
            match install::install() {
                Ok(path) => println!("rtk installed at {}", path.display()),
                Err(e) => {
                    eprintln!("tokex: install-rtk failed: {e}");
                    exit(1);
                }
            }
            return;
        }
        Some(Cmd::Graph) => {
            if let Err(e) = graphify::update_blocking() {
                eprintln!("tokex: graph update failed: {e}");
                exit(1);
            }
            return;
        }
        // No subcommand: read an intent as JSON from stdin (pipe mode).
        None => {
            // No subcommand and interactive: show full help rather than a cryptic usage line.
            if io::stdin().is_terminal() {
                Cli::command().print_help().ok();
                println!();
                exit(0);
            }
            let mut buf = String::new();
            if io::stdin().read_to_string(&mut buf).is_err() || buf.trim().is_empty() {
                eprintln!("no intent on stdin");
                exit(2);
            }
            match Intent::from_json(buf.trim()) {
                Ok(i) => i,
                Err(e) => {
                    eprintln!("{e}");
                    exit(2);
                }
            }
        }
    };

    run_intent(intent);
}

/// Shared run tail: apply config modes, orchestrate through rtk, exit with its code. Used by the
/// `run` subcommand, stdin-JSON mode, and the bare `tokex <command>` passthrough.
fn run_intent(intent: Intent) {
    let mut out = io::stdout();
    let mut err = io::stderr();

    // Apply configured modes. `--llm` / JSON `"llm": true` force the insight on always; the `llm`
    // compression mode only analyzes failures (a successful command stays token-free).
    let cfg = config::load();
    let opts = orchestrate::Options {
        raw: cfg.compression == "off",
        ultra_compact: cfg.rtk_verbosity == "ultra-compact",
        llm_on_failure: cfg.compression == "llm",
    };

    // Load LLM config when it could be used. Fail fast only when `--llm` explicitly demanded it.
    let llm_cfg = if intent.llm || opts.llm_on_failure {
        match llm::LlmConfig::from_config(&cfg) {
            Some(c) => Some(c),
            None if intent.llm => {
                eprintln!("tokex: LLM compression needs an API key — run `tokex setup`");
                exit(2);
            }
            None => None,
        }
    } else {
        None
    };

    match orchestrate::run(&intent, &mut out, &mut err, llm_cfg.as_ref(), &opts) {
        Ok(code) => {
            if cfg.graph_auto {
                graphify::auto_update(&intent.command);
            }
            exit(code);
        }
        Err(e) => {
            eprintln!("tokex: {e}");
            exit(1);
        }
    }
}

/// A first arg is not a subcommand (so it's a command or a prompt) when it isn't a flag and isn't
/// one of our known subcommands. ponytail: collisions (a binary named `run`) lose to the subcommand.
fn is_passthrough(first: &str) -> bool {
    !first.starts_with('-') && !SUBCOMMANDS.contains(&first)
}

/// Handle a single bare argument: a JSON / `category: text` / free-text prompt, or a lone command.
fn dispatch_one(arg: &str) {
    match prompt::classify(arg) {
        prompt::Dispatch::Command(cmd) => run_intent(Intent::from_command(cmd)),
        prompt::Dispatch::Json(s) => match prompt::parse_json(&s) {
            Ok(pairs) => run_prompt(pairs),
            Err(e) => {
                eprintln!("tokex: {e}");
                exit(2);
            }
        },
        prompt::Dispatch::Category(cat, text) => run_prompt(vec![(cat, text)]),
        prompt::Dispatch::Prompt(text) => run_prompt(vec![(String::new(), text)]),
    }
}

/// Run category prompts through the LLM and print the combined JSON answer to stdout (thinking
/// streams to stderr inside `prompt::run`).
fn run_prompt(pairs: Vec<(String, String)>) -> ! {
    let cfg = config::load();
    let llm_cfg = match llm::LlmConfig::from_config(&cfg) {
        Some(c) => c,
        None => {
            eprintln!("tokex: prompts need an API key — run `tokex setup`");
            exit(2);
        }
    };
    match prompt::run(&pairs, &llm_cfg) {
        Ok(v) => {
            println!("{}", serde_json::to_string_pretty(&v).unwrap());
            exit(0);
        }
        Err(e) => {
            eprintln!("tokex: {e}");
            exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passthrough_routes_commands_not_subcommands() {
        assert!(is_passthrough("git"));
        assert!(is_passthrough("ls"));
        assert!(!is_passthrough("run"));
        assert!(!is_passthrough("setup"));
        assert!(!is_passthrough("--help"));
        assert!(!is_passthrough("-V"));
    }
}
