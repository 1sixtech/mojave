use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::rpc::context::RpcApiContext;

pub(crate) fn spawn_block_processing_task(
    context: RpcApiContext,
    shutdown_token: CancellationToken,
) -> JoinHandle<()> {
    tokio::task::spawn(async move {
        tracing::info!("Starting block processing loop");
        loop {
            tokio::select! {
                block = context.block_queue.pop_wait() => {
                    let added_block = context.l1_context.blockchain.add_block(&block.0).await;
                    if let Err(added_block) = added_block {
                        tracing::error!(error= %added_block, "failed to add block to blockchain");
                        continue;
                    }

                    let update_block_number = context
                        .l1_context
                        .storage
                        .update_earliest_block_number(block.0.header.number)
                        .await;
                    if let Err(update_block_number) = update_block_number {
                        tracing::error!(error = %update_block_number, "failed to update earliest block number");
                    }

                    let forkchoice_context = context
                        .l1_context
                        .storage
                        .forkchoice_update(
                            None,
                            block.0.header.number,
                            block.0.header.hash(),
                            None,
                            None,
                        )
                        .await;
                    if let Err(forkchoice_context) = forkchoice_context {
                        tracing::error!(error = %forkchoice_context, "failed to update forkchoice");
                    }
                }
                _ = shutdown_token.cancelled() => {
                    tracing::info!("Shutting down block processing loop");
                    break;
                }
            }
        }
    })
}
