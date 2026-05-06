
# evalt AI Guide

Use evalt to create portable harness agnostic YAML eval tests for AI workflows.

- Tests run in a basic sandbox so file writes cannot occur outside the test environment
- Name files `*.eval.yaml` (or `*.eval.yml`)
- Tests can optionally come with fixtures/workspace containing test files that get copied into the sandbox
- Test files can be checked using `evalt check` subcommand
- You must NOT run tests yourself without the user's permission, instead opt for telling the user to run it themselves

## Example Test File

```yaml
version: 1
tests:
  - name: basic-hello
    desc: Very basic sanity check that the harness returns text.
    input:
      prompt: "Say hello in exactly five words."
    assertions:
      - left: output
        op: contains
        right: "hello"
  - name: basic-review
    desc: Ask for a simple explanation and let the AI reviewer judge quality.
    input:
      prompt: "Explain why the sky is blue in 2-3 sentences."
    assertions:
      - review:
          prompt: "Does the response correctly explain why the sky appears blue (Rayleigh scattering) in a clear and concise way, using 2-3 sentences?"
          pass_threshold: 0.7
```

You can find a full copy of the JSON schema and available options using the `evalt ai --schema` command.

## Common Assertions

Use deterministic assertions when possible.

- `left: output` — assistant final text
- `left: output.thinking` — thinking text, if available
- `left: duration-secs` — runtime seconds
- `left: turns` — number of turns
- `left: tokens.total` — total token usage, if available
- `left: tool.called` — whether a tool was called
- `left: tool.calls` — number of matching tool calls
- `left: file.exists` — final sandbox file exists
- `left: file.content` — final sandbox file contents

Operators:
- strings: `contains`, `not-contains`, `matches-regex`, `starts-with`, `ends-with`, `==`, `!=`
- numbers: `==`, `!=`, `<`, `<=`, `>`, `>=`
- booleans: `==`, `!=`

## Review Assertions

Use `review` only for semantic/quality checks that deterministic assertions cannot express.

```yaml
- review:
   prompt: "Did the answer explain the tradeoff clearly and correctly?"
   pass_threshold: 0.8
```

## Config and Profiles

Configs merge from lowest to highest priority:

1. Global config file
2. Test file level
3. Test level
4. CLI options

You can override global values at any point.

### Profiles

`evalt` has the concept of "profiles", you define them in your config using the `profiles` option:

```yaml
profiles:
  local:
    harness: pi
    extra-args:
      - "--model"
      - "llama-cpp-qwen/Qwen3.6-35B-A3B-UD-IQ3_S"
  fast:
    harness: pi
    extra-args: ["--model", "auto"]
profile: local
```

A profile is essentially a harness setup pointing at a model, reasoning level, or any other additional options you want to pass to the harness.
Then selecting a profile for a test is as simple as referring to it by its name at any level of the config.

## Best Practices

- Each test should be simple and serve a single purpose
- Although assertions can be nested with `any` and `all` operators, you should steer away from them if possible, preferring flat assertion lists
- Input prompts should be concise. The idea is if the AI can't do what is required of it from a simple prompt, then this should be solved via an AGENTS.md file or SKILL instead.
