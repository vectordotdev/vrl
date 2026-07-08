.PHONY: help all clippy fmt fmt-check test vrl-test typos \
	check-features check-licenses check-msrv check-deny check-wasm32 \
	check-docs check-lockfile generate-docs

CARGO_DENY_VERSION        := 0.18.9
CARGO_HACK_VERSION        := 0.5.29
CARGO_MSRV_VERSION        := 0.17.1
DD_LICENSE_TOOL_VERSION   := 1.0.6

# Install a cargo tool at a pinned version if not already installed.
# $(1) = tool name, $(2) = version
define ensure-cargo-tool
	@if ! cargo install --list | grep -q "^$(1) v$(2):"; then \
		echo "Installing $(1) v$(2)"; \
		cargo install $(1) --version $(2) --force --locked; \
	fi
endef

help: ## Show available targets
	@awk 'BEGIN {FS = ":.*##"} /^[a-zA-Z_-]+:.*##/ {printf "  \033[36m%-20s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

all: fmt-check clippy test vrl-test check-licenses check-wasm32 ## Run the common pre-PR checks

clippy: ## Lint with clippy (workspace, all targets, all features)
	cargo clippy --workspace --all-targets --all-features -- -D warnings

fmt: ## Format all code
	cargo fmt --all

fmt-check: ## Check formatting
	cargo fmt --check --all

test: ## Run cargo tests
	cargo test --workspace

vrl-test: ## Run VRL integration tests
	cargo run --package vrl-tests --bin vrl-tests

typos: ## Check spelling with `typos` (must be installed)
	typos

check-features: ## Verify all feature combinations compile
	$(call ensure-cargo-tool,cargo-hack,$(CARGO_HACK_VERSION))
	cargo hack check --feature-powerset --depth 1

check-licenses: ## Verify the 3rd-party license file is up to date
	$(call ensure-cargo-tool,dd-rust-license-tool,$(DD_LICENSE_TOOL_VERSION))
	dd-rust-license-tool check

check-msrv: ## Verify MSRV
	$(call ensure-cargo-tool,cargo-msrv,$(CARGO_MSRV_VERSION))
	cargo msrv verify

check-deny: ## Run cargo-deny checks
	$(call ensure-cargo-tool,cargo-deny,$(CARGO_DENY_VERSION))
	cargo deny --log-level error --all-features check all

check-wasm32: ## Check wasm32-unknown-unknown build of the stdlib
	rustup target add wasm32-unknown-unknown
	cargo check --target wasm32-unknown-unknown --no-default-features --features stdlib

check-docs: ## Check that rustdoc generates cleanly
	cargo doc --no-deps --workspace

check-lockfile: ## Verify Cargo.lock is in sync with Cargo.toml
	cargo update --workspace --locked

generate-docs: ## Regenerate docs/generated/
	cargo run -p vrl-docs -- --output docs/generated/
