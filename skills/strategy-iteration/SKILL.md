---
name: strategy-iteration
description: Use when iterating on a Pump strategy config or Rust strategy, including replay-db, sweep-db, compare-runs, clone-eval, clone-rank, and live-paper validation in this repo.
---

# Strategy Iteration

Use this skill when the task is about:

- tuning a strategy config
- backtesting
- parameter sweeps
- comparing run outputs
- evaluating clone similarity
- deciding whether a config is ready for live-paper

## Recommended Flow

1. Start from a config under `strategies/`.
2. Run replay.
3. Run sweep if needed.
4. Compare runs.
5. If cloning a wallet, run `clone-eval` or `clone-rank`.
6. Only then move to `live-paper`.

## Main Commands

Replay:

```bash
cargo run -p pump-agent-cli -- replay-db \
  --strategy-config ./strategies/examples/early-flow.toml \
  --save-run
```

Sweep:

```bash
cargo run -p pump-agent-cli -- sweep-db \
  --strategy early_flow \
  --strategy-config ./strategies/examples/early-flow.toml
```

Inspect:

```bash
cargo run -p pump-agent-cli -- runs --limit 20
cargo run -p pump-agent-cli -- run-inspect --id <RUN_ID> --fill-limit 50
cargo run -p pump-agent-cli -- compare-runs --left-id <A> --right-id <B>
```

Clone scoring:

```bash
cargo run -p pump-agent-cli -- clone-eval --address <PUBKEY> --strategy-config <CONFIG> --json
cargo run -p pump-agent-cli -- clone-rank --address <PUBKEY> --scan-limit 50 --top 10 --json
```

Live-paper:

```bash
cargo run -p pump-agent-cli -- live-paper \
  --strategy-config ./strategies/examples/early-flow.toml \
  --execution-jsonl ./runs/live-executions.jsonl \
  --save-run
```

## Heuristics

- Do not over-trust one run.
- Prefer comparing multiple runs plus clone metrics.
- Treat `live-paper` as validation, not discovery.
- If you need live execution traces during validation, prefer `--execution-jsonl` over waiting for `--save-run` shutdown persistence.
- If a wallet looks structurally different from built-in families, create a new variant instead of over-bending one config.

## Files To Touch

- Configs: `strategies/*.toml`
- Strategy code: `crates/core/src/strategy/`
- Strategy construction: `crates/cli/src/runtime/strategy.rs`

## Validation

Always finish with:

```bash
make ci
```
