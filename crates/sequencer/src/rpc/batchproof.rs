use crate::rpc::RpcApiContext;
use ethrex_rpc::{RpcErr, utils::RpcRequest};
use mojave_client::types::SignedProofResponse;
use mojave_signature::Verifier;
use serde_json::Value;

pub struct SendBatchProofRequest {
    signed_proof: SignedProofResponse,
}

impl SendBatchProofRequest {
    fn get_proof_response(rpc_req_params: &Option<Vec<Value>>) -> Result<Self, RpcErr> {
        let params = rpc_req_params
            .as_ref()
            .ok_or(RpcErr::BadParams("No params provided".to_owned()))?;

        if params.len() != 1 {
            return Err(RpcErr::BadParams(format!(
                "Expected exactly 1 parameter (SignedProofResponse), but {} were provided",
                params.len()
            )));
        }

        let signed_proof = serde_json::from_value::<SignedProofResponse>(params[0].clone())
            .map_err(|e| RpcErr::BadParams(format!("Invalid SignedProofResponse: {e}")))?;

        Ok(Self { signed_proof })
    }

    pub async fn call(request: &RpcRequest, context: RpcApiContext) -> Result<Value, RpcErr> {
        let data = Self::get_proof_response(&request.params)?;

        data.signed_proof
            .verifying_key
            .verify(
                &data.signed_proof.proof_response,
                &data.signed_proof.signature,
            )
            .map_err(|err| RpcErr::Internal(format!("Invalid signature: {err}")))?;

        let batch_number = data.signed_proof.proof_response.batch_number;
        if let Some(err) = data.signed_proof.proof_response.error {
            return Err(RpcErr::Internal(format!(
                "Error while generate proof: {err}"
            )));
        }
        let proof = data
            .signed_proof
            .proof_response
            .batch_proof
            .ok_or(RpcErr::Internal(
                "Empty proof received from prover".to_string(),
            ))?;

        let proof_type = proof.prover_type();

        context
            .rollup_store
            .store_proof_by_batch_and_type(batch_number, proof_type, proof)
            .await
            .map_err(|e| RpcErr::Internal(format!("Failed to store proof: {e}")))?;

        serde_json::to_value("Proof accepted").map_err(|e| RpcErr::Internal(e.to_string()))
    }
}
