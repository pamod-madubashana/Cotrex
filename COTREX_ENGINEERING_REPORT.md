# Cotrex Engineering Context Report

## Overview

Cotrex is a CLI toolkit for optimizing AI agent performance and reducing token usage. It acts as a middleman between AI coding agents (Claude Code, Codex, OpenCode, etc.) and the system, running terminal commands safely and returning structured, compressed results.

**Key Problem Solved:** AI agents generate massive amounts of raw output (build logs, test results, command output) that waste tokens and context window space. Cotrex normalizes, filters, and compresses this output into structured, agent-consumable insights.

**Target Users:** AI coding agents and developers using AI-assisted development workflows.

---

## Architecture

### High-Level Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                    AI Coding Agents                          │
│            (Claude Code, Codex, OpenCode, etc.)              │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    MCP Layer (stdio JSON-RPC)                │
│           Exposes: run, delegate, plan, graphify             │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    Cotrex Core                               │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │   Intent    │  │ Orchestrator│  │   Agent Prompt      │ │
│  │ Normalizer  │──│   (RTK)     │──│   (Decision Loop)   │ │
│  └─────────────┘  └─────────────┘  └─────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    RTK (Real-Time Kernel)                    │
│          Command execution, filtering, normalization         │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    cotrex-ai Runtime                         │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │   Contract  │  │   Runtime   │  │      Kernel         │ │
│  │   (Protocol)│  │  (Provider) │  │  (Event Store)      │ │
│  └─────────────┘  └─────────────┘  └─────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

### Main Execution Flow

1. **User/Agent Request:** AI agent sends a command or task via CLI or MCP
2. **Intent Normalization:** Request is normalized into an `Intent` struct
3. **RTK Orchestration:** Intent is forwarded to RTK for execution
4. **Output Normalization:** RTK output is normalized into line events with severity
5. **Optional LLM Compression:** Failed commands can be compressed into insights
6. **Structured Response:** Agent receives structured, token-efficient output

### Relationship Between Components

- **User:** Interacts via CLI (`cotrex "command"`) or through AI agents
- **AI Coding Agents:** Use MCP to call Cotrex tools (`run`, `delegate`, `plan`)
- **Cotrex MCP Layer:** JSON-RPC server exposing Cotrex capabilities
- **Cotrex Core:** Intent normalization, orchestration, agent decision loop
- **cotrex-ai Runtime:** Protocol-first AI runtime with event-sourced kernel
- **Providers:** Inference backends (mock, JSON fixture, future: llama.cpp, Candle)
- **External Tools:** RTK for command execution, graphify for knowledge graphs

---

## Repository Structure

### Main Repository (Cotrex)

```text
MVP/
├── src/
│   ├── main.rs              # Entry point
│   ├── cli.rs               # CLI types (clap)
│   ├── agent/               # Agentic task loop
│   │   ├── mod.rs
│   │   ├── permission.rs    # Permission gating
│   │   ├── prompt.rs        # Decision loop, roles, categories
│   │   └── tool.rs          # Built-in tools
│   ├── config/              # Configuration management
│   │   ├── mod.rs
│   │   ├── settings.rs      # Config load/save, setup
│   │   ├── install.rs       # RTK installation
│   │   ├── install_agent.rs # Agent skill installation
│   │   ├── update.rs        # Self-update
│   │   └── download.rs      # Binary downloads
│   ├── core/                # Core execution
│   │   ├── mod.rs
│   │   ├── intent.rs        # Intent normalization
│   │   ├── orchestrate.rs   # RTK orchestration
│   │   └── normalize.rs     # Output normalization
│   ├── dispatch/            # Command routing
│   │   ├── mod.rs
│   │   ├── cli.rs           # CLI argument parsing
│   │   └── dispatch.rs      # Main dispatch logic
│   ├── graphify/            # Knowledge graph integration
│   ├── llm/                 # LLM integration
│   │   ├── mod.rs
│   │   ├── compress.rs      # Output compression
│   │   └── mcp.rs           # MCP server
│   ├── script/              # Script runner
│   └── usage/               # Token usage tracking
├── vendor/
│   ├── rtk/                 # RTK submodule
│   └── graphify/            # Graphify submodule
├── cotrex-ai/               # cotrex-ai submodule
├── graphify-out/            # Generated knowledge graph
├── Cargo.toml               # Workspace configuration
└── AGENTS.md                # Agent instructions
```

