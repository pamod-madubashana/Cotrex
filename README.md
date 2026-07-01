<p align="center">
  <img src="assets/cotrex.png" alt="Cotrex" width="220">
</p>

<p align="center">
  <strong>Run terminal commands safely through your AI agent</strong>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/built_with-Rust-orange.svg" alt="Built with Rust">
  <img src="https://img.shields.io/badge/version-2.5.0-blue.svg" alt="Version 2.5.0">
  <img src="https://img.shields.io/github/actions/workflow/status/pamod-madubashana/Cotrex/ci.yml?branch=main&label=CI" alt="CI">
  <img src="https://img.shields.io/github/v/release/pamod-madubashana/Cotrex" alt="Latest Release">
  <img src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey.svg" alt="Platforms">
</p>

<p align="center">
  <a href="#what-is-cotrex">About</a> &bull;
  <a href="#installation">Install</a> &bull;
  <a href="#usage">Usage</a>
</p>

---

## What is Cotrex

Cotrex is a tool that lets AI agents run commands on your computer safely and reliably. It acts as a middleman — when an AI agent wants to run something like `git status` or `cargo build`, Cotrex handles it and gives back clear results.

- **What it does**: Runs terminal commands on behalf of your AI agent
- **Why it's useful**: Keeps AI agent interactions organized and predictable
- **How it works**: Cotrex takes a command, runs it safely, and returns a simple summary

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
cotrex run "git status"
cotrex git status        # same thing — the run subcommand is optional
```

### Ask a question

```bash
cotrex "what does the ? operator do?"    # answers your question
cotrex "list all rust projects here"     # runs a search and prints results
```

### Setup

Run `cotrex setup` to configure your API provider and preferences. This is only needed if you want to use AI-powered features like command output compression.

## License

[MIT](LICENSE)
