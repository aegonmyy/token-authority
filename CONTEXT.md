# CONTEXT.md — Token Authority (LP-0013)

> **For any AI or developer picking this up cold.**
> Read this file first. Everything needed to continue is here.

---

## What this repo is

A Lambda Prize ($600) submission for **LP-0013: Token program authorities** on the Logos Execution Zone (LEZ). The prize asks for:

1. An admin-authority library conforming to RFP-001 (single privileged admin, initialize/transfer/revoke lifecycle).
2. A token SPEL program that uses it to add a mint-authority model to LEZ fungible tokens.
3. Passing tests, typed SDK, examples, CU benchmarks, demo video, and a PR to `logos-co/lambda-prize`.

Prize spec: <https://github.com/logos-co/lambda-prize/issues/13>
RFP-001 spec: referenced inside LP-0013 — look at the `admin_authority` crate's module doc for the contract.

---

## Completion status

| Phase | Deliverable | Status | Notes |
|---|---|---|---|
| 1 | `admin_authority` crate (RFP-001) | ✅ Done | 18 tests, 0 warnings |
| 1 | `token_authority_core` crate | ✅ Done | 11 tests, 0 warnings |
| 2 | SPEL guest program (6 instructions) | ✅ Done | `cargo check` clean |
| 3 | `token_authority_sdk` host SDK | ✅ Done | 15 sim tests + doctest |
| 3 | `examples/fixed_supply` | ✅ Done | Runs, assertions pass |
| 3 | `examples/variable_supply` | ✅ Done | Runs, assertions pass |
| 3 | `README.md` | ✅ Done | Architecture, all instruction tables |
| 4 | Build guest ELF (RISC-Zero toolchain) | ⏳ Needs VPS | Requires `riscv32im` target |
| 4 | Deploy to LEZ devnet | ⏳ Needs VPS | `lgs localnet start` |
| 4 | Record CU benchmarks (all 6 instructions) | ⏳ Needs VPS | Cycle logs already in guest |
| 5 | Demo video (`RISC0_DEV_MODE=0`) | ⏳ Needs VPS | Record terminal session |
| 5 | Submit PR to `logos-co/lambda-prize` | ⏳ Last step | Draft below |

**Everything in Phases 1–3 is complete and locally committed (not pushed — user will create the remote).**
The only blocker is a Linux VPS with the RISC-Zero toolchain for Phases 4–5.

---

## File map

```
token-authority/
├── CONTEXT.md                     ← you are here
├── README.md                      ← project overview and prize docs
├── Cargo.toml                     ← workspace root (members: admin_authority,
│                                    token_authority_core, token_authority_sdk, examples)
│
├── admin_authority/               RFP-001 admin-authority primitive
│   └── src/lib.rs                 AdminConfig, require_admin, initialize/transfer/revoke
│                                  Error codes: 10xx (Unauthorized=1001, Revoked=1003, …)
│
├── token_authority_core/          Shared token state types (no SPEL/LEZ deps)
│   └── src/lib.rs                 TokenDef { name, decimals, total_supply, mint_authority }
│                                  TokenHolding { definition_id, balance }
│                                  apply_mint / apply_transfer / apply_burn helpers
│                                  Error codes: 20xx
│
├── token_authority_sdk/           Host SDK (no SPEL/LEZ deps, pure Rust)
│   └── src/
│       ├── lib.rs                 Re-exports + module-level doctest
│       ├── args.rs                Typed Borsh-serialisable instruction arg structs
│       ├── pda.rs                 Seed constants (b"token_def", b"mint_auth")
│       └── sim.rs                 SimulatedLedger — in-memory program execution for tests
│
├── examples/                      Runnable examples (no toolchain needed)
│   └── src/bin/
│       ├── fixed_supply.rs        Fixed supply: create, transfer, burn, mint-rejected
│       └── variable_supply.rs     Variable: TEAM→DAO rotate, DAO mints, DAO revokes
│
└── methods/
    └── guest/                     SEPARATE Cargo workspace (riscv32im target)
        ├── Cargo.toml             Deps: spel-framework, nssa_core, risc0-zkvm=3.0.5
        └── src/bin/
            └── token_authority.rs #[lez_program] with 6 #[instruction] handlers
```

