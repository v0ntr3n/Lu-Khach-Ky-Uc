#!/usr/bin/env bash
# ==============================================================================
# Teamobi2026 Safe Rust Server — One-Click Launcher (Linux / WSL2 / macOS)
# Boots `nro-server` on TCP 14445 with embedded `fjall` + `SRC/data` streaming.
# Usage:
#   ./start.sh                 # Auto-builds (when Cargo.toml is present) and runs
#   ./start.sh --rebuild       # Forces a fresh `cargo build --release` before launch
#   ./start.sh --bind 0.0.0.0:14449
# ==============================================================================
set -euo pipefail

cd "$(dirname "$0")"

if [[ -f "$HOME/.cargo/env" ]]; then
    # shellcheck disable=SC1091
    source "$HOME/.cargo/env"
fi

CONFIG_FILE="${NRO_CONFIG:-server.toml}"

ASSETS_DIR="SRC/data"
if [[ -n "${NRO_ASSETS_DIR:-}" ]]; then
    ASSETS_DIR="$NRO_ASSETS_DIR"
else
    for cand in "SRC/data" "Copy of Teamobi2026/SRC/data" "Teamobi2026/SRC/data" "data"; do
        if [[ -d "${cand}/update_data" || -d "${cand}/icon" ]]; then
            ASSETS_DIR="$cand"
            break
        fi
    done
fi

CMD_ARGS=(--config "$CONFIG_FILE" --assets-dir "$ASSETS_DIR")

REBUILD=0
for arg in "$@"; do
    if [[ "$arg" == "--rebuild" ]]; then
        REBUILD=1
    else
        CMD_ARGS+=("$arg")
    fi
done

run_cargo_release() {
    if cargo build --release -p nro-server -p nro-db; then
        return 0
    elif command -v rustup >/dev/null 2>&1 && rustup toolchain list | grep -q '^1\.100\.0'; then
        cargo +1.100.0 build --release -p nro-server -p nro-db
    elif command -v rustup >/dev/null 2>&1 && rustup toolchain list | grep -q '^nightly'; then
        cargo +nightly build --release -p nro-server -p nro-db
    else
        return 1
    fi
}

SERVER_BIN=""
if [[ "$REBUILD" -eq 0 && -f "./nro-server" ]]; then
    chmod +x "./nro-server" 2>/dev/null || true
    SERVER_BIN="./nro-server"
elif [[ -f "Cargo.toml" ]] && command -v cargo >/dev/null 2>&1; then
    echo "=== Ensuring nro-server & nro-import-sql release build is up to date ==="
    if ! run_cargo_release; then
        echo "[ERROR] Release build failed. Ensure Rust 1.100.0+ is installed: rustup toolchain install 1.100.0" >&2
        exit 1
    fi
    SERVER_BIN="./target/release/nro-server"
elif [[ -x "./target/release/nro-server" ]]; then
    SERVER_BIN="./target/release/nro-server"
else
    echo "[ERROR] Neither prebuilt ./nro-server nor Cargo toolchain was found." >&2
    echo "        Install Rust 1.100.0+ via https://rustup.rs/ or place ./nro-server in $(pwd)." >&2
    exit 1
fi

if [[ ! -d "${ASSETS_DIR}/update_data" && ! -d "${ASSETS_DIR}/icon" ]]; then
    echo "[WARN] Binary client assets not found at '${ASSETS_DIR}'." >&2
    echo "       Copy Teamobi2026 'SRC/data' into $(pwd)/SRC/data before connecting with Mod Local." >&2
fi

echo "=== Starting Teamobi2026 nro-server (Ctrl-C or type 'baotri' to save & stop) ==="
exec "$SERVER_BIN" "${CMD_ARGS[@]}"
