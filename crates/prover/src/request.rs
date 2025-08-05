use zkvm_interface::io::ProgramInput;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct ProverData {
    pub batch_number: u64,
    pub input: ProgramInput,
}

#[derive(Serialize, Deserialize)]
pub(crate) enum Request{
    Proof(ProverData)
}