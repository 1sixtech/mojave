use core::{result::Result::Ok, str::FromStr};
use std::cmp::Reverse;

use anyhow::anyhow;
use bitcoin::{
    absolute::LockTime,
    blockdata::{opcodes::all::OP_CHECKSIG, script},
    hashes::Hash,
    key::{TapTweak, TweakedPublicKey, UntweakedKeypair},
    secp256k1::{
        constants::SCHNORR_SIGNATURE_SIZE, schnorr::Signature, Message, XOnlyPublicKey, SECP256K1,
    },
    sighash::{Prevouts, SighashCache},
    taproot::{
        ControlBlock, LeafVersion, TapLeafHash, TaprootBuilder, TaprootBuilderError,
        TaprootSpendInfo,
    },
    transaction::Version,
    Address, Amount, Network, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid,
    Witness,
};
use bitcoincore_rpc::bitcoincore_rpc_json::ListUnspent;
use rand::{rngs::OsRng, RngCore};

use crate::{config::WriterConfig, BatchSubmitterError};

const BITCOIN_DUST_LIMIT: u64 = 546;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionState {
    NA,
    Submitted { confirmations: u64 },
    Finalized,
}

use bitcoincore_rpc::Client as BitcoinRPCClient;

pub struct Writer {
    config: WriterConfig,
    bitcoin_rpc_client: BitcoinRPCClient,
}

impl Writer {
    pub fn new(config: WriterConfig, bitcoin_rpc_client: BitcoinRPCClient) -> Self {
        Self { config, bitcoin_rpc_client }
    }

