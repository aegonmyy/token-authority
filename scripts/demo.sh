#!/usr/bin/env bash
# LP-0013 end-to-end demo — token authority lifecycle against a REAL local sequencer.
#
# Starts a standalone LEZ sequencer, deploys the guest ELF to it, then drives the
# full mint-authority lifecycle THROUGH that sequencer over the CLI with
# RISC0_DEV_MODE=0 (real RISC-Zero ZK proofs):
#
#   create -> mint -> rotate authority -> mint by rotated authority
#          -> revoke -> post-revoke mint REJECTED (error 1003)
#
# Every step submits a real transaction to the sequencer and prints its tx hash;
# the post-revoke mint is rejected on-chain by the authority guard. This is the
# same lifecycle script (scripts/testnet-lifecycle.sh) used to produce the public
# testnet evidence, here pointed at the local sequencer instead of the testnet.
#
# The fast in-process suite (cargo test -p integration_tests) still exists and
# runs in CI; this script is the on-a-real-sequencer end-to-end demo.
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
#   # Build the SPEL CLI (vendored at the pinned a58fbce2 tag):
#   cargo build --release --manifest-path vendor/spel-framework/spel-cli/Cargo.toml
#
# Then run from the repo root:
#   ./scripts/demo.sh
#
# Optional env overrides:
#   LEZ_NODE      path to the built LEZ node   (default ~/lez-node)
#   SPEL          path to the spel CLI binary  (default vendor build)
#   STEP_TIMEOUT  per-tx confirmation timeout  (default 600s for local proving)
set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LEZ_NODE="${LEZ_NODE:-$HOME/lez-node}"
SEQ_PORT="${SEQ_PORT:-3040}"
SEQUENCER_BIN="$LEZ_NODE/target/release/sequencer_service"
WALLET_BIN="$LEZ_NODE/target/release/wallet"
SPEL_BIN="${SPEL:-$REPO_DIR/vendor/spel-framework/spel-cli/target/release/spel}"
WALLET_HOME="${WALLET_HOME:-$REPO_DIR/.demo-wallet}"
SEQ_CONFIG="$LEZ_NODE/lez/sequencer/service/configs/debug/sequencer_config.json"
WALLET_CONFIG="$LEZ_NODE/lez/wallet/configs/debug/wallet_config.json"
GUEST_ELF="$REPO_DIR/token_authority.bin"

# The LEZ wallet prompts "Input password:" on a TTY; on EOF it uses an empty
# password and proceeds. Feed the whole script /dev/null on stdin so the wallet
# (and spel, which uses it) never block on a hidden prompt during the demo.
exec < /dev/null

export RISC0_DEV_MODE=0
export RUST_LOG=warn
export LEE_WALLET_HOME_DIR="$WALLET_HOME"

# ── Preflight ─────────────────────────────────────────────────────────────────
for bin in "$SEQUENCER_BIN" "$WALLET_BIN"; do
    [ -f "$bin" ] || {
        echo "ERROR: $bin not found"
        echo "Build it with: cd ~/lez-node && cargo build --release --features standalone -p sequencer_service && cargo build --release -p wallet"
        exit 1
    }
done
[ -f "$SPEL_BIN" ] || {
    echo "ERROR: spel CLI not found at $SPEL_BIN"
    echo "Build with: cargo build --release --manifest-path vendor/spel-framework/spel-cli/Cargo.toml"
    echo "or set SPEL=/path/to/spel"
    exit 1
}
[ -f "$GUEST_ELF" ] || {
    echo "ERROR: $GUEST_ELF not found"
    echo "Build with: cargo risczero build --manifest-path methods/guest/Cargo.toml"
    exit 1
}

echo "╔═══════════════════════════════════════════════════════════════╗"
echo "║  LP-0013 Token Authority — End-to-End Demo                     ║"
echo "║  Full lifecycle against a REAL local sequencer                 ║"
echo "║  RISC0_DEV_MODE=0  (real ZK proofs)                            ║"
echo "╚═══════════════════════════════════════════════════════════════╝"
echo ""

