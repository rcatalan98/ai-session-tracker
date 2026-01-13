.PHONY: setup deps hooks dev check smoke test lint fmt build clean help

# ============================================
# SETUP
# ============================================

setup: deps hooks  ## First-time setup
	@echo "✓ Setup complete. Run 'make dev' to start."

deps:  ## Install dependencies
	cargo build

hooks:  ## Install git hooks
	@git config core.hooksPath .githooks
	@chmod +x .githooks/* 2>/dev/null || true
	@echo "✓ Git hooks installed"

# ============================================
# DEVELOPMENT
# ============================================

dev:  ## Build and run (analyze command)
	cargo run -- analyze

run:  ## Run with arguments (usage: make run ARGS="bottlenecks --limit 5")
	cargo run -- $(ARGS)

check:  ## Fast type check (no build)
	cargo check

smoke:  ## Fast validation (~10s) - runs on pre-commit
	@echo "[1/2] Checking compilation..."
	@cargo check --quiet
	@echo "[2/2] Running unit tests..."
	@cargo test --lib --quiet 2>/dev/null || cargo test --quiet
	@echo "✓ Smoke tests passed"

# ============================================
# BUILD & TEST
# ============================================

test:  ## Run all tests
	cargo test

lint:  ## Check code style
	cargo fmt --check
	cargo clippy -- -D warnings

fmt:  ## Format code
	cargo fmt

build:  ## Production build
	cargo build --release
	@echo "Binary at: target/release/aist"

clean:  ## Clean build artifacts
	cargo clean

# ============================================
# INSTALL
# ============================================

install: build  ## Install to ~/.cargo/bin
	cargo install --path .
	@echo "✓ Installed. Run 'aist --help'"

# ============================================
# HELP
# ============================================

help:
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-12s\033[0m %s\n", $$1, $$2}'

.DEFAULT_GOAL := help
