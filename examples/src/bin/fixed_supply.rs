//! Example: fixed-supply fungible token.
//!
//! Demonstrates a token where the total supply is set at creation and
//! can never be increased. The mint authority is `None` from the start.
//!
//! Run with:
//!   cargo run --bin fixed_supply
//!
//! For the real on-chain flow, replace `SimulatedLedger` with the LEZ SPEL
//! executor loaded with the compiled `token_authority` ELF.

use token_authority_sdk::SimulatedLedger;

// Pretend account ids — on a real network these are 32-byte public keys.
const ISSUER:   [u8; 32] = [0x01u8; 32];
const ALICE:    [u8; 32] = [0x0Au8; 32];
const BOB:      [u8; 32] = [0x0Bu8; 32];

fn main() {
    println!("=== Fixed-supply token example ===\n");

    let mut ledger = SimulatedLedger::new();

    // ── 1. Create token ───────────────────────────────────────────────────────
    // `mint_authority: None` means supply is locked at creation.
    // The ISSUER receives the entire initial supply.
    ledger
        .new_fungible_token("LOGOS", 6, 1_000_000, None, ISSUER)
        .expect("new_fungible_token failed");

    let def = ledger.token_def().unwrap();
    println!(
        "Token created: {} (decimals={}, supply={})",
        def.name, def.decimals, def.total_supply
    );
    assert!(def.mint_authority.is_none(), "fixed supply must have no authority");
    print_balances(&ledger);

    // ── 2. Distribute tokens ──────────────────────────────────────────────────
    ledger.transfer_tokens(ISSUER, ALICE, 400_000).expect("transfer to ALICE failed");
    ledger.transfer_tokens(ISSUER, BOB,   250_000).expect("transfer to BOB failed");

    println!("\nAfter distribution:");
    print_balances(&ledger);

    // ── 3. Attempt to mint — must fail ────────────────────────────────────────
    let mint_result = ledger.mint_tokens(&ISSUER, ALICE, 1);
    assert!(mint_result.is_err(), "minting on a fixed-supply token must be rejected");
    println!("\nmint_tokens rejected (as expected): {:?}", mint_result.unwrap_err());

    // ── 4. ALICE burns some tokens ────────────────────────────────────────────
    ledger.burn_tokens(ALICE, 100_000).expect("burn failed");
    println!("\nAfter ALICE burns 100_000:");
    print_balances(&ledger);

    // ── 5. Verify supply decreased ────────────────────────────────────────────
    let supply = ledger.total_supply();
    assert_eq!(supply, 900_000, "supply should be 900_000 after burn");
    println!("\nFinal total supply: {} ✓", supply);
    println!("\nFixed-supply example complete.");
}

fn print_balances(l: &SimulatedLedger) {
    println!(
        "  ISSUER={:>10}  ALICE={:>10}  BOB={:>10}  total_supply={:>10}",
        l.balance_of(&ISSUER),
        l.balance_of(&ALICE),
        l.balance_of(&BOB),
        l.total_supply(),
    );
}
