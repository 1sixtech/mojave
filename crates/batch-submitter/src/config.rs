use bitcoin::Address;
use bitcoin::Network;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WriterConfig {
    pub operator_l1_addr: Address,
    pub network: Network,
    pub reveal_amount: u64,
}

impl WriterConfig {
    pub fn new(
        operator_l1_addr: Address,
        network: Network,
        reveal_amount: u64,
    ) -> Self {
        Self {
            operator_l1_addr,
            network,
            reveal_amount,
        }
    }
}
