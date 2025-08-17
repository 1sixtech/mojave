use ethrex_common::types::Block;
use mojave_signature::{Signature, VerifyingKey};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

// need to check whether we will use Message and contain other data or not
#[derive(Serialize, Deserialize)]
pub struct SignedBlock {
    pub block: Block,
    pub signature: Signature,
    pub verifying_key: VerifyingKey,
}

pub struct ParsedUrlsContext {
    pub urls: Arc<Mutex<Vec<Url>>>,
}
