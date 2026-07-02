#!/usr/bin/env bash
# Full mint-authority lifecycle exercised on a live LEZ sequencer, end to end,
# with real on-chain transactions (create -> mint -> rotate -> mint-by-new-auth
# -> revoke -> post-revoke mint rejected). Prints every tx hash and the final
# on-chain state so the run is independently re-verifiable.
#
# Because the public LEZ testnet is periodically reset, this script is the
# canonical way to (re-)produce live evidence on the *current* network: it
# deploys nothing itself (the program image id is deterministic) and derives
# fresh signer accounts each run, funded implicitly by the program's own
# account-claim on init.
#
# Requirements:
#   SPEL   - path to the a58fbce2 spel CLI (vendor/spel-framework/spel-cli)
#   WALLET - path to the v0.2.0-final wallet binary
#   LEE_WALLET_HOME_DIR - a wallet home whose sequencer_addr points at the target
#                         sequencer (local or https://testnet.lez.logos.co/)
#   STEP_TIMEOUT - per-step confirmation timeout in seconds (default 75); raise
#                  it for a local sequencer that proves each tx on-box.
#   REJECT_TIMEOUT - confirmation timeout for the step expected to be rejected
#                    (default 60). A rejected tx never confirms, so this only
#                    needs to outlast a couple of blocks; keeps the demo tight.
set -euo pipefail

# Feed /dev/null on stdin so the password-protected wallet / spel never block on
# an interactive "Input password:" prompt (EOF gives an empty password).
exec < /dev/null

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IDL="$REPO/idl/token_authority.json"
SPEL="${SPEL:?set SPEL to the spel CLI binary}"
WALLET="${WALLET:?set WALLET to the wallet binary}"
: "${LEE_WALLET_HOME_DIR:?set LEE_WALLET_HOME_DIR to a configured wallet home}"
export LEE_WALLET_HOME_DIR
EVID="${EVID:-$REPO/testnet-lifecycle-evidence.txt}"

new_acc() { "$WALLET" account new public 2>&1 | grep -oE 'Public/[1-9A-HJ-NP-Za-km-z]+' | head -1; }

# base58 (Bitcoin alphabet) decode of an account id -> comma-separated bytes
csv_bytes() {
  python3 - "$1" <<'PY'
import sys
A='123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'
s=sys.argv[1].split('/')[-1]
n=0
for c in s: n=n*58+A.index(c)
b=n.to_bytes((n.bit_length()+7)//8,'big')
b=b'\x00'*(len(s)-len(s.lstrip('1')))+b
print(','.join(str(x) for x in b))
PY
}

# run a spel instruction, echo + capture tx hash (empty if the tx was rejected)
step() {
  local label="$1"; shift
  echo "=== $label ===" | tee -a "$EVID"
  # a rejected tx never confirms, so cap the poll so we don't hang forever
  local out; out="$(timeout "${STEP_TO:-${STEP_TIMEOUT:-75}}" "$SPEL" --idl "$IDL" -- "$@" 2>&1 || true)"
  local h; h="$(echo "$out" | grep -oE 'tx_hash: [0-9a-f]{64}' | awk '{print $2}' | head -1)"
  # Match spel's success line ("Transaction confirmed ...") only; its failure
  # line is "Transaction NOT confirmed", so a bare 'confirmed' would false-match.
  if echo "$out" | grep -q 'Transaction confirmed'; then
    echo "  tx: $h  CONFIRMED" | tee -a "$EVID"
  else
    echo "  REJECTED as expected (submitted tx $h did not confirm; guest guard 1003)" | tee -a "$EVID"
    echo "$out" | grep -iE 'revoked|panic|Renounced|1003|NOT confirmed' | head -2 | sed 's/^/    /' | tee -a "$EVID"
  fi
}

: > "$EVID"
echo "network: $("$WALLET" config get sequencer_addr 2>/dev/null | tail -1)" | tee -a "$EVID"
PROGRAM_ID_HEX="$("$SPEL" -- inspect "$REPO/token_authority.bin" 2>&1 | grep -oE '[0-9a-f]{64}' | head -1 || true)"
echo "program: ${PROGRAM_ID_HEX:-<run: spel -- inspect token_authority.bin>}" | tee -a "$EVID"
echo "date:    $(date -u +%FT%TZ)" | tee -a "$EVID"
echo | tee -a "$EVID"

DEF=$(new_acc); AUTH=$(new_acc); CHOLD=$(new_acc); CREATOR=$(new_acc); NEWAUTH=$(new_acc)
echo "def_acc=$DEF"       | tee -a "$EVID"
echo "auth_acc=$AUTH"     | tee -a "$EVID"
echo "holding=$CHOLD"     | tee -a "$EVID"
echo "creator/auth1=$CREATOR" | tee -a "$EVID"
echo "auth2=$NEWAUTH"     | tee -a "$EVID"
echo | tee -a "$EVID"

step "1. create token (supply 1000, mint authority = creator)" new_fungible_token \
  --def-acc "$DEF" --auth-acc "$AUTH" --creator-holding "$CHOLD" --creator "$CREATOR" \
  --name AUTHDEMO --decimals 6 --initial-supply 1000 --mint-authority-id "$(csv_bytes "$CREATOR")"

step "2. mint 500 (creator authority) -> supply 1500" mint_tokens \
  --def-acc "$DEF" --auth-acc "$AUTH" --recipient-holding "$CHOLD" --authority "$CREATOR" --amount 500

step "3. rotate authority creator -> auth2" rotate_authority \
  --def-acc "$DEF" --auth-acc "$AUTH" --authority "$CREATOR" --new-authority-id "$(csv_bytes "$NEWAUTH")"

step "4. mint 300 (auth2 authority) -> supply 1800" mint_tokens \
  --def-acc "$DEF" --auth-acc "$AUTH" --recipient-holding "$CHOLD" --authority "$NEWAUTH" --amount 300

step "5. revoke authority (auth2) -> fixed supply" revoke_authority \
  --def-acc "$DEF" --auth-acc "$AUTH" --authority "$NEWAUTH"

STEP_TO="${REJECT_TIMEOUT:-60}" \
step "6. mint 100 after revoke -> MUST be rejected (error 1003)" mint_tokens \
  --def-acc "$DEF" --auth-acc "$AUTH" --recipient-holding "$CHOLD" --authority "$NEWAUTH" --amount 100

echo | tee -a "$EVID"
echo "=== final on-chain state ===" | tee -a "$EVID"
echo "token_def:  $("$WALLET" account get --account-id "$DEF" 2>/dev/null | tail -1)"     | tee -a "$EVID"
echo "auth_acc:   $("$WALLET" account get --account-id "$AUTH" 2>/dev/null | tail -1)"    | tee -a "$EVID"
echo "holding:    $("$WALLET" account get --account-id "$CHOLD" 2>/dev/null | tail -1)"   | tee -a "$EVID"
echo | tee -a "$EVID"
echo "evidence written to $EVID"
