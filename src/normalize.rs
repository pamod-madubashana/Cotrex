//! Stream Normalizer: RTK emits plain text lines; Tokex tags each as a typed event with severity.

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Channel {
    Stdout,
    Stderr,
}

impl Channel {
    fn as_str(self) -> &'static str {
        match self {
            Channel::Stdout => "stdout",
            Channel::Stderr => "stderr",
        }
    }
}

/// One normalized line of RTK output.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LineEvent {
    /// "stdout" | "stderr"
    #[serde(rename = "type")]
    pub channel: &'static str,
    pub line: String,
    pub severity: Severity,
}

/// Classify a line by keyword. Case-insensitive substring match — deliberately blunt.
/// ponytail: keyword heuristic; replace with per-tool parsers only if severity matters downstream.
pub fn classify(line: &str) -> Severity {
    let l = line.to_ascii_lowercase();
    if l.contains("error") || l.contains("failed") || l.contains("panic") || l.contains("fatal") {
        Severity::Error
    } else if l.contains("warning") || l.contains("warn") {
        Severity::Warning
    } else {
        Severity::Info
    }
}

pub fn normalize(channel: Channel, line: String) -> LineEvent {
    let severity = classify(&line);
    LineEvent {
        channel: channel.as_str(),
        line,
        severity,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_classifier() {
        assert_eq!(classify("error: cannot find crate serde"), Severity::Error);
        assert_eq!(classify("test result: FAILED"), Severity::Error);
        assert_eq!(classify("warning: unused import"), Severity::Warning);
        assert_eq!(classify("Compiling tokex v0.1.0"), Severity::Info);
    }

    #[test]
    fn normalize_tags_channel() {
        let e = normalize(Channel::Stderr, "panic at the disco".into());
        assert_eq!(e.channel, "stderr");
        assert_eq!(e.severity, Severity::Error);
    }
}
