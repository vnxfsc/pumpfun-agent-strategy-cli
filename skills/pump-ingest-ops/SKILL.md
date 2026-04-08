---
name: pump-ingest-ops
description: Use when working on Yellowstone ingest, live-paper stream handling, replay resume behavior, provider compatibility, or PostgreSQL event collection for this Pump Strategy Framework repo.
---

# Pump Ingest Ops

Use this skill when the task is about:

- `ingest`
- `live-paper`
- Yellowstone connectivity
- heartbeat / reconnect / resume behavior
- checking whether data is reaching PostgreSQL

## Repo Facts

- CLI entrypoints live in `crates/cli/src/commands/stream/`
- Yellowstone client logic lives in `crates/core/src/grpc.rs`
- PostgreSQL schema lives in `schema/postgres.sql`
- Runtime defaults and env resolution live in `crates/cli/src/commands/stream/config.rs`

## Ground Rules

- Treat provider-side replay resume as optional.
- Default operational path is `from_slot=None`.
- Only use `--resume-from-db` when the task explicitly wants replay resume.
- If a provider rejects replay position, prefer degrading gracefully over retry loops.

## Fast Checks

Check CLI help:

```bash
cargo run -p pump-agent-cli -- ingest --help
cargo run -p pump-agent-cli -- live-paper --help
```

Check whether data exists:

```bash
cargo run -p pump-agent-cli -- stats
cargo run -p pump-agent-cli -- tail --limit 20
```

Run ingest:

```bash
cargo run -p pump-agent-cli -- ingest
```

Run ingest with explicit replay attempt:

```bash
cargo run -p pump-agent-cli -- ingest --resume-from-db
```

## Debug Order

1. Confirm env or CLI args are present.
2. Confirm stream command path in `crates/cli/src/commands/stream/`.
3. Confirm PostgreSQL is reachable.
4. Confirm decoded events are being written.
5. Only then inspect provider-specific replay behavior.

## What Good Looks Like

- `ingest` connects and keeps printing decoded events
- `stats` shows event counts increasing
- `tail` shows recent trades and mints
- reconnect does not get stuck in replay-position loops

## Validation

Run:

```bash
cargo test --workspace
make ci
```