### cotrex-ai Submodule

```text
cotrex-ai/
├── contract/                # Protocol types (no logic)
├── runtime/                 # CapabilityProvider trait
├── kernel/                  # Event Store, projections, observation
├── execution/               # Execution engine, registry, executors
├── providers/
│   ├── mock/                # Deterministic mock responses
│   └── json/                # JSON fixture provider
├── examples/                # Usage examples
├── fixtures/                # JSON response fixtures
├── RFC/                     # Protocol definitions
├── ADR/                     # Architectural Decision Records
├── ARCHITECTURE.md          # Canonical architecture doc
├── Vision.md                # Philosophy and goals
└── AGENTS.md                # Agent instructions
```

### Git Submodules

| Submodule | Path | URL | Current Commit | Status |
|-----------|------|-----|----------------|--------|
| rtk | `vendor/rtk` | https://github.com/rtk-ai/rtk | `dev-0.44.0-rc.308` | Active |
| graphify | `vendor/graphify` | https://github.com/safishamsi/graphify | `v0.9.2-83-g13e2bdd` | Active |
| cotrex-ai | `cotrex-ai` | https://github.com/pamod-madubashana/cotrex-ai | `heads/master` | Active |

---

## Rust Workspace Details

### Main Crate: `cotrex`

- **Version:** 3.0.0
- **Edition:** 2021
- **Dependencies:** clap, serde, serde_json, ureq, inquire, dirs, toml, markdown-to-ansi, glob, regex, indicatif

### Workspace Members

| Crate | Purpose | Dependencies |
|-------|---------|--------------|
| `cotrex` (root) | CLI toolkit, MCP server, agent loop | clap, serde, ureq, etc. |
| `vendor/rtk` | Command execution, filtering | System libraries |

### cotrex-ai Workspace

| Crate | Purpose | Dependencies |
|-------|---------|--------------|
| `contract` | Protocol types only | serde, uuid, thiserror |
| `runtime` | CapabilityProvider trait | contract |
| `kernel` | Event Store, projections | uuid, thiserror |
| `execution` | Execution engine, executors | kernel, contract |
| `providers/mock` | Mock provider | runtime, contract |
| `providers/json` | JSON fixture provider | runtime, contract |

---

## Event System

### Event Types

```rust
enum EventPayload {
    FileChanged(FileChanged),
    ExecutionRequested(ExecutionRequested),
    ExecutionCompleted(ExecutionCompleted),
    ExecutionFailed(ExecutionFailed),
}
```

### Event Envelope

```rust
struct Event {
    id: Uuid,                    // Identity (stable across copies)
    sequence: u64,               // Ordering (assigned at commit time)
    occurred_at: SystemTime,     // Informational only
    payload: EventPayload,
}
```

### Event Lifecycle

1. **Created:** Event is constructed with payload
2. **Appended:** EventStore.append() assigns sequence number
3. **Committed:** Event is durably stored and visible to replay
4. **Projected:** Projections derive state from event stream
5. **Archived:** Event remains immutable forever

### Event Storage

- **Current:** In-memory only (`Mutex<Vec<Event>>`)
- **Future:** Disk-backed storage, write-ahead log (ADR-0006)
- **Guarantee:** Append-only, strict ordering, deterministic replay

### Projections

- **FileChangeProjection:** Derives file state from events
- **AiContextProjection:** Semantic summary for AI consumption
- **Lifecycle:** Created → Initialized → Processing → Failed/Rebuilding
- **Checkpointing:** Records processing position for resume

---

## Runtime Architecture

### Runtime Execution Flow

1. **AI Agent Request:** Agent sends capability request via MCP
2. **Intelligence Brain:** Orchestrates AI workflows, decides when to invoke capabilities
3. **cotrex-ai Runtime:** Receives typed request, validates protocol version
4. **Provider Dispatch:** Routes to appropriate CapabilityProvider implementation
5. **Inference:** Provider executes AI inference (local or remote)
6. **Response:** Structured response returned through the stack

### State Management

- **Event Store:** Single source of truth for all project state
- **Projections:** Derived state rebuilt from event replay
- **No Mutable State:** All state changes are events

### Context Management

