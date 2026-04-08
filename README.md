# Pump Strategy Framework

Pump.fun event-driven research and simulation framework for:

- Yellowstone gRPC ingest
- PostgreSQL event storage
- replay and live-paper strategy execution
- wallet behavior analysis
- clone-family scoring and strategy iteration
- agent-facing CLI and HTTP APIs

This project does **not** send real transactions.

## What This Repo Is For

Use this repo when an agent or operator needs to:

- collect Pump events into PostgreSQL
- analyze a wallet and summarize its behavior
- infer and evaluate candidate strategy families
- scaffold and iterate strategy configs
- replay, sweep, compare, and rank runs
- run local live-paper simulation
- store experiment and evaluation history

Use this repo less as a fully automatic "discover the true strategy" system.
It can narrow the search space, store research state, and explain candidate fits, but it is still a research platform, not a live trading engine.

## Repo Layout

- `crates/core`
  Domain logic: decoder, state, engine, broker, storage, strategies.
- `crates/app`
  Shared use-cases, API envelopes, clone scoring, replay orchestration.
- `crates/cli`
  Human-facing CLI commands and dashboard.
- `crates/server`
  HTTP server exposing agent-safe APIs over `app`.
- `skills`
  Repo-local agent guidance for common workflows.
- `strategies`
  Strategy configs, scaffold output, examples.
- `schema/postgres.sql`
  PostgreSQL schema including events, runs, tasks, experiments, hypotheses, evaluations.

## Preferred Agent Workflow

When an agent works in this repo, prefer this order:

1. Read `README.md`.
2. Read the relevant skill under `skills/`.
3. Prefer `cargo run -p pump-agent-server` or CLI `--format json` for machine-readable workflows.
4. Use `wallet-dossier`, `clone-report`, `explain-why`, and `suggest-next-experiment` before drilling into raw mint shards.
5. Use raw shard exports only when the higher-level summaries are insufficient.

If the task is stream or provider related, start from `skills/pump-ingest-ops/SKILL.md`.
If the task is wallet analysis or cloning, start from `skills/wallet-clone-analysis/SKILL.md`.
If the task is replay, sweep, or strategy tuning, start from `skills/strategy-iteration/SKILL.md`.
If the task is scaffolding a new config or strategy module, start from `skills/strategy-scaffolding/SKILL.md`.

## Machine-Readable Surfaces

The repo now has two stable agent-facing surfaces:

- CLI with global `--format human|json`
- HTTP server from `crates/server`

CLI example:

```bash
cargo run -p pump-agent-cli -- --format json runs --limit 20
```

Server example:

```bash
cargo run -p pump-agent-server -- --port 3001
```

Useful HTTP routes:

- `GET /healthz`
- `GET /v1/stats`
- `GET /v1/runs`
- `GET /v1/runs/{id}`
- `POST /v1/runs/compare`
- `POST /v1/address/inspect`
- `POST /v1/address/dossier`
- `POST /v1/address/mint-shards`
- `POST /v1/clone/report`
- `POST /v1/clone/explain-why`
- `POST /v1/clone/suggest-next-experiment`
- `POST /v1/clone/eval`
- `POST /v1/replay/db`
- `POST /v1/sweep/db`
- `POST /v1/tasks/ingest`
- `POST /v1/tasks/live-paper`
- `GET /v1/tasks/{task_id}`

Note:

- A few older commands still accept command-local `--json` flags for compatibility.
- For new automation, prefer the global `--format json` or the HTTP API.

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
  --execution-jsonl ./runs/live-executions.jsonl \
  --save-run
```

## Wallet Analysis And Clone Workflow

Start from the higher-level summaries before reading shard files.

Wallet dossier:

```bash
cargo run -p pump-agent-cli -- wallet-dossier --address <PUBKEY>
```

Structured clone report:

```bash
cargo run -p pump-agent-cli -- clone-report \
  --address <PUBKEY> \
  --json \
  --export
```

Why the current family was chosen:

```bash
cargo run -p pump-agent-cli -- explain-why --address <PUBKEY>
```

What to try next:

```bash
cargo run -p pump-agent-cli -- suggest-next-experiment --address <PUBKEY>
```

Mint-level wallet summary:

```bash
cargo run -p pump-agent-cli -- mint-shard-summary --address <PUBKEY> --limit 20
```

Direct validation:

```bash
cargo run -p pump-agent-cli -- clone-eval \
  --address <PUBKEY> \
  --strategy-config ./strategies/<generated>.toml \
  --json
```

Rank recent stored runs by similarity:

```bash
cargo run -p pump-agent-cli -- clone-rank \
  --address <PUBKEY> \
  --scan-limit 50 \
  --top 10 \
  --json
```

Use raw shard exports when the dossier and report are not enough:

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

Wallet-clone-driven scaffold:

```bash
cargo run -p pump-agent-cli -- clone-scaffold \
  --address <PUBKEY>
```

Current built-in strategy families:

- `momentum`
- `early_flow`
- `breakout`
- `liquidity_follow`
- `noop`

## Experiments And Tasks

The PostgreSQL schema now includes:

- `task_runs`
- `experiments`
- `hypotheses`
- `evaluations`

Use these to keep research state, long-running task history, and evaluation outcomes attached to a wallet or hypothesis rather than re-running ad hoc analysis every time.

## Runtime Notes

- `ingest` defaults to `from_slot=None`
- `live-paper` also defaults to `from_slot=None`
- use `--resume-from-db` only if you explicitly want provider-side replay
- `live-paper` does not persist events by default
- `live-paper` can append strategy executions in real time with `--execution-jsonl <path>`
- use `--persist-events` only when you intentionally want live events written into PostgreSQL
- provider-side replay resume is optional and should not be treated as guaranteed

## Dashboard

Run the local dashboard:

```bash
cargo run -p pump-agent-cli -- serve-dashboard --port 3000
```

Default URL:

```text
http://127.0.0.1:3000
```

Useful pages:

- `/`
- `/wallet?address=<PUBKEY>`
- `/compare?left_id=<RUN_A>&right_id=<RUN_B>`

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

At this stage this repo is sufficient as an agent research platform for:

- ingesting Pump data
- analyzing wallets
- producing wallet dossiers and mint summaries
- generating clone reports and explanations
- suggesting next experiments
- scaffolding strategy configs and Rust strategy modules
- replaying, sweeping, comparing, and ranking runs
- running local live-paper simulation
- storing experiment and evaluation history

What it is not:

- a live trading executor
- a guaranteed historical backfill system
- a complete autonomous strategy discovery system