---

## Key technical facts

### SPEL guest program

- Uses `#[lez_program]` + `#[instruction]` macros from `spel-framework` rev `73fc462`.
- Depends on `nssa_core` from `logos-blockchain/logos-execution-zone` tag `v0.1.2`.
- RISC-Zero version pinned: `risc0-zkvm = "=3.0.5"`.
- The guest is its **own Cargo workspace** (`methods/guest/Cargo.toml` contains `[workspace]`). It must be built separately with the `risc0` toolchain, not from the root workspace.
- Path deps point to `../../admin_authority` and `../../token_authority_core`.

### PDA seeds (must match exactly between guest and SDK)

```rust
b"token_def"  →  stores TokenDef (Borsh)
b"mint_auth"  →  stores AdminConfig (Borsh)
```

Holdings are non-PDA accounts keyed by holder — the client derives them.

### Account data format

All account data is **Borsh-encoded**. Read with `borsh::from_slice`, write via the struct's `.to_bytes()` method. No versioning prefix currently — add one if the schema changes post-launch.

### Cycle-count logging

Every instruction calls:
```rust
let cu_start = risc0_zkvm::guest::env::cycle_count();
// ... work ...
risc0_zkvm::guest::env::log(&format!("CU {} cycles={}", label, end - start));
```
After deployment, LEZ relays these log lines to stdout. Capture them for the benchmark table in `README.md`.

### Why `Option<[u8;32]>` for mint_authority in the guest args

SPEL macro-generated deserialization has trouble with `Option<[u8;32]>`. The guest instruction takes `mint_authority_id: Vec<u8>`:
- Empty `Vec` → `None` (fixed supply)
- 32-byte `Vec` → `Some([u8;32])` (authority account id)

The SDK's `NewFungibleTokenArgs::mint_authority_bytes()` handles this conversion.

### AdminConfig keeps TokenDef.mint_authority in sync

`rotate_authority` and `revoke_authority` update **both** `AdminConfig.admin` (in the `mint_auth` PDA) **and** `TokenDef.mint_authority` (in the `token_def` PDA). This redundancy lets clients read the current authority from either account. Keeping them in sync is the guest's responsibility.

---

## What to do next (ordered)

### Step 1 — Get a Linux VPS

Minimum spec: 4 CPU, 16 GB RAM, 100 GB NVMe, x86-64 Linux. A $44/mo 16 GB RAM VPS is fine.

```bash
# Confirm architecture
uname -m  # must print x86_64
```

### Step 2 — Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

### Step 3 — Install RISC-Zero toolchain

```bash
curl -L https://risczero.com/install | bash
rzup install
# Adds `cargo +risc0` and the riscv32im target
```

### Step 4 — Install LEZ CLI

```bash
cargo install --git https://github.com/logos-blockchain/logos-execution-zone lgs
```

### Step 5 — Clone the repo and verify tests pass

```bash
git clone <your-remote-url> token-authority
cd token-authority
cargo test --workspace --exclude token-authority-guest
# Expected: 44 passed, 0 failed
```

### Step 6 — Build the guest ELF

```bash
cd methods/guest
RISC0_DEV_MODE=0 cargo +risc0 build --release --target riscv32im-risc0-zkvm-elf
# ELF path: methods/guest/target/riscv32im-risc0-zkvm-elf/release/token_authority
```

This will take 10–20 minutes on first build (downloads RISC-Zero stdlib).

### Step 7 — Start LEZ localnet

```bash
lgs localnet start
# Keep this terminal open; it prints the RPC endpoint (usually http://127.0.0.1:8899)
```

### Step 8 — Deploy and benchmark