- **RequestMetadata:** UUID + timestamp attached to every request
- **ProtocolVersion:** Exact version match required (v1.0)
- **ProviderInfo:** Metadata about provider capabilities

### Task Execution

- **ExecutionEngine:** Orchestrates execution lifecycle
- **ExecutorRegistry:** Registers and resolves execution capabilities
- **Built-in Executors:** CommandExecutor, FileWriteExecutor, FileDeleteExecutor
- **Event Boundary:** stdout/stderr never stored in events

### Error Handling

- **CapabilityError:** Protocol-level errors (InvalidRequest, UnsupportedProtocolVersion)
- **RuntimeError:** Execution errors (Provider, InvalidResponse, Capability)
- **Failure Isolation:** One projection failure cannot corrupt others

---

## MCP Layer

### MCP Server Implementation

- **Protocol:** JSON-RPC 2.0 over stdio (newline-delimited)
- **Protocol Version:** 2024-11-05
- **Implementation:** Hand-rolled subset (initialize, tools/list, tools/call, ping)

### Available Tools

| Tool | Description | Input | Output |
|------|-------------|-------|--------|
| `run` | Run shell command through RTK | `command`, `llm` | Structured execution events |
| `set_agent` | Register agent platform for graphify | `agent` | Confirmation |
| `list_roles` | List available roles (planner, coder, etc.) | None | Role list |
| `delegate` | Delegate task to specific role | `task`, `role` | Analyzed answer |
| `plan` | Create ordered plan for task | `task` | Step-by-step plan |
| `usage` | Show token usage statistics | None | Usage summary |
| `graphify` | Query knowledge graph | `question`, `dfs`, `budget` | Graph query results |
| `graphify_path` | Find shortest path between concepts | `node_a`, `node_b` | Path |
| `graphify_explain` | Explain node and connections | `node_name` | Explanation |
| `graphify_add` | Add URL to knowledge graph | `url`, `author`, `contributor` | Confirmation |
| `graphify_save_result` | Save Q&A to knowledge graph | `question`, `answer`, `result_type`, `nodes` | Confirmation |
| `graphify_export` | Export graph in various formats | `format` | Export file |

### Tool Schemas

All tools use JSON input/output with structured schemas defined in `src/llm/mcp.rs`.

---

## AI Model Layer

### Current Model Support

- **Provider:** NVIDIA NIM API (default)
- **Endpoint:** `https://integrate.api.nvidia.com/v1/chat/completions`
- **Default Model:** `meta-llama/llama-3.1-8b-instruct`
- **Protocol:** OpenAI-compatible chat completions

### Role-Based Models

| Role | Model | Purpose |
|------|-------|---------|
| planner | `z-ai/glm-5.1` | Task planning |
| router | `nvidia/nemotron-3-nano-30b-a3b` | Action routing |
| orchestrator | `nvidia/nemotron-3-ultra-550b-a55b` | Workflow orchestration |
| coder | `deepseek-ai/deepseek-v4-flash` | Code generation |
| assistant | `qwen/qwen3-next-80b-a3b-instruct` | General assistance |

### Context Compression Strategy

**Problem:** Raw command output (build logs, test results) wastes tokens.

**Solution:** LLM compression into structured insights:

```json
{
  "status": "failed",
  "root_cause": "missing crate serde",
  "important_errors": ["E0432"],
  "suggested_fix": "add serde to dependencies"
}
```

**Implementation:**
- RTK filters logs heuristically first
- Optional LLM compression for failed commands
- Agent receives ~4 fields instead of raw output

### Response Summarization Strategy

- **User Mode:** Spinner + live-streamed generation + narrated steps
- **Model Mode:** Output only, no thinking or chatter
- **Decision Loop:** Model decides whether to run command or answer directly
- **Step Limit:** Max 6 command iterations before forced answer

---

## Implementation Status

### Milestone Table

| Version | Status | Details |
|---------|--------|---------|
| v1.0 | Completed | Initial release, RTK integration |
| v2.0 | Completed | MCP server, agent roles |
| v2.5.0 | Completed | Graphify integration |
| v2.8.1 | Completed | cotrex-ai submodule added |
| v3.0 | Current | Full cotrex-ai integration, tool execution loop, local inference |

### cotrex-ai Milestones

