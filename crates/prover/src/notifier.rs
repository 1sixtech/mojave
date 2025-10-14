use mojave_client::types::ProofResponse;
use tokio::sync::mpsc;

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Proof notifier error: {0}")]
    ProofNotifierError(#[from] tokio::sync::mpsc::error::TrySendError<ProofResponse>),
    #[error("Internal Error: {0}")]
    Internal(String),
}

pub struct Notifier {
    tx: mpsc::Sender<ProofResponse>,
}

impl Notifier {
    pub fn new(tx: mpsc::Sender<ProofResponse>) -> Self {
        Notifier { tx }
    }
}

pub trait ProofEvents: Send + Sync {
    fn on_proof_generated(&self, proof: ProofResponse) -> Result<()>;
}

impl ProofEvents for Notifier {
    fn on_proof_generated(&self, proof: ProofResponse) -> Result<()> {
        self.tx.try_send(proof)?;
        Ok(())
    }
}
