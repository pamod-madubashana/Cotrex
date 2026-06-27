---
name: cotrex
description: "Cotrex RTK orchestration skills for MVP. Run commands, inspect projects, and get normalized output."
---

# Cotrex Skills

**IMPORTANT:** You are an AI agent. Always use `cotrex -m` (model mode) for clean output.

## Two modes: commands vs prompts

cotrex has **commands** and **prompts**. They use different syntax — mixing them up will break.

### Commands (no quotes)
Known CLI commands like `cargo`, `git`, `npm`, `ls`. Pass them **without quotes**:
```bash
cotrex -m cargo test
cotrex -m git status
cotrex -m npm install
cotrex -m cargo build --release
```

### Prompts (quoted)
Natural language instructions. Pass them **inside double quotes**:
```bash
cotrex -m "show the project tree"
cotrex -m "list all rust projects"
cotrex -m "explain the architecture"
cotrex -m "install requirements and init"
```

## Rules

1. Always use `-m` — never bare `cotrex` or `cotrex run`.
2. **Commands = no quotes. Prompts = quoted.** Never mix.
3. One command at a time. Feed the result back before running the next.
4. Skip vendor/, target/, .git/ — they're noise.

## Installed for: MVP
