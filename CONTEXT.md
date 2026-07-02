# CONTEXT.md — Token Authority (LP-0013)

> **Read this first.** Written so any developer or AI picking this up cold has everything needed to understand the project, run it, and resolve reviewer feedback.

---

## What this is

A complete submission for **LP-0013: Token program authorities** on the Logos Execution Zone (LEZ). Prize: $600.

The LEZ token program is intentionally minimal — it has no concept of who can mint tokens after launch. This repo adds a mint-authority model: when a token is created you designate who can mint, that authority can be handed to a new address (rotation), or permanently burned (revocation). Once revoked, supply is fixed forever.

Prize spec: https://github.com/logos-co/lambda-prize/issues/13

---

## Current state — what is and isn't done

| Item | Status |
|---|---|
| `admin_authority` crate (RFP-001 library) | Done |
| `token_authority_core` crate (shared types) | Done |
| SPEL guest program (6 instructions) | Done |
| Pre-built guest ELF (`token_authority.bin`) | Done, committed |
| `token_authority_sdk` host SDK | Done |
| Two runnable examples | Done |
| IDL (`idl/token_authority.json`) | Done |
| CI (`.github/workflows/ci.yml`) | Done, green |
| 4 integration tests, `RISC0_DEV_MODE=0` | Done, all pass |
| CU benchmarks (all 6 instructions) | Done, in README + FURPS.md |
| Demo script (`scripts/demo.sh`) | Done, tested end-to-end |
| README deployment steps | Done |
| FURPS.md self-assessment | Done |
| **Ported to v0.2.0-final (a58fbce2)** | Done — `lee_core` + vendored a58fbce2 spel-framework |
| **Live LEZ testnet deploy + full lifecycle** | Done — `testnet.lez.logos.co`; see `docs/testnet-v020-evidence-20260702.md` |
| Testnet lifecycle exerciser (`scripts/testnet-lifecycle.sh`) | Done — reproducible; survives testnet resets |
| GitHub push | Done — `github.com/aegonmyy/token-authority` |
| **Video demo** | **Pending — user records this** |
| **PR to logos-co/lambda-prize** | **Pending — after video** |

---

## Repo layout

```
token-authority/
├── CONTEXT.md                     you are here
├── README.md                      user-facing overview, deployment steps, benchmarks
├── FURPS.md                       prize self-assessment (all criteria checked)
├── Cargo.toml                     workspace root
├── token_authority.bin            pre-built guest ELF (495 KB) — do not delete
├── idl/
│   └── token_authority.json       IDL: 6 instructions, 14 error codes
│
├── admin_authority/               RFP-001 mint-authority primitive
│   └── src/lib.rs                 AdminConfig, require_admin, initialize/transfer/revoke
│
├── token_authority_core/          Shared types, no SPEL/LEZ deps
│   └── src/lib.rs                 TokenDef, TokenHolding, apply_mint/transfer/burn
│
├── token_authority_sdk/           Host-side SDK, no SPEL/LEZ deps
│   └── src/
│       ├── lib.rs
│       ├── args.rs                Typed Borsh-serialisable instruction arg structs
│       ├── pda.rs                 Seed constants used by SimulatedLedger
│       └── sim.rs                 SimulatedLedger — in-memory execution for unit tests
│
├── examples/
│   └── src/bin/
│       ├── fixed_supply.rs        No authority from creation; confirms mint rejected
│       └── variable_supply.rs     TEAM mints → rotates to DAO → DAO revokes
│
├── methods/
│   ├── src/lib.rs                 Exposes TOKEN_AUTHORITY_ELF + TOKEN_AUTHORITY_ID
│   ├── build.rs                   Reads pre-built ELF at build time, computes image ID
│   └── guest/                     SEPARATE inner workspace (riscv32im target)
│       └── src/bin/
│           └── token_authority.rs #[lez_program] — 6 #[instruction] handlers
│
├── integration_tests/
│   └── tests/
│       └── token_authority.rs     4 lee integration tests (RISC0_DEV_MODE=0)
│
└── scripts/
    └── demo.sh                    End-to-end demo: starts sequencer, deploys, tests
```

---

## Architecture

### Three layers

| Layer | Crate | What it does |
|---|---|---|
| Authority primitive | `admin_authority` | RFP-001: `AdminConfig { admin: Option<[u8;32]> }`, require/initialize/transfer/revoke |
| Token state | `token_authority_core` | `TokenDef` (name, decimals, total_supply, mint_authority), `TokenHolding` (definition_id, balance), pure arithmetic |
| On-chain program | `methods/guest` | `#[lez_program]` wiring the two layers into 6 SPEL instructions |

