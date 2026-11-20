#!/usr/bin/env bash
# ==============================================================================
# Teamobi2026 Safe Rust Server — Google Colab / Ubuntu Verification & 4-Stage PGO
# Target Toolchain: Rust 1.100.0 (edition = "2024", resolver = "3", Embedded fjall)
# ==============================================================================
set -euo pipefail

cd "$(dirname "$0")"

echo "=== [Stage 0/5] Ensuring Rust 1.100.0 + llvm-tools-preview + clippy ==="
if ! command -v rustup >/dev/null 2>&1; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.100.0
    # shellcheck disable=SC1091
    source "$HOME/.cargo/env"
fi
rustup toolchain install 1.100.0 --profile minimal --component clippy --component rustfmt --component llvm-tools-preview
rustup default 1.100.0
rustc --version
cargo --version

SYSROOT="$(rustc --print sysroot)"
HOST_TRIPLE="$(rustc -vV | sed -n 's/^host: //p')"
LLVM_PROFDATA="${SYSROOT}/lib/rustlib/${HOST_TRIPLE}/bin/llvm-profdata"

echo "=== [Stage 1/5] Strict Workspace Typecheck & Clippy Gate (#![forbid(unsafe_code)]) ==="
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets

echo "=== [Stage 2/5] Running Full Workspace Unit, Proptest & fjall E2E Suites ==="
cargo test --workspace

echo "=== [Stage 3/5] Baseline Release Build (nro-server + nro-import-sql) & Replay Timing ==="
cargo build --release -p nro-server -p nro-db
./target/release/nro-server --benchmark-replay --replay-scale 2500

echo "=== [Stage 4/5] LLVM Profile-Guided Optimization (PGO) Instrumentation Run ==="
PGO_DIR="$(pwd)/target/pgo-profiles"
rm -rf "$PGO_DIR"
mkdir -p "$PGO_DIR"

RUSTFLAGS="-Cprofile-generate=${PGO_DIR}" \
    cargo build --release -p nro-server -p nro-db --target "$HOST_TRIPLE"

"./target/${HOST_TRIPLE}/release/nro-server" --benchmark-replay --replay-scale 4000
"$LLVM_PROFDATA" merge -o "${PGO_DIR}/merged.profdata" "$PGO_DIR"

echo "=== [Stage 5/5] Final LLVM PGO-Optimized Server & Importer Binary Build ==="
RUSTFLAGS="-Cprofile-use=${PGO_DIR}/merged.profdata -Cllvm-args=-pgo-warn-missing-function" \
    cargo build --release -p nro-server -p nro-db --target "$HOST_TRIPLE"

"./target/${HOST_TRIPLE}/release/nro-server" --benchmark-replay --replay-scale 2500
ls -lh "./target/${HOST_TRIPLE}/release/nro-server" "./target/${HOST_TRIPLE}/release/nro-import-sql"
echo "=== DONE: PGO-optimized binaries ready at ./target/${HOST_TRIPLE}/release/ ==="
