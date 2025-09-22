use core::result::Result::Ok;

use bitcoin::{
    Address, Amount, FeeRate, Network, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut,
    Txid, Witness,
    absolute::LockTime,
    blockdata::script,
    consensus::Encodable,
    hashes::Hash,
    key::UntweakedKeypair,
    secp256k1::{Message, SECP256K1, XOnlyPublicKey, constants::SCHNORR_SIGNATURE_SIZE},
    sighash::{Prevouts, SighashCache},
    taproot::{ControlBlock, LeafVersion, TapLeafHash, TaprootBuilder},
    transaction::Version,
};
use bitcoincore_rpc::{Client as BitcoinRPCClient, RpcApi, json::FundRawTransactionOptions};
use rand::{RngCore, rngs::OsRng};

use crate::error::{Error, Result};

const MAX_PUSH_SIZE: usize = 520;

pub struct BuilderContext {
    pub rpc_client: BitcoinRPCClient,
    pub fee_rate: FeeRate,
    pub operator_l1_addr: Address,
    pub network: Network,
    pub amount: Amount,
}

pub fn create_inscription_tx(
    ctx: &BuilderContext,
    payload: Vec<u8>,
) -> Result<(Transaction, Transaction)> {
    // step 1: generate keypair
    let key_pair = generate_key_pair()?;
    let public_key = XOnlyPublicKey::from_keypair(&key_pair).0;

    // step 2: create reveal script
    let reveal_script = build_reveal_script(&public_key, &payload)?;
    let reveal_leaf = (reveal_script, LeafVersion::TapScript);

    // create merkle tree with a single leaf containing the reveal script
    let taproot_spend_info = TaprootBuilder::new()
        .add_leaf(0, reveal_leaf.0.clone())?
        .finalize(SECP256K1, public_key)
        .map_err(|_| Error::Internal("Unable to create taproot spend info".to_string()))?;

    // Create reveal address
    let reveal_address = Address::p2tr(
        SECP256K1,
        public_key,
        taproot_spend_info.merkle_root(),
        ctx.network,
    );

    // Get control block
    let control_block = taproot_spend_info
        .control_block(&reveal_leaf)
        .ok_or(Error::Internal("Cannot create control block".to_string()))?;

    // Calculate commit value
    let commit_value = calculate_reveal_input_value(
        ctx.amount,
        ctx.fee_rate,
        &ctx.operator_l1_addr,
        &reveal_leaf.0,
        &control_block,
    )?;

    // step 3: build the commit tx
    let unfunded_commit_tx = build_unfunded_commit_tx(&reveal_address, commit_value)?;

    // Fund the commit tx. Additional utxos might be added to the output set
    let unsigned_commit_tx = fund_tx(ctx, &unfunded_commit_tx)?;

    // Verify that the first TxIn of the funded commit tx is our commitment
    if unsigned_commit_tx.output[0] != unfunded_commit_tx.output[0] {
        return Err(Error::Internal("Unexpected error".to_string()));
    }

    // step 4: build and sign the reveal tx
    let signed_reveal_tx = build_and_sign_reveal_tx(
        ctx.amount,
        &ctx.operator_l1_addr,
        &unsigned_commit_tx,
        &reveal_leaf.0,
        &control_block,
        &key_pair,
    )?;

    // step 5: sign the commit tx
    let signed_commit_tx = ctx
        .rpc_client
        .sign_raw_transaction_with_wallet(&unsigned_commit_tx, None, None)?
        .transaction()?;

    Ok((signed_commit_tx, signed_reveal_tx))
}

/// Encode tx in non-segwit format.
/// This is needed for fundrawtransaction RPC call, which expects a non-segwit tx
fn encode_tx_non_segwit(tx: &Transaction) -> Result<Vec<u8>> {
    let mut encoder: Vec<u8> = Vec::new();
    tx.version.consensus_encode(&mut encoder)?;
    tx.input.consensus_encode(&mut encoder)?;
    tx.output.consensus_encode(&mut encoder)?;
    tx.lock_time.consensus_encode(&mut encoder)?;

    Ok(encoder)
}

fn fund_tx(ctx: &BuilderContext, tx: &Transaction) -> Result<Transaction> {
    let tx_raw = encode_tx_non_segwit(tx)?;
    let funded_tx = ctx
        .rpc_client
        .fund_raw_transaction(
            &tx_raw,
            Some(&FundRawTransactionOptions {
                fee_rate: ctx.fee_rate.fee_vb(1000), // convert to sat/kvB
                change_position: Some(1),
                ..Default::default()
            }),
            None,
        )?
        .transaction()?;

    Ok(funded_tx)
}