### Account model

There are **no PDAs**. Every account is a signer-owned keypair. The program takes accounts as positional arguments decorated with `#[account(...)]`.

| Account | Attribute | Role |
|---|---|---|
| `def_acc` | `#[account(init, signer)]` or `#[account(mut)]` or `#[account()]` | Stores `TokenDef` (Borsh) |
| `auth_acc` | `#[account(init, signer)]` or `#[account(mut)]` or `#[account()]` | Stores `AdminConfig` (Borsh) |
| `*_holding` | `#[account(init, signer)]` or `#[account(mut)]` | Stores `TokenHolding` (Borsh) |
| Signers | `#[account(signer)]` | Read-only identity accounts |

All data is **Borsh-encoded**. Read with `borsh::from_slice`, write via `.to_bytes()`.

### Error code namespaces

| Range | Crate | Notable codes |
|---|---|---|
| 10xx | `admin_authority` | 1001 Unauthorized, 1003 Revoked, 1004 NullAuthority |
| 20xx | `token_authority_core` | 2001 SupplyOverflow, 2005 WrongDefinition, 2007 InsufficientFunds |
| 30xx | guest validation | 3001 EmptyName, 3002 DecimalsRange, 3003 BadAuthorityBytes |

### Program ID (image ID)

```
63a29a4ec2b24402807c319d14e5d9a6bd5b26a49088cb3c6c2c8cd6187d2a60
```

As `[u32; 8]` (little-endian words):
```rust
pub const TOKEN_AUTHORITY_ID: [u32; 8] = [
    1318756963, 38056642, 2637266048, 2799297812,
    2753977277, 1019971728, 3599510636, 1613397272,
];
```

This is computed from the ELF binary via `risc0_zkvm::compute_image_id`. It will change if the guest source or any of its dependencies change and the ELF is rebuilt.

---

## How to run things

### Unit + sim tests (no toolchain needed)

```bash
cargo test --workspace
# ~48 tests, 0 failures, 0 warnings
```

### Examples (no toolchain needed)

```bash
cargo run --bin fixed_supply
cargo run --bin variable_supply
```

### Integration tests (lee in-process, fast)

```bash
RISC0_DEV_MODE=1 cargo test -p integration_tests -- --nocapture
# 4 tests, completes in ~7 seconds
```

### Integration tests with real ZK proofs

```bash
RISC0_DEV_MODE=0 cargo test -p integration_tests -- --nocapture
# 4 tests, takes 4–8 minutes — each test generates a real RISC-Zero proof
# Output includes: R0VM[...] CU <instruction> cycles=N (compute unit benchmarks)
```

### Full end-to-end demo (sequencer + real proofs)

Prerequisites — build the LEZ node once:
```bash
git clone https://github.com/logos-blockchain/logos-execution-zone ~/lez-node
cd ~/lez-node
sudo apt-get install -y clang libclang-dev unzip python3-dev
cargo build --release --features standalone -p sequencer_service
cargo build --release -p wallet
```

Then from the repo root:
```bash
./scripts/demo.sh
```

The script:
1. Starts the standalone LEZ sequencer on port 3040
2. Imports a debug keypair and claims genesis balance
3. Deploys `token_authority.bin` via `wallet deploy-program`
4. Runs all 4 integration tests with `RISC0_DEV_MODE=0`
5. Prints a summary box and cleans up the sequencer on exit

Expected final output:
```
test result: ok. 4 passed; 0 failed; 0 ignored

╔═══════════════════════════════════════════════════════════════╗
║  Demo complete                                                ║
║  fixed_supply_mint_rejected    ✓  (error 1003 as expected)   ║
║  variable_supply_full_lifecycle ✓  (mint → rotate → revoke)  ║
║  transfer_tokens               ✓  (sender → recipient)       ║
║  burn_tokens                   ✓  (supply + balance reduced)  ║
║  RISC0_DEV_MODE=0  (real ZK proofs)                    ✓     ║
╚═══════════════════════════════════════════════════════════════╝
```

### Rebuild the guest ELF from source (rare)

