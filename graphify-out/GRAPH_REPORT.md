# Graph Report - MVP  (2026-07-30)

## Corpus Check
- 83 files · ~53,851 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 1275 nodes · 2740 edges · 68 communities (67 shown, 1 thin omitted)
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS · INFERRED: 6 edges (avg confidence: 0.8)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `d019af87`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- [[_COMMUNITY_prompt.rs|prompt.rs]]
- [[_COMMUNITY_Result_|Result_]]
- [[_COMMUNITY_mcp.rs|mcp.rs]]
- [[_COMMUNITY_dispatch.rs|dispatch.rs]]
- [[_COMMUNITY_intent.rs|intent.rs]]
- [[_COMMUNITY_mod.rs|mod.rs]]
- [[_COMMUNITY_install.rs|install.rs]]
- [[_COMMUNITY_permission.rs|permission.rs]]
- [[_COMMUNITY_tool.rs|tool.rs]]
- [[_COMMUNITY_install_agent.rs|install_agent.rs]]
- [[_COMMUNITY_update.rs|update.rs]]
- [[_COMMUNITY_CLAUDE|CLAUDE.md]]
- [[_COMMUNITY_LlmConfig|LlmConfig]]
- [[_COMMUNITY_embedded.rs|embedded.rs]]
- [[_COMMUNITY_embedded_graphify.rs|embedded_graphify.rs]]
- [[_COMMUNITY_README|README.md]]
- [[_COMMUNITY_script.rs|script.rs]]
- [[_COMMUNITY_AGENTS|AGENTS.md]]
- [[_COMMUNITY_normalize.rs|normalize.rs]]
- [[_COMMUNITY_Cli|Cli]]
- [[_COMMUNITY_Q how to fix graphify on Windows|Q: how to fix graphify on Windows]]
- [[_COMMUNITY_pipeline.rs|pipeline.rs]]
- [[_COMMUNITY_controller.rs|controller.rs]]
- [[_COMMUNITY_engine.rs|engine.rs]]
- [[_COMMUNITY_projection.rs|projection.rs]]
- [[_COMMUNITY_RFC-0003 Observation Pipeline|RFC-0003: Observation Pipeline]]
- [[_COMMUNITY_Architecture|Architecture]]
- [[_COMMUNITY_EventStore|EventStore]]
- [[_COMMUNITY_RFC-0002 Projection Engine|RFC-0002: Projection Engine]]
- [[_COMMUNITY_ai_context.rs|ai_context.rs]]
- [[_COMMUNITY_event.rs|event.rs]]
- [[_COMMUNITY_Result_|Result_]]
- [[_COMMUNITY_RFC-0001 Kernel Event Store|RFC-0001: Kernel Event Store]]
- [[_COMMUNITY_engine.rs|engine.rs]]
- [[_COMMUNITY_registry.rs|registry.rs]]
- [[_COMMUNITY_lib.rs|lib.rs]]
- [[_COMMUNITY_ExecutionError|ExecutionError]]
- [[_COMMUNITY_Cotrex AI Runtime|Cotrex AI Runtime]]
- [[_COMMUNITY_ExecutionResult|ExecutionResult]]
- [[_COMMUNITY_ProjectionEngine|ProjectionEngine]]
- [[_COMMUNITY_RFCs|RFCs]]
- [[_COMMUNITY_command.rs|command.rs]]
- [[_COMMUNITY_RFC-0004 Execution Engine|RFC-0004: Execution Engine]]
- [[_COMMUNITY_ADR-0002 Protocol Versioning Strategy|ADR-0002: Protocol Versioning Strategy]]
- [[_COMMUNITY_file_write.rs|file_write.rs]]
- [[_COMMUNITY_.new|.new]]
- [[_COMMUNITY_lib.rs|lib.rs]]
- [[_COMMUNITY_lib.rs|lib.rs]]
- [[_COMMUNITY_file_delete.rs|file_delete.rs]]
- [[_COMMUNITY_AGENTS.md — cotrex-ai|AGENTS.md — cotrex-ai]]
- [[_COMMUNITY_.execute|.execute]]
- [[_COMMUNITY_events.rs|events.rs]]
- [[_COMMUNITY_Guiding Principles|Guiding Principles]]
- [[_COMMUNITY_ADRs|ADRs]]
- [[_COMMUNITY_ProviderInfo|ProviderInfo]]
- [[_COMMUNITY_ADR-0006 Event Store Persistence Strategy|ADR-0006: Event Store Persistence Strategy]]
- [[_COMMUNITY_MockProvider|MockProvider]]
- [[_COMMUNITY_5. Execution Events|5. Execution Events]]
- [[_COMMUNITY_cli.rs|cli.rs]]
- [[_COMMUNITY_4. Execution Lifecycle|4. Execution Lifecycle]]
- [[_COMMUNITY_8. Failure Semantics|8. Failure Semantics]]

