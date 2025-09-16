use crate::{
    context::ProofCoordinatorContext,
    error::{Error, Result},
    types::ProofCoordinatorOptions,
};
use mojave_client::{
    MojaveClient,
    types::{ProofResponse, Strategy},
};
use mojave_node_lib::types::{MojaveNode, NodeOptions};
use tokio::sync::mpsc::Receiver;

pub async fn run(
    node: MojaveNode,
    node_options: &NodeOptions,
    options: &ProofCoordinatorOptions,
    batch_receiver: Receiver<u64>,
) -> Result<()> {
    const DEFAULT_ELASTICITY: u64 = 2;
    let sequencer_address = format!(
        "http://{}:{}",
        node_options.http_addr, node_options.http_port
    );
    let mut coordinator = ProofCoordinator::new(
        batch_receiver,
        options.prover_address.clone(),
        sequencer_address,
    )?;
    let context = ProofCoordinatorContext::new(
        node.rollup_store,
        node.store,
        node.blockchain.clone(),
        DEFAULT_ELASTICITY,
    );
    let cancel_token = node.cancel_token.clone();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    tracing::info!("Coordinator shutdown");
                    return;
                },
                ret = coordinator.process_new_block(&context) => {
                    if let Err(err) = ret {
                        tracing::error!("Error processing new block: {}", err);
                    }
                }
            }
        }
    });

    Ok(())
}

pub struct ProofCoordinator {
    pub(crate) client: MojaveClient,
    pub(crate) batch_receiver: Receiver<u64>,
    pub(crate) sequencer_address: String,
}

impl ProofCoordinator {
    pub fn new(
        batch_receiver: Receiver<u64>,
        prover_address: String,
        sequencer_address: String,
    ) -> Result<Self> {
        let prover_url = vec![prover_address];
        let client = MojaveClient::builder()
            .prover_urls(&prover_url)
            .build()
            .map_err(Error::Client)?;

        Ok(Self {
            client,
            batch_receiver,
            sequencer_address,
        })
    }

    pub async fn process_new_block(&mut self, context: &ProofCoordinatorContext) -> Result<()> {
        let batch_number = match self.batch_receiver.recv().await {
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
            .with_strategy(Strategy::Sequential)
            .send_proof_input(&input, &self.sequencer_address)
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
