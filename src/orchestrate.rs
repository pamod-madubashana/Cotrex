//! Execution Orchestrator: validate intent, forward to RTK, normalize the stream.
//!
//! AEM does not run the raw command — it spawns `rtk <args>` and reads RTK's pipes. Two reader
//! threads feed an mpsc channel so stdout and stderr interleave live.
//! ponytail: 2 threads + mpsc; swap to async only if we ever multiplex many concurrent execs.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;

use crate::intent::Intent;
use crate::normalize::{normalize, Channel, LineEvent, Severity};

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

/// Run the intent through RTK. Writes normalized NDJSON events to `machine` (stdout) and a
/// human summary to `human` (stderr). Returns the process exit code.
pub fn run(intent: &Intent, machine: &mut impl Write, human: &mut impl Write) -> Result<i32, String> {
    intent.validate()?;
    let args = intent.to_rtk_args();

    // PROCESS_START on the human channel only; machine channel is pure line/result events.
    writeln!(human, "› rtk {}", args.join(" ")).ok();

    let mut child = Command::new("rtk")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn rtk (is it on PATH?): {e}"))?;

    let (tx, rx) = mpsc::channel();
    let reader = |ch: Channel, pipe: Option<Box<dyn std::io::Read + Send>>, tx: mpsc::Sender<Msg>| {
        thread::spawn(move || {
            if let Some(p) = pipe {
                for line in BufReader::new(p).lines() {
                    match line {
                        Ok(l) => {
                            tx.send(Msg::Line(normalize(ch, l))).ok();
                        }
                        Err(_) => break,
                    }
                }
            }
            tx.send(Msg::Done).ok();
        })
    };

    let out = child.stdout.take().map(|p| Box::new(p) as Box<dyn std::io::Read + Send>);
    let err = child.stderr.take().map(|p| Box::new(p) as Box<dyn std::io::Read + Send>);
    reader(Channel::Stdout, out, tx.clone());
    reader(Channel::Stderr, err, tx);

    let mut errors = 0usize;
    let mut open = 2;
    while open > 0 {
        match rx.recv() {
            Ok(Msg::Line(ev)) => {
                if ev.severity == Severity::Error {
                    errors += 1;
                }
                writeln!(machine, "{}", serde_json::to_string(&ev).unwrap()).ok();
            }
            Ok(Msg::Done) => open -= 1,
            Err(_) => break,
        }
    }

    let code = child.wait().map_err(|e| format!("wait failed: {e}"))?.code().unwrap_or(-1);
    let status = if code == 0 { "ok" } else { "failed" };
    let result = Result_ { kind: "result", status, code };
    writeln!(machine, "{}", serde_json::to_string(&result).unwrap()).ok();
    writeln!(human, "‹ {status} (exit {code}, {errors} error line(s))").ok();
    Ok(code)
}
