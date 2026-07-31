---
id: intro
title: Introduction
slug: /
---

<p align="center">
  <img src="/Cotrex/img/cotrex.png" alt="Cotrex" width="180" />
</p>

# Cotrex

**RTK executes. Cotrex makes execution consumable by agents.**

Cotrex is a deterministic [RTK](https://github.com/rtk-ai/rtk) orchestration layer. It takes an
agent's intent, forwards it to RTK — the execution truth layer — and returns a **normalized,
dual-channel** result. Cotrex never runs a raw command itself; it invokes `rtk <subcommand>` and
tags what RTK emits.

- **Machine channel** (`stdout`): newline-delimited JSON, one event per line.
- **Human channel** (`stderr`): a short readable summary.

The model reads small, structured events instead of noisy logs; a human still gets a glanceable
trace. It's infrastructure, not an agent.

### v3.0.0 — Local AI Runtime

Cotrex v3.0.0 adds a full AI runtime with local model inference via llama.cpp, a tool execution loop,
and system management commands for autonomous coding tasks.

## How it works

```
agent intent  ──▶  parse  ──▶  map to rtk  ──▶  spawn rtk  ──▶  classify lines  ──▶  dual output
 (CLI | JSON       Intent     first token →     2 threads        severity for       stdout: raw lines
  | MCP)                      rtk subcommand     + mpsc           error count        + result footer
                                                                                     stderr: summary
```

The command's first token picks the RTK invocation: a known tool (`git`, `cargo`, `npm`, …) routes
to that dedicated rtk filter (`cargo test` → `rtk cargo test`); anything else falls back to
`rtk run -c "<command>"`.

## Three front-ends, one core

- **CLI** — `cotrex run "cargo test"`
- **stdin-JSON** — `echo '{"tool":"rtk","cmd":"git status"}' | cotrex`
- **MCP** — `cotrex mcp` exposes a `run` tool agents call natively

All three funnel into the same execution pipeline.

## New in v3.0.0

- **`cotrex init`** — First-run setup with optional model download
- **`cotrex model`** — Install, list, remove, and inspect local AI models
- **`cotrex doctor`** — System health checks and dependency verification
- **`cotrex demo`** — Runtime microscope for testing tool execution
- **`cotrex version`** — Version and build info

Next: [Installation](installation).
