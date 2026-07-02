//! Variable-supply token example: full authority lifecycle.
//!
//! TEAM mints -> rotates to DAO -> DAO mints -> DAO revokes -> supply locked.
//!
//! Run with: cargo run --bin variable_supply

use token_authority_sdk::SimulatedLedger;

const TEAM:     [u8; 32] = [0x01u8; 32];
const DAO:      [u8; 32] = [0x02u8; 32];
const ALICE:    [u8; 32] = [0x0Au8; 32];
const BOB:      [u8; 32] = [0x0Bu8; 32];
const TREASURY: [u8; 32] = [0x0Cu8; 32];

fn main() {
    println!("=== Variable-supply token example ===\n");

    let mut ledger = SimulatedLedger::new();

    ledger
        .new_fungible_token("LOGOS", 6, 50_000_000, Some(TEAM), TEAM)
        .expect("new_fungible_token failed");

    let def = ledger.token_def().unwrap();
    println!(
        "Token created: {} (decimals={}, supply={}, authority={})",
        def.name, def.decimals, def.total_supply,
        authority_label(def.mint_authority),
    );
    print_balances(&ledger);

    ledger.mint_tokens(&TEAM, ALICE, 10_000_000).expect("mint to ALICE failed");
    ledger.mint_tokens(&TEAM, BOB,    5_000_000).expect("mint to BOB failed");
    println!("\nAfter TEAM mints community allocation:");
    print_balances(&ledger);

    ledger.rotate_authority(&TEAM, DAO).expect("rotate_authority failed");
    println!(
        "\nAuthority rotated -> {}",
        authority_label(ledger.token_def().unwrap().mint_authority)
    );

    let stale_mint = ledger.mint_tokens(&TEAM, ALICE, 1);
    assert!(stale_mint.is_err());
    println!("TEAM mint attempt rejected: {:?}", stale_mint.unwrap_err());

    ledger
        .mint_tokens(&DAO, TREASURY, 20_000_000)
        .expect("DAO mint to TREASURY failed");
    println!("\nAfter DAO mints governance reserve:");
    print_balances(&ledger);

    ledger.revoke_authority(&DAO).expect("revoke_authority failed");
    println!(
        "\nMint authority revoked. Supply is now permanently fixed at {}.",
        ledger.total_supply()
    );

    let post_revoke = ledger.mint_tokens(&DAO, TREASURY, 1);
    assert!(post_revoke.is_err());
    println!("Post-revoke mint attempt rejected: {:?}", post_revoke.unwrap_err());

    ledger.transfer_tokens(ALICE, BOB, 1_000_000).expect("transfer failed");
    ledger.burn_tokens(BOB, 500_000).expect("burn failed");
    println!("\nAfter transfer and burn:");
    print_balances(&ledger);

    let final_supply = ledger.total_supply();
    println!("\nFinal total supply: {}", final_supply);
    assert_eq!(final_supply, 50_000_000 + 10_000_000 + 5_000_000 + 20_000_000 - 500_000);
    println!("Assertion passed");

    println!("\nVariable-supply example complete.");
}

fn authority_label(auth: Option<[u8; 32]>) -> &'static str {
    match auth {
        None                                => "NONE (fixed)",
        Some(id) if id == [0x01u8; 32]     => "TEAM",
        Some(id) if id == [0x02u8; 32]     => "DAO",
        _                                   => "UNKNOWN",
    }
}

fn print_balances(l: &SimulatedLedger) {
    println!(
        "  TEAM={:>12}  ALICE={:>12}  BOB={:>12}  TREASURY={:>12}  supply={:>12}",
        l.balance_of(&TEAM),
        l.balance_of(&ALICE),
        l.balance_of(&BOB),
        l.balance_of(&TREASURY),
        l.total_supply(),
    );
}
