# AGENTS.md

Project guidance for AI agents and contributors working on `evalt`, a Rust CLI for portable, YAML-defined eval tests for AI agent/model harnesses.

## Product vision

`evalt` should make AI workflow regression tests simple, reviewable, and CI-friendly.

The tool is:

- **Agent-centric**: test the real workflow: harness, model/profile, AGENTS/SKILL files, tools, workspace edits, and final output.
- **Harness agnostic**: Pi is the first harness; design for additional harnesses later.
- **Schema-first**: eval YAML and config YAML are generated/validated from Rust types and exportable for editor support.
- **Sandboxed**: tests run in isolated workspaces so file writes cannot escape the test environment.
- **Assertion-first**: deterministic assertions are preferred; AI review is available for semantic checks.
- **Profile-driven**: harness/model/reasoning setup is selected through named profiles, not inline `target.adapter` blocks.

## Current CLI

Implemented commands:

- `evalt check [path]` — discover and validate eval files without running them.
- `evalt run [-p|--path <path>] [selectors...]` — validate and run eval files; selectors match test names by substring.
- `evalt --json run ...` — print machine-readable JSON output.
- `evalt ai --schema` — print eval file JSON schema.
- `evalt ai --config-schema` — print config JSON schema.

Do not run eval tests without user permission; prefer telling the user what command to run.

## Eval file discovery

- Eval files use `*.eval.yaml` or `*.eval.yml`.
- Discovery is recursive from the requested file/dir, defaulting to the current directory.
- Common generated/vendor dirs should stay ignored, especially `.git/`, `target/`, `node_modules/`, `.venv/`, `dist/`, and `build/`.

## Current eval YAML shape

A file contains `version`, optional file-level `config`, and a list of `tests`.

```yaml
# yaml-language-server: $schema=./evalt.schema.json
version: 1
config:
  profile: local
  run:
    timeout-ms: 60000
    max-turns: 20

tests:
  - name: basic-hello
    desc: Very basic sanity check that the harness returns text.
    input:
      prompt: "Say hello in exactly five words."
    assertions:
      - left: output
        op: contains
        right: "hello"

  - name: semantic-review
    input:
      prompt: "Explain why the sky is blue in 2-3 sentences."
    assertions:
      - review:
          prompt: "Does the response correctly explain Rayleigh scattering in 2-3 sentences?"
          pass-threshold: 0.7
```

Notes:

- `target.adapter` / `target.model` is outdated. Use `config.profile` and named `profiles`.
- `input.prompt` is currently the supported input form.
- `workspace.copy` can copy fixtures into the sandbox before the test runs.

Workspace example:

```yaml
workspace:
  copy:
    - from: fixtures/todo.txt
      to: todo.txt
```

## Config and profiles

Configs merge from lowest to highest priority:

1. Global config: `$XDG_CONFIG_HOME/evalt/config.yaml` or `~/.config/evalt/config.yaml` on Unix; `%APPDATA%/evalt/config.yaml` on Windows.
2. Project config: nearest ancestor `.evalt.config.yaml`.
3. Eval file `config` block.
4. Individual test `config` block.
5. CLI options where supported.

Current config shape:

```yaml
profiles:
  local:
    harness: pi
    extra-args:
      - "--model"
      - "auto"
  fast:
    harness: pi
    extra-args: ["--model", "github-copilot/gemini-3-flash-preview"]

profile: local

reviewer:
  profile: fast
  default-pass-threshold: 0.8
  system-prompt: "Optional reviewer system prompt override."

run:
  concurrent-tests: 10 # global-only; cannot be set in eval/test config
  timeout-ms: 120000
  max-turns: 20
  budgets:
    max-tokens: 2000
    max-cost-usd: 0.10
```

Rules:

- A selected `profile` is required to run tests.
- Profiles define the harness and arbitrary harness `extra-args`.
- `reviewer.profile` defaults to the main selected profile when omitted.
- `run.concurrent-tests` is global-only; validation rejects it at eval-file or individual-test level.

## Assertions

Prefer deterministic assertions when possible.

Comparison assertion targets:

- `left: output` — assistant final text.
- `left: output.thinking` — thinking text, if available.
- `left: duration-secs` — runtime seconds.
- `left: turns` — number of turns.
- `left: tokens.input`, `tokens.output`, `tokens.thinking`, `tokens.total`, `tokens.cost-usd` — token/cost metrics when available.
- `left: tool.called` — whether a matching tool was called.
- `left: tool.calls` — count of matching tool calls.
- `left: file.exists` — final sandbox file existence.
- `left: file.content` — final sandbox file contents.

Operators:

- Strings: `contains`, `not-contains`, `matches-regex`, `starts-with`, `ends-with`, `==`, `!=`.
- Numbers: `==`, `!=`, `<`, `<=`, `>`, `>=`.
- Booleans: `==`, `!=`.

Nested assertions are supported with `all`, `any`, and `not`, but keep tests flat when practical.

Review assertions:

```yaml
- review:
    prompt: "Did the answer explain the tradeoff clearly and correctly?"
    pass-threshold: 0.8
```

Use review only for semantic/quality checks that deterministic assertions cannot express.

## Architecture

Current important modules:

- `src/types/cli.rs` — CLI definitions.
- `src/types/yaml_spec/` — eval/config/assertion YAML types and schema generation inputs.
- `src/types/config/stack.rs` — config discovery, merge order, defaults, and profile resolution.
- `src/harness/` — harness adapter trait and Pi adapter.
- `src/sandbox/` — sandbox abstraction and cage implementation.
- `src/engine.rs` — run orchestration, assertions, reviewer flow.
- `src/output/` — pretty/JSON reporting.

## Development principles

- Keep schema, Rust types, examples, `SKILL.md`, and docs in sync.
- Prefer strongly typed YAML over ad-hoc path lookups.
- Keep harness-specific behavior behind adapters/profiles.
- Preserve sandbox guarantees when adding workspace/tool behavior.
- Use `null`/missing optional metrics rather than faking unavailable token/cost data.
- Make config/profile validation errors actionable.
- Keep pretty output separate from run logic.
- Do not introduce README/API claims that are not implemented unless marked roadmap/status.

## Roadmap / open areas

- Additional harness adapters beyond Pi.
- Better normalized traces for messages/tool calls/token usage/costs.
- More reporter formats, including HTML.
- CI integrations.
- Richer fixture/workspace controls.
- Tool mocking / expected tool call ergonomics.
