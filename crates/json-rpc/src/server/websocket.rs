use crate::{api::eth_pubsub::EthPubSubApi, config::RpcConfig, error::RpcServerError};
use futures::stream::StreamExt;
use jsonrpsee::{
    Extensions, PendingSubscriptionSink, RpcModule, SubscriptionMessage,
    core::SubscriptionResult,
    server::{Server, ServerHandle},
    types::Params,
};
use mohave_chain_types::rpc::pubsub::{Params as SubscriptionParams, SubscriptionKind};
use std::{marker::PhantomData, sync::Arc};

pub struct WebsocketServer<T: EthPubSubApi> {
    _backend: PhantomData<T>,
}

impl<T: EthPubSubApi> WebsocketServer<T> {
    pub async fn init(config: &RpcConfig, backend: T) -> Result<ServerHandle, RpcServerError> {
        let mut rpc_module = RpcModule::new(backend);
        rpc_module.register_subscription(
            "eth_subscribe",
            "eth_subscription",
            "eth_unsubscribe",
            Self::subscribe,
        )?;

        let server = Server::builder()
            .build(&config.websocket_address)
            .await
            .map_err(RpcServerError::Build)?;

        Ok(server.start(rpc_module))
    }

    async fn subscribe(
        parameter: Params<'static>,
        pending: PendingSubscriptionSink,
        backend: Arc<T>,
        _extensions: Extensions,
    ) -> SubscriptionResult {
        let mut parameter = parameter.sequence();
        let kind = parameter.next::<SubscriptionKind>()?;
        let log_parameter = parameter.optional_next::<SubscriptionParams>()?;
        match kind {
            SubscriptionKind::NewHeads => Self::new_heads(pending, backend.clone()).await,
            SubscriptionKind::Logs => Self::logs(pending, backend.clone(), log_parameter).await,
            SubscriptionKind::NewPendingTransactions => {
                Self::new_pending_transactions(pending, backend.clone()).await
            }
            SubscriptionKind::Syncing => return Err(SubscriptionError::Unsupported.into()),
        }
    }

    /// Handler for [EthPubSubApi::subscribe_new_heads]
    async fn new_heads(pending: PendingSubscriptionSink, backend: Arc<T>) -> SubscriptionResult {
        let sink = pending.accept().await?;
        tokio::spawn(async move {
            let mut stream = backend.subscribe_new_heads().await;
            while let Some(header) = stream.next().await {
                let message = SubscriptionMessage::from_json(&header).unwrap();
                if sink.send(message).await.is_err() {
                    break;
                }
            }

            sink.closed().await;
        });
        Ok(())
    }

    /// Handler for [EthPubSubApi::subscribe_logs]
    async fn logs(
        pending: PendingSubscriptionSink,
        backend: Arc<T>,
        parameter: Option<SubscriptionParams>,
    ) -> SubscriptionResult {
        let sink = pending.accept().await?;

        let filter = if let Some(SubscriptionParams::Logs(filter)) = parameter {
            Some(filter)
        } else {
            None
        };

        tokio::spawn(async move {
            let mut stream = backend.subscribe_logs(filter).await;
            while let Some(logs) = stream.next().await {
                let message = SubscriptionMessage::from_json(&logs).unwrap();
                if sink.send(message).await.is_err() {
                    break;
                }
            }

            sink.closed().await;
        });
        Ok(())
    }

    /// Handler for [EthPubSubApi::subscribe_new_pending_transaction]
    async fn new_pending_transactions(
        pending: PendingSubscriptionSink,
        backend: Arc<T>,
    ) -> SubscriptionResult {
        let sink = pending.accept().await?;
        tokio::spawn(async move {
            let mut stream = backend.subscribe_new_pending_transaction().await;
            while let Some(new_pending_transaction) = stream.next().await {
                let message = SubscriptionMessage::from_json(&new_pending_transaction).unwrap();
                if sink.send(message).await.is_err() {
                    break;
                }
            }

            sink.closed().await;
        });
        Ok(())
    }
}

pub enum SubscriptionError {
    Unsupported,
    InvalidParameter,
}

impl std::fmt::Display for SubscriptionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported => write!(f, "Unsupported subscription kind"),
            Self::InvalidParameter => write!(f, "Invalid parameter"),
        }
    }
}
