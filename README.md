
<p align="center">
  <img src="assets/evalt-logo.svg" alt="evalt" width="420">
</p>

<p align="center">
  <strong>Stop vibe-checking agents. Test them.</strong>
</p>

<p align="center">
  Portable YAML evals for AI agents and model harnesses.<br>
  Define your tests once and run them against your own workflow.
</p>

<p align="center">
  <img src="assets/demo.gif" alt="evalt product demo" width="720">
</p>

Don't just test your model. Test your skills, agent instructions,
extensions/plugins, tools, and the real environment where your AI actually
runs.

Make AI evals simple.

> `evalt` is pronounced "ee-val-tee", as in eval tests.

## Why evalt

Most AI eval tools are model-centric. `evalt` is "agent-centric".

It is designed for real harnesses and real workflows, because AI is not just
the model you use, but is also the skills, agent files and tools you give it.

## Quick example

Create `carwash.eval.yaml` in your project somewhere.

```yaml
# carwash.eval.yaml
version: 1
config:
  profiles:
    main:
      harness: pi
  profile: main
tests:
  - name: car-wash-reasoning
    input:
      prompt: >
        I want to wash my car and the car wash place is 15 meters away from me,
        should I walk or drive?
    assertions:
      - left: output
        op: contains
        right: drive
      - review:
          prompt: >
            Did the answer correctly choose driving because the car itself
            needs to be taken to the car wash, instead of only considering the
            short distance?
```

Check that the eval file is valid:

```bash
evalt check
```

Then run it:

```bash
evalt run
```

`evalt` discovers `*.eval.yaml` files recursively, runs each test through your
selected profile, checks deterministic assertions, and uses an AI reviewer for
the semantic judgement.

> The `main` profile uses the Pi agent harness and lets Pi choose the model
> from your existing local configuration.

Feel free to look in the projects `./examples` folder for more examples.

## Installation

`evalt` currently supports macOS and Linux.

The tool is currently early and not yet published.

For now, build from source:

```bash
cargo install --path .
```

> [!NOTE]
> `evalt` uses the [cage](https://github.com/Warashi/cage) CLI to restrict file
> access during test runs. `cage` must be installed separately.

## Usage

Validate eval files without running them:

```bash
evalt check
# or for JSON output:
evalt --json check
```

Run all evals under the current directory:

```bash
evalt run
# or for JSON output:
evalt --json run
```

Run evals from a specific file or directory:

```bash
evalt run --path examples
```

Run tests whose names match a selector:

```bash
evalt run car-wash
```

Print the embedded AI guide, designed for coding agents:

```bash
evalt ai
```

Print schemas for editor integration or AI tooling:

```bash
evalt ai --schema
evalt ai --config-schema
```

### YAML autocomplete setup

You can write the eval schema to a stable location and reference it from any
eval file:

```bash
mkdir -p ~/.config/evalt
evalt ai --schema > ~/.config/evalt/evalt.schema.json
```

Then add this comment at the top of each `*.eval.yaml` file, replacing the
path with your absolute home directory path:

```yaml
# macOS example:
# yaml-language-server: $schema=/Users/YOUR_USER/.config/evalt/evalt.schema.json

# Linux example:
# yaml-language-server: $schema=/home/YOUR_USER/.config/evalt/evalt.schema.json
```

### Creating evals with your agent

`evalt` includes embedded AI-facing docs and schemas, so you can ask your
coding agent to help create evals for your project.

For example:

> Use the `evalt` CLI to build an eval for this bug so I can catch it next time.

The agent can call `evalt ai`, `evalt ai --schema`, and `evalt check` to learn
the format, draft a test, and validate it before you run it.

> [!WARNING]
> `evalt` uses `cage` to block file writes outside the allowed sandbox, but this
> is not a full security sandbox. Commands can still mutate external state
> through networks, services, credentials, or other side effects. Only run evals
> you trust. For stronger isolation, run `evalt` inside a container or VM.

## Configuration

`evalt` uses profiles to decide how a test should run. A profile points to a
harness, plus any extra arguments that should be passed to it.

```yaml
profiles:
 local:
   harness: pi
 fast:
   harness: pi
   extra-args: ["--model", "github-copilot/gpt-5.4"]

profile: local
```

Config options can be defined in multiple places and are merged from lowest to
highest priority:

1. Global config: ~/.config/evalt/config.yaml
2. Project config: .evalt.config.yaml
3. Eval file config: under `config` key
4. Individual test config: under `tests.config`
5. CLI options, where supported

This lets you set common defaults once, then override them for a specific
project, eval file, or test.

Example .evalt.config.yaml:

```yaml
# List of profiles
profiles:
  local: # Name of profile
    harness: pi # Harness to use
    extra-args: ["--model", "llamacpp/qwen3.6-30B"] # Extra CLI args to pass into the `pi` command

profile: local # Default profile to use

reviewer: # Reviewer config
  profile: local # Default profile to use for the reviewer
  default-pass-threshold: 0.8 # The default passing threshold (between 0 and 1) to use if not specified (default 0.8)

run: # Run configuration options
  concurrent-tests: 10 # The number of tests to run concurrently
  timeout-ms: 120000 # The test timeout in milliseconds
  max-turns: 20 # The max number of turns before interupt
  budgets: # Budget configuration options
    max-tokens: 2000 # The max number of tokens used in this test before interupt
    max-cost-usd: 0.10 # The max cost for this test before interupt (if applicable)
```

> [!WARNING]
> Evals can call real models and tools. Use `timeout-ms`, `max-turns`, and
> budget limits to avoid unexpected long-running or expensive test runs.
> If your profile uses a personal AI subscription or account-backed harness,
> high concurrency may trigger provider rate limits, quota exhaustion,
> temporary blocks, or violate provider terms. Keep `run.concurrent-tests`
> conservative, especially for CI or high-volume evals, and prefer official
> API/project keys with appropriate billing and rate limits.

## How it works

The flow:

1) Discover `*.eval.yaml` files
1) Validate them against the schema
1) Run each eval through an adaptor
1) Capture the raw output from the harness and translate it
1) Use captured data to apply assertions
1) Output report

## Roadmap

- Claude Code adaptor
- More adaptors (Antigravity, Opencode, etc.)
- HTML reports
- CI/CD integrations

## Status

`evalt` is still quite early and is actively evolving.

It is possible there will be breaking changes before version 1.0.

## Development transparency

`evalt` was built with AI assistance, but it is not vibe coded. AI helped with
implementation, review, and iteration. Every change was either hand-written or
reviewed before being committed.

The goal is software that can be understood, reviewed, tested, and maintained.
Not generated code shipped on faith.

## Acknowledgements

`evalt` is inspired by and built around a few great tools and ideas:

- [Pi](https://github.com/earendil-works/pi) — the first supported agent harness.
- [cage](https://github.com/Warashi/cage) — used to restrict filesystem access during test runs.
- [`cargo test`](https://doc.rust-lang.org/cargo/commands/cargo-test.html) — inspiration for simple, developer-friendly test runner UX.

