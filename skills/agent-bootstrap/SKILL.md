---
name: agent-bootstrap
description: Use when an agent starts working in this Pump Strategy Framework repo and needs the fastest safe orientation on what to read first, what commands to run first, and which behaviors to avoid.
---

# Agent Bootstrap

Use this skill at the start of work in this repo.

## Read First

1. `README.md`
2. `Makefile`
3. `CONTRIBUTING.md`
4. the relevant guide under `skills/`

Then choose one of:

- `skills/pump-ingest-ops/SKILL.md`
- `skills/wallet-clone-analysis/SKILL.md`
- `skills/strategy-iteration/SKILL.md`
- `skills/strategy-scaffolding/SKILL.md`

## First Commands

See what the CLI can do:

```bash
cargo run -p pump-agent-cli -- --help
```

See what the HTTP server can do:

```bash
cargo run -p pump-agent-server -- --help
```

See the shortcut surface:

```bash
make help
```

Check repo quality gate before and after meaningful changes:

```bash
make ci
```

## Choose The Right Path

If the task is about stream health, provider behavior, ingest, or live-paper:

- use `pump-ingest-ops`

If the task is about understanding one address or exporting mint shards:

- use `wallet-clone-analysis`

If the task is about tuning configs, replay, sweep, compare, or live-paper readiness:

- use `strategy-iteration`

If the task is about generating a new config or Rust strategy skeleton:

- use `strategy-scaffolding`

## Important Repo Truths

- This repo is for analysis, replay, simulation, and cloning support.
- It does not submit real transactions.
- Prefer the HTTP API or CLI `--format json` for agent automation.
- Provider-side replay resume is optional and should not be treated as guaranteed.
- `live-paper` can write strategy executions incrementally with `--execution-jsonl <path>`.
- `live-paper --save-run` is useful for run inspection, but it saves on shutdown rather than on every fill.
- `wallet-dossier`, `clone-report`, `explain-why`, and `suggest-next-experiment` are the preferred high-level wallet workflow.
- Exported wallet mint shards are a drill-down tool, not the default first step.
- Built-in strategy families are useful approximations, not complete truth.

## Avoid These Mistakes

- Do not assume `from_slot` replay always works.
- Do not treat `clone-report` as final truth.
- Do not jump straight to raw shard reading if dossier and clone summaries already answer the question.
- Do not add manual strategy registration edits before checking whether scaffold commands already do it.
- Do not jump into `live-paper` before replay and sweep sanity checks.
- Do not optimize only for PnL if the task is wallet cloning; check similarity metrics too.

## Default Safe Workflow

1. Read the docs listed above.
2. Run `make help`.
3. If changing code, run `make ci` before and after.
4. If the task is wallet cloning:
   - `cargo run -p pump-agent-cli -- wallet-dossier --address <PUBKEY>`
   - `make clone-report ADDRESS=<PUBKEY>`
   - `cargo run -p pump-agent-cli -- explain-why --address <PUBKEY>`
   - `cargo run -p pump-agent-cli -- suggest-next-experiment --address <PUBKEY>`
   - only then read exported shards if needed
   - `make clone-scaffold ADDRESS=<PUBKEY>`
   - `make clone-eval ADDRESS=<PUBKEY> CONFIG=<CONFIG>`
5. If the task is strategy iteration:
   - `make replay-db`
   - `make sweep-db`
   - inspect runs
   - only then `make live-paper`
