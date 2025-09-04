use crate::rpc::RpcApiContext;
use mojave_client::types::{ProofResult, SignedProofResponse};
use mojave_signature::types::Verifier;
use mojave_utils::rpc::error::{Error, Result};

pub async fn accept_signed_proof(ctx: &RpcApiContext, signed: SignedProofResponse) -> Result<()> {
    signed
        .verifying_key
        .verify(&signed.proof_response, &signed.signature)
        .map_err(|err| Error::Internal(format!("Invalid signature: {err}")))?;

    let batch_number = signed.proof_response.batch_number;
    let proof = match signed.proof_response.result {
        ProofResult::Proof(proof) => proof,
        ProofResult::Error(err) => {
            return Err(Error::Internal(format!(
                "Error while generate proof: {err}"
            )));
        }
    };

    let proof_type = proof.prover_type();
    ctx.rollup_store
        .store_proof_by_batch_and_type(batch_number, proof_type, proof)
        .await
        .map_err(|e| Error::Internal(format!("Failed to store proof: {e}")))?;

    Ok(())
}
