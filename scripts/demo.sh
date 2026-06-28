#!/usr/bin/env bash
# LP-0013 end-to-end demo — token authority lifecycle on LEZ localnet.
# RISC0_DEV_MODE=0 (real ZK proofs).
#
# Prerequisites (run once on a fresh VPS):
#   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
#   curl -L https://risczero.com/install | bash && rzup install
#   cargo install --git https://github.com/logos-blockchain/logos-execution-zone lgs
#
# Then just: ./scripts/demo.sh
set -euo pipefail

export RISC0_DEV_MODE=0
export RUST_LOG=info

GUEST_ELF=methods/guest/target/riscv32im-risc0-zkvm-elf/release/token_authority

# ── Preflight ─────────────────────────────────────────────────────────────────
if [ ! -f "$GUEST_ELF" ]; then
    echo "Building guest ELF (requires RISC-Zero toolchain)..."
    cd methods/guest
    cargo +risc0 build --release --target riscv32im-risc0-zkvm-elf
    cd ../..
fi

echo "=== LP-0013 Token Authority — E2E Demo ==="
echo "RISC0_DEV_MODE=${RISC0_DEV_MODE}"
echo "ELF: ${GUEST_ELF}"
echo ""

# ── Step 1: Start localnet ────────────────────────────────────────────────────
echo "[1] Starting LEZ localnet..."
lgs localnet start
sleep 8

# ── Step 2: Deploy ────────────────────────────────────────────────────────────
echo "[2] Deploying token_authority program..."
PROGRAM_ID=$(lgs program deploy "$GUEST_ELF" --output-program-id)
echo "    program_id = $PROGRAM_ID"

# ── Step 3: Fixed-supply token (no mint authority) ────────────────────────────
echo ""
echo "[3] Creating fixed-supply token (mint_authority = None)..."
RISC0_DEV_MODE=0 lgs program invoke "$PROGRAM_ID" new_fungible_token \
    --args '{
        "name": "FIXED",
        "decimals": 6,
        "initial_supply": 1000000,
        "mint_authority_id": []
    }'
echo "    FIXED token created — supply locked at 1,000,000"

echo "[3] Verifying mint is rejected (no authority)..."
set +e
RISC0_DEV_MODE=0 lgs program invoke "$PROGRAM_ID" mint_tokens \
    --args '{"amount": 100}' 2>&1 | grep -q "1003\|Revoked\|Unauthorized"
MINT_EXIT=$?
set -e
[ $MINT_EXIT -eq 0 ] && echo "    Mint correctly rejected ✓" || { echo "ERROR: mint not rejected"; exit 1; }

# ── Step 4: Variable-supply token (with mint authority) ───────────────────────
echo ""
echo "[4] Creating variable-supply token (TEAM is mint authority)..."
TEAM_KEY="0101010101010101010101010101010101010101010101010101010101010101"
DAO_KEY="0202020202020202020202020202020202020202020202020202020202020202"

RISC0_DEV_MODE=0 lgs program invoke "$PROGRAM_ID" new_fungible_token \
    --args "{
        \"name\": \"VAR\",
        \"decimals\": 6,
        \"initial_supply\": 50000000,
        \"mint_authority_id\": \"${TEAM_KEY}\"
    }"
echo "    VAR token created — TEAM is mint authority"

echo "[4] TEAM mints additional tokens..."
RISC0_DEV_MODE=0 lgs program invoke "$PROGRAM_ID" mint_tokens \
    --signer "$TEAM_KEY" \
    --args '{"amount": 10000000}'
echo "    10,000,000 VAR minted ✓"

# ── Step 5: Authority rotation ────────────────────────────────────────────────
echo ""
echo "[5] Rotating authority: TEAM → DAO..."
RISC0_DEV_MODE=0 lgs program invoke "$PROGRAM_ID" rotate_authority \
    --signer "$TEAM_KEY" \
    --args "{\"new_authority_id\": \"${DAO_KEY}\"}"
echo "    Authority transferred to DAO ✓"

echo "[5] Verifying TEAM can no longer mint (must be rejected)..."
set +e
RISC0_DEV_MODE=0 lgs program invoke "$PROGRAM_ID" mint_tokens \
    --signer "$TEAM_KEY" \
    --args '{"amount": 1}' 2>&1 | grep -q "1001\|Unauthorized"
TEAM_EXIT=$?
set -e
[ $TEAM_EXIT -eq 0 ] && echo "    Old authority correctly rejected ✓" || { echo "ERROR: old authority not rejected"; exit 1; }

echo "[5] DAO mints governance reserve..."
RISC0_DEV_MODE=0 lgs program invoke "$PROGRAM_ID" mint_tokens \
    --signer "$DAO_KEY" \
    --args '{"amount": 5000000}'
echo "    5,000,000 VAR minted by DAO ✓"

# ── Step 6: Authority revocation ─────────────────────────────────────────────
echo ""
echo "[6] DAO permanently revokes authority..."
RISC0_DEV_MODE=0 lgs program invoke "$PROGRAM_ID" revoke_authority \
    --signer "$DAO_KEY"
echo "    Authority revoked — supply is now permanently fixed ✓"

echo "[6] Verifying no further minting possible..."
set +e
RISC0_DEV_MODE=0 lgs program invoke "$PROGRAM_ID" mint_tokens \
    --signer "$DAO_KEY" \
    --args '{"amount": 1}' 2>&1 | grep -q "1003\|Revoked"
REVOKE_EXIT=$?
set -e
[ $REVOKE_EXIT -eq 0 ] && echo "    Post-revoke mint correctly rejected ✓" || { echo "ERROR: revoked authority still minting"; exit 1; }

# ── Summary ───────────────────────────────────────────────────────────────────
echo ""
echo "=== Demo complete ==="
echo "    Fixed-supply token: mint rejected from genesis        ✓"
echo "    Variable-supply token: mint → rotate → revoke cycle   ✓"
echo "    Authority rotation: TEAM→DAO, old key rejected        ✓"
echo "    Post-revoke mint rejected (E_REVOKED=1003)            ✓"
echo "    RISC0_DEV_MODE=0 (real ZK proofs)                     ✓"
echo ""
echo "    program_id = $PROGRAM_ID"

lgs localnet stop 2>/dev/null || true