## God Nodes (most connected - your core abstractions)
1. `Result_` - 112 edges
2. `EventStoreError` - 42 edges
3. `EventStore` - 31 edges
4. `ExecutionError` - 28 edges
5. `ExecutionRequest` - 24 edges
6. `ExecutionResult` - 21 edges
7. `ProjectionEngine` - 20 edges
8. `FileChangeProjection` - 20 edges
9. `command_request()` - 19 edges
10. `tools_call()` - 19 edges

## Surprising Connections (you probably didn't know these)
- `FakeExecutor` --references--> `Result_`  [EXTRACTED]
  cotrex-ai/execution/src/engine.rs → src/core/orchestrate.rs
- `validate_raw_path()` --references--> `Result_`  [EXTRACTED]
  cotrex-ai/execution/src/executor/path_validation.rs → src/core/orchestrate.rs
- `resolve_within()` --references--> `Result_`  [EXTRACTED]
  cotrex-ai/execution/src/executor/path_validation.rs → src/core/orchestrate.rs
- `AgentController` --references--> `ExecutionEngine`  [EXTRACTED]
  cotrex-ai/agent/src/controller.rs → cotrex-ai/execution/src/engine.rs
- `SucceedExecutor` --implements--> `Executor`  [EXTRACTED]
  cotrex-ai/agent/src/controller.rs → cotrex-ai/execution/src/executor.rs

## Import Cycles
- 1-file cycle: `cotrex-ai/execution/src/executor/command.rs -> cotrex-ai/execution/src/executor/command.rs`
- 1-file cycle: `cotrex-ai/kernel/src/engine.rs -> cotrex-ai/kernel/src/engine.rs`

## Communities (68 total, 1 thin omitted)

### Community 0 - "prompt.rs"
Cohesion: 0.08
Nodes (47): Arc, AtomicBool, Drop, Instant, JoinHandle, Response, build_tree_fallback(), build_tree_from_files() (+39 more)

### Community 1 - "Result_"
Cohesion: 0.13
Nodes (40): FnOnce, add_url(), auto_update(), bootstrap_detached(), clear_skill_marker(), cluster_only(), current_agent(), current_project_dir() (+32 more)

### Community 2 - "mcp.rs"
Cohesion: 0.11
Nodes (38): Config, config_path(), defaults_are_safe(), load(), Default, Option, PathBuf, Self (+30 more)

### Community 3 - "dispatch.rs"
Cohesion: 0.22
Nodes (17): Intent, dispatch_cmd(), dispatch_graph(), dispatch_one(), exec_opts(), fulfill(), is_passthrough(), load_llm_or_exit() (+9 more)

### Community 4 - "intent.rs"
Cohesion: 0.13
Nodes (16): Item, Iterator, cli_and_json_agree(), default_action(), default_tool(), gh_pr_create_maps_direct(), has_shell_operators(), Into (+8 more)

### Community 5 - "mod.rs"
Cohesion: 0.19
Nodes (23): MutexGuard, bytes_to_tokens(), chrono_now(), footer(), footer_contains_token_counts(), footer_shows_failed_status(), format_cost(), get_global() (+15 more)

### Community 6 - "install.rs"
Cohesion: 0.25
Nodes (14): asset_for(), asset_name(), download_url(), ensure_rtk(), find_bin(), install(), on_path(), Option (+6 more)

### Community 7 - "permission.rs"
Cohesion: 0.20
Nodes (11): Action, default_permissions_allow_read_tools(), default_permissions_ask_on_write_tools(), is_command_risky(), Permissions, HashMap, Option, Self (+3 more)

### Community 8 - "tool.rs"
Cohesion: 0.24
Nodes (13): Regex, builtins(), resolve_path(), Path, PathBuf, String, Value, Vec (+5 more)

### Community 9 - "install_agent.rs"
Cohesion: 0.26
Nodes (14): agent_skills_dir(), cotrex_skill(), current_project_dir(), graphify_skill(), inject_agents_md_rules(), install_agent(), is_project_dir(), list_installed() (+6 more)

### Community 10 - "update.rs"
Cohesion: 0.16
Nodes (20): ProgressBar, download_with_progress(), format_bytes(), Path, String, spinner(), asset_for(), cleanup_old_backups() (+12 more)

