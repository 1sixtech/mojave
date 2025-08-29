#[derive(Debug, Clone)]
pub struct BlockProducerOptions {
    pub full_node_addresses: Vec<String>,
    pub prover_address: String,
    pub block_time: u64,
    pub private_key: String,
}
