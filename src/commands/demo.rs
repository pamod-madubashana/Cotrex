//! `cotrex --demo` — runtime microscope.
//!
//! Proves the full tool execution path works without an LLM:
//! stdin JSON → parse_decision → resolve → validate → permission → execute → output.

use std::io::Write;

use crate::agent::permission::Permissions;
use crate::agent::prompt::{parse_decision, Decision};
use crate::agent::tool::{validate_args, ToolContext};

pub fn run() {
    println!(
        "Cotrex Runtime v{} (demo mode)\n",
        env!("CARGO_PKG_VERSION")
    );

    let stdin = std::io::stdin();
    let perms = Permissions::default();

    loop {
        eprint!("> ");
        let _ = std::io::stderr().flush();

        let mut input = String::new();
        if stdin.read_line(&mut input).unwrap_or(0) == 0 {
            break;
        }
        let input = input.trim();
        if input == "exit" || input.is_empty() {
            break;
        }

        match parse_decision(input) {
            Decision::Tool { tool, args, .. } => {
                println!("\nTool: {}", tool.name);
                println!("Args: {args}\n");

                // Validate
                if let Err(e) = validate_args(tool, &args) {
                    println!("Validation error: {e}\n");
                    continue;
                }

                // Permission (real path, no bypass)
                match perms.evaluate(tool.name, None) {
                    crate::agent::permission::Action::Deny => {
                        println!("Permission denied by policy.\n");
                        continue;
                    }
                    crate::agent::permission::Action::Ask => {
                        eprint!("  {}? (y/n) ", tool.description);
                        let _ = std::io::stderr().flush();
                        let mut confirm = String::new();
                        if std::io::stdin().read_line(&mut confirm).unwrap_or(0) == 0
                            || !matches!(confirm.trim().to_ascii_lowercase().as_str(), "y" | "yes")
                        {
                            println!("Permission denied.\n");
                            continue;
                        }
                    }
                    crate::agent::permission::Action::Allow => {}
                }

                // Execute
                let ctx = ToolContext {
                    workdir: std::env::current_dir().unwrap(),
                };
                match (tool.execute)(&ctx, &args) {
                    Ok(output) => println!("Result:\n{output}\n"),
                    Err(e) => println!("Error: {e}\n"),
                }
            }
            Decision::Answer(text) => {
                println!("{text}\n");
            }
            _ => {
                println!("Expected JSON: {{\"tool\":\"<name>\",\"args\":{{...}}}}\n");
            }
        }
    }
}
