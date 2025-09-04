use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct ProverOptions {
    pub prover_port: u16,
    pub prover_host: String,
    pub queue_capacity: usize,
    pub aligned_mode: bool,
    pub private_key: String,
}

impl Default for ProverOptions {
    fn default() -> Self {
        Self {
            prover_port: 3900,
            prover_host: "0.0.0.0".to_string(),
            queue_capacity: 100,
            aligned_mode: false,
            private_key: String::new(),
        }
    }
}
