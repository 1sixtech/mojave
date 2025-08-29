#[derive(Debug, Clone)]
pub struct BlockProducerOptions {
    pub prover_address: String,
    pub block_time: u64,
    pub private_key: String,
}
