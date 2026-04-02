set dotenv-load := true

# List available commands
default:
    @just --list

# Install all dependencies
install:
    pnpm install

# Copy system yt-dlp, ffmpeg, ffprobe into src-tauri/binaries/ for local dev.
# Uses dynamically-linked system binaries — fine for dev since you have them installed.
setup-bins:
    #!/usr/bin/env bash
    set -euo pipefail
    TARGET=$(rustc -vV | grep 'host:' | awk '{print $2}')
    BIN_DIR=src-tauri/binaries
    mkdir -p "$BIN_DIR"
    for tool in yt-dlp ffmpeg ffprobe; do
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

# Download static (self-contained) ffmpeg + ffprobe for release builds.
# Unlike setup-bins, these don't depend on Homebrew being installed on the user's machine.
setup-bins-release:
    #!/usr/bin/env bash
    set -euo pipefail
    TARGET=$(rustc -vV | grep 'host:' | awk '{print $2}')
    BIN_DIR=src-tauri/binaries
    mkdir -p "$BIN_DIR"

    # yt-dlp standalone binary
    YTDLP_DEST="$BIN_DIR/yt-dlp-${TARGET}"
    if [ ! -f "$YTDLP_DEST" ]; then
        echo "Downloading yt-dlp..."
        curl -fsSL https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_macos \
            -o "$YTDLP_DEST"
        chmod +x "$YTDLP_DEST"
    fi

    # ffmpeg + ffprobe: use ffbinaries static build for x86_64.
    # No static arm64 build is available for macOS from any public source — CI uses
    # the FedericoCarboni/setup-ffmpeg action which handles this. Local release builds
    # on Apple Silicon will use the Homebrew binary (dynamically linked) which works
    # on machines with Homebrew but not on clean installs. Use CI artifacts for distribution.
    case "$TARGET" in
        x86_64-apple-darwin)
            FFBIN=$(curl -fsSL https://ffbinaries.com/api/v1/version/latest)
            for tool in ffmpeg ffprobe; do
                dest="$BIN_DIR/${tool}-${TARGET}"
                if [ ! -f "$dest" ]; then
                    echo "Downloading static $tool (osx-64)..."
                    URL=$(echo "$FFBIN" | python3 -c "import sys,json; print(json.load(sys.stdin)['bin']['osx-64']['$tool'])")
                    curl -fsSL "$URL" -o /tmp/${tool}.zip
                    unzip -o /tmp/${tool}.zip "$tool" -d /tmp
                    cp /tmp/$tool "$dest"
                    chmod +x "$dest"
                    rm /tmp/${tool}.zip /tmp/${tool}
                fi
            done
            ;;
        aarch64-apple-darwin)
            echo "WARNING: no static arm64 ffmpeg available locally — using Homebrew (dynamic)."
            echo "         Local builds will not work on machines without Homebrew. Use CI for distribution."
            for tool in ffmpeg ffprobe; do
                dest="$BIN_DIR/${tool}-${TARGET}"
                if [ ! -f "$dest" ]; then
                    src=$(which "$tool" 2>/dev/null || true)
                    [ -z "$src" ] && echo "ERROR: $tool not found — brew install ffmpeg" && exit 1
                    cp "$src" "$dest"
                fi
            done
            ;;
        *)
            echo "ERROR: unsupported target $TARGET"
            exit 1
            ;;
    esac

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

# Run all CI checks locally (clippy + test)
ci: lint test

# Open app data directory (macOS)
open-data:
    open "$HOME/Library/Application Support/com.wavesplit.app"

# Wipe app data (destructive — deletes DB and all tracks)
reset-data:
    rm -rf "$HOME/Library/Application Support/com.wavesplit.app"

# Delete the cached demucs binary so it re-downloads on next launch
reset-demucs:
    rm -f "$HOME/Library/Application Support/com.wavesplit.app/demucs/demucs"

# Build and test the demucs sidecar locally. Usage: just test-demucs /path/to/audio.wav
test-demucs audio:
    #!/usr/bin/env bash
    set -euo pipefail

    VENV=/tmp/demucs-build-venv

    if [ ! -d "$VENV" ]; then
        echo "Creating venv..."
        python3.11 -m venv "$VENV"
        "$VENV/bin/pip" install torch==2.5.0 torchaudio==2.5.0 --index-url https://download.pytorch.org/whl/cpu
        "$VENV/bin/pip" install "numpy<2" demucs pyinstaller certifi soundfile
    fi

    echo "Building demucs binary..."
    cd src/analysis
    "$VENV/bin/pyinstaller" \
        --onefile \
        --collect-all demucs \
        --collect-all certifi \
        --collect-all soundfile \
        --collect-binaries torch \
        --name demucs-local-test \
        demucs_runner.py

    echo "Running separation on {{ audio }}..."
    TORCH_HOME="$HOME/Library/Application Support/com.wavesplit.app/demucs/cache" \
        dist/demucs-local-test --name htdemucs -o /tmp/demucs-test-out "{{ audio }}"

    echo ""
    echo "Output stems:"
    ls /tmp/demucs-test-out/htdemucs/*/

# Release a new version. Usage: just release 0.2.0
# Bumps versions, commits, tags, and pushes. CI builds artifacts and creates a draft release.
release version:
    #!/usr/bin/env bash
    set -euo pipefail

    if ! echo "{{ version }}" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+$'; then
        echo "ERROR: version must be semver (e.g. 0.2.0)"
        exit 1
    fi

    if ! git diff --quiet || ! git diff --cached --quiet; then
        echo "ERROR: working tree has uncommitted changes"
        exit 1
    fi

    if git rev-parse "v{{ version }}" >/dev/null 2>&1; then
        echo "ERROR: tag v{{ version }} already exists"
        exit 1
    fi

    echo "Bumping to {{ version }}..."

    # Cargo.toml — safe: all deps use inline { version = "..." } syntax
    sed -i '' 's/^version = "[^"]*"/version = "{{ version }}"/' src-tauri/Cargo.toml

    # JSON files — node for reliable parsing
    node -e "
      const fs = require('fs');
      for (const f of ['src-tauri/tauri.conf.json', 'package.json']) {
        const p = JSON.parse(fs.readFileSync(f, 'utf8'));
        p.version = '{{ version }}';
        fs.writeFileSync(f, JSON.stringify(p, null, 2) + '\n');
      }
    "

    # Sync Cargo.lock
    source "$HOME/.cargo/env" && cargo update --manifest-path src-tauri/Cargo.toml --package wavesplit

    git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json package.json
    git commit -m "chore: release v{{ version }}"
    git tag -a "v{{ version }}" -m "v{{ version }}"
    git push origin main --follow-tags

    echo ""
    echo "CI is building artifacts and will create a draft release."
    echo "Publish at: https://github.com/asawo/wavesplit/releases"
