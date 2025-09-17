use core::{result::Result::Ok, str::FromStr};

use anyhow::anyhow;
use bitcoin::{
    Address, Amount, FeeRate, Network, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut,
    Txid, Witness,
    absolute::LockTime,
    blockdata::script,
    consensus::Encodable,
    hashes::Hash,
    key::UntweakedKeypair,
    secp256k1::{
        Message, SECP256K1, XOnlyPublicKey, constants::SCHNORR_SIGNATURE_SIZE, schnorr::Signature,
    },
    sighash::{Prevouts, SighashCache},
    taproot::{ControlBlock, LeafVersion, TapLeafHash, TaprootBuilder},
    transaction::Version,
};
use bitcoincore_rpc::{Client as BitcoinRPCClient, RpcApi, json};
use rand::{RngCore, rngs::OsRng};

use crate::error::BatchSubmitterError;

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
) -> Result<(Transaction, Transaction), BatchSubmitterError> {
    // step 1: generate keypair
    let key_pair = generate_key_pair()?;
    let public_key = XOnlyPublicKey::from_keypair(&key_pair).0;

    // step 2: create reveal script
    let reveal_script = build_reveal_script(&public_key, &payload)?;
    // create merkle tree with a single leaf containing the reveal script
    let taproot_spend_info = TaprootBuilder::new()
        .add_leaf(0, reveal_script.clone())?
        .finalize(SECP256K1, public_key)
        .map_err(|_| anyhow!("Unable to create taproot spend info"))?;

    // Create reveal address
    let reveal_address = Address::p2tr(
        SECP256K1,
        public_key,
        taproot_spend_info.merkle_root(),
        ctx.network,
    );

    // Get control block
    let control_block = taproot_spend_info
        .control_block(&(reveal_script.clone(), LeafVersion::TapScript))
        .ok_or(anyhow!("Cannot create control block".to_string()))?;

    // Calculate commit value
    let commit_value = calculate_reveal_input_value(
        ctx.amount,
        ctx.fee_rate,
        ctx.operator_l1_addr.clone(),
        &reveal_script,
        &control_block,
    );

    // step 3: build the commit tx
    let unfunded_commit_tx = build_unfunded_commit_tx(reveal_address.clone(), commit_value)?;

    // Fund the commit tx. Additional utxos might be added to the output set
    let unsigned_commit_tx = fund_tx(ctx, unfunded_commit_tx)?;

    // step 4: build and sign the reveal tx
    let signed_reveal_tx = build_and_sign_reveal_tx(
        ctx.amount,
        ctx.operator_l1_addr.clone(),
        unsigned_commit_tx.clone(),
        &reveal_script,
        &control_block,
        &key_pair,
    )?;

    // step 5: sign the commit tx
    let signed_commit_tx = ctx
        .rpc_client
        .sign_raw_transaction_with_wallet(&unsigned_commit_tx, None, None)?
        .transaction()
        .unwrap();

    Ok((signed_commit_tx, signed_reveal_tx))
}

/// Encode tx in non-segwit format.
/// This is needed for fundrawtransaction RPC call, which expects a non-segwit tx
fn encode_tx_non_segwit(tx: Transaction) -> Vec<u8> {
    let mut encoder: Vec<u8> = Vec::new();
    tx.version.consensus_encode(&mut encoder).unwrap();
    tx.input.consensus_encode(&mut encoder).unwrap();
    tx.output.consensus_encode(&mut encoder).unwrap();
    tx.lock_time.consensus_encode(&mut encoder).unwrap();

    encoder
}

fn fund_tx(ctx: &BuilderContext, tx: Transaction) -> Result<Transaction, BatchSubmitterError> {
    let tx_raw = encode_tx_non_segwit(tx);
    let funded_tx = ctx
        .rpc_client
        .fund_raw_transaction(
            &tx_raw,
            Some(&json::FundRawTransactionOptions {
                fee_rate: ctx.fee_rate.fee_vb(1000), // convert to sat/kvB
                change_position: Some(1),
                ..Default::default()
            }),
            None,
        )?
        .transaction()
        .unwrap();

    Ok(funded_tx)
}

fn generate_key_pair() -> Result<UntweakedKeypair, anyhow::Error> {
    let mut rand_bytes = [0; 32];
    OsRng.fill_bytes(&mut rand_bytes);
    Ok(UntweakedKeypair::from_seckey_slice(SECP256K1, &rand_bytes)?)
}

fn build_reveal_script(
    taproot_public_key: &XOnlyPublicKey,
    payload: &[u8],
) -> Result<ScriptBuf, anyhow::Error> {
    let mut script_builder = script::Builder::new()
        .push_x_only_key(taproot_public_key)
        .push_opcode(bitcoin::opcodes::all::OP_CHECKSIG)
        .push_opcode(bitcoin::opcodes::all::OP_IF);

    const MAX_PUSH_SIZE: usize = 520;
    for chunk in payload.chunks(MAX_PUSH_SIZE) {
        script_builder = script_builder.push_slice(script::PushBytesBuf::try_from(chunk.to_vec())?);
    }
    script_builder = script_builder.push_opcode(bitcoin::opcodes::all::OP_ENDIF);

    Ok(script_builder.into_script())
}

// Estimate the required input value for reveal_tx
fn calculate_reveal_input_value(
    amount: Amount,
    fee_rate: FeeRate,
    recipient: Address,
    reveal_script: &script::ScriptBuf,
    control_block: &ControlBlock,
) -> Amount {
    let tx = Transaction {
        version: Version::TWO,
        input: vec![TxIn {
            previous_output: OutPoint {
                txid: Txid::from_str(
                    "0000000000000000000000000000000000000000000000000000000000000000",
                )
                .unwrap(),
                vout: 0,
            },
            script_sig: script::Builder::new().into_script(),
            sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
            witness: Witness::from_slice(&[
                Signature::from_slice(&[0; SCHNORR_SIGNATURE_SIZE])
                    .unwrap()
                    .as_ref()
                    .to_vec(),
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
    fee_rate.fee_vb(tx.vsize() as u64).unwrap() + amount
}

fn build_unfunded_commit_tx(
    recipient: Address,
    output_value: Amount,
) -> Result<Transaction, BatchSubmitterError> {
    // Build outputs
    let mut outputs: Vec<TxOut> = vec![];

    // The first output contains the taproot commitment
    outputs.push(TxOut {
        value: output_value,
        script_pubkey: recipient.script_pubkey(),
    });

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
    recipient: Address,
    unsigned_commit_tx: Transaction,
    reveal_script: &ScriptBuf,
    control_block: &ControlBlock,
    key_pair: &UntweakedKeypair,
) -> Result<Transaction, BatchSubmitterError> {
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
    let sighash = cache
        .taproot_script_spend_signature_hash(
            0,
            &Prevouts::All(&[unsigned_commit_tx.output[0].clone()]),
            TapLeafHash::from_script(reveal_script, LeafVersion::TapScript),
            bitcoin::sighash::TapSighashType::Default,
        )
        .unwrap();

    let signature = SECP256K1.sign_schnorr(
        &Message::from_digest_slice(sighash.as_byte_array()).unwrap(),
        key_pair,
    );

    // Set the witness field for the taproot input
    let witness = cache.witness_mut(0).unwrap();
    witness.push(signature.as_ref());
    witness.push(reveal_script);
    witness.push(control_block.serialize());

    Ok(cache.into_transaction())
}
