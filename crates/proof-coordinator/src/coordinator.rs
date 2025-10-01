use crate::{
    error::{Error, Result},
    types::{ProofCoordinatorOptions, Request, Response},
};
use mojave_client::{
    MojaveClient,
    types::{ProofResponse, ProofResult, ProverData, Strategy},
};
use mojave_node_lib::types::{MojaveNode, NodeOptions};

use ethrex_blockchain::Blockchain;
use ethrex_common::types::{BlobsBundle, Block};
use ethrex_storage::Store;
use ethrex_storage_rollup::StoreRollup;

use guest_program::input::ProgramInput;

use std::sync::Arc;

pub struct ProofCoordinator {
    client: MojaveClient,
    sequencer_address: String,
    rollup_store: StoreRollup,
    store: Store,
    blockchain: Arc<Blockchain>,
    elasticity_multiplier: u64,
}

impl ProofCoordinator {
    pub fn new(
        node: MojaveNode,
        node_options: &NodeOptions,
        options: &ProofCoordinatorOptions,
    ) -> Result<Self> {
        const DEFAULT_ELASTICITY: u64 = 2;

        let sequencer_address = format!(
            "http://{}:{}",
            node_options.http_addr, node_options.http_port
        );

        let prover_url = vec![options.prover_address.clone()];
        let client = MojaveClient::builder()
            .prover_urls(&prover_url)
            .build()
            .map_err(Error::Client)?;

        Ok(Self {
            client,
            sequencer_address,
            rollup_store: node.rollup_store,
            store: node.store,
            blockchain: node.blockchain,
            elasticity_multiplier: DEFAULT_ELASTICITY,
        })
    }

    async fn store_proof(&self, proof_response: ProofResponse, batch_number: u64) -> Result<()> {
        let batch_proof = match proof_response.result {
            ProofResult::Proof(proof) => proof,
            ProofResult::Error(err) => {
                return Err(Error::ProofFailed(batch_number, err.to_string()));
            }
        };

        let prover_type = batch_proof.prover_type();
        if self
            .rollup_store
            .get_proof_by_batch_and_type(batch_number, prover_type)
            .await?
            .is_some()
        {
            tracing::info!(
                ?batch_number,
                ?prover_type,
                "A proof was received for a batch and type that is already stored"
            );
        } else {
            self.rollup_store
                .store_proof_by_batch_and_type(batch_number, prover_type, batch_proof)
                .await?;
        }

        Ok(())
    }

    async fn create_prover_input(&self, batch_number: u64) -> Result<ProverData> {
        let Some(block_numbers) = self
            .rollup_store
            .get_block_numbers_by_batch(batch_number)
            .await?
        else {
            return Err(Error::ItemNotFoundInStore(format!(
                "Batch number {batch_number} not found in store"
            )));
        };

        let blocks = self.fetch_blocks(block_numbers).await?;

        let witness = self
            .blockchain
            .generate_witness_for_blocks(&blocks)
            .await
            .map_err(Error::from)?;

        let (blob_commitment, blob_proof) = {
            let blob = self
                .rollup_store
                .get_blobs_by_batch(batch_number)
                .await?
                .ok_or(Error::MissingBlob(batch_number))?;
            let BlobsBundle {
                mut commitments,
                mut proofs,
                ..
            } = BlobsBundle::create_from_blobs(&blob)?;
            match (commitments.pop(), proofs.pop()) {
                (Some(commitment), Some(proof)) => (commitment, proof),
                _ => return Err(Error::MissingBlob(batch_number)),
            }
        };

        tracing::debug!("Created prover input for batch {batch_number}");

        Ok(ProverData {
            batch_number,
            input: ProgramInput {
                execution_witness: witness,
                blocks,
                blob_commitment,
                blob_proof,
                elasticity_multiplier: self.elasticity_multiplier,
            },
        })
    }

    async fn fetch_blocks(&self, block_numbers: Vec<u64>) -> Result<Vec<Block>> {
        let mut blocks = vec![];
        // TODO: if we see a bottle neck here parallelize the workload here
        for block_number in block_numbers {
            let header = self
                .store
                .get_block_header(block_number)?
                .ok_or(Error::StorageDataIsNone)?;
            let body = self
                .store
                .get_block_body(block_number)
                .await?
                .ok_or(Error::StorageDataIsNone)?;
            blocks.push(Block::new(header, body));
        }
        Ok(blocks)
    }
}

impl mojave_task::Task for ProofCoordinator {
    type Request = Request;
    type Response = Response;
    type Error = Error;

    async fn handle_request(&mut self, request: Self::Request) -> Result<Self::Response> {
        match request {
            Request::ProcessBatch(batch_number) => {
                let input = match self.create_prover_input(batch_number).await {
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
                self.store_proof(proof, batch_number).await?;
                Ok(Response::Ack)
            }
        }
    }

    async fn on_shutdown(&mut self) -> Result<()> {
        tracing::info!("Shutting down proof coordinator");
        Ok(())
    }
}
