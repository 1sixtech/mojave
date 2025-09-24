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
use mojave_task::Task;
use tokio::sync::mpsc::Receiver;

pub enum Request {
    ProcessBatch(u64),
    StoreProof(ProofResponse, u64),
}

#[derive(Debug)]
pub enum Response {
    Ack,
}

pub async fn run(
    node: MojaveNode,
    node_options: &NodeOptions,
    options: &ProofCoordinatorOptions,
    mut batch_receiver: Receiver<u64>,
) -> Result<()> {
    const DEFAULT_ELASTICITY: u64 = 2;
    let sequencer_address = format!(
        "http://{}:{}",
        node_options.http_addr, node_options.http_port
    );
    let context = ProofCoordinatorContext::new(
        node.rollup_store,
        node.store,
        node.blockchain.clone(),
        DEFAULT_ELASTICITY,
    );
    let coordinator =
        ProofCoordinator::new(options.prover_address.clone(), sequencer_address, context)?;
    let handle = coordinator.spawn();

    let cancel_token = node.cancel_token.clone();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                batch = batch_receiver.recv() => {
                    match batch {
                        Some(batch_number) => {
                           if let Err(err) = handle.request(Request::ProcessBatch(batch_number)).await{
                            tracing::error!("Error processing batch {batch_number}: {err}")
                           }
                        },
                        None => {
                            tracing::info!("Batch channel closed; coordinator forwarder exiting");
                            return;
                        }
                    }
                }
                _ = cancel_token.cancelled() => {
                    let _ = handle.shutdown().await;
                    return;
                }
            }
        }
    });

    Ok(())
}

pub struct ProofCoordinator {
    pub(crate) client: MojaveClient,
    pub(crate) sequencer_address: String,
    pub(crate) context: ProofCoordinatorContext,
}

impl ProofCoordinator {
    pub fn new(
        prover_address: String,
        sequencer_address: String,
        context: ProofCoordinatorContext,
    ) -> Result<Self> {
        let prover_url = vec![prover_address];
        let client = MojaveClient::builder()
            .prover_urls(&prover_url)
            .build()
            .map_err(Error::Client)?;

        Ok(Self {
            client,
            sequencer_address,
            context,
        })
    }
}

impl mojave_task::Task for ProofCoordinator {
    type Request = Request;
    type Response = Response;
    type Error = Error;

    async fn handle_request(&self, request: Self::Request) -> Result<Self::Response> {
        match request {
            Request::ProcessBatch(batch_number) => {
                let input = match self.context.create_prover_input(batch_number).await {
                    Ok(input) => input,
                    Err(e) => return Err(e),
                };

                self.client
                    .request()
                    .with_strategy(Strategy::Sequential)
                    .send_proof_input(&input, &self.sequencer_address)
                    .await
                    .map_err(|e| Error::Custom(e.to_string()))?;

                Ok(Response::Ack)
            }
            Request::StoreProof(proof, batch_number) => {
                self.context.store_proof(proof, batch_number).await?;
                Ok(Response::Ack)
            }
        }
    }

    async fn on_shutdown(&self) -> Result<()> {
        tracing::info!("Shutting down proof coordinator");
        Ok(())
    }
}
