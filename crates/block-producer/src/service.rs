use crate::{
    BlockProducerContext,
    error::{Error, Result},
    rpc::start_api,
    types::BlockProducerOptions,
};
use ethrex_common::types::Block;
use mojave_node_lib::{
    node::get_client_version,
    types::{MojaveNode, NodeConfigFile, NodeOptions},
    utils::{
        get_authrpc_socket_addr, get_http_socket_addr, read_jwtsecret_file, store_node_config_file,
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
) -> Result<()> {
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
            if let Err(error) = block_producer.build_block().await {
                tracing::error!("Failed to build a block: {}", error);
            }
            tokio::time::sleep(Duration::from_millis(block_time)).await;
        }
    });

    let local_node_record = node.local_node_record.lock().await.clone();

    let api_task = start_api(
        get_http_socket_addr(&node_options.http_addr, &node_options.http_port).await?,
        get_authrpc_socket_addr(&node_options.authrpc_addr, &node_options.authrpc_port).await?,
        node.store,
        node.blockchain,
        read_jwtsecret_file(&node_options.authrpc_jwtsecret).await?,
        node.local_p2p_node,
        local_node_record,
        node.syncer,
        node.peer_handler,
        get_client_version(),
        node.rollup_store,
    );
    tokio::select! {
        res = api_task => {
            if let Err(error) = res {
                tracing::error!("API task returned error: {}", error);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Shutting down the block producer..");
            let node_config_path = PathBuf::from(node.data_dir.clone()).join("node_config.json");
            tracing::info!("Storing config at {:?}...", node_config_path);
            node.cancel_token.cancel();
            let node_config = NodeConfigFile::new(node.peer_table.clone(), node.local_node_record.lock().await.clone()).await;
            store_node_config_file(node_config, node_config_path).await;
            tokio::time::sleep(Duration::from_secs(1)).await;
            tracing::info!("Successfully shut down the block producer.");
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

    pub async fn build_block(&self) -> Result<Block> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .try_send(Message::BuildBlock(sender))
            .map_err(|error| match error {
                TrySendError::Full(_) => Error::Full,
                TrySendError::Closed(_) => Error::Stopped,
            })?;
        receiver.await?
    }
}

async fn handle_message(context: &BlockProducerContext, message: Message) {
    match message {
        Message::BuildBlock(sender) => {
            if let Err(e) = sender.send(context.build_block().await) {
                tracing::warn!(error = ?e, "Failed to send built block over channel");
            }
        }
    }
}

#[allow(clippy::large_enum_variant)]
enum Message {
    BuildBlock(oneshot::Sender<Result<Block>>),
}
