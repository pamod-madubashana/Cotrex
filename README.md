<p align="center">
  <img src="assets/cotrex.png" alt="Cotrex" width="220">
</p>

<p align="center">
  <strong>Run terminal commands safely through your AI agent</strong>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/built_with-Rust-orange.svg" alt="Built with Rust">
  <img src="https://img.shields.io/badge/version-3.0.0-blue.svg" alt="Version 3.0.0">
  <img src="https://img.shields.io/github/actions/workflow/status/pamod-madubashana/Cotrex/ci.yml?branch=main&label=CI" alt="CI">
  <img src="https://img.shields.io/github/v/release/pamod-madubashana/Cotrex" alt="Latest Release">
  <img src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey.svg" alt="Platforms">
  <img src="https://img.shields.io/badge/edition-2021-purple.svg" alt="Rust 2021">
</p>

<p align="center">
  <a href="#what-is-cotrex">About</a> &bull;
  <a href="#installation">Install</a> &bull;
  <a href="#usage">Usage</a>
</p>

---

## What is Cotrex

Cotrex is a CLI toolkit and AI runtime for building autonomous coding agents. It acts as a middleman between AI coding agents (Claude Code, Codex, OpenCode, etc.) and your system, running terminal commands safely and returning structured, compressed results.

- **What it does**: Runs terminal commands on behalf of your AI agent, manages local AI models, and provides a decision loop for agentic tasks
- **Why it's useful**: Keeps AI agent interactions organized and predictable, with local inference capabilities
- **How it works**: Cotrex takes a command, runs it safely, and returns a simple summary. It also orchestrates local AI models via llama.cpp for autonomous coding tasks.

## Installation

### Quick install (recommended)

Run the install script for your platform:

| Platform | Command |
|----------|---------|
| **macOS / Linux** | `curl -sL https://raw.githubusercontent.com/pamod-madubashana/Cotrex/main/Scripts/install.sh \| bash` |
| **Windows (PowerShell)** | `irm https://raw.githubusercontent.com/pamod-madubashana/Cotrex/main/Scripts/install.ps1 \| iex` |

### Manual install

1. Download the archive for your platform from [Releases](https://github.com/pamod-madubashana/Cotrex/releases/latest)
2. Extract `cotrex`
3. Put it on your `PATH`
4. Run `cotrex --version` to confirm it works

## Usage

### Run a command

```bash
cotrex -c git status
cotrex -c cargo build
cotrex run "git status"        # alternative syntax
```

### Ask a question

```bash
cotrex "what does the ? operator do?"    # answers your question
cotrex "list all rust projects here"     # runs a search and prints results
```

### Machine prompt (deterministic)

```bash
cotrex -m "explain the architecture"
cotrex -m graphify query "overview"
```

### First-time setup

```bash
cotrex init             # auto-download model and configure
cotrex --no-download    # init without downloading the model
```

### Model management

```bash
cotrex model list           # list available models
cotrex model install qwen2.5-0.5b  # download and install a model
cotrex model remove qwen2.5-0.5b   # remove a model
cotrex model info qwen2.5-0.5b     # show model details
```

### System diagnostics

```bash
cotrex doctor              # check system health and dependencies
cotrex version             # show version info
```

### Agent demo

```bash
echo '{"tool":"read","path":"Cargo.toml"}' | cotrex demo    # run tool execution demo
```

### Setup

Run `cotrex setup` to configure your API provider and preferences. This is only needed if you want to use AI-powered features like command output compression.

## License

[MIT](LICENSE)
