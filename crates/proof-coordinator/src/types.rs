use mojave_client::types::ProofResponse;

#[derive(Debug, Clone)]
pub struct ProofCoordinatorOptions {
    pub prover_address: String,
}
pub enum Request {
    ProcessBatch(u64),
    StoreProof(ProofResponse, u64),
}

#[derive(Debug)]
pub enum Response {
    Ack,
}
