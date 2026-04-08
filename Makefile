.PHONY: \
	fmt fmt-check lint test test-doc ci help \
	scaffold scaffold-rust replay-db sweep-db live-paper ingest dashboard \
	stats tail runs \
	clone-brief clone-report clone-scaffold clone-eval clone-rank

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all --check

lint:
	cargo clippy --workspace --all-targets -- -D warnings

test:
	cargo test --workspace

test-doc:
	cargo test --workspace --doc

ci: fmt-check lint test test-doc

help:
	@printf "%s\n" \
	"make ci" \
	"make ingest" \
	"make replay-db" \
	"make sweep-db" \
	"make live-paper" \
	"make stats" \
	"make tail" \
	"make runs" \
	"make scaffold" \
	"make scaffold-rust NAME=alpha_flow" \
	"make clone-brief ADDRESS=<PUBKEY>" \
	"make clone-report ADDRESS=<PUBKEY>" \
	"make clone-scaffold ADDRESS=<PUBKEY>" \
	"make clone-eval ADDRESS=<PUBKEY> CONFIG=./strategies/examples/early-flow.toml" \
	"make clone-rank ADDRESS=<PUBKEY>"

scaffold:
	cargo run -p pump-agent-cli -- strategy-scaffold --strategy early_flow --output ./strategies/new-strategy.toml

scaffold-rust:
	cargo run -p pump-agent-cli -- strategy-scaffold --name $(NAME) --strategy early_flow --output ./strategies/$(NAME).toml

replay-db:
	cargo run -p pump-agent-cli -- replay-db --save-run --strategy-config ./strategies/examples/early-flow.toml

sweep-db:
	cargo run -p pump-agent-cli -- sweep-db --strategy early_flow --strategy-config ./strategies/examples/early-flow.toml

live-paper:
	cargo run -p pump-agent-cli -- live-paper --strategy-config ./strategies/examples/early-flow.toml --save-run

ingest:
	cargo run -p pump-agent-cli -- ingest

dashboard:
	cargo run -p pump-agent-cli -- serve-dashboard --port 3000

stats:
	cargo run -p pump-agent-cli -- stats

tail:
	cargo run -p pump-agent-cli -- tail --limit 20

runs:
	cargo run -p pump-agent-cli -- runs --limit 20

clone-brief:
	cargo run -p pump-agent-cli -- address-brief --address $(ADDRESS) --json --export

clone-report:
	cargo run -p pump-agent-cli -- clone-report --address $(ADDRESS) --json --export

clone-scaffold:
	cargo run -p pump-agent-cli -- clone-scaffold --address $(ADDRESS)

clone-eval:
	cargo run -p pump-agent-cli -- clone-eval --address $(ADDRESS) --strategy-config $(CONFIG) --json

clone-rank:
	cargo run -p pump-agent-cli -- clone-rank --address $(ADDRESS) --scan-limit 50 --top 10 --json