fn generate_key_pair() -> Result<UntweakedKeypair> {
    let mut rand_bytes = [0; 32];
    OsRng.fill_bytes(&mut rand_bytes);
    Ok(UntweakedKeypair::from_seckey_slice(SECP256K1, &rand_bytes)?)
}

fn build_reveal_script(taproot_public_key: &XOnlyPublicKey, payload: &[u8]) -> Result<ScriptBuf> {
    let mut script_builder = script::Builder::new()
        .push_x_only_key(taproot_public_key)
        .push_opcode(bitcoin::opcodes::all::OP_CHECKSIG)
        .push_opcode(bitcoin::opcodes::OP_FALSE)
        .push_opcode(bitcoin::opcodes::all::OP_IF);

    for chunk in payload.chunks(MAX_PUSH_SIZE) {
        let data = script::PushBytesBuf::try_from(chunk.to_vec())?;
        script_builder = script_builder.push_slice(data);
    }
    script_builder = script_builder.push_opcode(bitcoin::opcodes::all::OP_ENDIF);

    Ok(script_builder.into_script())
}

// Estimate the required input value for reveal_tx
fn calculate_reveal_input_value(
    amount: Amount,
    fee_rate: FeeRate,
    recipient: &Address,
    reveal_script: &script::ScriptBuf,
    control_block: &ControlBlock,
) -> Result<Amount> {
    let tx = Transaction {
        version: Version::TWO,
        input: vec![TxIn {
            previous_output: OutPoint {
                txid: Txid::all_zeros(),
                vout: 0,
            },
            script_sig: script::Builder::new().into_script(),
            sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
            witness: Witness::from_slice(&[
                vec![0; SCHNORR_SIGNATURE_SIZE],
                reveal_script.to_bytes(),
                control_block.serialize(),
            ]),
        }],
        output: vec![TxOut {
            script_pubkey: recipient.script_pubkey(),
            value: amount,
        }],
        lock_time: LockTime::ZERO,
    };

    let fee = fee_rate
        .fee_vb(tx.vsize() as u64)
        .ok_or(Error::Internal("Overflow error".to_string()))?;
    Ok(fee + amount)
}

fn build_unfunded_commit_tx(recipient: &Address, output_value: Amount) -> Result<Transaction> {
    // The first output contains the taproot commitment
    let outputs: Vec<TxOut> = vec![TxOut {
        value: output_value,
        script_pubkey: recipient.script_pubkey(),
    }];

    let commit_txn = Transaction {
        version: Version::TWO,
        input: vec![],
        output: outputs,
        lock_time: LockTime::ZERO,
    };

    Ok(commit_txn)
}

