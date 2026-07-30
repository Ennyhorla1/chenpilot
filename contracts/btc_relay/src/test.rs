#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Bytes, BytesN, Env, Vec};

use btc_relay_crypto::BtcCryptoContract;

fn setup(env: &Env) -> (Address, Address, BtcRelayContractClient) {
    let admin = Address::generate(env);
    let token = Address::generate(env);

    let crypto_id = env.register(BtcCryptoContract, ());
    let contract_id = env.register(BtcRelayContract, ());
    let client = BtcRelayContractClient::new(env, &contract_id);
    client.initialize(&admin, &token, &1, &crypto_id);
    (admin, token, client)
}

fn setup_with_confirmations(env: &Env, min_confirmations: u32) -> BtcRelayContractClient {
    let admin = Address::generate(env);
    let token = Address::generate(env);
    let crypto_id = env.register(BtcCryptoContract, ());
    let contract_id = env.register(BtcRelayContract, ());
    let client = BtcRelayContractClient::new(env, &contract_id);
    client.initialize(&admin, &token, &min_confirmations, &crypto_id);
    client
}

fn make_header(env: &Env, merkle_root: &BytesN<32>) -> Bytes {
    let mut header = [0u8; 80];
    header[72] = 0xff;
    header[73] = 0xff;
    header[74] = 0x7f;
    header[75] = 0x20;

    let root_arr = merkle_root.to_array();
    for i in 0..32 {
        header[36 + i] = root_arr[i];
    }
    Bytes::from_slice(env, &header)
}

fn dsha256(env: &Env, data: &Bytes) -> BytesN<32> {
    let first: BytesN<32> = env.crypto().sha256(data).into();
    let first_bytes = Bytes::from_slice(env, first.to_array().as_ref());
    env.crypto().sha256(&first_bytes).into()
}

fn single_leaf_proof(env: &Env, tx_id: &BytesN<32>) -> (BytesN<32>, Vec<BytesN<32>>) {
    let mut combined = Bytes::new(env);
    combined.extend_from_slice(tx_id.to_array().as_ref());
    combined.extend_from_slice(tx_id.to_array().as_ref());
    let root = dsha256(env, &combined);

    let mut proof = Vec::new(env);
    proof.push_back(tx_id.clone());
    (root, proof)
}

fn proof_at_depth(
    env: &Env,
    tx_id: &BytesN<32>,
    depth: u32,
) -> (BytesN<32>, Vec<BytesN<32>>) {
    let mut current = tx_id.clone();
    let mut proof = Vec::new(env);

    for _ in 0..depth {
        let sibling = current.clone();
        let mut combined = Bytes::new(env);
        combined.extend_from_slice(current.to_array().as_ref());
        combined.extend_from_slice(sibling.to_array().as_ref());
        current = dsha256(env, &combined);
        proof.push_back(sibling);
    }

    (current, proof)
}

#[test]
fn test_initialize_and_get_config() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, token, client) = setup(&env);

    let cfg = client.get_config();
    assert_eq!(cfg.admin, admin);
    assert_eq!(cfg.wrapped_btc_token, token);
    assert_eq!(cfg.min_confirmations, 1);
}

#[test]
#[should_panic]
fn test_double_initialize() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, token, client) = setup(&env);
    let crypto_id = Address::generate(&env);
    client.initialize(&admin, &token, &1, &crypto_id);
}

#[test]
fn test_valid_spv_proof_accepted() {
    let env = Env::default();
    env.mock_all_auths();
    let (_admin, _token, client) = setup(&env);

    let recipient = Address::generate(&env);
    let tx_id = BytesN::from_array(&env, &[0xabu8; 32]);
    let (root, proof) = single_leaf_proof(&env, &tx_id);
    let header = make_header(&env, &root);

    let result = client.verify_and_claim(&SpvProof {
        block_header: header,
        tx_id: tx_id.clone(),
        merkle_proof: proof,
        tx_index: 0,
        amount_sat: 100_000_000,
        recipient: recipient.clone(),
    });

    assert_eq!(result.0, recipient);
    assert_eq!(result.1, 100_000_000i128);
    assert!(client.is_claimed(&tx_id));
}

#[test]
#[should_panic]
fn test_replay_attack_blocked() {
    let env = Env::default();
    env.mock_all_auths();
    let (_admin, _token, client) = setup(&env);

    let recipient = Address::generate(&env);
    let tx_id = BytesN::from_array(&env, &[0xbbu8; 32]);
    let (root, proof) = single_leaf_proof(&env, &tx_id);
    let header = make_header(&env, &root);

    client.verify_and_claim(&SpvProof {
        block_header: header.clone(),
        tx_id: tx_id.clone(),
        merkle_proof: proof.clone(),
        tx_index: 0,
        amount_sat: 50_000_000,
        recipient: recipient.clone(),
    });
    assert!(client.is_claimed(&tx_id));

    client.verify_and_claim(&SpvProof {
        block_header: header,
        tx_id,
        merkle_proof: proof,
        tx_index: 0,
        amount_sat: 50_000_000,
        recipient,
    });
}