Only needed if the guest source changes. Requires Docker:
```bash
cargo risczero build --manifest-path methods/guest/Cargo.toml
sudo cp methods/guest/target/riscv32im-risc0-zkvm-elf/docker/token_authority.bin token_authority.bin
sudo chown $USER:$USER token_authority.bin
```

After rebuilding, the image ID will change. Update `TOKEN_AUTHORITY_ID` in `methods/src/lib.rs` and the program ID in `README.md`, `FURPS.md`, and `scripts/demo.sh`.

---

## Key technical facts a new developer needs to know

### The guest is a separate inner workspace

`methods/guest/Cargo.toml` contains `[workspace]`. It cannot be compiled from the root workspace. The root's `methods/` crate reads the pre-built ELF via `build.rs` at compile time — it never invokes the riscv toolchain itself. This means integration tests work without any RISC-Zero toolchain installed.

### `#![cfg_attr(not(test), no_main)]` — why

SPEL's `#[lez_program]` macro generates `pub fn main()` at crate root. The `#![no_main]` attribute is required for the riscv target but breaks `cargo test`. The `cfg_attr` conditional applies `no_main` only when building for the target, not during host tests.

Similarly, `risc0_zkvm::guest::entry!(main)` is guarded with `#[cfg(not(test))]`.

### `#[account()]` requires parentheses

The SPEL macro parse step rejects `#[account]` (bare). Every account attribute must be `#[account()]`, `#[account(mut)]`, `#[account(signer)]`, or `#[account(init, signer)]`. Bare `#[account]` is a compile error.

### Nonce protocol in integration tests

`message.nonces.len()` must equal the number of **signers**, not the number of accounts. One nonce per signing key, in the same order as the keys passed to `WitnessSet::for_message`.

### `DefaultAccountModifiedWithoutClaim` error

If an account starts in DEFAULT state (no program owner set) and the instruction modifies it, the sequencer rejects the transaction with this error. The integration tests avoid this by pre-inserting recipient/holding accounts via `state.force_insert_account(...)` with `program_owner` set to `program_id()`. See `insert_empty_holding()` in `integration_tests/tests/token_authority.rs`.

### `TokenDef.mint_authority` and `AdminConfig.admin` are always kept in sync

`rotate_authority` and `revoke_authority` update both the `auth_acc` (which stores `AdminConfig`) and the `def_acc` (which stores `TokenDef`). Both fields reflect the current authority. The gate (`require_admin`) reads only from `auth_acc`.

### `mint_authority_id: Vec<u8>` instead of `Option<[u8;32]>`

The SPEL macro has difficulty deserializing `Option<[u8;32]>` from instruction args. The guest takes a `Vec<u8>` instead:
- Empty vec → `None` (fixed supply)
- 32-byte vec → `Some([u8;32])` (authority account id)

The SDK's `NewFungibleTokenArgs::mint_authority_bytes()` handles this conversion on the host side.

### LEZ wallet CLI — the actual commands

The deploy tool is `wallet` (not `lgs` — that CLI does not exist):
```bash
export LEE_WALLET_HOME_DIR=~/.lez-wallet
wallet account import public --private-key <hex>
wallet vault claim --account-id <id> --amount <n>
wallet deploy-program <elf-path>
```

The sequencer is `sequencer_service`, not a Docker container:
```bash
RISC0_DEV_MODE=0 sequencer_service configs/debug/sequencer_config.json
```

---

## CU benchmarks (RISC0_DEV_MODE=0, measured on AWS c5.2xlarge)

| Instruction | Cycles |
|---|---|
| `new_fungible_token` | 1 380 |
| `mint_tokens` | 4 072 |
| `transfer_tokens` | 2 606 |
| `burn_tokens` | 2 510 |
| `rotate_authority` | 3 166 |
| `revoke_authority` | 2 708 |

---

## Errors you will see during a normal test run (all expected)

```
thread 'unnamed' panicked at ... Program error 1003: admin authority has been revoked
```
This appears in `fixed_supply_mint_rejected` and `variable_supply_full_lifecycle`. The test deliberately triggers a rejection and asserts the error code. The panic is the guest signalling an error — lee catches it and the test checks it. This is correct behaviour.

```
thread 'unnamed' panicked at ... Program error 1001: signer is not the admin authority
```
Same — appears in `variable_supply_full_lifecycle` step 4 (old authority rejected after rotation).

---

## Common errors and how to fix them

