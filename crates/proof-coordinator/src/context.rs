use crate::error::{Error, Result};
use ethrex_blockchain::Blockchain;
use ethrex_common::types::{BlobsBundle, Block};
use ethrex_storage::Store;
use ethrex_storage_rollup::StoreRollup;
use guest_program::input::ProgramInput;
use mojave_client::types::{ProofResponse, ProofResult, ProverData};
use std::sync::Arc;

pub struct ProofCoordinatorContext {
    pub(crate) rollup_store: StoreRollup,
    pub(crate) store: Store,
    pub(crate) blockchain: Arc<Blockchain>,
    pub(crate) elasticity_multiplier: u64,
}

impl ProofCoordinatorContext {
    pub fn new(
        rollup_store: StoreRollup,
        store: Store,
        blockchain: Arc<Blockchain>,
        elasticity_multiplier: u64,
    ) -> Self {
        Self {
            rollup_store,
            store,
            blockchain,
            elasticity_multiplier,
        }
    }
    pub(crate) async fn store_proof(
        &self,
        proof_response: ProofResponse,
        batch_number: u64,
    ) -> Result<()> {
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

    pub async fn create_prover_input(&self, batch_number: u64) -> Result<ProverData> {
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
                db: witness,
                blocks,
                blob_commitment,
                blob_proof,
                elasticity_multiplier: self.elasticity_multiplier,
            },
        })
    }

    async fn fetch_blocks(&self, block_numbers: Vec<u64>) -> Result<Vec<Block>> {
        let mut blocks = vec![];
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