### Community 11 - "CLAUDE.md"
Cohesion: 0.14
Nodes (12): Architecture, Branch & PR workflow (must follow), Commands, Commit & attribution rules (must follow), Config & modes (`cotrex setup`), Getting rtk, graphify code map (`graphify.rs`), Invariants (+4 more)

### Community 12 - "LlmConfig"
Cohesion: 0.23
Nodes (10): with_model(), compress(), Insight, LlmConfig, parse_insight(), parses_fenced_json(), Option, Self (+2 more)

### Community 13 - "embedded.rs"
Cohesion: 0.38
Nodes (9): embedded_rtk_path(), embedded_rtk_path_is_deterministic(), extract_rtk(), is_embedded(), is_embedded_matches_cfg(), marker_path(), Option, PathBuf (+1 more)

### Community 14 - "embedded_graphify.rs"
Cohesion: 0.38
Nodes (9): embedded_graphify_path(), embedded_graphify_path_is_deterministic(), extract_graphify(), graphify_version(), is_embedded(), is_embedded_matches_cfg(), marker_path(), Option (+1 more)

### Community 15 - "README.md"
Cohesion: 0.20
Nodes (9): Ask a question, Installation, License, Manual install, Quick install (recommended), Run a command, Setup, Usage (+1 more)

### Community 16 - "script.rs"
Cohesion: 0.33
Nodes (7): ensure_dir(), exec_command(), PathBuf, String, Write, run(), scripts_dir()

### Community 17 - "AGENTS.md"
Cohesion: 0.22
Nodes (7): Build & Test, Commit Rules, Conventions, Core Contract, Module Map, RULE 0: USE COTREX — NO EXCEPTIONS, RULE 1: GRAPHIFY FIRST

### Community 18 - "normalize.rs"
Cohesion: 0.22
Nodes (11): classify(), LineEvent, normalize(), normalize_keeps_line_verbatim(), String, Severity, Msg, Option (+3 more)

### Community 19 - "Cli"
Cohesion: 0.40
Nodes (4): Cli, Cmd, Cmd, Option

### Community 25 - "Q: how to fix graphify on Windows"
Cohesion: 0.50
Nodes (3): Answer, Q: how to fix graphify on Windows, Source Nodes

### Community 26 - "pipeline.rs"
Cohesion: 0.08
Nodes (50): accept_normal_file(), accept_obs(), custom_ignore_pattern(), FilterDecision, ObservationFilter, reject_git_directory(), reject_hidden_files(), reject_path_outside_root() (+42 more)

### Community 27 - "controller.rs"
Cohesion: 0.06
Nodes (47): add_observation(), AgentContext, context_starts_empty(), Observation, Self, String, SystemTime, Uuid (+39 more)

### Community 28 - "engine.rs"
Cohesion: 0.11
Nodes (43): command_request(), engine_denied(), engine_with_executor(), event_payloads(), execution_duration_is_engine_measured_not_executor_provided(), execution_duration_propagation(), execution_failure_duration_propagation(), execution_requested_before_completed() (+35 more)

### Community 29 - "projection.rs"
Cohesion: 0.13
Nodes (30): FileOperation, checkpoint_advances_on_process(), checkpoint_associativity_full_rebuild_equals_resume(), checkpoint_resets_on_rebuild(), checkpoint_starts_at_zero(), FileChangeProjection, FileRecord, lifecycle_created_to_initialized() (+22 more)

### Community 30 - "RFC-0003: Observation Pipeline"
Cohesion: 0.12
Nodes (16): 11. Backpressure Handling, 13. Statistics, 15. Invariants, 16. Non-Goals, 1. Purpose, 2. Scope, 3. Glossary, 4. Architecture Position (+8 more)

### Community 31 - "Architecture"
Cohesion: 0.06
Nodes (35): Allowed, Architectural Invariants, Architecture, Concurrency, cotrex-ai Runtime, Deferred Architectural Decisions, Dependency Direction, Documentation Hierarchy (+27 more)

### Community 32 - "EventStore"
Cohesion: 0.14
Nodes (20): EventPayload, append_assigns_sequential_sequence(), append_is_atomic(), backpressure_blocks_at_capacity(), committed_events_remain_available_after_backpressure(), EventStore, failed_append_consumes_no_sequence(), file_changed_payload() (+12 more)

### Community 33 - "RFC-0002: Projection Engine"
Cohesion: 0.07
Nodes (29): 10. Checkpointing, 11. Failure Semantics, 12. Invariants, 13. Non-Goals, 1. Purpose, 2. Glossary, 3. Projection Model, 4. Event Processing Guarantees (+21 more)

