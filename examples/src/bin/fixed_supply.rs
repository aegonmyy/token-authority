//! Fixed-supply token example: mint authority is None from creation, so supply can never increase.
//!
//! Run with: cargo run --bin fixed_supply

use token_authority_sdk::SimulatedLedger;

const ISSUER: [u8; 32] = [0x01u8; 32];
const ALICE:  [u8; 32] = [0x0Au8; 32];
const BOB:    [u8; 32] = [0x0Bu8; 32];

fn main() {
    println!("=== Fixed-supply token example ===\n");

    let mut ledger = SimulatedLedger::new();

    ledger
        .new_fungible_token("LOGOS", 6, 1_000_000, None, ISSUER)
        .expect("new_fungible_token failed");

    let def = ledger.token_def().unwrap();
    println!(
        "Token created: {} (decimals={}, supply={})",
        def.name, def.decimals, def.total_supply
    );
    assert!(def.mint_authority.is_none());
    print_balances(&ledger);

    ledger.transfer_tokens(ISSUER, ALICE, 400_000).expect("transfer to ALICE failed");
    ledger.transfer_tokens(ISSUER, BOB,   250_000).expect("transfer to BOB failed");

    println!("\nAfter distribution:");
    print_balances(&ledger);

    let mint_result = ledger.mint_tokens(&ISSUER, ALICE, 1);
    assert!(mint_result.is_err());
    println!("\nmint_tokens rejected (as expected): {:?}", mint_result.unwrap_err());

    ledger.burn_tokens(ALICE, 100_000).expect("burn failed");
    println!("\nAfter ALICE burns 100_000:");
    print_balances(&ledger);

    let supply = ledger.total_supply();
    assert_eq!(supply, 900_000);
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
