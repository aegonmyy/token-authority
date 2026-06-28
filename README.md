# Token Authority — LP-0013

> **Lambda Prize submission** — Token program authorities for the Logos Execution Zone (LEZ).

Extends the LEZ fungible-token model with a fully auditable mint-authority layer built on top of the [RFP-001 admin-authority library](./admin_authority). Supply can be fixed at creation or managed by a rotating authority that can be permanently revoked on-chain.

---

## Architecture

```
token-authority/
├── admin_authority/          RFP-001 admin-authority library  (10xx errors)
├── token_authority_core/     Shared types: TokenDef, TokenHolding, helpers (20xx errors)
├── token_authority_sdk/      Host-side SDK: typed args, PDA seeds, SimulatedLedger
├── examples/
│   ├── fixed_supply.rs       End-to-end: no mint authority from genesis
│   └── variable_supply.rs    End-to-end: TEAM→DAO authority rotation → revoke
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
| `new_fungible_token` | none (permissionless) | `token_def` (init PDA), `mint_auth` (init PDA), `creator_holding` (mut), `creator` (signer) |
| `mint_tokens` | mint authority | `token_def` (mut PDA), `mint_auth` (PDA), `recipient_holding` (mut), `authority` (signer) |
| `transfer_tokens` | sender signature | `token_def` (PDA), `sender_holding` (mut), `recipient_holding` (mut), `sender` (signer) |
| `burn_tokens` | holder signature | `token_def` (mut PDA), `holder_holding` (mut), `holder` (signer) |
| `rotate_authority` | current authority | `token_def` (mut PDA), `mint_auth` (mut PDA), `authority` (signer) |
| `revoke_authority` | current authority | `token_def` (mut PDA), `mint_auth` (mut PDA), `authority` (signer) — **irreversible** |

PDA seeds: `token_def` = `b"token_def"`, `mint_auth` = `b"mint_auth"`.

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
cargo run --bin variable_supply   # authority lifecycle: mint → rotate → revoke
```

### Run all tests

```bash
cargo test --workspace --exclude token-authority-guest
# 44/44 pass, 0 warnings
```

Test coverage:
- `admin_authority` — 18 tests: new, require_admin, initialize, transfer, revoke, Borsh roundtrips, full lifecycle
- `token_authority_core` — 11 tests: mint, transfer, burn, overflow/underflow, Borsh roundtrips
- `token_authority_sdk` — 15 sim tests + 1 doctest: all instruction paths, double-init, full lifecycle

### Type-check the guest program

```bash
cd methods/guest && cargo check
```

---

## On-chain deployment (requires VPS / Linux + RISC-Zero toolchain)

### Prerequisites

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install RISC-Zero toolchain
curl -L https://risczero.com/install | bash && rzup install

# Install LEZ CLI
cargo install --git https://github.com/logos-blockchain/logos-execution-zone lgs
```

### Build the guest ELF

```bash
cd methods/guest
cargo +risc0 build --release --target riscv32im-risc0-zkvm-elf
# ELF → methods/guest/target/riscv32im-risc0-zkvm-elf/release/token_authority
```

### Deploy and benchmark

```bash
# Start local LEZ network
lgs localnet start

# Deploy program (record program_id from output)
lgs program deploy methods/guest/target/riscv32im-risc0-zkvm-elf/release/token_authority

# Run new_fungible_token with RISC0_DEV_MODE=0 for real proof
RISC0_DEV_MODE=0 lgs program invoke <program_id> new_fungible_token \
  --args '{"name":"LOGOS","decimals":6,"initial_supply":1000000,"mint_authority_id":[]}'

# Cycle counts appear in LEZ logs as:
#   CU new_fungible_token cycles=NNNNNN
#   CU mint_tokens cycles=NNNNNN
#   ... etc.
```

---

## How `admin_authority` satisfies RFP-001

RFP-001 requires a self-sufficient, agnostic library that:

| Requirement | Implementation |
|---|---|
| Single admin PDA | `AdminConfig { admin: Option<[u8;32]> }` stored at `pda=[literal("mint_auth")]` |
| Gated access | `require_admin(&config, signer)` — returns `Err(Revoked)` or `Err(Unauthorized)` |
| Initialize | `initialize_admin(existing_bytes, admin)` — fails if non-empty (already-init guard) |
| Transfer | `transfer_admin(&config, signer, new_admin)` — signer must be current admin |
| Revoke | `revoke_admin(&config, signer)` — sets `admin = None`; all future calls fail with `Revoked` |
| No SPEL/LEZ deps | `admin_authority` crate has zero SPEL/LEZ dependencies — pure Rust, usable anywhere |

---

## CU benchmarks

> To be filled in after deployment on LEZ devnet. See `CONTEXT.md` for instructions.

| Instruction | CU (cycles) |
|---|---|
| `new_fungible_token` | TBD |
| `mint_tokens` | TBD |
| `transfer_tokens` | TBD |
| `burn_tokens` | TBD |
| `rotate_authority` | TBD |
| `revoke_authority` | TBD |