```bash
# Deploy program
lgs program deploy \
  methods/guest/target/riscv32im-risc0-zkvm-elf/release/token_authority

# Note the program_id printed. Then invoke each instruction with RISC0_DEV_MODE=0.
# The cycle logs appear as lines starting with "CU " in LEZ output.
# Fill in README.md's benchmark table with those numbers.
```

### Step 9 — Record demo video

Record a terminal session showing:
1. `lgs localnet start` running
2. `RISC0_DEV_MODE=0 lgs program deploy ...` with proof generation output
3. `new_fungible_token` invocation (show real ZK proof generated)
4. `mint_tokens` invocation
5. `rotate_authority` invocation
6. `revoke_authority` invocation
7. Failed `mint_tokens` after revocation

Upload to YouTube / Loom and note the URL.

### Step 10 — Submit PR

Create a PR to `logos-co/lambda-prize` with the body below (fill in `TODO` items).

---

## PR submission draft

```markdown
## LP-0013: Token program authorities

### Deliverables

- **`admin_authority` (RFP-001)** — agnostic mint-authority library. Zero SPEL/LEZ deps;
  usable in any Rust program. Implements `AdminConfig { admin: Option<[u8;32]> }` PDA with
  `require_admin()` gate + initialize / transfer_admin / revoke_admin state transitions.
  18 unit tests covering all paths including lifecycle and Borsh roundtrips.

- **`token_authority_core`** — `TokenDef` extended with `mint_authority: Option<[u8;32]>`,
  `TokenHolding`, and pure arithmetic helpers. 11 unit tests.

- **SPEL guest program** — `token_authority` ELF with 6 instructions:
  `new_fungible_token`, `mint_tokens`, `transfer_tokens`, `burn_tokens`,
  `rotate_authority`, `revoke_authority`. Authority gate wired to `admin_authority::require_admin`.

- **`token_authority_sdk`** — typed instruction-arg structs (Borsh-serialisable),
  PDA seed constants, `SimulatedLedger` for CI testing without RISC-Zero toolchain.
  15 simulation tests + doctest.

- **Examples** — `fixed_supply` and `variable_supply` both run and assert correct results.

### CU benchmarks (RISC0_DEV_MODE=0, LEZ devnet)

| Instruction | CU (cycles) |
|---|---|
| `new_fungible_token` | TODO |
| `mint_tokens` | TODO |
| `transfer_tokens` | TODO |
| `burn_tokens` | TODO |
| `rotate_authority` | TODO |
| `revoke_authority` | TODO |

### Demo video

TODO: link

### Test output

```
cargo test --workspace --exclude token-authority-guest
44 passed; 0 failed; 0 ignored
```
```

---

## Common errors

| Error | Cause | Fix |
|---|---|---|
| `error: no such command: +risc0` | RISC-Zero toolchain not installed | `rzup install` |
| `SIGILL` or `illegal instruction` | Running guest ELF on x86 directly | Use LEZ executor, never `./token_authority` directly |
| `AdminError::Revoked` when minting | Authority was revoked | Check `mint_auth` PDA; authority is `None` |
| `AdminError::Unauthorized` | Wrong signer | Signer account id must match `AdminConfig.admin` |
| `SpelError::SerializationError` | Corrupt or empty account data | Account was not initialised; check `new_fungible_token` ran first |
| Disk full during guest build | RISC-Zero downloads a large stdlib | Free 10+ GB; delete `methods/guest/target` to reset |
| `cargo check` passes but SPEL macros emit wrong code | SPEL framework API changed | Pin to rev `73fc462eb8f0a4d00f1a846437c627ec2e523f83` |

---

## Git history

```
61bc348  phase 3: token_authority_sdk + examples (44/44 tests, 0 warnings)
3f698e0  phase 2: SPEL guest program — token_authority (6 instructions, 0 warnings)
0034e87  phase 1: admin_authority (RFP-001) and token_authority_core — 29/29 tests pass
```

No remote set yet — user will `git remote add origin <url>` and push when ready.
