use ethrex_blockchain::error::{ChainError, MempoolError};
use ethrex_rpc::RpcErrorMetadata;
use ethrex_storage::error::StoreError;
use ethrex_vm::EvmError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum RpcErr {
    #[error(transparent)]
    EthrexRPC(ethrex_rpc::RpcErr),
    #[error("EthClient error: {0}")]
    EthClientError(#[from] ethrex_rpc::clients::EthClientError),
    #[error("Custom error: {0}")]
    CustomError(String),
    #[error("Blockchain error: {0}")]
    BlockchainError(#[from] ChainError),
}

impl From<RpcErr> for RpcErrorMetadata {
    fn from(value: RpcErr) -> Self {
        match value {
            RpcErr::EthrexRPC(err) => err.into(),
            RpcErr::CustomError(err) => RpcErrorMetadata {
                code: -38000,
                data: None,
                message: err,
            },
            RpcErr::BlockchainError(err) => RpcErrorMetadata {
                code: -38001,
                data: None,
                message: err.to_string(),
            },
            RpcErr::EthClientError(err) => RpcErrorMetadata {
                code: -38002,
                data: None,
                message: err.to_string(),
            },
        }
    }
}

impl From<serde_json::Error> for RpcErr {
    fn from(error: serde_json::Error) -> Self {
        Self::EthrexRPC(ethrex_rpc::RpcErr::BadParams(error.to_string()))
    }
}

// TODO: Actually return different errors for each case
// here we are returning a BadParams error
impl From<MempoolError> for RpcErr {
    fn from(err: MempoolError) -> Self {
        match err {
            MempoolError::StoreError(err) => {
                Self::EthrexRPC(ethrex_rpc::RpcErr::Internal(err.to_string()))
            }
            other_err => Self::EthrexRPC(ethrex_rpc::RpcErr::BadParams(other_err.to_string())),
        }
    }
}

impl From<secp256k1::Error> for RpcErr {
    fn from(err: secp256k1::Error) -> Self {
        Self::EthrexRPC(ethrex_rpc::RpcErr::Internal(format!(
            "Cryptography error: {err}"
        )))
    }
}

#[derive(Debug)]
pub enum RpcNamespace {
    Engine,
    Eth,
    Admin,
    Debug,
    Web3,
    Net,
    Mempool,
    Mojave,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum RpcRequestId {
    Number(u64),
    String(String),
}

impl From<RpcRequestId> for ethrex_rpc::utils::RpcRequestId {
    fn from(id: RpcRequestId) -> Self {
        match id {
            RpcRequestId::Number(num) => ethrex_rpc::utils::RpcRequestId::Number(num),
            RpcRequestId::String(str) => ethrex_rpc::utils::RpcRequestId::String(str),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RpcRequest {
    pub id: RpcRequestId,
    pub jsonrpc: String,
    pub method: String,
    pub params: Option<Vec<Value>>,
}

impl RpcRequest {
    pub fn namespace(&self) -> Result<RpcNamespace, RpcErr> {
        let mut parts = self.method.split('_');
        let Some(namespace) = parts.next() else {
            return Err(RpcErr::EthrexRPC(ethrex_rpc::RpcErr::MethodNotFound(
                self.method.clone(),
            )));
        };
        resolve_namespace(namespace, self.method.clone())
    }
}

impl From<RpcRequest> for ethrex_rpc::utils::RpcRequest {
    fn from(req: RpcRequest) -> Self {
        ethrex_rpc::utils::RpcRequest {
            id: req.id.into(),
            jsonrpc: req.jsonrpc,
            method: req.method,
            params: req.params,
        }
    }
}

impl From<&RpcRequest> for ethrex_rpc::utils::RpcRequest {
    fn from(req: &RpcRequest) -> Self {
        ethrex_rpc::utils::RpcRequest {
            id: req.id.clone().into(),
            jsonrpc: req.jsonrpc.clone(),
            method: req.method.clone(),
            params: req.params.clone(),
        }
    }
}

pub fn resolve_namespace(maybe_namespace: &str, method: String) -> Result<RpcNamespace, RpcErr> {
    match maybe_namespace {
        "engine" => Ok(RpcNamespace::Engine),
        "eth" => Ok(RpcNamespace::Eth),
        "mojave" => Ok(RpcNamespace::Mojave),
        "admin" => Ok(RpcNamespace::Admin),
        "debug" => Ok(RpcNamespace::Debug),
        "web3" => Ok(RpcNamespace::Web3),
        "net" => Ok(RpcNamespace::Net),
        // TODO: The namespace is set to match geth's namespace for compatibility, consider changing it in the future
        "txpool" => Ok(RpcNamespace::Mempool),
        _ => Err(RpcErr::EthrexRPC(ethrex_rpc::RpcErr::MethodNotFound(
            method,
        ))),
    }
}

impl Default for RpcRequest {
    fn default() -> Self {
        RpcRequest {
            id: RpcRequestId::Number(1),
            jsonrpc: "2.0".to_string(),
            method: "".to_string(),
            params: None,
        }
    }
}

pub fn rpc_response<E>(id: RpcRequestId, res: Result<Value, E>) -> Result<Value, RpcErr>
where
    E: Into<RpcErrorMetadata>,
{
    Ok(match res {
        Ok(result) => serde_json::to_value(RpcSuccessResponse {
            id,
            jsonrpc: "2.0".to_string(),
            result,
        }),
        Err(error) => serde_json::to_value(RpcErrorResponse {
            id,
            jsonrpc: "2.0".to_string(),
            error: error.into(),
        }),
    }?)
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RpcSuccessResponse {
    pub id: RpcRequestId,
    pub jsonrpc: String,
    pub result: Value,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RpcErrorResponse {
    pub id: RpcRequestId,
    pub jsonrpc: String,
    pub error: RpcErrorMetadata,
}

/// Failure to read from DB will always constitute an internal error
impl From<StoreError> for RpcErr {
    fn from(value: StoreError) -> Self {
        RpcErr::EthrexRPC(ethrex_rpc::RpcErr::Internal(value.to_string()))
    }
}

impl From<EvmError> for RpcErr {
    fn from(value: EvmError) -> Self {
        RpcErr::EthrexRPC(ethrex_rpc::RpcErr::Vm(value.to_string()))
    }
}

pub fn parse_json_hex(hex: &serde_json::Value) -> Result<u64, String> {
    if let Value::String(maybe_hex) = hex {
        let trimmed = maybe_hex.trim_start_matches("0x");
        let maybe_parsed = u64::from_str_radix(trimmed, 16);
        maybe_parsed.map_err(|_| format!("Could not parse given hex {maybe_hex}"))
    } else {
        Err(format!("Could not parse given hex {hex}"))
    }
}

#[cfg(test)]
pub mod test_utils {
    use std::{net::SocketAddr, str::FromStr, sync::Arc};

    use ethrex_blockchain::Blockchain;
    use ethrex_common::H512;
    use ethrex_p2p::{
        peer_handler::PeerHandler,
        sync_manager::SyncManager,
        types::{Node, NodeRecord},
    };
    use ethrex_rpc::EthClient;
    use ethrex_storage::{EngineType, Store};
    use ethrex_storage_rollup::{EngineTypeRollup, StoreRollup};
    use k256::ecdsa::SigningKey;
    use secp256k1::rand;
    use tokio::sync::oneshot::Receiver;

    use crate::rpc::{
        clients::mojave::Client, full_node::start_api as start_api_full_node,
        sequencer::start_api as start_api_sequencer,
    };

    pub const TEST_GENESIS: &str = include_str!("../../../../test_data/genesis.json");
    pub const TEST_SEQUENCER_ADDR: &str = "127.0.0.1:8502";
    pub const TEST_NODE_ADDR: &str = "127.0.0.1:8500";

    pub fn example_p2p_node() -> Node {
        let public_key_1 = H512::from_str("d860a01f9722d78051619d1e2351aba3f43f943f6f00718d1b9baa4101932a1f5011f16bb2b1bb35db20d6fe28fa0bf09636d26a87d31de9ec6203eeedb1f666").unwrap();
        Node::new("127.0.0.1".parse().unwrap(), 30303, 30303, public_key_1)
    }

    pub fn example_local_node_record() -> NodeRecord {
        let public_key_1 = H512::from_str("d860a01f9722d78051619d1e2351aba3f43f943f6f00718d1b9baa4101932a1f5011f16bb2b1bb35db20d6fe28fa0bf09636d26a87d31de9ec6203eeedb1f666").unwrap();
        let node = Node::new("127.0.0.1".parse().unwrap(), 30303, 30303, public_key_1);
        let signer = SigningKey::random(&mut rand::rngs::OsRng);

        NodeRecord::from_node(&node, 1, &signer).unwrap()
    }

    pub async fn example_rollup_store() -> StoreRollup {
        let rollup_store = StoreRollup::new(".", EngineTypeRollup::InMemory)
            .expect("Failed to create StoreRollup");
        rollup_store
            .init()
            .await
            .expect("Failed to init rollup store");
        rollup_store
    }

    pub async fn start_test_api_full_node(
        sequencer_addr: Option<SocketAddr>,
        http_addr: Option<SocketAddr>,
        authrpc_addr: Option<SocketAddr>,
    ) -> (Client, Receiver<()>) {
        let http_addr = http_addr.unwrap_or(TEST_NODE_ADDR.parse().unwrap());
        let authrpc_addr = authrpc_addr.unwrap_or("127.0.0.1:8501".parse().unwrap());
        let storage =
            Store::new("", EngineType::InMemory).expect("Failed to create in-memory storage");
        storage
            .add_initial_state(serde_json::from_str(TEST_GENESIS).unwrap())
            .await
            .expect("Failed to build test genesis");
        let blockchain = Arc::new(Blockchain::default_with_store(storage.clone()));
        let jwt_secret = Default::default();
        let local_p2p_node = example_p2p_node();
        let rollup_store = example_rollup_store().await;
        let sequencer_addr = match sequencer_addr {
            Some(addr) => addr,
            None => TEST_SEQUENCER_ADDR.parse().unwrap(),
        };
        let url = format!("http://{sequencer_addr}");
        let client = Client::new(vec![&url]).unwrap();
        let eth_client = EthClient::new(&url).unwrap();

        let rpc_api = start_api_full_node(
            http_addr,
            authrpc_addr,
            storage,
            blockchain,
            jwt_secret,
            local_p2p_node,
            example_local_node_record(),
            SyncManager::dummy(),
            PeerHandler::dummy(),
            "ethrex/test".to_string(),
            rollup_store,
            client.clone(),
            eth_client,
        );
        let (full_node_tx, full_node_rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            full_node_tx.send(()).unwrap();
            rpc_api.await.unwrap()
        });

        (client, full_node_rx)
    }

    pub async fn start_test_api_sequencer(
        node_urls: Option<Vec<SocketAddr>>,
        http_addr: Option<SocketAddr>,
        authrpc_addr: Option<SocketAddr>,
    ) -> (Client, Receiver<()>) {
        let http_addr = http_addr.unwrap_or_else(|| TEST_SEQUENCER_ADDR.parse().unwrap());
        let authrpc_addr = authrpc_addr.unwrap_or_else(|| "127.0.0.1:8503".parse().unwrap());
        let storage =
            Store::new("", EngineType::InMemory).expect("Failed to create in-memory storage");
        storage
            .add_initial_state(serde_json::from_str(TEST_GENESIS).unwrap())
            .await
            .expect("Failed to build test genesis");
        let blockchain = Arc::new(Blockchain::default_with_store(storage.clone()));
        let jwt_secret = Default::default();
        let local_p2p_node = example_p2p_node();
        let rollup_store = example_rollup_store().await;
        let default_node_url = format!("http://{TEST_NODE_ADDR}");
        let node_urls: Vec<String> = match node_urls {
            Some(addrs) => addrs.iter().map(|addr| format!("http://{addr}")).collect(),
            None => vec![default_node_url.to_string()],
        };

        let node_urls: Vec<&str> = node_urls.iter().map(|s| s.as_str()).collect();
        let client = Client::new(node_urls).unwrap();

        let rpc_api = start_api_sequencer(
            http_addr,
            authrpc_addr,
            storage,
            blockchain,
            jwt_secret,
            local_p2p_node,
            example_local_node_record(),
            SyncManager::dummy(),
            PeerHandler::dummy(),
            "ethrex/test".to_string(),
            rollup_store,
            client.clone(),
        );

        let (sequencer_tx, sequencer_rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            sequencer_tx.send(()).unwrap();
            rpc_api.await.unwrap()
        });

        (client, sequencer_rx)
    }
}