    pub fn create_inscription_tx(
        &self,
        payload: Vec<u8>,
    ) -> Result<(Transaction, Transaction), BatchSubmitterError> {
        // Create commit key
        let key_pair = generate_key_pair()?;
        let public_key = XOnlyPublicKey::from_keypair(&key_pair).0;

        // Start creating envelope content
        let reveal_script = build_reveal_script(&public_key, &payload)?;
        // Create spend info for tapscript
        let taproot_spend_info = TaprootBuilder::new()
            .add_leaf(0, reveal_script.clone())?
            .finalize(SECP256K1, public_key)
            .map_err(|_| anyhow!("???????????"))?;  // FIXME

        // Create reveal address
        let reveal_address = Address::p2tr(
            SECP256K1,
            public_key,
            Some(taproot_spend_info.merkle_root()),
            self.config.network,
        );

        // Calculate commit value
        let commit_value = calculate_commit_output_value(
            &self.config.operator_l1_addr,
            self.config.reveal_amount,
            self.config.inscription_fee_rate,
            &reveal_script,
            &taproot_spend_info,
        );

        let utxos = self.bitcoin_rpc_client.list_unspent(None, None, None, None, None)?;

        // step 3: build the commit tx
        let (unsigned_commit_tx, _) = build_commit_tx(
            utxos,
            reveal_address.clone(),
            self.config.operator_l1_addr.clone(),
            commit_value,
            self.config.inscription_fee_rate,
        )?;

        let output_to_reveal = unsigned_commit_tx.output[0].clone();

        // step 4: build the reveal tx
        let mut reveal_tx = build_reveal_tx(
            unsigned_commit_tx.clone(),
            self.config.operator_l1_addr.clone(),
            self.config.reveal_amount,
            self.config.inscription_fee_rate,
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

        Ok((unsigned_commit_tx, reveal_tx))
    }
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
        .push_opcode(OP_CHECKSIG)
        .push_opcode(bitcoin::opcodes::OP_IF);

    const MAX_PUSH_SIZR: usize = 520;
    for chunk in payload.chunks(MAX_PUSH_SIZR) {
        script_builder = script_builder.push_slice(chunk);
    }
    script_builder = script_builder.push_opcode(bitcoin::opcodes::OP_ENDIF);

    Ok(script_builder.into_script())
}

fn get_size(
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

fn choose_utxos(
    utxos: &[ListUnspent],
    amount: u64,
) -> Result<(Vec<ListUnspent>, u64), BatchSubmitterError> {
    let mut bigger_utxos: Vec<&ListUnspent> = utxos
        .iter()
        .filter(|utxo| utxo.amount.to_sat() >= amount)
        .collect();
    let mut sum = 0;

    if !bigger_utxos.is_empty() {
        // sort vec by amount (small first)
        bigger_utxos.sort_by_key(|&x| x.amount);

        // single utxo will be enough
        // so return the transaction
        let utxo = bigger_utxos[0];
        sum += utxo.amount.to_sat();

        Ok((vec![utxo.clone()], sum))
    } else {
        let mut smaller_utxos: Vec<&ListUnspent> = utxos
            .iter()
            .filter(|utxo| utxo.amount.to_sat() < amount)
            .collect();

        // sort vec by amount (large first)
        smaller_utxos.sort_by_key(|x| Reverse(&x.amount));

        let mut chosen_utxos: Vec<ListUnspent> = vec![];

        for utxo in smaller_utxos {
            sum += utxo.amount.to_sat();
            chosen_utxos.push(utxo.clone());

            if sum >= amount {
                break;
            }
        }

        if sum < amount {
            return Err(BatchSubmitterError::WalletError(format!(
                "insufficient funds for tx (need {} sats, have {} sats)",
                amount, sum
            )));
        }

        Ok((chosen_utxos, sum))
    }
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

fn calculate_commit_output_value(
    recipient: &Address,
    reveal_value: u64,
    fee_rate: u64,
    reveal_script: &script::ScriptBuf,
    taproot_spend_info: &TaprootSpendInfo,
) -> u64 {
    get_size(
        &default_txin(),
        &[TxOut {
            script_pubkey: recipient.script_pubkey(),
            value: Amount::from_sat(reveal_value),
        }],
        Some(reveal_script),
        Some(
            &taproot_spend_info
                .control_block(&(reveal_script.clone(), LeafVersion::TapScript))
                .expect("Cannot create control block"),
        ),
    ) as u64
        * fee_rate
        + reveal_value
}

fn build_commit_tx(
    utxos: Vec<ListUnspent>,
    recipient: Address,
    change_address: Address,
    output_value: u64,
    fee_rate: u64,
) -> Result<(Transaction, Vec<ListUnspent>), BatchSubmitterError> {
    // get single input single output transaction size
    let mut size = get_size(
        &default_txin(),
        &[TxOut {
            script_pubkey: recipient.script_pubkey(),
            value: Amount::from_sat(output_value),
        }],
        None,
        None,
    );
    let mut last_size = size;

    let utxos: Vec<ListUnspent> = utxos
        .iter()
        .filter(|utxo| utxo.spendable && utxo.solvable && utxo.amount.to_sat() > BITCOIN_DUST_LIMIT)
        .cloned()
        .collect();

    let (commit_txn, consumed_utxo) = loop {
        let fee = (last_size as u64) * fee_rate;

        let input_total = output_value + fee;

        let res = choose_utxos(&utxos, input_total)?;

        let (chosen_utxos, sum) = res;

        let mut outputs: Vec<TxOut> = vec![];
        outputs.push(TxOut {
            value: Amount::from_sat(output_value),
            script_pubkey: recipient.script_pubkey(),
        });

        let mut direct_return = false;
        if let Some(excess) = sum.checked_sub(input_total) {
            if excess >= BITCOIN_DUST_LIMIT {
                outputs.push(TxOut {
                    value: Amount::from_sat(excess),
                    script_pubkey: change_address.script_pubkey(),
                });
            } else {
                // if dust is left, leave it for fee
                direct_return = true;
            }
        }

        let inputs: Vec<TxIn> = chosen_utxos
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

        size = get_size(&inputs, &outputs, None, None);

        if size == last_size || direct_return {
            let commit_txn = Transaction {
                lock_time: LockTime::ZERO,
                version: Version(2),
                input: inputs,
                output: outputs,
            };

            break (commit_txn, chosen_utxos);
        }

        last_size = size;
    };

    Ok((commit_txn, consumed_utxo))
}

fn build_reveal_tx(
    input_transaction: Transaction,
    recipient: Address,
    output_value: u64,
    fee_rate: u64,
    reveal_script: &ScriptBuf,
    control_block: &ControlBlock,
) -> Result<Transaction, BatchSubmitterError> {
    let outputs: Vec<TxOut> = vec![TxOut {
        value: Amount::from_sat(output_value),
        script_pubkey: recipient.script_pubkey(),
    }];

    let v_out_for_reveal = 0u32;
    let input_utxo = input_transaction.output[v_out_for_reveal as usize].clone();
    let txn_id = input_transaction.compute_txid();

    let inputs = vec![TxIn {
        previous_output: OutPoint {
            txid: txn_id,
            vout: v_out_for_reveal,
        },
        script_sig: script::Builder::new().into_script(),
        witness: Witness::new(),
        sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
    }];
    let size = get_size(&inputs, &outputs, Some(reveal_script), Some(control_block));
    let fee = (size as u64) * fee_rate;
    let input_required = Amount::from_sat(output_value + fee);
    if input_utxo.value < Amount::from_sat(BITCOIN_DUST_LIMIT) || input_utxo.value < input_required
    {
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

fn sign_reveal_tx(
    reveal_tx: &mut Transaction,
    output_to_reveal: &TxOut,
    reveal_script: &script::ScriptBuf,
    taproot_spend_info: &TaprootSpendInfo,
    key_pair: &UntweakedKeypair,
) -> Result<(), anyhow::Error> {
    let mut sighash_cache = SighashCache::new(reveal_tx);
    let signature_hash = sighash_cache.taproot_script_spend_signature_hash(
        0,
        &Prevouts::All(&[output_to_reveal]),
        TapLeafHash::from_script(reveal_script, LeafVersion::TapScript),
        bitcoin::sighash::TapSighashType::Default,
    )?;

    let mut randbytes = [0; 32];
    OsRng.fill_bytes(&mut randbytes);

    let signature = SECP256K1.sign_schnorr_with_aux_rand(
        &Message::from_digest_slice(signature_hash.as_byte_array())?,
        key_pair,
        &randbytes,
    );

    let witness = sighash_cache.witness_mut(0).unwrap();
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