# ── Step 1: Start standalone sequencer ────────────────────────────────────────
echo "[1/4] Starting LEZ sequencer in standalone mode (port 3040)..."

rm -rf "$WALLET_HOME"
mkdir -p "$WALLET_HOME"
# Point the wallet at our sequencer's port (SEQ_PORT), not a hard-coded 3040.
cp "$WALLET_CONFIG" "$WALLET_HOME/wallet_config.json"
sed -i "s#\"sequencer_addr\": \"[^\"]*\"#\"sequencer_addr\": \"http://127.0.0.1:$SEQ_PORT\"#" \
    "$WALLET_HOME/wallet_config.json"

SEQUENCER_WORK="$REPO_DIR/.seq-workdir"
rm -rf "$SEQUENCER_WORK" && mkdir -p "$SEQUENCER_WORK"

cd "$SEQUENCER_WORK"
RISC0_DEV_MODE=0 RUST_LOG=warn "$SEQUENCER_BIN" -p "$SEQ_PORT" "$SEQ_CONFIG" \
    > "$REPO_DIR/.seq.log" 2>&1 &
SEQ_PID=$!
cd "$REPO_DIR"

cleanup() {
    kill "$SEQ_PID" 2>/dev/null || true
    rm -rf "$SEQUENCER_WORK" "$WALLET_HOME" "$REPO_DIR/.seq.log"
}
trap cleanup EXIT

sleep 5
# Fail loudly if OUR sequencer did not come up — otherwise the wallet would
# silently talk to whatever else is on this port.
if ! ps -p "$SEQ_PID" >/dev/null 2>&1; then
    echo "ERROR: sequencer (pid $SEQ_PID) exited during startup. Last log lines:"
    tail -8 "$REPO_DIR/.seq.log"
    if grep -q "Address already in use" "$REPO_DIR/.seq.log"; then
        echo "→ Port $SEQ_PORT is already in use. Re-run with SEQ_PORT=<free port>."
    fi
    exit 1
fi
echo "    Sequencer running (pid=$SEQ_PID, port=$SEQ_PORT)"
echo "    sequencer_addr = $("$WALLET_BIN" config get sequencer_addr 2>/dev/null | tail -1)"

# ── Step 2: Fund the deployer account ─────────────────────────────────────────
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

# ── Step 3: Deploy the guest ELF to the local sequencer ───────────────────────
echo "[3/4] Deploying token_authority.bin to the standalone sequencer..."

"$WALLET_BIN" deploy-program "$GUEST_ELF" > /dev/null 2>&1
sleep 4

PROGRAM_ID="63a29a4ec2b24402807c319d14e5d9a6bd5b26a49088cb3c6c2c8cd6187d2a60"
echo "    program_id = $PROGRAM_ID"
echo "    Program deployed to the local sequencer ✓"

# ── Step 4: Drive the full lifecycle THROUGH the sequencer ────────────────────
echo ""
echo "[4/4] Running the mint-authority lifecycle on the local sequencer"
echo "      (real transactions, real ZK proofs at RISC0_DEV_MODE=0)..."
echo ""

SPEL="$SPEL_BIN" \
WALLET="$WALLET_BIN" \
LEE_WALLET_HOME_DIR="$WALLET_HOME" \
EVID="$REPO_DIR/demo-lifecycle-evidence.txt" \
STEP_TIMEOUT="${STEP_TIMEOUT:-600}" \
    "$REPO_DIR/scripts/testnet-lifecycle.sh"

echo ""
echo "╔═══════════════════════════════════════════════════════════════╗"
echo "║  Demo complete — full lifecycle confirmed on the local        ║"
echo "║  sequencer; each tx hash + final on-chain state above and in   ║"
echo "║  demo-lifecycle-evidence.txt.                                  ║"
echo "║                                                               ║"
echo "║  create → mint → rotate → mint(rotated) → revoke              ║"
echo "║  → post-revoke mint REJECTED (error 1003)                     ║"
echo "║                                                               ║"
echo "║  RISC0_DEV_MODE=0  (real ZK proofs)                     ✓     ║"
echo "╚═══════════════════════════════════════════════════════════════╝"