#[test]
#[should_panic]
fn test_invalid_header_length() {
    let env = Env::default();
    env.mock_all_auths();
    let (_admin, _token, client) = setup(&env);

    let recipient = Address::generate(&env);
    let tx_id = BytesN::from_array(&env, &[0x01u8; 32]);
    let (_, proof) = single_leaf_proof(&env, &tx_id);

    client.verify_and_claim(&SpvProof {
        block_header: Bytes::from_slice(&env, &[0u8; 40]),
        tx_id,
        merkle_proof: proof,
        tx_index: 0,
        amount_sat: 1,
        recipient,
    });
}

#[test]
#[should_panic]
fn test_invalid_merkle_proof_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (_admin, _token, client) = setup(&env);

    let recipient = Address::generate(&env);
    let tx_id = BytesN::from_array(&env, &[0xddu8; 32]);
    let wrong_root = BytesN::from_array(&env, &[0x00u8; 32]);
    let header = make_header(&env, &wrong_root);

    let mut proof = Vec::new(&env);
    proof.push_back(BytesN::from_array(&env, &[0xeeu8; 32]));

    client.verify_and_claim(&SpvProof {
        block_header: header,
        tx_id,
        merkle_proof: proof,
        tx_index: 0,
        amount_sat: 1,
        recipient,
    });
}

#[test]
#[should_panic]
fn test_insufficient_confirmations() {
    let env = Env::default();
    env.mock_all_auths();
    let client = setup_with_confirmations(&env, 6);

    let recipient = Address::generate(&env);
    let tx_id = BytesN::from_array(&env, &[0xffu8; 32]);
    let (root, proof) = single_leaf_proof(&env, &tx_id);
    let header = make_header(&env, &root);

    client.verify_and_claim(&SpvProof {
        block_header: header,
        tx_id,
        merkle_proof: proof,
        tx_index: 0,
        amount_sat: 1,
        recipient,
    });
}

#[test]
fn test_deep_reorg_does_not_revert_or_halt_claims() {
    let env = Env::default();
    env.mock_all_auths();
    let client = setup_with_confirmations(&env, 6);
    let recipient = Address::generate(&env);

    let original_tx = BytesN::from_array(&env, &[0x11u8; 32]);
    let (original_root, original_proof) = proof_at_depth(&env, &original_tx, 6);
    let original_header = make_header(&env, &original_root);

    client.verify_and_claim(&SpvProof {
        block_header: original_header,
        tx_id: original_tx.clone(),
        merkle_proof: original_proof,
        tx_index: 0,
        amount_sat: 1,
        recipient: recipient.clone(),
    });
    assert!(client.is_claimed(&original_tx));

    // A competing branch is represented by a different valid seven-level SPV
    // proof, deeper than the configured six-confirmation window. The relay
    // has no header-chain state with which to identify this as a
    // reorganization, so it accepts it as an independent claim.
    let competing_tx = BytesN::from_array(&env, &[0x22u8; 32]);
    let (competing_root, competing_proof) = proof_at_depth(&env, &competing_tx, 7);
    let competing_header = make_header(&env, &competing_root);

    client.verify_and_claim(&SpvProof {
        block_header: competing_header,
        tx_id: competing_tx.clone(),
        merkle_proof: competing_proof,
        tx_index: 0,
        amount_sat: 1,
        recipient,
    });

    assert!(client.is_claimed(&original_tx));
    assert!(client.is_claimed(&competing_tx));
}

#[test]
fn test_update_config() {
    let env = Env::default();
    env.mock_all_auths();
    let (admin, token, client) = setup(&env);

    let new_admin = Address::generate(&env);
    let crypto_id = env.register(BtcCryptoContract, ());
    client.update_config(&Config {
        admin: new_admin.clone(),
        wrapped_btc_token: token.clone(),
        min_confirmations: 6,
        crypto_contract: crypto_id,
    });

    let cfg = client.get_config();
    assert_eq!(cfg.admin, new_admin);
    assert_eq!(cfg.wrapped_btc_token, token);
    assert_eq!(cfg.min_confirmations, 6);
    assert_eq!(cfg.crypto_contract, crypto_id);
    assert_ne!(cfg.admin, admin);
}
