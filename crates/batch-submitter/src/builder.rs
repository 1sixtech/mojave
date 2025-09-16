use core::{result::Result::Ok, str::FromStr};

use anyhow::anyhow;
use bitcoin::{
    Address, Amount, Network, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid,
    Witness,
    absolute::LockTime,
    blockdata::script,
    consensus,
    hashes::Hash,
    key::UntweakedKeypair,
    secp256k1::{
        Message, SECP256K1, XOnlyPublicKey, constants::SCHNORR_SIGNATURE_SIZE, schnorr::Signature,
    },
    sighash::{Prevouts, SighashCache},
    taproot::{ControlBlock, LeafVersion, TapLeafHash, TaprootBuilder},
    transaction::Version,
};
use bitcoincore_rpc::{Client as BitcoinRPCClient, RpcApi};
use rand::{RngCore, rngs::OsRng};

use crate::BatchSubmitterError;

pub struct BuilderContext {
    rpc_client: BitcoinRPCClient,
    fee_rate: u64,
    operator_l1_addr: Address,
    network: Network,
    amount: u64,
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
    let commit_value = calculate_reveal_input_value(ctx, &reveal_script, &control_block);

    // step 3: build the commit tx
    let unfunded_commit_tx = build_unfunded_commit_tx(reveal_address.clone(), commit_value)?;

    // Fund the commit tx. Additional utxos might be added to the output set
    let unsigned_commit_tx: Transaction = consensus::encode::deserialize_hex(
        std::str::from_utf8(
            &ctx.rpc_client
                .fund_raw_transaction(&unfunded_commit_tx, None, None)?
                .hex,
        )
        .unwrap(),
    )
    .unwrap();

    // step 4: build and sign the reveal tx
    let signed_reveal_tx = build_and_sign_reveal_tx(
        ctx,
        unsigned_commit_tx.clone(),
        &reveal_script,
        &control_block,
        &key_pair,
    )?;

    // step 5: sign the commit tx
    let signed_commit_tx: Transaction = consensus::encode::deserialize_hex(
        std::str::from_utf8(
            &ctx.rpc_client
                .sign_raw_transaction_with_wallet(&unsigned_commit_tx, None, None)?
                .hex,
        )
        .unwrap(),
    )
    .unwrap();

    Ok((signed_commit_tx, signed_reveal_tx))
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
    ctx: &BuilderContext,
    reveal_script: &script::ScriptBuf,
    control_block: &ControlBlock,
) -> u64 {
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
            witness: Witness::from_slice(&[
                Signature::from_slice(&[0; SCHNORR_SIGNATURE_SIZE])
                    .unwrap()
                    .as_ref()
                    .to_vec(),
                reveal_script.to_bytes(),
                control_block.serialize(),
            ]),
            sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
        }],
        output: vec![TxOut {
            script_pubkey: ctx.operator_l1_addr.script_pubkey(),
            value: Amount::from_sat(ctx.amount),
        }],
        lock_time: LockTime::ZERO,
    };
    tx.vsize() as u64 * ctx.fee_rate + ctx.amount
}

fn build_unfunded_commit_tx(
    recipient: Address,
    output_value: u64,
) -> Result<Transaction, BatchSubmitterError> {
    // Build outputs
    let mut outputs: Vec<TxOut> = vec![];

    // The first output contains the taproot commitment
    outputs.push(TxOut {
        value: Amount::from_sat(output_value),
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
    ctx: &BuilderContext,
    unsigned_commit_tx: Transaction,
    reveal_script: &ScriptBuf,
    control_block: &ControlBlock,
    key_pair: &UntweakedKeypair,
) -> Result<Transaction, BatchSubmitterError> {
    let outputs: Vec<TxOut> = vec![TxOut {
        value: Amount::from_sat(ctx.amount),
        script_pubkey: ctx.operator_l1_addr.script_pubkey(),
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
