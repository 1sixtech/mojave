use ethrex_l2_common::prover::BatchProof;
use ethrex_prover_lib::{prove, to_batch_proof};
use tokio::sync::mpsc::{Receiver, Sender};
use zkvm_interface::io::ProgramInput;

#[allow(unused)]
const QUEUE_SIZE: usize = 100;

pub struct ProverData {
    pub batch_number: u64,
    pub input: ProgramInput,
}

#[allow(unused)]
pub struct Prover {
    aligned_mode: bool,
    new_input_receiver: Receiver<ProverData>,
    proof_sender: Sender<(BatchProof, u64)>,
}

impl Prover {
    /// Creates a new instance of the Prover.
    ///
    /// ```rust,ignore
    /// use mojave_prover::Prover;
    ///
    /// let (mut prover, _, _) = Prover::new(true);
    /// tokio::spawn(async move {
    ///     prover.start().await;
    /// });
    /// ```
    #[allow(unused)]
    pub fn new(aligned_mode: bool) -> (Self, Sender<ProverData>, Receiver<(BatchProof, u64)>) {
        let (new_input_sender, new_input_receiver) = tokio::sync::mpsc::channel(QUEUE_SIZE);
        let (proof_sender, proof_receiver) = tokio::sync::mpsc::channel(QUEUE_SIZE);

        (
            Prover {
                aligned_mode,
                new_input_receiver,
                proof_sender,
            },
            new_input_sender,
            proof_receiver,
        )
    }

    #[allow(unused)]
    pub async fn start(&mut self) {
        loop {
            if let Some(data) = self.new_input_receiver.recv().await {
                let Ok(batch_proof) = prove(data.input, self.aligned_mode)
                    .and_then(|proof| to_batch_proof(proof, self.aligned_mode))
                    .inspect_err(|e| tracing::error!("error generating proof {e}"))
                else {
                    continue;
                };

                if let Err(e) = self
                    .proof_sender
                    .send((batch_proof, data.batch_number))
                    .await
                {
                    tracing::error!("error sending proof {e}");
                }
            } else {
                tracing::error!("Stopping the prover because sender dropped. This is a bug.");
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::{sync::oneshot, time::timeout};

    #[tokio::test(flavor = "current_thread")]
    async fn start_exits_when_sender_dropped() {
        let (mut prover, sender, _receiver) = Prover::new(true);
        let (started_tx, started_rx) = oneshot::channel();
        let handle = tokio::spawn(async move {
            started_tx.send(()).ok();
            prover.start().await;
        });
        started_rx.await.expect("prover task did not start");
        drop(sender);
        timeout(Duration::from_millis(100), handle)
            .await
            .expect("Prover::start did not exit")
            .unwrap();
    }
}