| Error | Cause | Fix |
|---|---|---|
| `cannot find value 'main' in this scope` during Docker ELF build | Stale Docker build cache | `sudo docker builder prune -f` then rebuild |
| `expected attribute arguments in parentheses: #[account(...)]` | Bare `#[account]` in guest source | Change to `#[account()]` |
| `linking with rust-lld failed: undefined symbol: main` | `entry!(main)` removed or not guarded | Keep `#[cfg(not(test))] risc0_zkvm::guest::entry!(main);` |
| `Mismatch between number of nonces and signatures` | Nonce vec length ≠ signer count | One nonce per signer, not per account |
| `DefaultAccountModifiedWithoutClaim` | Recipient account has no program owner | Use `insert_empty_holding()` to pre-initialise the account |
| `libpython3.12.so not found` during wallet build | Missing python dev headers | `sudo apt-get install -y python3-dev` |
| `stdbool.h not found` during RocksDB build | Missing clang | `sudo apt-get install -y clang libclang-dev llvm-dev` |
| `unzip: not found` during rapidsnark build | Missing unzip | `sudo apt-get install -y unzip` |
| `Permission denied` copying Docker ELF | Docker builds as root | `sudo cp ...` then `sudo chown $USER:$USER ...` |
| Port 3040 already in use | Another sequencer running | `pkill sequencer_service` or use `LEZ_NODE` env var |

---

## What a reviewer will do

From the prize spec: *"Evaluators will independently clone the repository and run the demo script from a clean environment; the script must succeed without modification."*

Their flow:
1. Clone `github.com/aegonmyy/token-authority`
2. Build the LEZ node from `github.com/logos-blockchain/logos-execution-zone` (see demo.sh comments)
3. Run `./scripts/demo.sh` — expects it to complete with 4 tests passing
4. Read the README, FURPS.md, and code
5. May ask technical follow-up questions about implementation decisions

**Things they are most likely to flag:**

- **No video link in README** — add a YouTube/Loom URL to README.md after recording, then push
- **CI uses `RISC0_DEV_MODE=1`** — this is intentional; GitHub Actions doesn't have the compute for real proofs. The demo script runs `RISC0_DEV_MODE=0`. Both are documented.
- **`token_authority.bin` committed** — the ELF is committed so the demo works without the riscv toolchain. The source is in `methods/guest/src/bin/token_authority.rs` and can be rebuilt.
- **Committer identity** — commits show `Ubuntu <ubuntu@ip-172-31...>`. This is cosmetic and won't affect evaluation.

---

## What still needs to be done before submission

1. **Record the video** — run `./scripts/demo.sh`, narrate what you see. Must show `RISC0_DEV_MODE=0` in the output. Upload to YouTube (unlisted is fine).
2. **Add video link to README.md** — add a `## Demo video` section with the URL, then `git push`.
3. **Submit PR to `logos-co/lambda-prize`** — open a PR against that repo linking to this repo and the video.

---

## Dependency versions (pinned)

```toml
# methods/guest/Cargo.toml
spel-framework = { git = "https://github.com/logos-blockchain/logos-execution-zone", rev = "73fc462" }
lee_core      = { git = "https://github.com/logos-blockchain/logos-execution-zone", rev = "a58fbce2" }
risc0-zkvm     = "=3.0.5"

# integration_tests/Cargo.toml
lee           = { git = "https://github.com/logos-blockchain/logos-execution-zone", rev = "a58fbce2" }
lee_core      = { git = "https://github.com/logos-blockchain/logos-execution-zone", rev = "a58fbce2" }
```

If the Logos team updates `logos-execution-zone` and breaks the build, pin to tag `v0.1.2` explicitly.

---

## Git history

```
4eba81a  Remove AI indicators: strip verbose docstrings, stale PDA refs, what-comments
7b2ffe3  Add video script and fix unused import warning      ← video_script.md was here, removed from history
1c42dc5  Fix stale PDA/lgs references; update deployment status to done
6f1149f  Fill in CU cycle benchmarks in FURPS.md
e3605ff  Add integration tests + pre-built guest ELF for LP-0013
c638231  add CI, demo script, IDL, FURPS self-assessment
0c0f6ac  docs: README.md and CONTEXT.md
61bc348  phase 3: token_authority_sdk + examples (44/44 tests, 0 warnings)
3f698e0  phase 2: SPEL guest program — token_authority (6 instructions, 0 warnings)
0034e87  phase 1: admin_authority (RFP-001) and token_authority_core — 29/29 tests pass
```
