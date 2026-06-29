use nssa::{
    program::Program,
    program_deployment_transaction::{self, ProgramDeploymentTransaction},
    public_transaction, PrivateKey, PublicKey, PublicTransaction, V03State,
};
use nssa_core::account::{Account, AccountId, Data, Nonce};
use token_authority_core::TokenHolding;
use serde::{Deserialize, Serialize};

// ── Key fixtures ─────────────────────────────────────────────────────────────

fn k(seed: u8) -> PrivateKey { PrivateKey::try_new([seed; 32]).expect("valid") }

fn program_id() -> nssa_core::program::ProgramId {
    token_authority_methods::TOKEN_AUTHORITY_ID
}

fn id(key: &PrivateKey) -> AccountId {
    AccountId::from(&PublicKey::new_from_private_key(key))
}

fn id_bytes(key: &PrivateKey) -> Vec<u8> {
    id(key).as_ref().to_vec()
}

// ── Deployment ───────────────────────────────────────────────────────────────

fn deploy(state: &mut V03State) {
    let msg = program_deployment_transaction::Message::new(
        token_authority_methods::TOKEN_AUTHORITY_ELF.to_vec(),
    );
    state
        .transition_from_program_deployment_transaction(&ProgramDeploymentTransaction::new(msg))
        .expect("deploy must succeed");
}

fn base_state() -> V03State {
    let mut state = V03State::new_with_genesis_accounts(&[], vec![], 0);
    deploy(&mut state);
    state
}

fn insert_empty_holding(state: &mut V03State, account: AccountId, def: AccountId) {
    let def_bytes: [u8; 32] = def.as_ref().try_into().expect("AccountId is 32 bytes");
    let holding = TokenHolding::new_zero(def_bytes);
    let acc = Account {
        program_owner: program_id(),
        balance: 0,
        data: Data::try_from(holding.to_bytes()).expect("holding data valid"),
        nonce: Nonce(0),
    };
    state.force_insert_account(account, acc);
}

