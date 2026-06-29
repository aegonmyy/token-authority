#!/usr/bin/env bash
# LP-0013 end-to-end demo — token authority lifecycle.
# Starts a standalone LEZ sequencer, deploys the guest ELF, then runs
# integration tests that exercise every instruction with RISC0_DEV_MODE=0
# (real RISC-Zero ZK proofs).
#
# Prerequisites (run once):
#   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
#   curl -L https://risczero.com/install | bash && rzup install
#   sudo apt-get install -y clang libclang-dev unzip python3-dev
#
#   # Build the LEZ node tools:
#   git clone https://github.com/logos-blockchain/logos-execution-zone ~/lez-node
#   cd ~/lez-node
#   cargo build --release --features standalone -p sequencer_service
#   cargo build --release -p wallet
#
# Then run from the repo root:
#   ./scripts/demo.sh
#
# Optional: set LEZ_NODE env var to override the default ~/lez-node path.
set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LEZ_NODE="${LEZ_NODE:-$HOME/lez-node}"
SEQUENCER_BIN="$LEZ_NODE/target/release/sequencer_service"
WALLET_BIN="$LEZ_NODE/target/release/wallet"
WALLET_HOME="$REPO_DIR/.demo-wallet"
SEQ_CONFIG="$LEZ_NODE/lez/sequencer/service/configs/debug/sequencer_config.json"
WALLET_CONFIG="$LEZ_NODE/lez/wallet/configs/debug/wallet_config.json"
GUEST_ELF="$REPO_DIR/token_authority.bin"

export RISC0_DEV_MODE=0
export RUST_LOG=warn
export LEE_WALLET_HOME_DIR="$WALLET_HOME"

# ── Preflight ─────────────────────────────────────────────────────────────────
for bin in "$SEQUENCER_BIN" "$WALLET_BIN"; do
    [ -f "$bin" ] || {
        echo "ERROR: $bin not found"
        echo "Build it with: cd ~/lez-node && cargo build --release --features standalone -p sequencer_service"
        exit 1
    }
done
[ -f "$GUEST_ELF" ] || {
    echo "ERROR: $GUEST_ELF not found"
    echo "Build with: cargo risczero build --manifest-path methods/guest/Cargo.toml"
    exit 1
}

echo "╔═══════════════════════════════════════════════════════════════╗"
echo "║  LP-0013 Token Authority — End-to-End Demo                   ║"
echo "║  RISC0_DEV_MODE=0  (real ZK proofs)                          ║"
echo "╚═══════════════════════════════════════════════════════════════╝"
echo ""

# ── Step 1: Start standalone sequencer ────────────────────────────────────────
echo "[1/4] Starting LEZ sequencer in standalone mode..."

rm -rf "$WALLET_HOME"
mkdir -p "$WALLET_HOME"
cp "$WALLET_CONFIG" "$WALLET_HOME/"

SEQUENCER_WORK="$REPO_DIR/.seq-workdir"
rm -rf "$SEQUENCER_WORK" && mkdir -p "$SEQUENCER_WORK"

cd "$SEQUENCER_WORK"
RISC0_DEV_MODE=0 RUST_LOG=warn "$SEQUENCER_BIN" "$SEQ_CONFIG" \
    > "$REPO_DIR/.seq.log" 2>&1 &
SEQ_PID=$!

cleanup() {
    kill "$SEQ_PID" 2>/dev/null || true
    rm -rf "$SEQUENCER_WORK" "$WALLET_HOME" "$REPO_DIR/.seq.log"
}
trap cleanup EXIT

sleep 5
echo "    Sequencer running (pid=$SEQ_PID, port=3040)"

# ── Step 2: Fund wallet ────────────────────────────────────────────────────────
echo "[2/4] Importing debug account and claiming genesis balance..."

"$WALLET_BIN" account import public \
    --private-key 7f273098f25b71e6c005a9519f2678da8d1c7f01f6a27778e2d9948abdf901fb \
    > /dev/null 2>&1

"$WALLET_BIN" vault claim \
    --account-id Public/CbgR6tj5kWx5oziiFptM7jMvrQeYY3Mzaao6ciuhSr2r \
    --amount 10000 \
    > /dev/null 2>&1

sleep 3
echo "    Account CbgR6t... funded ✓"

# ── Step 3: Deploy ─────────────────────────────────────────────────────────────
echo "[3/4] Deploying token_authority.bin to sequencer..."

"$WALLET_BIN" deploy-program "$GUEST_ELF" > /dev/null 2>&1
sleep 4

# The program ID equals the RISC-Zero image ID encoded as little-endian u32 words → hex.
# TOKEN_AUTHORITY_ID from methods/src/lib.rs (computed at build time):
PROGRAM_ID="4f0d73b40b59ed05c7a78fba0448975d79406dafc7b12cf8edb0ebfed74778f3"

echo "    program_id = $PROGRAM_ID"
echo "    Program deployed to standalone LEZ sequencer ✓"

# ── Step 4: Run integration tests with real ZK proofs ─────────────────────────
echo ""
echo "[4/4] Running integration tests (RISC0_DEV_MODE=0 — real ZK proofs)..."
echo "      Watch for: CU <instruction> cycles=N lines (compute unit benchmarks)"
echo ""

cd "$REPO_DIR"
RISC0_DEV_MODE=0 cargo test -p integration_tests -- --nocapture 2>&1

echo ""
echo "╔═══════════════════════════════════════════════════════════════╗"
echo "║  Demo complete                                                ║"
echo "║                                                               ║"
echo "║  fixed_supply_mint_rejected    ✓  (error 1003 as expected)   ║"
echo "║  variable_supply_full_lifecycle ✓  (mint → rotate → revoke)  ║"
echo "║  transfer_tokens               ✓  (sender → recipient)       ║"
echo "║  burn_tokens                   ✓  (supply + balance reduced)  ║"
echo "║                                                               ║"
echo "║  RISC0_DEV_MODE=0  (real ZK proofs)                    ✓     ║"
echo "║  program_id = $PROGRAM_ID       ║"
echo "╚═══════════════════════════════════════════════════════════════╝"
