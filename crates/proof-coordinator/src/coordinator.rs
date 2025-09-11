use crate::{
    context::ProofCoordinatorContext,
    error::{Error, Result},
};
use mojave_client::{
    MojaveClient,
    types::{ProofResponse, Strategy},
};
use tokio::sync::mpsc::Receiver;

pub struct ProofCoordinator {
    pub(crate) client: MojaveClient,
    /// Comes from the block builder
    pub(crate) proof_data_receiver: Receiver<u64>,
}

impl ProofCoordinator {
    pub fn new(proof_data_receiver: Receiver<u64>, prover_address: &str) -> Result<Self> {
        let prover_url = vec![prover_address.to_string()];
        let client = MojaveClient::builder()
            .prover_urls(&prover_url)
            .build()
            .map_err(Error::Client)?;

        Ok(Self {
            client,
            proof_data_receiver,
        })
    }

    pub async fn process_new_block(&mut self, context: ProofCoordinatorContext) -> Result<()> {
        let batch_number = match self.proof_data_receiver.recv().await {
            Some(batch_number) => batch_number,
            None => return Ok(()),
        };

        let input = match context.create_prover_input(batch_number).await {
            Ok(input) => input,
            Err(e) => return Err(e),
        };

        let _job_id = self
            .client
            .request()
            .strategy(Strategy::Sequential)
            .send_proof_input(&input)
            .await
            .map_err(|e| Error::Custom(e.to_string()))?;

        Ok(())
    }

    pub async fn store_proof(
        &self,
        context: &ProofCoordinatorContext,
        proof_response: ProofResponse,
        batch_number: u64,
    ) -> Result<()> {
        context.store_proof(proof_response, batch_number).await
    }
}
