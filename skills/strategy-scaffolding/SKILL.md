---
name: strategy-scaffolding
description: Use when creating a new strategy config, a new Rust strategy module, or a wallet-clone-driven scaffold in this Pump Strategy Framework repo.
---

# Strategy Scaffolding

Use this skill when the task is about:

- generating a new config
- generating a new Rust strategy module
- generating a clone strategy from a wallet
- avoiding manual strategy registration edits

## Preferred Commands

Config only:

```bash
cargo run -p pump-agent-cli -- strategy-scaffold \
  --strategy early_flow \
  --output ./strategies/my-strategy.toml
```

Rust module plus config:

```bash
cargo run -p pump-agent-cli -- strategy-scaffold \
  --name alpha_flow \
  --strategy early_flow \
  --output ./strategies/alpha-flow.toml
```

Wallet-clone-driven scaffold:

```bash
cargo run -p pump-agent-cli -- clone-scaffold \
  --address <PUBKEY>
```

## Important Rules

- Prefer scaffold commands over manual registration edits.
- Generated Rust modules live in `crates/core/src/strategy/`.
- Generated configs live in `strategies/`.
- Shared strategy wiring lives in `crates/app/src/strategy.rs`.
- If the scaffold was only a test artifact, remove both:
  - the generated Rust module
  - the generated config
  and also undo registration changes.

## Built-in Families

- `momentum`
- `early_flow`
- `breakout`
- `liquidity_follow`
- `noop`

Use:

- `early_flow` when the target looks like fresh-mint flow with strong early activity
- `momentum` when the target looks like broader event-driven follow-through
- `breakout` when the target looks like delayed confirmation and post-threshold entry
- `liquidity_follow` when the target looks like flow that keys off increasing participation and depth
- `noop` only for plumbing or baseline tests

## After Scaffolding

1. Read the generated config.
2. If clone-driven, compare it with `clone-report`.
3. Run replay.
4. Run clone-eval if the task is wallet replication.

## Validation

Run:

```bash
cargo test --workspace
make ci
```
