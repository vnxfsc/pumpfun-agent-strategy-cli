---
name: wallet-clone-analysis
description: Use when analyzing a Pump wallet, exporting its touched mints, inferring rough strategy families, reading clone reports, or preparing context for an agent to reverse-engineer strategy behavior.
---

# Wallet Clone Analysis

Use this skill when the task is about:

- analyzing one wallet address
- understanding roundtrips and entry behavior
- exporting touched mints for offline reasoning
- generating clone reports
- deciding whether existing strategy families are sufficient

## Primary Commands

Compact brief plus export:

```bash
cargo run -p pump-agent-cli -- address-brief \
  --address <PUBKEY> \
  --json \
  --export
```

Structured clone report:

```bash
cargo run -p pump-agent-cli -- clone-report \
  --address <PUBKEY> \
  --json \
  --export
```

Detailed commands:

```bash
cargo run -p pump-agent-cli -- address-inspect --address <PUBKEY>
cargo run -p pump-agent-cli -- address-roundtrips --address <PUBKEY> --limit 50
cargo run -p pump-agent-cli -- address-timeline --address <PUBKEY> --limit 100
cargo run -p pump-agent-cli -- infer-strategy --address <PUBKEY>
cargo run -p pump-agent-cli -- fit-params --address <PUBKEY> --family early_flow
```

## Export Layout

Read:

- `exports/<address>/index.json`
- `exports/<address>/<mint>.jsonl`

Prefer reading `index.json` first, then only the most relevant mint shards.

## Interpretation Rules

- Treat code-level family inference as coarse guidance.
- The exported mint shards are the deeper source of truth.
- Look for repeated entry context, not just isolated PnL.
- Watch for discretionary-looking exceptions that pure parameter fits may miss.

## Suggested Agent Loop

1. Generate brief.
2. Generate clone report.
3. Read `index.json`.
4. Read a small set of representative mint shards.
5. Decide whether current family is good enough.
6. If yes, tune configs.
7. If no, create a new strategy variant.

## Validation

If you propose a clone:

```bash
cargo run -p pump-agent-cli -- clone-eval --address <PUBKEY> --strategy-config <CONFIG> --json
cargo run -p pump-agent-cli -- clone-rank --address <PUBKEY> --scan-limit 50 --top 10 --json
```
