use anyhow::anyhow;
use bitcoin::{
    Address, Network, Transaction,
    secp256k1::{SECP256K1, XOnlyPublicKey},
    taproot::{LeafVersion, TaprootBuilder},
};
use bitcoincore_rpc::RpcApi;

use crate::{
    BatchSubmitterError,
    writer::{
        build_commit_tx, build_reveal_script, build_reveal_tx, calculate_commit_output_value,
        generate_key_pair, sign_reveal_tx,
    },
};

use bitcoincore_rpc::Client as BitcoinRPCClient;

pub struct WriterContext {
    rpc_client: BitcoinRPCClient,
    fee_rate: u64,
    operator_l1_addr: Address,
    network: Network,
    amount: u64,
}

impl WriterContext {
    pub fn create_inscription_tx(
        &self,
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
            self.network,
        );

        // Calculate commit value
        let commit_value = calculate_commit_output_value(
            &self.operator_l1_addr,
            self.amount,
            self.fee_rate,
            &reveal_script,
            &taproot_spend_info,
        );

        let utxos = self.rpc_client.list_unspent(None, None, None, None, None)?;

        // step 3: build the commit tx
        let (unsigned_commit_tx, _) = build_commit_tx(
            utxos,
            reveal_address.clone(),
            self.operator_l1_addr.clone(),
            commit_value,
            self.fee_rate,
        )?;

        let output_to_reveal = unsigned_commit_tx.output[0].clone();

        // step 4: build the reveal tx
        let mut reveal_tx = build_reveal_tx(
            unsigned_commit_tx.clone(),
            self.operator_l1_addr.clone(),
            self.amount,
            self.fee_rate,
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
