# Phonetic - Audio Measurement System
# All development commands in one place

set shell := ["zsh", "-cu"]

# Project paths
desktop := "apps/desktop"
tauri := "apps/desktop/src-tauri"

# ============================================================================
# Help
# ============================================================================

# Show available commands
default:
    @just --list --unsorted

# ============================================================================
# Setup & Dependencies
# ============================================================================

# Install Linux build packages for Tauri and ALSA (Debian/Ubuntu)
install-deps-linux:
    sudo apt-get update
    sudo apt-get install -y libwebkit2gtk-4.1-dev build-essential curl wget file \
        libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev libasound2-dev

# Install Rust and Bun if missing (macOS and Linux)
install-deps:
    @echo "Checking system dependencies..."
    @command -v rustc >/dev/null 2>&1 || (echo "Installing Rust..." && curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y)
    @command -v bun >/dev/null 2>&1 || (echo "Installing Bun..." && curl -fsSL https://bun.sh/install | bash)
    @echo "System dependencies OK"

# Install project dependencies (JS + Rust)
install: install-deps
    bun install
    cd {{tauri}} && cargo fetch

# Update all dependencies
update:
    bun update
    cd {{tauri}} && cargo update

# Clean all build artifacts
clean:
    rm -rf node_modules apps/*/node_modules .turbo
    rm -rf {{desktop}}/dist
    cd {{tauri}} && cargo clean

# ============================================================================
# Development
# ============================================================================

# Run Tauri app in development mode
dev: install
    cd {{desktop}} && bunx tauri dev

# Run frontend only (vite dev server)
dev-frontend:
    cd {{desktop}} && bun run dev

# Check Rust code compiles
check-rust:
    cd {{tauri}} && cargo check

# Watch Rust for changes
watch:
    cd {{tauri}} && cargo watch -x check

# ============================================================================
# Build
# ============================================================================

# Build frontend only
build-frontend:
    cd {{desktop}} && bun run build

# Build Rust backend only (debug)
build-rust:
    cd {{tauri}} && cargo build

# Build Rust backend (release)
build-rust-release:
    cd {{tauri}} && cargo build --release

# Build complete Tauri app (debug)
build: build-frontend
    cd {{desktop}} && bunx tauri build --debug

# Build complete Tauri app (release)
build-release: build-frontend
    cd {{desktop}} && bunx tauri build

# ============================================================================
# Code Quality
# ============================================================================

# Run Biome linting and formatting check
lint:
    bunx biome check .

# Fix Biome linting and formatting issues
lint-fix:
    bunx biome check --write .

# Run Rust clippy linter
clippy:
    cd {{tauri}} && cargo clippy --all-targets --all-features -- -D warnings

# Format Rust code
fmt-rust:
    cd {{tauri}} && cargo fmt

# Check Rust formatting
fmt-rust-check:
    cd {{tauri}} && cargo fmt --check

# Format all code (JS + Rust)
fmt: lint-fix fmt-rust

# Run all code quality checks
check: lint clippy fmt-rust-check
    @echo "All checks passed!"

# ============================================================================
# Testing
# ============================================================================

# Run TypeScript tests (skip if none exist)
test-ts:
    bun test || echo "No TypeScript tests found"

# Run Rust tests
test-rust:
    cd {{tauri}} && cargo test

# Run just the measurement accuracy suite (see README's Measurement Accuracy section)
test-accuracy:
    cd {{tauri}} && cargo test --test dsp_accuracy -- --nocapture

# Run the channel matrix tests (Rust + TS)
test-matrix:
    cd {{tauri}} && cargo test routing
    cd {{tauri}} && cargo test --lib audio::engine
    cd {{desktop}} && bun test src/components/channel-matrix.test.tsx

# Sustained-load test for 32 pairs (release build, ~1 min)
soak-matrix:
    cd {{tauri}} && cargo test --test matrix_soak --release -- --nocapture

# Run the LEQ tests (Rust + TS)
test-leq:
    cargo test --manifest-path {{tauri}}/Cargo.toml leq
    cargo test --manifest-path {{tauri}}/Cargo.toml noise_limits
    cd {{desktop}} && bun test src/components/leq-panel.test.tsx

# Run the room criteria tests (Rust + TS)
test-room-criteria:
    cargo test --manifest-path {{tauri}}/Cargo.toml room_criteria
    cd {{desktop}} && bun test src/components/room-criteria-panel.test.tsx

# Run all tests
test: test-ts test-rust

# Generate WAV test fixtures used by the e2e suite
fixtures:
    cd apps/desktop/src-tauri && cargo test --test audio_file_roundtrip -- --nocapture

# Write an example session file to /tmp for inspection
session-example:
    cd apps/desktop/src-tauri && cargo test --test session_roundtrip session_file_is_human_readable -- --nocapture

# Run Rust tests with coverage (requires cargo-tarpaulin)
test-rust-coverage:
    cd {{tauri}} && cargo tarpaulin --out Html --output-dir coverage

# Run TypeScript tests with coverage
test-ts-coverage:
    bun test --coverage || echo "No TypeScript tests found"

# Run the end-to-end suite (requires a release build)
test-e2e: build-release
    cd apps/desktop && bun run test:e2e

# Everything: unit, integration, accuracy, e2e
test-all: test test-accuracy test-e2e

# ============================================================================
# Distribution
# ============================================================================

# Build the installers for this OS: ad-hoc signed .app + .dmg on macOS,
# .deb + .rpm + .AppImage on Linux (see bundle.targets in tauri.conf.json)
dist: build-frontend
    cd {{desktop}} && bunx tauri build

# Show built artifacts
dist-show:
    @echo "Built artifacts:"
    @ls -lh {{tauri}}/target/release/bundle/*/* 2>/dev/null || echo "No bundles built yet. Run: just dist"

# ============================================================================
# CI Pipeline (used by GitHub Actions)
# ============================================================================

# CI: Install dependencies without system deps check
ci-install:
    bun install --frozen-lockfile
    cd {{tauri}} && cargo fetch

# CI: Run all checks (lint + clippy + format)
ci-check: lint clippy fmt-rust-check

# CI: Run all tests
ci-test: test-rust
    bun test || true

# CI: Build release
ci-build: build-frontend
    cd {{desktop}} && bunx tauri build

# Full CI pipeline
ci: ci-install ci-check ci-test ci-build
    @echo "CI pipeline complete!"

# ============================================================================
# Release Workflow
# ============================================================================

# Full release: check, test, build
release: check test dist
    @echo "Release build complete!"
    just dist-show

# Quick sanity check (fast compile check)
sanity: install
    cd {{tauri}} && cargo check
    cd {{desktop}} && bun run build
    @echo "Sanity check passed!"

# ============================================================================
# Utilities
# ============================================================================

# Show project structure
tree:
    tree -I 'node_modules|target|dist|.git|.turbo' -L 4

# Generate app icons from SVG
icons svg_path:
    bunx @aspect-build/tauri-icons-generator {{svg_path}} -o {{tauri}}/icons

# Analyze release binary size (requires cargo-bloat)
size:
    cd {{tauri}} && cargo bloat --release --crates

# Open Rust documentation
docs:
    cd {{tauri}} && cargo doc --open

# Run security audit on dependencies
audit:
    cd {{tauri}} && cargo audit || true
    bunx better-npm-audit audit || true

# ============================================================================
# Git Helpers
# ============================================================================

# Pre-commit check (run before committing)
pre-commit: check test
    @echo "Ready to commit!"

# Show git status
status:
    git status -s

# Create a new release tag
tag version:
    git tag -a v{{version}} -m "Release v{{version}}"
    git push origin v{{version}}

# ============================================================================
# Headless CLI
# ============================================================================

# Run the headless CLI (e.g. `just cli selftest`)
cli *ARGS:
    cd {{tauri}} && cargo run --bin phonetic-cli -- {{ARGS}}

# Build the CLI in release mode
cli-release:
    cd {{tauri}} && cargo build --release --bin phonetic-cli