| Milestone | Status | Details |
|-----------|--------|---------|
| 1 | Complete | Protocol + Runtime + Mock provider |
| 2 | Complete | Documentation consolidation |
| 3 | Complete | Documentation frozen |
| 4 | Complete | RFC-0001: Kernel Event Store (in-memory) |
| 5 | Complete | RFC-0002: Projection Engine |
| 6 | Complete | RFC-0003: Observation Pipeline |
| 7 | Complete | RFC-0004: Execution Engine |
| 8 | Complete | RFC-0007: Local Provider Runtime |
| 9 | Complete | RFC-0008: llama.cpp Provider |
| J | Complete | Tool Execution Loop — built-in tools, permission model, demo mode |
| K | Complete | System Management — init, doctor, version commands |
| L | Complete | Packaging — cross-platform release archives |
| M | Complete | Distribution — GitHub Releases with quality gates |

### Completed Features

- Intent normalization and RTK orchestration
- MCP server with 11 tools
- Agent decision loop with role-based models
- Graphify knowledge graph integration
- Token usage tracking
- Self-update mechanism
- Event-sourced kernel (in-memory)
- Projection engine with multiple projections
- Observation pipeline
- Execution engine with built-in executors
- Tool execution loop with built-in tools (read/write/edit/glob/grep)
- Permission model for tool safety
- Demo mode for testing tool execution
- System management commands (init, doctor, version, model)
- Local model inference via llama.cpp
- Cross-platform release packaging

### Current Milestone

**v3.0.0 Released:** Full cotrex-ai integration with tool execution loop, local model inference, and system management commands.

### Next Milestone

**Phase N:** Complete the agentic loop with streaming responses, multi-turn conversations, and production-grade provider management.

---

## Testing Status

### Main Workspace (Cotrex + RTK)

```text
test result: FAILED. 2342 passed; 16 failed; 8 ignored; 0 measured; 0 filtered out
```

**Note:** All 16 failures are in RTK vendor crate (process spawn issues on Windows), not in Cotrex core.

### cotrex-ai Workspace

```text
contract: 17 passed; 0 failed; 0 ignored
runtime: 10 passed; 0 failed; 0 ignored
kernel: 64 passed; 0 failed; 0 ignored
execution: 90 passed; 0 failed; 1 ignored
providers/mock: 11 passed; 0 failed; 0 ignored
providers/json: 4 passed; 0 failed; 0 ignored
Total: 196 passed; 0 failed; 1 ignored
```

### Test Coverage

- **Core:** Intent normalization, orchestration, dispatch
- **Agent:** Decision loop, permissions, roles
- **MCP:** Tool dispatch, protocol handling
- **cotrex-ai:** Protocol types, runtime, kernel, execution

---

## Known Problems / Technical Debt

### Unfinished Features

1. **Event Store Persistence:** Currently in-memory only (RFC-0001 §13)
2. **Real AI Provider:** No real inference provider implemented yet (Milestone 8)
3. **cotrex-ai Integration:** Protocol bridge not yet connected (RFC-0005)
4. **Graphify Agent Detection:** Auto-detection sometimes fails

### Architectural Risks

1. **RTK Vendor Tests:** 16 failing tests due to Windows process spawn issues
2. **In-Memory Event Store:** Data lost on process restart
3. **No Streaming Responses:** Protocol v1 explicitly excludes streaming
4. **Synchronous API:** May need async if multi-provider concurrency required

### Temporary Implementations

1. **Mock Provider:** Deterministic responses only, no real inference
2. **JSON Fixture Provider:** Hardcoded responses for testing
3. **In-Memory Checkpoints:** Projection state not persisted

### Areas Needing Redesign

1. **Agent Decision Loop:** Current implementation is complex (1200+ lines)
2. **Permission System:** Pattern-based, may need capability-based security
3. **Graphify Integration:** Skill installation is fragile

### Performance Concerns

1. **LLM Compression Latency:** Network round-trip for every failed command
2. **Event Replay:** O(n) for large event stores
3. **Token Usage:** Tracking adds overhead to every execution

---

## Future Roadmap

### Near-Term (Next 3 months)

1. **RFC-0005:** AI Runtime Integration - Connect Cotrex with cotrex-ai
2. **Real Provider:** Implement Candle or llama.cpp provider
3. **Event Persistence:** Disk-backed event store (ADR-0006)
4. **Fix RTK Tests:** Resolve Windows process spawn issues

### Medium-Term (3-6 months)

