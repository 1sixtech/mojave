use bitcoin::Address;
use bitcoin::Network;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WriterConfig {
    pub inscription_fee_rate: u64,
    pub operator_l1_addr: Address,
    pub network: Network,
    pub reveal_amount: u64,
}

impl WriterConfig {
    pub fn new(
        inscription_fee_rate: u64,
        operator_l1_addr: Address,
        network: Network,
        reveal_amount: u64,
    ) -> Self {
        Self {
            inscription_fee_rate,
            operator_l1_addr: operator_l1_addr,
            network,
            reveal_amount,
        }
    }
}
