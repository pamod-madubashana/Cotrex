//! AEM — Agent Execution Middleware.
//! A deterministic RTK orchestration layer: normalize agent intent, forward to RTK, normalize
//! the stream. AEM does not own execution; RTK does.

mod intent;
mod llm;
mod normalize;
mod orchestrate;
mod plan;

use std::io::{self, IsTerminal, Read};
use std::process::exit;

use clap::{Parser, Subcommand};

use intent::Intent;

#[derive(Parser)]
#[command(name = "aem", version, about = "Deterministic RTK orchestration layer for AI agents")]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run a command through RTK and stream normalized events.
    Run {
        /// Compress output into a compact LLM insight (needs AEM_LLM_URL/AEM_LLM_KEY).
        #[arg(long)]
        llm: bool,
        /// The command line, e.g. "cargo test".
        command: String,
    },
    /// Recommend a tech stack for a task.
    PlanStack {
        /// Free-text task description.
        task: String,
    },
}

fn main() {
    let cli = Cli::parse();
    let mut out = io::stdout();
    let mut err = io::stderr();

    let intent = match cli.cmd {
        Some(Cmd::Run { llm, command }) => {
            let mut i = Intent::from_command(command);
            i.llm = llm;
            i
        }
        Some(Cmd::PlanStack { task }) => {
            let p = plan::plan(&task);
            println!("{}", serde_json::to_string_pretty(&p).unwrap());
            return;
        }
        // No subcommand: read an intent as JSON from stdin (pipe mode).
        None => {
            if io::stdin().is_terminal() {
                eprintln!("usage: aem run \"<cmd>\" | aem plan-stack \"<task>\" | echo '<intent json>' | aem");
                exit(2);
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

    // Load LLM config only when requested; fail fast on missing setup rather than after running.
    let llm_cfg = if intent.llm {
        match llm::LlmConfig::from_env() {
            Some(c) => Some(c),
            None => {
                eprintln!("aem: --llm requires AEM_LLM_URL and AEM_LLM_KEY (set them or add a .env; see README)");
                exit(2);
            }
        }
    } else {
        None
    };

    match orchestrate::run(&intent, &mut out, &mut err, llm_cfg.as_ref()) {
        Ok(code) => exit(code),
        Err(e) => {
            eprintln!("aem: {e}");
            exit(1);
        }
    }
}
