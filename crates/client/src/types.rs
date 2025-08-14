use ethrex_common::types::Block;
use mojave_signature::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};

// need to check whether we will use Message and contain other data or not
#[derive(Serialize, Deserialize)]
pub struct SignedBlock {
    pub block: Block,
    pub signature: Signature,
    pub verifying_key: VerifyingKey,
}
