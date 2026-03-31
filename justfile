set dotenv-load := true

# List available commands
default:
    @just --list

# Install all dependencies
install:
    pnpm install

# Copy system yt-dlp and ffmpeg into src-tauri/binaries/ for local dev.
# Tauri requires externalBin files to exist at build time even in dev mode.
setup-bins:
    #!/usr/bin/env bash
    set -euo pipefail
    TARGET=$(rustc -vV | grep 'host:' | awk '{print $2}')
    BIN_DIR=src-tauri/binaries
    mkdir -p "$BIN_DIR"
    for tool in yt-dlp ffmpeg; do
        dest="$BIN_DIR/${tool}-${TARGET}"
        if [ ! -f "$dest" ]; then
            src=$(which "$tool" 2>/dev/null || true)
            if [ -z "$src" ]; then
                echo "ERROR: $tool not found — install with: brew install $tool"
                exit 1
            fi
            cp "$src" "$dest"
            echo "copied $src → $dest"
        fi
    done

# Start dev server (hot reload)
dev: setup-bins
    pnpm run tauri dev

# Build release binary + installer
build: setup-bins
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
