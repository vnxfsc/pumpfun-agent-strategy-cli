# Contributing

This repository is meant to work well for both humans and coding agents.

## Before You Change Anything

Run the local quality gate:

```bash
make ci
```

That is the minimum bar for changes touching:

- strategy logic
- replay or live-paper behavior
- persistence and snapshots
- wallet analysis / clone analysis
- dashboard rendering
- scaffolding commands

## Development Principles

- Keep strategy logic in `crates/core/src/strategy/`.
- Keep strategy parameters in `strategies/*.toml`.
- Prefer `strategy-scaffold` or `clone-scaffold` over manual registration edits.
- Prefer adding tests when you change behavior, not just when you add files.
- Do not treat provider-side replay as guaranteed; keep stream resume optional.
- Do not optimize docs for completeness at the expense of operator clarity.

## Common Workflows

### Strategy Iteration

1. Start from a config:

```bash
make scaffold
```

2. Edit the generated file under `strategies/`.

3. Backtest:

```bash
make replay-db
```

4. Sweep:

```bash
make sweep-db
```

5. Compare:

```bash
make runs
cargo run -p pump-agent-cli -- compare-runs --left-id <id> --right-id <id>
```

6. Promote to live-paper only after replay and sweep sanity checks:

```bash
make live-paper
```

### Wallet Clone Research

1. Start with a compact brief:

```bash
make clone-brief ADDRESS=<PUBKEY>
```

2. Generate a structured clone report:

```bash
make clone-report ADDRESS=<PUBKEY>
```

3. Scaffold the suggested clone:

```bash
make clone-scaffold ADDRESS=<PUBKEY>
```

4. Read the exported `index.json` and selected `mint.jsonl` shards.

5. Evaluate candidate configs:

```bash
make clone-eval ADDRESS=<PUBKEY> CONFIG=./strategies/examples/early-flow.toml
```

6. Rank recent stored runs:

```bash
make clone-rank ADDRESS=<PUBKEY>
```

Important:
- the CLI gives structure and scoring
- the exported shards are still the high-value source for deeper reasoning
- strategy family inference is a heuristic, not truth

## Code Layout

- `crates/core`
  Engine, broker, strategy logic, decoder, storage primitives.
- `crates/cli`
  CLI commands for ingest, replay, analysis, clone support, and dashboard.
- `strategies`
  Strategy configs and experiment notes.
- `.config/nextest.toml`
  Test runner defaults for CI and local runs.

## Documentation Standard

When updating docs:

- prefer showing the recommended path first
- mark optional or advanced paths as such
- keep examples runnable
- keep README aligned with Makefile shortcuts
- keep internal docs aligned with actual command names

## When To Add Tests

Add or update tests when changing:

- strategy entry or exit rules
- run persistence or config serialization
- wallet roundtrip extraction
- clone scoring logic
- stream runtime config behavior
- dashboard rendering logic
