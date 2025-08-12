use std::sync::Arc;

use ethrex_blockchain::Blockchain;
use ethrex_common::types::{BlobsBundle, Block};
use ethrex_l2_common::prover::BatchProof;
use ethrex_storage::Store;
use ethrex_storage_rollup::StoreRollup;
use tokio::sync::mpsc::Receiver;
use zkvm_interface::io::ProgramInput;

use crate::errors::ProofCoordinatorError;

use mojave_prover::{ProverClient, ProverData};

mod errors;

pub struct ProofCoordinator {
    /// Come from the block builder
    proof_data_receiver: Receiver<u64>,
    /// Send to the prover
    prover_tcp_addr: String,
}

impl ProofCoordinator {
    pub fn new(proof_data_receiver: Receiver<u64>, prover_tcp_addr: String) -> Self {
        Self {
            proof_data_receiver,
            prover_tcp_addr,
        }
    }

    pub async fn process_new_block(
        &mut self,
        context: ProofCoordinatorContext,
    ) -> Result<(), ProofCoordinatorError> {
        let batch_number = match self.proof_data_receiver.recv().await {
            Some(batch_number) => batch_number,
            None => return Ok(()),
        };

        let input = match context.create_prover_input(batch_number).await {
            Ok(input) => input,
            Err(e) => return Err(e),
        };

        let (batch_number, batch_proof) = self.request_proof_from_prover(input).await?;

        context.store_proof(batch_proof, batch_number).await?;

        Ok(())
    }

    async fn request_proof_from_prover(
        &self,
        prover_data: ProverData,
    ) -> Result<(u64, BatchProof), ProofCoordinatorError> {
        let mut client = ProverClient::new(&self.prover_tcp_addr, 300);
        let batch_number = prover_data.batch_number;
        let proof = client
            .get_proof(prover_data)
            .await
            .map_err(|e| ProofCoordinatorError::Custom(e.to_string()))?;
        Ok((batch_number, proof))
    }
}

pub struct ProofCoordinatorContext {
    rollup_store: StoreRollup,
    store: Store,
    blockchain: Arc<Blockchain>,
    elasticity_multiplier: u64,
}

impl ProofCoordinatorContext {
    async fn store_proof(
        &self,
        batch_proof: BatchProof,
        batch_number: u64,
    ) -> Result<(), ProofCoordinatorError> {
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
            // If not, store it
            self.rollup_store
                .store_proof_by_batch_and_type(batch_number, prover_type, batch_proof)
                .await?;
        }

        Ok(())
    }

    pub async fn create_prover_input(
        &self,
        batch_number: u64,
    ) -> Result<ProverData, ProofCoordinatorError> {
        let Some(block_numbers) = self
            .rollup_store
            .get_block_numbers_by_batch(batch_number)
            .await?
        else {
            return Err(ProofCoordinatorError::ItemNotFoundInStore(format!(
                "Batch number {batch_number} not found in store"
            )));
        };

        let blocks = self.fetch_blocks(block_numbers).await?;

        let witness = self
            .blockchain
            .generate_witness_for_blocks(&blocks)
            .await
            .map_err(ProofCoordinatorError::from)?;

        // Get blobs bundle cached by the L1 Committer (blob, commitment, proof)
        let (blob_commitment, blob_proof) = {
            let blob = self
                .rollup_store
                .get_blobs_by_batch(batch_number)
                .await?
                .ok_or(ProofCoordinatorError::MissingBlob(batch_number))?;
            let BlobsBundle {
                mut commitments,
                mut proofs,
                ..
            } = BlobsBundle::create_from_blobs(&blob)?;
            match (commitments.pop(), proofs.pop()) {
                (Some(commitment), Some(proof)) => (commitment, proof),
                _ => return Err(ProofCoordinatorError::MissingBlob(batch_number)),
            }
        };

        tracing::debug!("Created prover input for batch {batch_number}");

        Ok(ProverData {
            batch_number,
            input: ProgramInput {
                db: witness,
                blocks,
                blob_commitment,
                blob_proof,
                elasticity_multiplier: self.elasticity_multiplier,
            },
        })
    }

    async fn fetch_blocks(
        &self,
        block_numbers: Vec<u64>,
    ) -> Result<Vec<Block>, ProofCoordinatorError> {
        let mut blocks = vec![];
        for block_number in block_numbers {
            let header = self
                .store
                .get_block_header(block_number)?
                .ok_or(ProofCoordinatorError::StorageDataIsNone)?;
            let body = self
                .store
                .get_block_body(block_number)
                .await?
                .ok_or(ProofCoordinatorError::StorageDataIsNone)?;
            blocks.push(Block::new(header, body));
        }
        Ok(blocks)
    }
}
