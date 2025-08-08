use serde::{Deserialize, Serialize};
use zkvm_interface::io::ProgramInput;

#[derive(Deserialize, Serialize)]
pub struct ProverData {
    pub batch_number: u64,
    pub input: ProgramInput,
}
