# Token Authority — LP-0013

> **Lambda Prize submission** — Token program authorities for the Logos Execution Zone (LEZ).

Extends the LEZ fungible-token model with a fully auditable mint-authority layer built on top of the [RFP-001 admin-authority library](./admin_authority). Supply can be fixed at creation or managed by a rotating authority that can be permanently revoked on-chain.

> **Live on the public LEZ v0.2.0 testnet.** The full mint-authority lifecycle
> (create -> mint -> rotate -> mint by the rotated authority -> revoke -> post-revoke
> mint rejected) is confirmed **on-chain** at `https://testnet.lez.logos.co/`
> (LEZ `v0.2.0`, commit `a58fbce2`). Tx hashes + final state:
> [`docs/testnet-v020-evidence-20260702.md`](docs/testnet-v020-evidence-20260702.md).
> Re-produce on the current testnet (periodically reset) with
> `scripts/testnet-lifecycle.sh`.

---

## Architecture

```
token-authority/
├── admin_authority/          RFP-001 admin-authority library  (10xx errors)
├── token_authority_core/     Shared types: TokenDef, TokenHolding, helpers (20xx errors)
├── token_authority_sdk/      Host-side SDK: typed args, PDA seeds, SimulatedLedger
├── examples/
│   ├── fixed_supply.rs       End-to-end: no mint authority from genesis
│   └── variable_supply.rs    End-to-end: TEAM->DAO authority rotation -> revoke
└── methods/
    └── guest/                SPEL guest program (RISC-Zero ELF, riscv32im target)
        └── src/bin/token_authority.rs
```

### Three-layer design

| Layer | Crate | Role |
|---|---|---|
| Authority primitive | `admin_authority` | RFP-001: `AdminConfig { admin: Option<[u8;32]> }` PDA, `require_admin()` gate, initialize / transfer / revoke ops |
| Token state | `token_authority_core` | `TokenDef` (name, decimals, total_supply, **mint_authority**), `TokenHolding` (definition_id, balance), pure arithmetic helpers |
| On-chain program | `methods/guest` | `#[lez_program]` — 6 SPEL instructions wiring the two layers together |

---

## Instructions

| Instruction | Gate | Accounts |
|---|---|---|
| `new_fungible_token` | none (permissionless) | `def_acc` (init, signer), `auth_acc` (init, signer), `creator_holding` (init, signer), `creator` (signer) |
| `mint_tokens` | mint authority | `def_acc` (mut), `auth_acc` (read), `recipient_holding` (mut), `authority` (signer) |
| `transfer_tokens` | sender signature | `def_acc` (read), `sender_holding` (mut), `recipient_holding` (mut), `sender` (signer) |
| `burn_tokens` | holder signature | `def_acc` (mut), `holder_holding` (mut), `holder` (signer) |
| `rotate_authority` | current authority | `def_acc` (mut), `auth_acc` (mut), `authority` (signer) |
| `revoke_authority` | current authority | `def_acc` (mut), `auth_acc` (mut), `authority` (signer) — **irreversible** |

Account identities are signer-derived (no PDAs); each account key pair controls its own account.

---

## Error codes

| Range | Crate | Codes |
|---|---|---|
| 10xx | `admin_authority` | 1001 Unauthorized, 1002 AlreadyInitialized, 1003 Revoked, 1004 NullAuthority |
| 20xx | `token_authority_core` | 2001 SupplyOverflow, 2002 BalanceOverflow, 2003 BalanceUnderflow, 2004 SupplyUnderflow, 2005 WrongDefinition, 2006 ZeroAmount, 2007 InsufficientFunds |
| 30xx | guest validation | 3001 EmptyName, 3002 DecimalsRange, 3003 BadAuthorityBytes |

---

## Quick start

### Run examples (no RISC-Zero toolchain needed)

```bash
cargo run --bin fixed_supply      # fixed supply from genesis
cargo run --bin variable_supply   # authority lifecycle: mint -> rotate -> revoke
```

### Run all tests

```bash
# Unit + simulation tests (no toolchain needed):
cargo test --workspace --exclude token-authority-guest --exclude integration_tests

# Integration tests (lee in-process sequencer, RISC0_DEV_MODE=0 = real proofs):
RISC0_DEV_MODE=0 cargo test -p integration_tests -- --nocapture
# 4 tests: fixed-supply mint rejection, full authority lifecycle, transfer, burn
```

### Type-check the guest program

```bash
cd methods/guest && cargo check
```

---

## On-chain deployment (requires Linux + RISC-Zero toolchain)

### Prerequisites

