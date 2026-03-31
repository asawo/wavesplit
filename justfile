set dotenv-load := true

# List available commands
default:
    @just --list

# Install all dependencies
install:
    pnpm install

# Start dev server (hot reload)
dev:
    pnpm run tauri dev

# Build release binary + installer
build:
    pnpm run tauri build

# Build Rust only (no frontend)
build-rust:
    source "$HOME/.cargo/env" && cargo build --manifest-path src-tauri/Cargo.toml

# Check Rust code without building
check:
    source "$HOME/.cargo/env" && cargo check --manifest-path src-tauri/Cargo.toml

# Run Rust tests
test:
    source "$HOME/.cargo/env" && cargo test --manifest-path src-tauri/Cargo.toml

# Format Rust code
fmt:
    source "$HOME/.cargo/env" && cargo fmt --manifest-path src-tauri/Cargo.toml

# Lint Rust code
lint:
    source "$HOME/.cargo/env" && cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings

# Open app data directory (macOS)
open-data:
    open "$HOME/Library/Application Support/com.wavesplit.app"

# Wipe app data (destructive — deletes DB and all tracks)
reset-data:
    rm -rf "$HOME/Library/Application Support/com.wavesplit.app"
