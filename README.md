# Pump Strategy Framework

Pump.fun event-driven research and simulation framework for:

- Yellowstone gRPC ingest
- PostgreSQL event storage
- replay and live-paper strategy execution
- wallet behavior analysis
- strategy cloning support for coding agents

This project does **not** send real transactions.

## What This Repo Is For

Use this repo when you want an agent or human operator to:

- collect Pump events into PostgreSQL
- analyze a wallet and export its touched mints
- infer rough strategy families from observed behavior
- scaffold a new strategy or clone strategy
- backtest and sweep candidate configs
- compare runs and rank clone similarity
- run local live-paper simulation

Use this repo less as a fully automatic “discover the true strategy” system.
The code can summarize, score, and narrow the search space, but deep interpretation still comes from reading the exported `mint.jsonl` shards.

## Repo Layout

- `crates/core`
  Domain logic: decoder, market state, engine, broker, storage, strategies.
- `crates/cli`
  CLI commands for ingest, replay, analysis, cloning, and dashboard.
- `strategies`
  Strategy configs, experiment notes, examples.
- `schema/postgres.sql`
  PostgreSQL schema.
- `pumpfun`
  Pump IDL and protocol notes.

## Main Operator Workflow

1. Ingest market data:

```bash
cargo run -p pump-agent-cli -- ingest
```

2. Inspect what has been collected:

```bash
cargo run -p pump-agent-cli -- stats
cargo run -p pump-agent-cli -- tail --limit 20
```

3. Backtest a config:

```bash
cargo run -p pump-agent-cli -- replay-db \
  --strategy-config ./strategies/examples/early-flow.toml \
  --save-run
```

4. Sweep parameters:

```bash
cargo run -p pump-agent-cli -- sweep-db \
  --strategy early_flow \
  --strategy-config ./strategies/examples/early-flow.toml \
  --buy-sol-values 0.15,0.2 \
  --min-total-buy-sol-values 0.8,1.0,1.2
```

5. Compare and inspect runs:

```bash
cargo run -p pump-agent-cli -- runs --limit 20
cargo run -p pump-agent-cli -- run-inspect --id 1 --fill-limit 50
cargo run -p pump-agent-cli -- compare-runs --left-id 1 --right-id 2
```

6. Run local live-paper only after replay and sweep checks:

```bash
cargo run -p pump-agent-cli -- live-paper \
  --strategy-config ./strategies/examples/early-flow.toml \
  --save-run
```

## Agent Clone Workflow

This is the important workflow if an agent is trying to reverse-engineer a wallet.

1. Generate a compact brief and export shards:

```bash
cargo run -p pump-agent-cli -- address-brief \
  --address <PUBKEY> \
  --json \
  --export
```

2. Generate a structured clone report:

```bash
cargo run -p pump-agent-cli -- clone-report \
  --address <PUBKEY> \
  --json \
  --export
```

3. Scaffold the recommended clone strategy:

```bash
cargo run -p pump-agent-cli -- clone-scaffold \
  --address <PUBKEY>
```

4. Evaluate the hypothesis directly:

```bash
cargo run -p pump-agent-cli -- clone-eval \
  --address <PUBKEY> \
  --strategy-config ./strategies/<generated>.toml \
  --json
```

5. Or evaluate a stored run:

```bash
cargo run -p pump-agent-cli -- clone-eval \
  --address <PUBKEY> \
  --run-id <RUN_ID> \
  --json
```

6. Rank recent stored runs by similarity:

```bash
cargo run -p pump-agent-cli -- clone-rank \
  --address <PUBKEY> \
  --scan-limit 50 \
  --top 10 \
  --json
```

The intent is:
- code narrows the search space
- exported shards let the agent read actual mint-level context
- the agent proposes or edits strategy logic
- replay and clone-eval verify whether the new idea is closer

## Wallet Analysis Commands

Address overview:

```bash
cargo run -p pump-agent-cli -- address-inspect --address <PUBKEY>
```

Trade timeline:

```bash
cargo run -p pump-agent-cli -- address-timeline --address <PUBKEY> --limit 100
```

Roundtrips:

```bash
cargo run -p pump-agent-cli -- address-roundtrips --address <PUBKEY> --limit 50
```

Feature summary:

```bash
cargo run -p pump-agent-cli -- address-features --address <PUBKEY>
```

Family inference:

```bash
cargo run -p pump-agent-cli -- infer-strategy --address <PUBKEY>
```

Family-local param fitting:

```bash
cargo run -p pump-agent-cli -- fit-params --address <PUBKEY> --family early_flow
```

Shard export:

```bash
cargo run -p pump-agent-cli -- address-export \
  --address <PUBKEY> \
  --output ./exports
```

Export layout:

```text
exports/<address>/index.json
exports/<address>/<mint>.jsonl
```

## Strategy Scaffolding

Config only:

```bash
cargo run -p pump-agent-cli -- strategy-scaffold \
  --strategy early_flow \
  --output ./strategies/my-early-flow.toml
```

Rust strategy module plus config:

```bash
cargo run -p pump-agent-cli -- strategy-scaffold \
  --name alpha_flow \
  --strategy early_flow \
  --output ./strategies/alpha-flow.toml
```

Current built-in strategy families:

- `momentum`
- `early_flow`
- `noop`

Examples:

- `strategies/examples/early-flow.toml`
- `strategies/examples/momentum.toml`

## Runtime Notes

- `ingest` defaults to `from_slot=None`
- `live-paper` also defaults to `from_slot=None`
- use `--resume-from-db` only if you explicitly want provider-side replay
- `live-paper` does not persist events by default
- use `--persist-events` only when you intentionally want live events written into PostgreSQL
- on providers with weak replay support, treating resume as optional is the intended behavior

Explicit resume examples:

```bash
cargo run -p pump-agent-cli -- ingest --resume-from-db
```

```bash
cargo run -p pump-agent-cli -- live-paper \
  --strategy-config ./strategies/examples/early-flow.toml \
  --persist-events \
  --resume-from-db
```

## Dashboard

Run the local dashboard:

```bash
cargo run -p pump-agent-cli -- serve-dashboard --port 3000
```

Default URL:

```text
http://127.0.0.1:3000
```

## Environment

Runtime settings can be provided via `.env`:

```env
YELLOWSTONE_ENDPOINT=https://your-yellowstone-endpoint
YELLOWSTONE_X_TOKEN=your-token
YELLOWSTONE_COMMITMENT=processed
YELLOWSTONE_HEARTBEAT_SECS=15
YELLOWSTONE_RECONNECT_DELAY_SECS=3
YELLOWSTONE_CONNECT_TIMEOUT_SECS=10
YELLOWSTONE_REQUEST_TIMEOUT_SECS=30
YELLOWSTONE_HTTP2_KEEP_ALIVE_INTERVAL_SECS=15
YELLOWSTONE_KEEP_ALIVE_TIMEOUT_SECS=10
YELLOWSTONE_TCP_KEEPALIVE_SECS=30
YELLOWSTONE_MAX_DECODING_MESSAGE_SIZE=67108864
DATABASE_URL=postgres://user:pass@localhost:5432/pump_agent
```

## Quality Gate

```bash
make ci
```

This runs:

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo test --workspace --doc`

## Current Status

At this stage this repo is sufficient as an agent CLI base for:

- ingesting Pump data
- analyzing wallets
- exporting mint shards for offline reasoning
- generating clone reports
- scaffolding strategy configs and Rust strategy modules
- replaying, sweeping, comparing, and ranking runs
- running live-paper simulation

What it is not:

- a live trading executor
- a guaranteed historical backfill system
- a complete autonomous strategy discovery system