// ── Instruction enum ─────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
enum Instruction {
    NewFungibleToken {
        name: String,
        decimals: u8,
        initial_supply: u128,
        mint_authority_id: Vec<u8>,
    },
    MintTokens      { amount: u128 },
    TransferTokens  { amount: u128 },
    BurnTokens      { amount: u128 },
    RotateAuthority { new_authority_id: Vec<u8> },
    RevokeAuthority,
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn send(
    state: &mut V03State,
    accounts: Vec<AccountId>,
    nonces: Vec<Nonce>,
    instruction: Instruction,
    signers: &[&PrivateKey],
) {
    let msg = public_transaction::Message::try_new(
        program_id(), accounts, nonces, instruction,
    ).expect("message");
    let witness = public_transaction::WitnessSet::for_message(&msg, signers);
    state.transition_from_public_transaction(&PublicTransaction::new(msg, witness), 0, 0)
        .expect("tx must succeed");
}

fn send_err(
    state: &mut V03State,
    accounts: Vec<AccountId>,
    nonces: Vec<Nonce>,
    instruction: Instruction,
    signers: &[&PrivateKey],
) -> String {
    let msg = public_transaction::Message::try_new(
        program_id(), accounts, nonces, instruction,
    ).expect("message");
    let witness = public_transaction::WitnessSet::for_message(&msg, signers);
    state.transition_from_public_transaction(&PublicTransaction::new(msg, witness), 0, 0)
        .expect_err("must fail")
        .to_string()
}

// ── Tests ────────────────────────────────────────────────────────────────────

// Key assignments (each unique seed → unique AccountId):
// 10 = def,  11 = auth_pda,  12 = creator_holding,  13 = creator (signer)
// 14 = authority,  15 = new_authority,  16 = recipient
// 20 = def2, 21 = auth2_pda, 22 = creator2_holding, 23 = creator2

/// Fixed-supply: any mint attempt is rejected.
#[test]
fn fixed_supply_mint_rejected() {
    let def_k    = k(10);
    let auth_k   = k(11);
    let holding_k = k(12);
    let creator_k = k(13);
    let mint_k   = k(14);

    let def_id     = id(&def_k);
    let auth_id    = id(&auth_k);
    let holding_id = id(&holding_k);
    let creator_id = id(&creator_k);
    let mint_id    = id(&mint_k);

    let mut state = base_state();

    send(
        &mut state,
        vec![def_id, auth_id, holding_id, creator_id],
        vec![Nonce(0); 4],
        Instruction::NewFungibleToken {
            name: String::from("FIXED"),
            decimals: 6,
            initial_supply: 1_000_000,
            mint_authority_id: vec![],
        },
        &[&def_k, &auth_k, &holding_k, &creator_k],
    );

    let err = send_err(
        &mut state,
        vec![def_id, auth_id, holding_id, mint_id],
        vec![Nonce(0)],
        Instruction::MintTokens { amount: 100 },
        &[&mint_k],
    );
    assert!(
        err.contains("1003") || err.contains("1004")
            || err.contains("Revoked") || err.contains("NullAuthority"),
        "expected authority error, got: {err}"
    );
}

/// Full lifecycle: create → mint → rotate → old rejected → new mints → revoke → post-revoke rejected.
#[test]
fn variable_supply_full_lifecycle() {
    let def_k      = k(20);
    let auth_k     = k(21);
    let holding_k  = k(22);
    let creator_k  = k(23);
    let authority_k = k(14);
    let new_auth_k = k(15);
    let recipient_k = k(16);

    let def_id       = id(&def_k);
    let auth_id      = id(&auth_k);
    let holding_id   = id(&holding_k);
    let creator_id   = id(&creator_k);
    let authority_id = id(&authority_k);
    let new_auth_id  = id(&new_auth_k);
    let recipient_id = id(&recipient_k);

    let mut state = base_state();

    // 1. Create variable-supply token.
    send(
        &mut state,
        vec![def_id, auth_id, holding_id, creator_id],
        vec![Nonce(0); 4],
        Instruction::NewFungibleToken {
            name: String::from("VAR"),
            decimals: 6,
            initial_supply: 50_000_000,
            mint_authority_id: id_bytes(&authority_k),
        },
        &[&def_k, &auth_k, &holding_k, &creator_k],
    );

    // 2. Authority mints to recipient (pre-insert empty holding so it's already claimed).
    insert_empty_holding(&mut state, recipient_id, def_id);
    send(
        &mut state,
        vec![def_id, auth_id, recipient_id, authority_id],
        vec![Nonce(0)],
        Instruction::MintTokens { amount: 10_000_000 },
        &[&authority_k],
    );

    // 3. Rotate authority to new_auth.
    send(
        &mut state,
        vec![def_id, auth_id, authority_id],
        vec![Nonce(1)],
        Instruction::RotateAuthority { new_authority_id: id_bytes(&new_auth_k) },
        &[&authority_k],
    );

    // 4. Old authority must be rejected (1001).
    let err = send_err(
        &mut state,
        vec![def_id, auth_id, recipient_id, authority_id],
        vec![Nonce(2)],
        Instruction::MintTokens { amount: 1 },
        &[&authority_k],
    );
    assert!(
        err.contains("1001") || err.contains("Unauthorized"),
        "old authority should be rejected with 1001, got: {err}"
    );

    // 5. New authority mints.
    send(
        &mut state,
        vec![def_id, auth_id, recipient_id, new_auth_id],
        vec![Nonce(0)],
        Instruction::MintTokens { amount: 5_000_000 },
        &[&new_auth_k],
    );

    // 6. Revoke authority.
    send(
        &mut state,
        vec![def_id, auth_id, new_auth_id],
        vec![Nonce(1)],
        Instruction::RevokeAuthority,
        &[&new_auth_k],
    );

    // 7. Post-revoke mint rejected (1003).
    let err = send_err(
        &mut state,
        vec![def_id, auth_id, recipient_id, new_auth_id],
        vec![Nonce(2)],
        Instruction::MintTokens { amount: 1 },
        &[&new_auth_k],
    );
    assert!(
        err.contains("1003") || err.contains("Revoked"),
        "post-revoke mint should fail with 1003, got: {err}"
    );
}

/// Transfer moves tokens between two distinct holdings.
#[test]
fn transfer_tokens() {
    let def_k       = k(30);
    let auth_k      = k(31);
    let holding_k   = k(32);
    let creator_k   = k(33);
    let recipient_k = k(34);

    let def_id       = id(&def_k);
    let auth_id      = id(&auth_k);
    let holding_id   = id(&holding_k);
    let creator_id   = id(&creator_k);
    let recipient_id = id(&recipient_k);

    let mut state = base_state();

    send(
        &mut state,
        vec![def_id, auth_id, holding_id, creator_id],
        vec![Nonce(0); 4],
        Instruction::NewFungibleToken {
            name: String::from("TRF"),
            decimals: 0,
            initial_supply: 1_000,
            mint_authority_id: vec![],
        },
        &[&def_k, &auth_k, &holding_k, &creator_k],
    );

    insert_empty_holding(&mut state, recipient_id, def_id);
    send(
        &mut state,
        vec![def_id, holding_id, recipient_id, creator_id],
        vec![Nonce(1)],
        Instruction::TransferTokens { amount: 300 },
        &[&creator_k],
    );
}

/// Burn reduces supply.
#[test]
fn burn_tokens() {
    let def_k     = k(40);
    let auth_k    = k(41);
    let holding_k = k(42);
    let creator_k = k(43);

    let def_id     = id(&def_k);
    let auth_id    = id(&auth_k);
    let holding_id = id(&holding_k);
    let creator_id = id(&creator_k);

    let mut state = base_state();

    send(
        &mut state,
        vec![def_id, auth_id, holding_id, creator_id],
        vec![Nonce(0); 4],
        Instruction::NewFungibleToken {
            name: String::from("BURN"),
            decimals: 0,
            initial_supply: 500,
            mint_authority_id: vec![],
        },
        &[&def_k, &auth_k, &holding_k, &creator_k],
    );

    send(
        &mut state,
        vec![def_id, holding_id, creator_id],
        vec![Nonce(1)],
        Instruction::BurnTokens { amount: 200 },
        &[&creator_k],
    );
}
