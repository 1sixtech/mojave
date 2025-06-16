use crate::backend::Backend;
use futures::stream::Stream;
use mohave_chain_rpc::api::eth_pubsub::EthPubSubApi;
use mohave_chain_types::{
    network::AnyHeader,
    primitives::B256,
    rpc::{Filter, Header, Log},
};

impl EthPubSubApi for Backend {
    async fn subscribe_new_heads(&self) -> impl Stream<Item = Header<AnyHeader>> + Send + Unpin {
        self.pubsub_service().subscribe_new_heads()
    }

    async fn subscribe_logs(
        &self,
        filter: Option<Box<Filter>>,
    ) -> impl Stream<Item = Log> + Send + Unpin {
        self.pubsub_service().subscribe_logs(filter)
    }

    async fn subscribe_new_pending_transaction(&self) -> impl Stream<Item = B256> + Send + Unpin {
        self.pubsub_service().subscribe_new_pending_transaction()
    }
}
