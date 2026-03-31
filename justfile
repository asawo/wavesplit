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

# Release a new version. Usage: just release 0.2.0
# Bumps versions in Cargo.toml, tauri.conf.json, and package.json,
# commits, pushes, and publishes a GitHub release (which triggers the build workflow).
release version:
    #!/usr/bin/env bash
    set -euo pipefail

    # Validate semver format
    if ! echo "{{ version }}" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+$'; then
        echo "ERROR: version must be semver (e.g. 0.2.0)"
        exit 1
    fi

    # Ensure working tree is clean
    if ! git diff --quiet || ! git diff --cached --quiet; then
        echo "ERROR: working tree has uncommitted changes"
        exit 1
    fi

    echo "Bumping to {{ version }}..."

    # Cargo.toml (deps use inline { version = "..." } so this only matches [package])
    sed -i '' 's/^version = "[^"]*"/version = "{{ version }}"/' src-tauri/Cargo.toml

    # tauri.conf.json
    sed -i '' 's/"version": "[^"]*"/"version": "{{ version }}"/' src-tauri/tauri.conf.json

    # package.json
    sed -i '' 's/"version": "[^"]*"/"version": "{{ version }}"/' package.json

    # Update Cargo.lock
    source "$HOME/.cargo/env" && cargo update --manifest-path src-tauri/Cargo.toml --package wavesplit

    git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json package.json
    git commit -m "chore: release v{{ version }}"
    git push origin main

    gh release create "v{{ version }}" \
        --title "Wavesplit v{{ version }}" \
        --generate-notes