### Community 34 - "ai_context.rs"
Cohesion: 0.06
Nodes (48): Formatter, ai_context_is_projection(), ai_context_recent_changes_tracked(), ai_context_starts_empty(), ai_context_summary_after_rebuild(), AiContextProjection, AiContextState, AiContextSummary (+40 more)

### Community 35 - "event.rs"
Cohesion: 0.13
Nodes (14): event_is_clone(), event_payload_clone(), event_payload_equality_across_variants(), event_payload_wraps_execution_completed(), event_payload_wraps_execution_failed(), event_payload_wraps_execution_requested(), ExecutionCompleted, ExecutionFailed (+6 more)

### Community 36 - "Result_"
Cohesion: 0.40
Nodes (5): 7. Filtering Rules, Additional Rejection Rules, Custom Patterns, Default Ignore Patterns, Filter Contract

### Community 37 - "RFC-0001: Kernel Event Store"
Cohesion: 0.08
Nodes (25): 10. Event Identity and Envelope, 11. Invariants, 12. Non-Goals, 13.1 Verification, 13. MVP Storage Limitation, 1. Purpose, 2. Glossary, 3.1 Global Ordering (+17 more)

### Community 38 - "engine.rs"
Cohesion: 0.50
Nodes (4): 10. Failure Semantics, Append Failure, Translation Failure, Watcher Failure

### Community 39 - "registry.rs"
Cohesion: 0.15
Nodes (16): RegistryError, Display, discriminant_from_action(), duplicate_registration_rejected(), ExecutionActionDiscriminant, ExecutorRegistry, lookup_missing_returns_none(), register_and_lookup() (+8 more)

### Community 40 - "lib.rs"
Cohesion: 0.14
Nodes (10): build_summary_request_roundtrip(), capability_request_is_clone(), CapabilityError, explain_rust_request_roundtrip(), ProtocolVersion, RequestMetadata, Default, Self (+2 more)

### Community 41 - "ExecutionError"
Cohesion: 0.24
Nodes (9): ExecutionError, Error, From, Option, Self, resolve_within(), Path, PathBuf (+1 more)

### Community 42 - "Cotrex AI Runtime"
Cohesion: 0.10
Nodes (20): ADRs, Architecture, Build, Cotrex AI Runtime, Current Status, Documentation, Execution Runtime, Kernel (+12 more)

### Community 43 - "ExecutionResult"
Cohesion: 0.14
Nodes (11): Executor, Send, Sync, succeed_executor_returns_ok(), SucceedExecutor, DummyExecutor, ExecutionResult, Option (+3 more)

### Community 44 - "ProjectionEngine"
Cohesion: 0.50
Nodes (4): 12. Lifecycle, Allowed Transitions, Invalid Transitions, States

### Community 45 - "RFCs"
Cohesion: 0.12
Nodes (15): Backpressure Behavior, Event Ordering Guarantees, Event Store Write Ordering, Implementation Scope, Mandatory Definitions, Naming Convention, Process, Projection Consistency (+7 more)

### Community 46 - "command.rs"
Cohesion: 0.30
Nodes (13): code_to_i32(), command_request(), CommandExecutor, missing_executable(), non_command_run_action_returns_error(), non_zero_exit_is_success(), permission_failure(), Option (+5 more)

### Community 47 - "RFC-0004: Execution Engine"
Cohesion: 0.06
Nodes (34): 10. Invariants, 11. Non-Goals, 1. Purpose, 2. Glossary, 3. Architecture Position, 4. Execution Lifecycle, 5. Execution Events, 6. Security Boundary (+26 more)

### Community 48 - "ADR-0002: Protocol Versioning Strategy"
Cohesion: 0.14
Nodes (13): ADR-0002: Protocol Versioning Strategy, Alternatives Considered, Backward Compatibility, Consequences, Context, Decision, Future Review Trigger, Negative (+5 more)

### Community 49 - "file_write.rs"
Cohesion: 0.34
Nodes (12): basic_write(), binary_content(), create_parent_directories(), FileWriteExecutor, non_file_write_action_returns_error(), overwrite_existing_file(), reject_absolute_path(), reject_nested_traversal() (+4 more)

### Community 50 - ".new"
Cohesion: 0.38
Nodes (11): build_failure_loads_fixture(), build_success_loads_fixture(), execute_returns_matching_variant(), explain_rust_loads_fixture(), fixtures_dir(), JsonProvider, metadata(), missing_fixture_returns_error() (+3 more)

### Community 51 - "lib.rs"
Cohesion: 0.18
Nodes (6): CapabilityProvider, CapabilityProviderExt, EchoProvider, Send, Sync, T

