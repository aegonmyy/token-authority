# Live LEZ v0.2.0 Testnet Evidence — 2026-07-02

The mint-authority model is deployed and exercised **end-to-end on the current
public LEZ testnet**, with the full lifecycle (create → mint → rotate → mint by
the rotated authority → revoke → post-revoke mint rejected) confirmed on-chain.
Every transaction below was included in a block and is independently
re-verifiable.

## Environment

| Field | Value |
| --- | --- |
| Network | `https://testnet.lez.logos.co/` (public LEZ testnet) |
| LEZ ref | `v0.2.0` final, commit `a58fbce2` |
| Program ID (image id) | `63a29a4ec2b24402807c319d14e5d9a6bd5b26a49088cb3c6c2c8cd6187d2a60` |
| Program binary | `token_authority.bin` (deterministic; `spel program-id` == `methods/build.rs` == `cargo risczero`) |
| Explorer | `https://explorer.testnet.lez.logos.co/` |
| Date | 2026-07-02 UTC |

The program is deployed with `wallet deploy-program token_authority.bin`; the
lifecycle transactions below are themselves proof the deployed program is live
(they execute its instructions on-chain).

## Accounts (this run)

| Role | Account |
| --- | --- |
| token_def | `Public/FiTy87G8wX59fFMKXJ4VYUpHf3xWd1S9JxpfwFsmm72V` |
| auth (AdminConfig) | `Public/5dpT3HkxroYF4vxmgw7MjMXnUvUajVxJmdVWddDaJ5hA` |
| holding | `Public/621EpFw7FmLn299YbuqLLYNiRfFg9pzLHLFokPUsPTzB` |
| authority #1 (creator) | `Public/DMM5PoAkFUhWvUPfnu8EgSdpxfWDmPfqf2xZLzTTkzaq` |
| authority #2 (rotated) | `Public/GNMHsCC5EVFvyRdnQvGABh8ZTSHsSYMMFP8hMLCposo7` |

Signer accounts are fresh keypairs; the program claims the `init` accounts on
first use, so no genesis pre-funding is required.

## Lifecycle transactions (all confirmed in a block)

| # | Operation | Effect | Tx hash |
| --- | --- | --- | --- |
| 1 | `new_fungible_token` | create `AUTHDEMO`, supply 1000, mint authority = creator | `4fee352813b6e36bfc226432c7379deb3943f5b6eaa0b0c7c8b14477254ad94d` |
| 2 | `mint_tokens` (creator) | mint 500 → supply 1500 | `112459026bc3114ad86b13d33edf2cdb1f11fc1a249f22b1c28de59689563bda` |
| 3 | `rotate_authority` | authority creator → authority #2 | `bfd1f1bef1c7797e6a375129a2e7d59a4e92b27715c0fd3b671cd147ce87cc44` |
| 4 | `mint_tokens` (authority #2) | mint 300 → supply 1800 (proves rotation) | `3da0cf3edc3ae61cfaccc661837803ef7023b5cc67c1b0b23ec9a8eb4a3721ad` |
| 5 | `revoke_authority` | authority → None (supply now fixed) | `2097083f54e07f8a60348244b1ecbfb493818209b05f56474812a38b04055cbe` |
| 6 | `mint_tokens` after revoke | **rejected** by the authority guard (error 1003); supply stays 1800 | (not included — deterministic rejection) |

Each hash resolves via `wallet chain-info transaction --hash <h>` against the
testnet, or the block explorer.

## Final on-chain state (independently queried)

```
token_def:  data = 08000000 4155544844454d4f 06 08070000000000000000000000000000 00
            => TokenDef { name: "AUTHDEMO", decimals: 6, total_supply: 1800, mint_authority: None }
auth_acc:   data = 00
            => AdminConfig { admin: None }        # revoked → supply permanently fixed
holding:    data = <definition_id> 08070000000000000000000000000000
            => TokenHolding { balance: 1800 }
```

The post-revoke mint (step 6) did **not** change supply (still 1800), which is
the on-chain proof that revocation makes supply fixed. The deterministic `1003`
rejection is also covered by `integration_tests/tests/token_authority.rs`.

## Reproduce / re-verify (survives testnet resets)

The public testnet is periodically reset. To (re-)produce live evidence on the
*current* network in one command:

```bash
SPEL=vendor/spel-framework/spel-cli/target/release/spel \
WALLET=<path to v0.2.0-final wallet> \
LEE_WALLET_HOME_DIR=<wallet home pointed at testnet.lez.logos.co> \
./scripts/testnet-lifecycle.sh
```

It derives fresh accounts, runs all six steps, prints every tx hash, and reads
back the final state — so a reviewer can re-confirm on-chain at any time, even
after a reset.
