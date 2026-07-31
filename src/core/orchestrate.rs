//! Execution Orchestrator: validate intent, forward to RTK, normalize the stream.
//!
//! Cotrex does not run the raw command — it spawns `rtk <args>` and reads RTK's pipes. Two reader
//! threads feed an mpsc channel so stdout and stderr interleave live.
//! ponytail: 2 threads + mpsc; swap to async only if we ever multiplex many concurrent execs.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;

use crate::core::intent::Intent;
use crate::core::normalize::{normalize, LineEvent, Severity};

/// Keep the last N output lines for the LLM. RTK already pre-filters, so this is a token guard.
/// ponytail: last 200 lines; raise only if compression misses context past that window.
const RAW_CAP: usize = 200;

#[derive(serde::Serialize)]
struct Result_ {
    #[serde(rename = "type")]
    kind: &'static str,
    status: &'static str,
    code: i32,
}

enum Msg {
    Line(LineEvent),
    Done,
}

/// Execution options derived from config modes.
pub struct Options {
    /// compression=off: bypass rtk filtering (raw `rtk run -c`).
    pub raw: bool,
    /// rtk_verbosity=ultra-compact: pass `--ultra-compact` to rtk.
    pub ultra_compact: bool,
    /// compression=llm: reserved (no-op; LLM compression removed).
    #[allow(dead_code)]
    pub llm_on_failure: bool,
    /// Emit the `{"type":"result", …}` footer on the machine channel. Off for model-mode prompts
    /// where the caller wants the command's output and nothing else.
    pub footer: bool,
    /// Suppress human-facing noise: command echo (`› rtk …`), JSON result footer, and summary
    /// line (`‹ ok …`). On for direct CLI usage where the user just wants the command's output.
    pub quiet: bool,
}

/// Run the intent through RTK. Writes normalized NDJSON events to `machine` (stdout) and a
/// human summary to `human` (stderr). Returns the process exit code.
pub fn run(
    intent: &Intent,
    machine: &mut impl Write,
    human: &mut impl Write,
    opts: &Options,
) -> Result<i32, String> {
    intent.validate()?;
    let mut args = if opts.raw {
        vec![
            "run".to_string(),
            "-c".to_string(),
            intent.command.trim().to_string(),
        ]
    } else {
        intent.to_rtk_args()
    };
    if opts.ultra_compact {
        // `--ultra-compact` is a global flag on rtk's *top-level* CLI. Native filters (git, …)
        // forward trailing hyphen-args to the underlying tool, so appended it leaks into e.g.
        // `git status --short` and git errors. It must precede the subcommand: `rtk --ultra-compact git …`.
        args.insert(0, "--ultra-compact".to_string());
    }

    // PROCESS_START on the human channel only; machine channel is pure line/result events.
    if !opts.quiet {
        writeln!(human, "› rtk {}", args.join(" ")).ok();
    }

    let rtk = crate::config::install::ensure_rtk()?;
    let mut child = Command::new(&rtk)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn rtk at {}: {e}", rtk.display()))?;

    let (tx, rx) = mpsc::channel();
    let reader = |pipe: Option<Box<dyn std::io::Read + Send>>, tx: mpsc::Sender<Msg>| {
        thread::spawn(move || {
            if let Some(p) = pipe {
                for line in BufReader::new(p).lines() {
                    match line {
                        Ok(l) => {
                            tx.send(Msg::Line(normalize(l))).ok();
                        }
                        Err(_) => break,
                    }
                }
            }
            tx.send(Msg::Done).ok();
        })
    };

    let out = child
        .stdout
        .take()
        .map(|p| Box::new(p) as Box<dyn std::io::Read + Send>);
    let err = child
        .stderr
        .take()
        .map(|p| Box::new(p) as Box<dyn std::io::Read + Send>);
    reader(out, tx.clone());
    reader(err, tx);

    let mut errors = 0usize;
    let mut raw: Vec<String> = Vec::new();
    let mut open = 2;
    while open > 0 {
        match rx.recv() {
            Ok(Msg::Line(ev)) => {
                if ev.severity == Severity::Error {
                    errors += 1;
                }
                // Pass the line through verbatim — raw output is the whole point of compression.
                // Status/severity ride on the single result footer (and the insight on failure).
                writeln!(machine, "{}", ev.line).ok();
                raw.push(ev.line);
                if raw.len() > RAW_CAP {
                    raw.remove(0);
                }
            }
            Ok(Msg::Done) => open -= 1,
            Err(_) => break,
        }
    }

    let code = child
        .wait()
        .map_err(|e| format!("wait failed: {e}"))?
        .code()
        .unwrap_or(-1);
    let status = if code == 0 { "ok" } else { "failed" };
    if opts.footer && !opts.quiet {
        let result = Result_ {
            kind: "result",
            status,
            code,
        };
        writeln!(machine, "{}", serde_json::to_string(&result).unwrap()).ok();
    }
    if !opts.quiet {
        writeln!(human, "‹ {status} (exit {code}, {errors} error line(s))").ok();
    }
    Ok(code)
}
