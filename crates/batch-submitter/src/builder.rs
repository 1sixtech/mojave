use core::{result::Result::Ok, str::FromStr};
use std::cmp::Reverse;

use anyhow::anyhow;
use bitcoin::{
    Address, Amount, Network, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid,
    Witness,
    absolute::LockTime,
    blockdata::script,
    hashes::Hash,
    key::UntweakedKeypair,
    secp256k1::{
        Message, SECP256K1, XOnlyPublicKey, constants::SCHNORR_SIGNATURE_SIZE, schnorr::Signature,
    },
    sighash::{Prevouts, SighashCache},
    taproot::{ControlBlock, LeafVersion, TapLeafHash, TaprootBuilder, TaprootSpendInfo},
    transaction::Version,
};
use bitcoincore_rpc::{Client as BitcoinRPCClient, RpcApi, json::ListUnspentResultEntry};
use rand::{RngCore, rngs::OsRng};

use crate::BatchSubmitterError;

const BITCOIN_DUST_LIMIT: u64 = 546;

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

    // Calculate commit value
    let commit_value = calculate_reveal_input_value(ctx, &reveal_script, &taproot_spend_info);

    let utxos = ctx.rpc_client.list_unspent(None, None, None, None, None)?;

    // step 3: build the commit tx
    let (unsigned_commit_tx, _) =
        build_commit_tx(ctx, utxos, reveal_address.clone(), commit_value)?;

    let output_to_reveal = unsigned_commit_tx.output[0].clone();

    // step 4: build the reveal tx
    let mut reveal_tx = build_reveal_tx(
        ctx,
        unsigned_commit_tx.clone(),
        &reveal_script,
        &taproot_spend_info
            .control_block(&(reveal_script.clone(), LeafVersion::TapScript))
            .ok_or(anyhow!("Cannot create control block".to_string()))?,
    )?;

    // step 4: sign the reveal tx
    sign_reveal_tx(
        &mut reveal_tx,
        &output_to_reveal,
        &reveal_script,
        &taproot_spend_info,
        &key_pair,
    )?;

    // step 5: sign the commit tx

    Ok((unsigned_commit_tx, reveal_tx))
}

pub fn generate_key_pair() -> Result<UntweakedKeypair, anyhow::Error> {
    let mut rand_bytes = [0; 32];
    OsRng.fill_bytes(&mut rand_bytes);
    Ok(UntweakedKeypair::from_seckey_slice(SECP256K1, &rand_bytes)?)
}

