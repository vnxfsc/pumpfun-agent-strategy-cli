---
name: wallet-clone-analysis
description: Use when analyzing a Pump wallet, exporting its touched mints, inferring rough strategy families, reading clone reports, or preparing context for an agent to reverse-engineer strategy behavior.
---

# Wallet Clone Analysis

Use this skill when the task is about:

- analyzing one wallet address
- understanding roundtrips and entry behavior
- exporting touched mints for deeper drill-down
- generating clone reports
- deciding whether existing strategy families are sufficient
- preparing structured context for an agent

## Preferred Order

1. `wallet-dossier`
2. `clone-report`
3. `explain-why`
4. `suggest-next-experiment`
5. `mint-shard-summary`
6. raw shard export only if the higher-level outputs are still ambiguous

## Primary Commands

Wallet dossier:

```bash
cargo run -p pump-agent-cli -- wallet-dossier --address <PUBKEY>
```

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

Why the family fit looks this way:

```bash
cargo run -p pump-agent-cli -- explain-why --address <PUBKEY>
```

What experiment to try next:

```bash
cargo run -p pump-agent-cli -- suggest-next-experiment --address <PUBKEY>
```

Mint-level wallet summary:

```bash
cargo run -p pump-agent-cli -- mint-shard-summary --address <PUBKEY> --limit 20
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
- Prefer dossier, report, explanation, and suggestion outputs before raw shards.
- Exported mint shards are the deeper source of truth when the structured summaries are insufficient.
- Look for repeated entry context, not just isolated PnL.
- Watch for discretionary-looking exceptions that pure parameter fits may miss.
- Prefer machine-readable outputs for automation:
  - global `--format json` where supported
  - command-local `--json` on the older compatibility paths

## Suggested Agent Loop

1. Generate `wallet-dossier`.
2. Generate `clone-report`.
3. Read `explain-why`.
4. Read `suggest-next-experiment`.
5. If still unclear, read `index.json`.
6. Read a small set of representative mint shards.
7. Decide whether current family is good enough.
8. If yes, tune configs.
9. If no, create a new strategy variant.

## Validation

If you propose a clone:

```bash
cargo run -p pump-agent-cli -- clone-eval --address <PUBKEY> --strategy-config <CONFIG> --json
cargo run -p pump-agent-cli -- clone-rank --address <PUBKEY> --scan-limit 50 --top 10 --json
```
