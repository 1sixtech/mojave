use crate::rpc::{RpcApiContext};
use ethrex_l2_common::prover::BatchProof;
use ethrex_rpc::{
    RpcErr,
    utils::RpcRequest,
};
use serde_json::Value;

pub struct SendBatchProofRequest(BatchProof);

impl SendBatchProofRequest {
    fn get_batchproof(rpc_req_params: &Option<Vec<Value>>) -> Result<Self, RpcErr> {
        let params = rpc_req_params
            .as_ref()
            .ok_or(RpcErr::BadParams("No params provided".to_owned()))?;

        if params.len() != 1 {
            return Err(RpcErr::BadParams(format!(
                "Expected one param and {} were provided",
                params.len()
            )));
        }

        // Deserialize JSON → BatchProof
        let proof = serde_json::from_value::<BatchProof>(params[0].clone())
            .map_err(|e| RpcErr::BadParams(format!("Invalid BatchProof: {}", e)))?;

        Ok(Self(proof))
    }

    pub async fn call(request: &RpcRequest, context: RpcApiContext) -> Result<Value, RpcErr> {
        let data = Self::get_batchproof(&request.params)?;

        // TODO: Sequencer logic
        // e.g., context.proof_store.store(data.0).await?;

        // data.0.proof();
        
        // For now, return success
        serde_json::to_value("Proof accepted")
            .map_err(|e| RpcErr::Internal(e.to_string()))
    }
}