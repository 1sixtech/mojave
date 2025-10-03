use std::collections::{HashMap, hash_map::Entry};

use ethrex_common::{
    Address, H256,
    types::{AccountUpdate, PrivilegedL2Transaction},
};
use ethrex_l2_common::l1_messages::{L1Message, get_l1_message_hash};

#[derive(Default)]
pub(crate) struct BatchAccumulator {
    pub(crate) messages: Vec<L1Message>,
    pub(crate) privileged_txs: Vec<PrivilegedL2Transaction>,
    pub(crate) account_updates: HashMap<Address, AccountUpdate>,
    pub(crate) message_hashes: Vec<H256>,
    pub(crate) privileged_tx_hashes: Vec<H256>,
}

impl BatchAccumulator {
    pub(crate) fn add_block_data(
        &mut self,
        messages: Vec<L1Message>,
        privileged_txs: Vec<PrivilegedL2Transaction>,
        account_updates: Vec<AccountUpdate>,
    ) {
        self.message_hashes
            .extend(messages.iter().map(get_l1_message_hash));
        self.messages.extend(messages);

        self.privileged_tx_hashes.extend(
            privileged_txs
                .iter()
                .filter_map(|tx| tx.get_privileged_hash()),
        );
        self.privileged_txs.extend(privileged_txs);

        for update in account_updates {
            match self.account_updates.entry(update.address) {
                Entry::Occupied(mut e) => e.get_mut().merge(update),
                Entry::Vacant(v) => {
                    v.insert(update);
                }
            };
        }
    }

    pub(crate) fn get_account_updates_vec(&self) -> Vec<AccountUpdate> {
        self.account_updates.values().cloned().collect()
    }
}