1. **Provider Registry:** Multiple provider support
2. **Streaming Responses:** Protocol v2 with streaming
3. **Plugin System:** Extensible capability framework
4. **Distributed Execution:** Multi-machine support

### Long-Term Vision

**Cotrex as Agent Operating System:**

1. **Unified Project State:** Single source of truth for all project reality
2. **Structured AI Integration:** Typed protocols, not free-form prompts
3. **Provider Independence:** Swap AI backends without changing kernel
4. **Event-Sourced History:** All state changes recorded as events
5. **Composable Capabilities:** Closed set of capabilities, protocol revisions

**Middle Layer Between Humans and AI Agents:**

- Humans interact via natural language
- Cotrex translates to structured capabilities
- AI agents execute via typed protocols
- Results returned as compressed insights

---

## Important Files

### Core Architecture

| File | Purpose | Why Important |
|------|---------|---------------|
| `src/main.rs` | Entry point | Module tree root |
| `src/dispatch/dispatch.rs` | Command routing | All user input flows through here |
| `src/core/intent.rs` | Intent normalization | Converts user input to structured intent |
| `src/core/orchestrate.rs` | RTK orchestration | Spawns RTK, reads pipes, normalizes stream |
| `src/agent/prompt.rs` | Agent decision loop | Model decides run vs answer |

### MCP Layer

| File | Purpose | Why Important |
|------|---------|---------------|
| `src/llm/mcp.rs` | MCP server | Exposes Cotrex to AI agents |
| `src/llm/compress.rs` | LLM compression | Reduces token usage |

### Configuration

| File | Purpose | Why Important |
|------|---------|---------------|
| `src/config/settings.rs` | Config management | User preferences, API keys |
| `src/config/install.rs` | RTK installation | Ensures RTK is available |

### cotrex-ai Protocol

| File | Purpose | Why Important |
|------|---------|---------------|
| `cotrex-ai/contract/src/lib.rs` | Protocol types | Defines the interface contract |
| `cotrex-ai/runtime/src/lib.rs` | CapabilityProvider trait | Core provider interface |
| `cotrex-ai/kernel/src/event.rs` | Event types | Event envelope and payloads |
| `cotrex-ai/kernel/src/store.rs` | Event Store | Append-only event storage |

### Documentation

| File | Purpose | Why Important |
|------|---------|---------------|
| `AGENTS.md` | Agent instructions | How agents should interact |
| `cotrex-ai/ARCHITECTURE.md` | Canonical architecture | Source of truth for design |
| `cotrex-ai/Vision.md` | Philosophy | Why Cotrex exists |
| `cotrex-ai/RFC/` | Protocol definitions | Implementation strategy |

---

## Appendix: RFC Status

| RFC | Title | Status | Implementation |
|-----|-------|--------|----------------|
| RFC-0001 | Kernel Event Store | Implemented | In-memory event store |
| RFC-0002 | Projection Engine | Implemented | Multiple projections, checkpoints |
| RFC-0003 | Observation Pipeline | Implemented | File system watching |
| RFC-0004 | Execution Engine | Implemented | Command, file write/delete executors |
| RFC-0005 | AI Runtime Integration | Implemented | Connect Cotrex with cotrex-ai |
| RFC-0006 | Persistent Event Store | Implemented | File-backed JSONL store |
| RFC-0007 | Local Provider Runtime | Implemented | Provider lifecycle, context builder |
| RFC-0008 | llama.cpp Provider | Implemented | Real inference via llama.cpp FFI |
| RFC-0009 | Inference Pipeline | Implemented | Full inference pipeline |
| RFC-0010 | Model Output Contract | Implemented | Model output parsing and validation |

## Appendix: ADR Status

| ADR | Title | Status |
|-----|-------|--------|
| ADR-0001 | Event Sourcing | Accepted |
| ADR-0002 | Protocol Versioning Strategy | Accepted |
| ADR-0003 | Closed Capability Protocol | Accepted |
| ADR-0004 | Cargo Workspace | Accepted |
| ADR-0005 | AI as Advisory Layer | Accepted |
| ADR-0006 | Event Store Persistence Strategy | Accepted |

---

**Report Generated:** 2026-07-31
**Cotrex Version:** 3.0.0
**cotrex-ai Version:** v1.0.0
**Protocol Version:** 1.0