fn build_and_sign_reveal_tx(
    amount: Amount,
    recipient: &Address,
    unsigned_commit_tx: &Transaction,
    reveal_script: &ScriptBuf,
    control_block: &ControlBlock,
    key_pair: &UntweakedKeypair,
) -> Result<Transaction> {
    let outputs: Vec<TxOut> = vec![TxOut {
        value: amount,
        script_pubkey: recipient.script_pubkey(),
    }];

    let commit_txid = unsigned_commit_tx.compute_txid();

    let inputs = vec![TxIn {
        previous_output: OutPoint {
            txid: commit_txid,
            vout: 0,
        },
        script_sig: script::Builder::new().into_script(),
        sequence: Sequence::ENABLE_RBF_NO_LOCKTIME, // Since there's only one output, RBF might not work
        witness: Witness::new(),
    }];

    // Partial tx
    let tx = Transaction {
        version: Version::TWO,
        input: inputs,
        output: outputs,
        lock_time: LockTime::ZERO,
    };

    let mut cache = SighashCache::new(tx);
    let sighash = cache.taproot_script_spend_signature_hash(
        0,
        &Prevouts::All(&[&unsigned_commit_tx.output[0]]),
        TapLeafHash::from_script(reveal_script, LeafVersion::TapScript),
        bitcoin::sighash::TapSighashType::Default,
    )?;

    let signature = SECP256K1.sign_schnorr(
        &Message::from_digest_slice(sighash.as_byte_array())?,
        key_pair,
    );

    // Set the witness field for the taproot input
    let witness = cache
        .witness_mut(0)
        .ok_or(Error::Internal("Unable to get witness".to_string()))?;
    witness.push(signature.as_ref());
    witness.push(reveal_script);
    witness.push(control_block.serialize());

    Ok(cache.into_transaction())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::{opcodes, script::PushBytesBuf};
    use std::str::FromStr;

    fn get_public_key() -> XOnlyPublicKey {
        XOnlyPublicKey::from_str("4aa2ea0baac4158535936264f2027a3e7dc31bf1966c8f48b8a5087f256582f7")
            .unwrap()
    }

    fn get_testnet_address() -> Address {
        Address::from_str("tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx")
            .unwrap()
            .require_network(Network::Testnet)
            .unwrap()
    }

    #[test]
    fn test_generate_key_pair() {
        let key_pair = generate_key_pair().expect("Should generate a key pair without error");
        // Verify that a public key can be derived from the generated key pair
        let _public_key = XOnlyPublicKey::from_keypair(&key_pair).0;
    }

    #[test]
    fn test_build_reveal_script_small_payload() {
        let public_key = get_public_key();

        let script = build_reveal_script(&public_key, &[]).unwrap();
        let expected_script = ScriptBuf::from_hex(
            "204aa2ea0baac4158535936264f2027a3e7dc31bf1966c8f48b8a5087f256582f7ac006368",
        )
        .unwrap();
        assert_eq!(script, expected_script);

        let script = build_reveal_script(&public_key, b"Hello, world!").unwrap();
        let expected_script = ScriptBuf::from_hex("204aa2ea0baac4158535936264f2027a3e7dc31bf1966c8f48b8a5087f256582f7ac00630d48656c6c6f2c20776f726c642168").unwrap();
        assert_eq!(script, expected_script);
    }

    #[test]
    fn test_build_reveal_script_chunked_payload() {
        let public_key = get_public_key();
        // Create a payload larger than MAX_PUSH_SIZE (520 bytes)
        let mut long_payload = vec![0; 60000];
        OsRng.fill_bytes(&mut long_payload);

        let script = build_reveal_script(&public_key, &long_payload).unwrap();

        let mut expected_script_builder = script::Builder::new()
            .push_x_only_key(&public_key)
            .push_opcode(opcodes::all::OP_CHECKSIG)
            .push_opcode(opcodes::OP_FALSE)
            .push_opcode(opcodes::all::OP_IF);

        for chunk in long_payload.chunks(MAX_PUSH_SIZE) {
            expected_script_builder =
                expected_script_builder.push_slice(PushBytesBuf::try_from(chunk.to_vec()).unwrap());
        }
        expected_script_builder = expected_script_builder.push_opcode(opcodes::all::OP_ENDIF);
        let expected_script = expected_script_builder.into_script();

        assert_eq!(script, expected_script);
    }

    #[test]
    fn test_build_unfunded_commit_tx() {
        let recipient = get_testnet_address();
        let output_value = Amount::from_sat(1000);

        let tx = build_unfunded_commit_tx(&recipient, output_value).unwrap();

        assert_eq!(tx.version, Version::TWO);
        assert!(tx.input.is_empty());
        assert_eq!(tx.output.len(), 1);
        assert_eq!(tx.output[0].value, output_value);
        assert_eq!(tx.output[0].script_pubkey, recipient.script_pubkey());
        assert_eq!(tx.lock_time, LockTime::ZERO);
    }

    #[test]
    fn test_calculate_reveal_input_value() {
        let recipient = get_testnet_address();

        let public_key = get_public_key();
        // OP_0 OP_IF OP_PUSHBYTES_11 48656c6c6f20576f726c64 OP_ENDIF
        let reveal_script = ScriptBuf::from_hex("00630b48656c6c6f20576f726c6468").unwrap();

        let taproot_spend_info = TaprootBuilder::new()
            .add_leaf(0, reveal_script.clone())
            .unwrap()
            .finalize(SECP256K1, public_key)
            .unwrap();
        let control_block = taproot_spend_info
            .control_block(&(reveal_script.clone(), LeafVersion::TapScript))
            .unwrap();

        let calculated_value = calculate_reveal_input_value(
            Amount::from_sat(5000),
            FeeRate::from_sat_per_vb(10).unwrap(),
            &recipient,
            &reveal_script,
            &control_block,
        )
        .unwrap();

        assert_eq!(calculated_value, Amount::from_sat(6120));

        let calculated_value = calculate_reveal_input_value(
            Amount::from_sat(0),
            FeeRate::from_sat_per_vb(1).unwrap(),
            &recipient,
            &reveal_script,
            &control_block,
        )
        .unwrap();

        assert_eq!(calculated_value, Amount::from_sat(112));
    }
}