### Community 52 - "lib.rs"
Cohesion: 0.29
Nodes (12): build_summary_compilation_failed(), build_summary_linker_error(), build_summary_success(), build_summary_unknown_exit_code(), execute_returns_matching_variant_for_build_summary(), execute_returns_matching_variant_for_explain_rust(), explain_rust_empty_source(), explain_rust_with_function() (+4 more)

### Community 53 - "file_delete.rs"
Cohesion: 0.38
Nodes (10): delete_existing_file(), delete_missing_file_is_idempotent(), delete_request(), FileDeleteExecutor, non_file_delete_action_returns_error(), reject_absolute_path(), reject_nested_traversal(), reject_symlink_escape() (+2 more)

### Community 54 - "AGENTS.md — cotrex-ai"
Cohesion: 0.18
Nodes (11): AGENTS.md — cotrex-ai, Architecture, Commands, Documentation Hierarchy, Error Split, Kernel Modules, Protocol Types, Provider Trait (+3 more)

### Community 55 - ".execute"
Cohesion: 0.31
Nodes (7): BuildSummaryRequest, BuildSummaryResponse, CapabilityRequest, CapabilityResponse, Option, build_summary_response(), RuntimeError

### Community 56 - "events.rs"
Cohesion: 0.31
Nodes (7): ExecutionCompleted, ExecutionFailed, ExecutionRequested, PathBuf, String, SystemTime, Uuid

### Community 57 - "Guiding Principles"
Cohesion: 0.18
Nodes (10): AI Is A Consumer, Closed Capability System, Contracts Over Models, Guiding Principles, Implementation Follows Architecture, Kernel Owns Reality, Long-Term Goals, Philosophy (+2 more)

### Community 58 - "ADRs"
Cohesion: 0.25
Nodes (7): ADR Index, ADRs, Naming Convention, Process, Purpose, References, Status Labels

### Community 59 - "ProviderInfo"
Cohesion: 0.29
Nodes (7): CapabilityKind, ExplainRustRequest, ExplainRustResponse, ProviderInfo, String, Vec, explain_rust_response()

### Community 60 - "ADR-0006: Event Store Persistence Strategy"
Cohesion: 0.29
Nodes (6): ADR-0006: Event Store Persistence Strategy, Consequences, Context, Decision, References, Verification

### Community 63 - "5. Execution Events"
Cohesion: 0.50
Nodes (4): 14. Guarantees, Event Creation, No Semantic Interpretation, Ordering

### Community 64 - "cli.rs"
Cohesion: 0.33
Nodes (5): Cli, Cmd, GraphAction, Cmd, Option

### Community 65 - "4. Execution Lifecycle"
Cohesion: 0.50
Nodes (4): 6. Translation Rules, Timestamp, Translation Map, Translation Output

### Community 66 - "8. Failure Semantics"
Cohesion: 0.50
Nodes (4): 8. Duplicate Notification Policy, Future Work, MVP Behavior, Rationale

## Knowledge Gaps
- **227 isolated node(s):** `ProtocolVersion`, `CapabilityError`, `Cmd`, `Msg`, `Cmd` (+222 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **1 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `Result_` connect `ai_context.rs` to `prompt.rs`, `Result_`, `mcp.rs`, `intent.rs`, `install.rs`, `tool.rs`, `install_agent.rs`, `update.rs`, `LlmConfig`, `script.rs`, `normalize.rs`, `pipeline.rs`, `controller.rs`, `engine.rs`, `projection.rs`, `EventStore`, `registry.rs`, `ExecutionError`, `ExecutionResult`, `command.rs`, `file_write.rs`, `lib.rs`, `file_delete.rs`, `.execute`?**
  _High betweenness centrality (0.413) - this node is a cross-community bridge._
- **Why does `ExecutionError` connect `ExecutionError` to `EventStore`, `ai_context.rs`, `registry.rs`, `ExecutionResult`, `command.rs`, `file_write.rs`, `file_delete.rs`, `controller.rs`, `engine.rs`?**
  _High betweenness centrality (0.032) - this node is a cross-community bridge._
- **Why does `dispatch()` connect `mcp.rs` to `ai_context.rs`?**
  _High betweenness centrality (0.030) - this node is a cross-community bridge._
- **What connects `ProtocolVersion`, `CapabilityError`, `Cmd` to the rest of the system?**
  _227 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `prompt.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.07720782654680064 - nodes in this community are weakly interconnected._
- **Should `Result_` be split into smaller, more focused modules?**
  _Cohesion score 0.12727272727272726 - nodes in this community are weakly interconnected._
- **Should `mcp.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.10909090909090909 - nodes in this community are weakly interconnected._