```bash
# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# RISC-Zero toolchain + Docker (for guest ELF build)
curl -L https://risczero.com/install | bash && rzup install

# System deps (for building the LEZ node)
sudo apt-get install -y clang libclang-dev unzip python3-dev

# LEZ node (sequencer + wallet CLI)
git clone https://github.com/logos-blockchain/logos-execution-zone ~/lez-node
cd ~/lez-node
cargo build --release --features standalone -p sequencer_service
cargo build --release -p wallet
```

### Build the guest ELF

The pre-built ELF (`token_authority.bin`) is included in the repo.  
To rebuild from source:

```bash
# From repo root (Docker required):
cargo risczero build --manifest-path methods/guest/Cargo.toml
# ELF -> methods/guest/target/riscv32im-risc0-zkvm-elf/docker/token_authority.bin
sudo cp methods/guest/target/riscv32im-risc0-zkvm-elf/docker/token_authority.bin token_authority.bin
```

### Run the end-to-end demo

```bash
./scripts/demo.sh
```

The script:
1. Starts a standalone LEZ sequencer (no external chain needed)
2. Imports the debug account and claims genesis balance
3. Deploys `token_authority.bin` to the sequencer — prints the on-chain program ID
4. Drives the **full mint-authority lifecycle through the sequencer** over the
   CLI at `RISC0_DEV_MODE=0` (real ZK proofs): create -> mint -> rotate -> mint by
   the rotated authority -> revoke -> post-revoke mint **rejected** (error 1003).
   Every step submits a real transaction and prints its tx hash; evidence is also
   written to `demo-lifecycle-evidence.txt`.

This drives the same lifecycle script used for the public-testnet evidence
(`scripts/testnet-lifecycle.sh`), pointed at the local sequencer. It additionally
requires the vendored SPEL CLI:

```bash
cargo build --release --manifest-path vendor/spel-framework/spel-cli/Cargo.toml
```

The fast in-process suite (`cargo test -p integration_tests`, which runs in CI)
is unchanged; `demo.sh` is the on-a-real-sequencer end-to-end demo.

**Program ID** (deterministic image id — identical on standalone and the live testnet):
```
63a29a4ec2b24402807c319d14e5d9a6bd5b26a49088cb3c6c2c8cd6187d2a60
```

To exercise the lifecycle against the **live testnet** instead of standalone:
```bash
SPEL=vendor/spel-framework/spel-cli/target/release/spel \
WALLET=<v0.2.0-final wallet> \
LEE_WALLET_HOME_DIR=<wallet home pointed at testnet.lez.logos.co> \
./scripts/testnet-lifecycle.sh
```

### Manual deploy

```bash
export LEE_WALLET_HOME_DIR=~/.lez-wallet
mkdir -p $LEE_WALLET_HOME_DIR
cp ~/lez-node/lez/wallet/configs/debug/wallet_config.json $LEE_WALLET_HOME_DIR/

# In a separate terminal, start the sequencer:
cd ~/lez-node/lez/sequencer/service
RISC0_DEV_MODE=0 ~/lez-node/target/release/sequencer_service configs/debug/sequencer_config.json

# Import account and fund it:
~/lez-node/target/release/wallet account import public \
    --private-key 7f273098f25b71e6c005a9519f2678da8d1c7f01f6a27778e2d9948abdf901fb
~/lez-node/target/release/wallet vault claim \
    --account-id Public/CbgR6tj5kWx5oziiFptM7jMvrQeYY3Mzaao6ciuhSr2r --amount 10000

# Deploy the program:
~/lez-node/target/release/wallet deploy-program token_authority.bin
```

---

## How `admin_authority` satisfies RFP-001

RFP-001 requires a self-sufficient, agnostic library that:

| Requirement | Implementation |
|---|---|
| Single admin account | `AdminConfig { admin: Option<[u8;32]> }` stored in a signer-owned account |
| Gated access | `require_admin(&config, signer)` — returns `Err(Revoked)` or `Err(Unauthorized)` |
| Initialize | `initialize_admin(existing_bytes, admin)` — fails if non-empty (already-init guard) |
| Transfer | `transfer_admin(&config, signer, new_admin)` — signer must be current admin |
| Revoke | `revoke_admin(&config, signer)` — sets `admin = None`; all future calls fail with `Revoked` |
| No SPEL/LEZ deps | `admin_authority` crate has zero SPEL/LEZ dependencies — pure Rust, usable anywhere |

---

## CU benchmarks

Measured with `RISC0_DEV_MODE=0` on AWS c5.2xlarge (8 vCPU / 15 GB RAM).
The guest emits `CU <instruction> cycles=N` via `log_cycles()` at the end of each handler.

| Instruction | CU (cycles) |
|---|---|
| `new_fungible_token` | 1 380 |
| `mint_tokens` | 4 072 |
| `transfer_tokens` | 2 606 |
| `burn_tokens` | 2 510 |
| `rotate_authority` | 3 166 |
| `revoke_authority` | 2 708 |
