use crate::rpc::{RpcApiContext};
use ethrex_l2_common::prover::BatchProof;
use ethrex_rpc::{
    RpcErr,
    utils::RpcRequest,
};
use serde_json::Value;

pub struct SendBatchProofRequest {
    batch_number: u64,
    proof: BatchProof,
}

impl SendBatchProofRequest {
    fn get_batchproof(rpc_req_params: &Option<Vec<Value>>) -> Result<Self, RpcErr> {
        let params = rpc_req_params
            .as_ref()
            .ok_or(RpcErr::BadParams("No params provided".to_owned()))?;

        if params.len() != 2 {
            return Err(RpcErr::BadParams(format!(
                "Expected two params (batch_number, proof) but {} were provided",
                params.len()
            )));
        }

        let batch_number = serde_json::from_value::<u64>(params[0].clone())
            .map_err(|e| RpcErr::BadParams(format!("Invalid batch_number: {}", e)))?;

        let proof = serde_json::from_value::<BatchProof>(params[1].clone())
            .map_err(|e| RpcErr::BadParams(format!("Invalid BatchProof: {}", e)))?;

        Ok(Self {
            batch_number,
            proof,
        })
    }

    pub async fn call(request: &RpcRequest, context: RpcApiContext) -> Result<Value, RpcErr> {
        let data = Self::get_batchproof(&request.params)?;

        let proof_type = data.proof.prover_type();

        context
            .rollup_store
            .store_proof_by_batch_and_type(data.batch_number, proof_type, data.proof.clone())
            .await
            .map_err(|e| RpcErr::Internal(format!("Failed to store proof: {}", e)))?;
        
        // For now, return success
        serde_json::to_value("Proof accepted")
            .map_err(|e| RpcErr::Internal(e.to_string()))
    }
}