pub fn build_reveal_script(
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

/// Build a transaction based on the arguments and return its vsize.
fn get_tx_vsize(
    inputs: &[TxIn],
    outputs: &[TxOut],
    script: Option<&ScriptBuf>,
    control_block: Option<&ControlBlock>,
) -> usize {
    let mut tx = Transaction {
        input: inputs.to_vec(),
        output: outputs.to_vec(),
        lock_time: LockTime::ZERO,
        version: Version(2),
    };

    for i in 0..tx.input.len() {
        tx.input[i].witness.push(
            Signature::from_slice(&[0; SCHNORR_SIGNATURE_SIZE])
                .unwrap()
                .as_ref(),
        );
    }

    match (script, control_block) {
        (Some(sc), Some(cb)) if tx.input.len() == 1 => {
            tx.input[0].witness.push(sc);
            tx.input[0].witness.push(cb.serialize());
        }
        _ => {}
    }

    tx.vsize()
}

fn coinselect(
    utxos: &[ListUnspentResultEntry],
    amount: u64,
) -> Result<(Vec<ListUnspentResultEntry>, u64), BatchSubmitterError> {
    // ideally fund_raw_transaction should be used
    // sort from large to small, use a simple coin selection
    let mut sorted_utxos: Vec<&ListUnspentResultEntry> = utxos.iter().collect();
    sorted_utxos.sort_by_key(|&x| Reverse(&x.amount));

    let mut selected_utxos: Vec<ListUnspentResultEntry> = vec![];

    let mut sum = 0;

    for utxo in sorted_utxos {
        sum += utxo.amount.to_sat();
        selected_utxos.push(utxo.clone());

        if sum >= amount {
            break;
        }
    }

    if sum < amount {
        return Err(BatchSubmitterError::WalletError(format!(
            "insufficient funds (need {} sats, have {} sats)",
            amount, sum
        )));
    }

    Ok((selected_utxos, sum))
}

fn default_txin() -> Vec<TxIn> {
    vec![TxIn {
        previous_output: OutPoint {
            txid: Txid::from_str(
                "0000000000000000000000000000000000000000000000000000000000000000",
            )
            .unwrap(),
            vout: 0,
        },
        script_sig: script::Builder::new().into_script(),
        witness: Witness::new(),
        sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
    }]
}

// Compute the required input value for reveal_tx
pub fn calculate_reveal_input_value(
    ctx: &BuilderContext,
    reveal_script: &script::ScriptBuf,
    taproot_spend_info: &TaprootSpendInfo,
) -> u64 {
    get_tx_vsize(
        &default_txin(),
        &[TxOut {
            script_pubkey: ctx.operator_l1_addr.script_pubkey(),
            value: Amount::from_sat(ctx.amount),
        }],
        Some(reveal_script),
        Some(
            &taproot_spend_info
                .control_block(&(reveal_script.clone(), LeafVersion::TapScript))
                .expect("Cannot create control block"),
        ),
    ) as u64
        * ctx.fee_rate
        + ctx.amount
}

/// Build `commit_tx`
/// - `utxos`: the input utxo set
/// - `recipient`: the address to receive the output
/// - `output_value`: the value to send to the recipient
pub fn build_commit_tx(
    ctx: &BuilderContext,
    utxos: Vec<ListUnspentResultEntry>,
    recipient: Address,
    output_value: u64,
) -> Result<(Transaction, Vec<ListUnspentResultEntry>), BatchSubmitterError> {
    // get single input single output transaction size
    let mut size = get_tx_vsize(
        &default_txin(),
        &[TxOut {
            script_pubkey: recipient.script_pubkey(),
            value: Amount::from_sat(output_value),
        }],
        None,
        None,
    );
    let mut last_size = size;

    let utxos: Vec<ListUnspentResultEntry> = utxos
        .iter()
        .filter(|utxo| utxo.spendable && utxo.solvable && utxo.amount.to_sat() > BITCOIN_DUST_LIMIT)
        .cloned()
        .collect();

    // Repeatedly enlarge the size (fee) until a tx can be built
    let (commit_txn, consumed_utxo) = loop {
        let fee = (last_size as u64) * ctx.fee_rate;

        let input_total = output_value + fee;

        let (selected_utxos, sum) = coinselect(&utxos, input_total)?;

        // build outputs
        let mut outputs: Vec<TxOut> = vec![];
        outputs.push(TxOut {
            value: Amount::from_sat(output_value),
            script_pubkey: recipient.script_pubkey(),
        });

        // add change output if needed
        if let Some(excess) = sum.checked_sub(input_total)
            && excess >= BITCOIN_DUST_LIMIT
        {
            outputs.push(TxOut {
                value: Amount::from_sat(excess),
                script_pubkey: ctx.operator_l1_addr.script_pubkey(),
            });
        }

        // build inputs
        let inputs: Vec<TxIn> = selected_utxos
            .iter()
            .map(|u| TxIn {
                previous_output: OutPoint {
                    txid: u.txid,
                    vout: u.vout,
                },
                script_sig: script::Builder::new().into_script(),
                witness: Witness::new(),
                sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
            })
            .collect();

        size = get_tx_vsize(&inputs, &outputs, None, None);

        if size <= last_size {
            // we overestimated the fee, the tx can be built
            let commit_txn = Transaction {
                lock_time: LockTime::ZERO,
                version: Version(2),
                input: inputs,
                output: outputs,
            };

            break (commit_txn, selected_utxos);
        }

        last_size = size;
    };

    Ok((commit_txn, consumed_utxo))
}

/// Build `reveal_tx`
pub fn build_reveal_tx(
    ctx: &BuilderContext,
    unsigned_commit_tx: Transaction,
    reveal_script: &ScriptBuf,
    control_block: &ControlBlock,
) -> Result<Transaction, BatchSubmitterError> {
    let outputs: Vec<TxOut> = vec![TxOut {
        value: Amount::from_sat(ctx.amount),
        script_pubkey: ctx.operator_l1_addr.script_pubkey(),
    }];

    let input_utxo = unsigned_commit_tx.output[0].clone();
    if input_utxo.value < Amount::from_sat(BITCOIN_DUST_LIMIT) {
        return Err(BatchSubmitterError::WalletError(format!(
            "input utxo value {} is below dust limit",
            input_utxo.value.to_sat()
        )));
    }

    let commit_txid = unsigned_commit_tx.compute_txid();

    let inputs = vec![TxIn {
        previous_output: OutPoint {
            txid: commit_txid,
            vout: 0,
        },
        script_sig: script::Builder::new().into_script(),
        witness: Witness::new(),
        sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
    }];

    let size = get_tx_vsize(&inputs, &outputs, Some(reveal_script), Some(control_block));
    let fee = (size as u64) * ctx.fee_rate;
    let input_required = Amount::from_sat(ctx.amount + fee);
    if input_utxo.value < input_required {
        return Err(BatchSubmitterError::WalletError(format!(
            "insufficient funds for tx (need {} sats, have {} sats)",
            input_required.to_sat(),
            input_utxo.value.to_sat(),
        )));
    }
    let tx = Transaction {
        lock_time: LockTime::ZERO,
        version: Version(2),
        input: inputs,
        output: outputs,
    };

    Ok(tx)
}

/// Sign `reveal_tx`
pub fn sign_reveal_tx(
    reveal_tx: &mut Transaction,
    output_to_reveal: &TxOut,
    reveal_script: &script::ScriptBuf,
    taproot_spend_info: &TaprootSpendInfo,
    key_pair: &UntweakedKeypair,
) -> Result<(), anyhow::Error> {
    let mut cache = SighashCache::new(reveal_tx);
    let signature_hash = cache.taproot_script_spend_signature_hash(
        0,
        &Prevouts::All(&[output_to_reveal]),
        TapLeafHash::from_script(reveal_script, LeafVersion::TapScript),
        bitcoin::sighash::TapSighashType::Default,
    )?;

    let signature = SECP256K1.sign_schnorr(
        &Message::from_digest_slice(signature_hash.as_byte_array())?,
        key_pair
    );

    let witness = cache.witness_mut(0).unwrap();
    witness.push(signature.as_ref());
    witness.push(reveal_script);
    witness.push(
        taproot_spend_info
            .control_block(&(reveal_script.clone(), LeafVersion::TapScript))
            .ok_or(anyhow!("Could not create control block"))?
            .serialize(),
    );

    Ok(())
}
