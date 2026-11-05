# Thin wrappers over the commands in AGENTS.md. Run `make help` for targets.
M := src-tauri/Cargo.toml
APP := src-tauri/target/debug/bundle/macos/Narihito's Diary.app/Contents/MacOS/diary-client

.PHONY: help install dev build bundle bundle-smoke fmt fmt-check clippy lint test test-rust test-ui \
	typecheck versions gate smoke exclusion clean

help: ## List targets
	@grep -E '^[a-z-]+:.*##' $(MAKEFILE_LIST) | awk -F':.*## ' '{printf "%-14s %s\n", $$1, $$2}'

install: ## Install npm dependencies from the lockfile
	npm ci

dev: ## Run the app in development
	npm run tauri dev

build: ## Frontend production build (version check, tsc, vite)
	npm run build

bundle: ## Ordinary debug .app bundle (no fixtures)
	npm run tauri build -- --debug --bundles app

bundle-smoke: ## Debug .app bundle with the scaffold-smoke fixture feature
	npm run tauri build -- --debug --bundles app --features scaffold-smoke

fmt: ## Format Rust and the rest (writes files)
	cargo fmt --manifest-path $(M)
	npx prettier --write .

fmt-check: ## Check formatting (G01, G04)
	cargo fmt --manifest-path $(M) --check
	npx prettier --check .

clippy: ## Rust lint (G02)
	cargo clippy --manifest-path $(M) --all-targets -- -D warnings

lint: ## ESLint (G05)
	npx eslint . --max-warnings 0

test-rust: ## Rust tests (G03)
	cargo test --manifest-path $(M)

test-ui: ## UI tests (G06)
	npx vitest run

test: test-rust test-ui ## All tests

typecheck: ## TypeScript check
	npm run typecheck

versions: ## Check package/Cargo/Tauri versions agree
	npm run check:versions

gate: ## Full gate G01-G06: parallel, quiet, PASS/FAIL per command
	scripts/gate.sh

smoke: bundle-smoke ## Packaged native smoke (needs a GUI session)
	python3 scripts/smoke/server.py "$(APP)"

exclusion: bundle ## Rebuild ordinary bundle and verify fixture markers are absent
	cargo build --manifest-path $(M) -p diary-server
	node scripts/check-fixture-exclusion.mjs

clean: ## Remove build output (dist and cargo target)
	cargo clean --manifest-path $(M)
	rm -rf dist
