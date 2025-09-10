use crate::rpc::context::RpcApiContext;
use mojave_client::types::SignedBlock;
use mojave_signature::types::Verifier;
use mojave_utils::{
    ordered_block::OrderedBlock,
    rpc::error::{Error, Result},
};

pub async fn ingest_signed_block(ctx: &RpcApiContext, signed: SignedBlock) -> Result<()> {
    signed
        .verifying_key
        .verify(&signed.block.header.hash(), &signed.signature)
        .map_err(|error| Error::Internal(error.to_string()))?;

    let block = signed.block;
    let number = block.header.number;
    ctx.pending_signed_blocks
        .push_signed(OrderedBlock(block))
        .await;
    tracing::info!("Received the block number: {}", number);
    Ok(())
}
