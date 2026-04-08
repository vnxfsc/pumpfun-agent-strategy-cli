# Strategy Workspace

This directory is the working area for:

- strategy configs
- clone hypotheses
- experiment notes
- backtest iteration

## Directory Rules

- Keep one config file per experiment under `strategies/`.
- Use readable names, preferably kebab-case or date-prefixed names.
- Keep cloned wallet variants explicit in the filename.
- Keep notes next to the config that produced them.
- Put run IDs and sweep batch IDs in the note file, not in the config.

Examples:

- `strategies/early-flow-baseline.toml`
- `strategies/2026-04-08-early-flow-tight.toml`
- `strategies/dddd-confirmed-flow.toml`
- `strategies/2026-04-08-wallet-x-clone-v2.toml`

## Recommended Workflow

### For Normal Strategy Iteration

1. Scaffold a config:

```bash
cargo run -p pump-agent-cli -- strategy-scaffold \
  --strategy early_flow \
  --output ./strategies/early-flow-baseline.toml
```

2. Backtest:

```bash
cargo run -p pump-agent-cli -- replay-db \
  --strategy-config ./strategies/early-flow-baseline.toml \
  --save-run
```

3. Sweep:

```bash
cargo run -p pump-agent-cli -- sweep-db \
  --strategy early_flow \
  --strategy-config ./strategies/early-flow-baseline.toml \
  --buy-sol-values 0.15,0.2 \
  --min-total-buy-sol-values 0.8,1.0,1.2
```

4. Compare:

```bash
cargo run -p pump-agent-cli -- compare-runs \
  --left-id 10 \
  --right-id 11
```

5. Only then move into live-paper:

```bash
cargo run -p pump-agent-cli -- live-paper \
  --strategy-config ./strategies/early-flow-baseline.toml \
  --save-run
```

### For Wallet Clone Work

1. Generate the brief and export:

```bash
cargo run -p pump-agent-cli -- address-brief \
  --address <PUBKEY> \
  --json \
  --export
```

2. Generate the clone report:

```bash
cargo run -p pump-agent-cli -- clone-report \
  --address <PUBKEY> \
  --json \
  --export
```

3. Scaffold the next clone:

```bash
cargo run -p pump-agent-cli -- clone-scaffold \
  --address <PUBKEY>
```

4. Evaluate the generated config:

```bash
cargo run -p pump-agent-cli -- clone-eval \
  --address <PUBKEY> \
  --strategy-config ./strategies/<generated>.toml \
  --json
```

5. Rank against stored runs if needed:

```bash
cargo run -p pump-agent-cli -- clone-rank \
  --address <PUBKEY> \
  --scan-limit 50 \
  --top 10 \
  --json
```

## Strategy Code

If you are adding a new Rust strategy, the source lives in:

- `crates/core/src/strategy/<name>.rs`

Use:

- `strategy-scaffold` for general strategy bootstrapping
- `clone-scaffold` for wallet-clone-driven bootstrapping

These commands handle registration for you. The TOML file here only holds parameters; the Rust file defines logic.

## What To Keep In Notes

For each serious experiment, keep a sibling `.md` note with:

- why this config exists
- what address or hypothesis it targets
- relevant run IDs
- relevant sweep batch IDs
- what changed from the previous config
- what the next question is
