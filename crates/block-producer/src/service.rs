use crate::{
    BlockProducerContext, BlockProducerError, rpc::start_api, types::BlockProducerOptions,
};
use ethrex_common::types::Block;
use mojave_client::{MojaveClient, types::Strategy};
use mojave_node_lib::{
    node::get_client_version,
    types::{MojaveNode, NodeOptions},
    utils::{
        NodeConfigFile, get_authrpc_socket_addr, get_http_socket_addr, read_jwtsecret_file,
        store_node_config_file,
    },
};
use std::{path::PathBuf, time::Duration};
use tokio::sync::{
    mpsc::{self, error::TrySendError},
    oneshot,
};
use tokio_stream::{StreamExt, wrappers::ReceiverStream};
use tracing::error;

pub async fn run(
    node: MojaveNode,
    node_options: &NodeOptions,
    block_producer_options: &BlockProducerOptions,
) -> Result<(), Box<dyn std::error::Error>> {
    let mojave_client = MojaveClient::builder()
        .private_key(block_producer_options.private_key.clone())
        .full_node_urls(&block_producer_options.full_node_addresses)
        .prover_urls(std::slice::from_ref(&block_producer_options.prover_address))
        .build()
        .unwrap_or_else(|error| {
            tracing::error!("Failed to build the client: {}", error);
            std::process::exit(1);
        });

    let context = BlockProducerContext::new(
        node.store.clone(),
        node.blockchain.clone(),
        node.rollup_store.clone(),
        node.genesis.coinbase,
    );
    let block_time = block_producer_options.block_time;
    let block_producer = BlockProducer::start(context, 100);
    tokio::spawn(async move {
        loop {
            match block_producer.build_block().await {
                Ok(block) => mojave_client
                    .request()
                    .strategy(Strategy::Race)
                    .send_broadcast_block(&block)
                    .await
                    .unwrap_or_else(|error| tracing::error!("{}", error)),
                Err(error) => {
                    tracing::error!("Failed to build a block: {}", error);
                }
            }
            tokio::time::sleep(Duration::from_millis(block_time)).await;
        }
    });

    start_api(
        get_http_socket_addr(&node_options.http_addr, &node_options.http_port),
        get_authrpc_socket_addr(&node_options.authrpc_addr, &node_options.authrpc_port),
        node.store,
        node.blockchain,
        read_jwtsecret_file(&node_options.authrpc_jwtsecret)?,
        node.local_p2p_node,
        node.local_node_record.lock().await.clone(),
        node.syncer,
        node.peer_handler,
        get_client_version(),
        node.rollup_store,
    )
    .await?;
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Shutting down the full node..");
            let node_config_path = PathBuf::from(node.data_dir.clone()).join("node_config.json");
            tracing::info!("Storing config at {:?}...", node_config_path);
            node.cancel_token.cancel();
            let node_config = NodeConfigFile::new(node.peer_table.clone(), node.local_node_record.lock().await.clone()).await;
            store_node_config_file(node_config, node_config_path).await;
            tokio::time::sleep(Duration::from_secs(1)).await;
            tracing::info!("Successfully shut down the full node.");
        }
    }

    Ok(())
}

#[derive(Clone)]
pub struct BlockProducer {
    sender: mpsc::Sender<Message>,
}

impl BlockProducer {
    pub fn start(context: BlockProducerContext, channel_capacity: usize) -> Self {
        let (sender, receiver) = mpsc::channel(channel_capacity);
        let mut receiver = ReceiverStream::new(receiver);

        tokio::spawn(async move {
            while let Some(message) = receiver.next().await {
                handle_message(&context, message).await;
            }

            error!("Block builder stopped because the sender dropped.");
        });
        Self { sender }
    }

    pub async fn build_block(&self) -> Result<Block, BlockProducerError> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .try_send(Message::BuildBlock(sender))
            .map_err(|error| match error {
                TrySendError::Full(_) => BlockProducerError::Full,
                TrySendError::Closed(_) => BlockProducerError::Stopped,
            })?;
        receiver.await?
    }
}

async fn handle_message(context: &BlockProducerContext, message: Message) {
    match message {
        Message::BuildBlock(sender) => {
            let _ = sender.send(context.build_block().await);
        }
    }
}

#[allow(clippy::large_enum_variant)]
enum Message {
    BuildBlock(oneshot::Sender<Result<Block, BlockProducerError>>),
}
