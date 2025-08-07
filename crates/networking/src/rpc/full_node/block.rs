use crate::rpc::{
    RpcHandler, SignedBlock,
    full_node::{RpcApiContextFullNode, types::ordered_block::OrderedBlock},
    utils::RpcErr,
};

use ethrex_common::types::{Block, BlockBody, Transaction};
use ethrex_rpc::{clients::eth::BlockByNumber, types::block::RpcBlock};

use mojave_signature::{Signature, Verifier, VerifyingKey};
use serde_json::Value;

pub struct BroadcastBlockRequest {
    block: Block,
    signature: Signature,
    verifying_key: VerifyingKey,
}

impl RpcHandler<RpcApiContextFullNode> for BroadcastBlockRequest {
    fn parse(params: &Option<Vec<Value>>) -> Result<Self, RpcErr> {
        let signed_block = get_block_data(params)?;
        Ok(Self {
            block: signed_block.block,
            signature: signed_block.signature,
            verifying_key: signed_block.verifying_key,
        })
    }

    async fn handle(&self, context: RpcApiContextFullNode) -> Result<Value, RpcErr> {
        // Check if the signature and sender are valid. If verification fails, return an error
        // immediately without processing the block.
        self.verifying_key
            .verify(&self.block.header.hash(), &self.signature)?;

        let latest_block_number = context.l1_context.storage.get_latest_block_number().await? + 1;
        for block_number in latest_block_number..self.block.header.number {
            let block = context
                .eth_client
                .get_block_by_number(BlockByNumber::Number(block_number))
                .await?;
            let block = rpc_block_to_block(block);

            context.block_queue.push(OrderedBlock(block)).await;
        }

        context
            .block_queue
            .push(OrderedBlock(self.block.clone()))
            .await;
        tracing::info!("Received the block number: {}", self.block.header.number);
        Ok(Value::Null)
    }
}

fn rpc_block_to_block(rpc_block: RpcBlock) -> Block {
    match rpc_block.body {
        ethrex_rpc::types::block::BlockBodyWrapper::Full(full_block_body) => {
            // transform RPCBlock to normal block
            let transactions: Vec<Transaction> = full_block_body
                .transactions
                .iter()
                .map(|b| b.tx.clone())
                .collect();

            Block::new(
                rpc_block.header,
                BlockBody {
                    ommers: full_block_body.uncles,
                    transactions,
                    withdrawals: Some(full_block_body.withdrawals),
                },
            )
        }
        ethrex_rpc::types::block::BlockBodyWrapper::OnlyHashes(..) => {
            unreachable!()
        }
    }
}

fn get_block_data(req: &Option<Vec<Value>>) -> Result<SignedBlock, RpcErr> {
    let params = req
        .as_ref()
        .ok_or(RpcErr::EthrexRPC(ethrex_rpc::RpcErr::BadParams(
            "No params provided".to_owned(),
        )))?;

    if params.len() != 1 {
        return Err(RpcErr::EthrexRPC(ethrex_rpc::RpcErr::BadParams(format!(
            "Expected exactly 1 parameter (SignedBlock), but {} were provided",
            params.len()
        ))));
    }

    let signed_block_param =
        params
            .first()
            .ok_or(RpcErr::EthrexRPC(ethrex_rpc::RpcErr::BadParams(
                "Missing SignedBlock parameter".to_owned(),
            )))?;

    let signed_block = serde_json::from_value::<SignedBlock>(signed_block_param.clone())?;

    Ok(signed_block)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ctor::ctor;
    use ethrex_common::{
        Address, Bloom, Bytes, H256, U256,
        types::{Block, BlockBody, BlockHeader},
    };
    use mojave_signature::{Signer, SigningKey};

    use serde_json::json;

    use crate::rpc::{
        clients::mojave::Client as MojaveClient,
        utils::test_utils::{
            TEST_GENESIS, example_local_node_record, example_p2p_node, example_rollup_store,
        },
    };
    use ethrex_blockchain::Blockchain;
    use ethrex_p2p::{peer_handler::PeerHandler, sync_manager::SyncManager};
    use ethrex_rpc::{EthClient, GasTipEstimator, NodeData, RpcApiContext as L1Context};
    use ethrex_storage::{EngineType, Store};
    use mojave_chain_utils::unique_heap::AsyncUniqueHeap;
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };
    use tokio::sync::Mutex as TokioMutex;

    #[ctor]
    fn test_setup() {
        unsafe {
            std::env::set_var(
                "PUBLIC_KEY",
                "624eba5dd4b00f5293c09cf8bdf5508f7edcb5a59836d608da5150bec7110582",
            )
        };
        println!("PUBLIC_KEY initialized for all tests");
    }

    fn create_signed_block() -> SignedBlock {
        let block = create_test_block();
        let hash = block.hash();
        let secret = "433887ac4e37c40872643b0f77a5919db9c47b0ad64650ed5a79dd05bbd6f197";
        let private_key_bytes = hex::decode(secret).expect("Failed to decode private key from hex");
        let private_key_array: [u8; 32] = private_key_bytes
            .try_into()
            .expect("invalid length for private key");
        let signing_key: SigningKey = SigningKey::from_slice(&private_key_array).unwrap();
        let signature: Signature = SigningKey::sign(&signing_key, &hash).unwrap();
        let verifying_key = signing_key.verifying_key();
        SignedBlock {
            block,
            signature,
            verifying_key,
        }
    }

    fn create_test_block() -> Block {
        Block {
            header: BlockHeader {
                parent_hash: H256::zero(),
                ommers_hash: H256::zero(),
                coinbase: Address::zero(),
                state_root: H256::zero(),
                transactions_root: H256::zero(),
                receipts_root: H256::zero(),
                logs_bloom: Bloom::default(),
                difficulty: U256::zero(),
                number: 1u64,
                gas_limit: 21000u64,
                gas_used: 0u64,
                timestamp: 0u64,
                extra_data: Bytes::new(),
                prev_randao: H256::zero(),
                nonce: 0u64,
                base_fee_per_gas: Some(0u64),
                withdrawals_root: None,
                blob_gas_used: None,
                excess_blob_gas: None,
                parent_beacon_block_root: None,
                requests_hash: None,
                ..Default::default()
            },
            body: BlockBody {
                transactions: vec![],
                ommers: vec![],
                withdrawals: None,
            },
        }
    }

    #[test]
    fn test_get_block_data_success() {
        let signed_block = create_signed_block();
        let block_json = serde_json::to_value(&signed_block).unwrap();
        let params = Some(vec![block_json]);

        let result = get_block_data(&params);
        assert!(result.is_ok());
        let parsed_block = result.unwrap();
        assert_eq!(
            parsed_block.block.header.number,
            signed_block.block.header.number
        );
    }

    #[test]
    fn test_get_block_data_no_params() {
        let result = get_block_data(&None);
        assert!(result.is_err());
        if let Err(RpcErr::EthrexRPC(ethrex_rpc::RpcErr::BadParams(msg))) = result {
            assert_eq!(msg, "No params provided");
        } else {
            panic!("Expected BadParams error");
        }
    }

    #[test]
    fn test_get_block_data_empty_params() {
        let params = Some(vec![]);
        let result = get_block_data(&params);
        assert!(result.is_err());
        if let Err(RpcErr::EthrexRPC(ethrex_rpc::RpcErr::BadParams(msg))) = result {
            assert_eq!(
                msg,
                "Expected exactly 1 parameter (SignedBlock), but 0 were provided"
            );
        } else {
            panic!("Expected BadParams error");
        }
    }

    #[test]
    fn test_get_block_data_too_many_params() {
        let block = create_signed_block();
        let block_json = serde_json::to_value(block.block).unwrap();
        let params = Some(vec![
            block_json.clone(),
            json!("signature"),
            json!("extra_param"),
        ]);

        let result = get_block_data(&params);
        assert!(result.is_err());
        if let Err(RpcErr::EthrexRPC(ethrex_rpc::RpcErr::BadParams(msg))) = result {
            assert_eq!(
                msg,
                "Expected exactly 1 parameter (SignedBlock), but 3 were provided"
            );
        } else {
            panic!("Expected BadParams error");
        }
    }

    #[test]
    fn test_get_block_data_invalid_block_format() {
        let invalid_block = json!({"invalid": "data"});
        let params = Some(vec![invalid_block]);

        let result = get_block_data(&params);
        assert!(result.is_err());
        // Should be a serde deserialization error converted to BadParams
        assert!(matches!(
            result,
            Err(RpcErr::EthrexRPC(ethrex_rpc::RpcErr::BadParams(_)))
        ));
    }

    #[test]
    fn test_broadcast_block_request_parse_success() {
        let block = create_signed_block();
        let block_json = serde_json::to_value(block).unwrap();
        let params = Some(vec![block_json]);

        let result = BroadcastBlockRequest::parse(&params);
        assert!(result.is_ok());
    }

    #[test]
    fn test_broadcast_block_request_parse_no_params() {
        let result = BroadcastBlockRequest::parse(&None);
        assert!(result.is_err());
    }

    #[test]
    fn test_broadcast_block_request_parse_invalid_params() {
        let invalid_params = Some(vec![json!({"invalid": "block"})]);
        let result = BroadcastBlockRequest::parse(&invalid_params);
        assert!(result.is_err());
    }

    #[test]
    fn test_rpc_block_to_block_with_minimal_json() {
        let rpc_block_json = json!({
            "hash": "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
            "size": "0x200",
            "number": "0xa",
            "gasLimit": "0x1c9c380",
            "gasUsed": "0x5208",
            "timestamp": "0x5f5e100",
            "parentHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "difficulty": "0x1",
            "totalDifficulty": "0xa",
            "nonce": "0x0",
            "sha3Uncles": "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347",
            "logsBloom": "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
            "transactionsRoot": "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
            "stateRoot": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "receiptsRoot": "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
            "miner": "0x0000000000000000000000000000000000000000",
            "extraData": "0x",
            "mixHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "baseFeePerGas": "0x3b9aca00",
            "transactions": [],
            "uncles": [],
            "withdrawals": []
        });

        let rpc_block_result: Result<RpcBlock, _> = serde_json::from_value(rpc_block_json);

        match rpc_block_result {
            Ok(rpc_block) => {
                let result_block = rpc_block_to_block(rpc_block);

                assert_eq!(result_block.header.number, 10u64); // 0xa = 10
                assert_eq!(result_block.header.gas_limit, 30000000u64); // 0x1c9c380
                assert_eq!(result_block.header.gas_used, 21000u64); // 0x5208
                assert_eq!(result_block.header.base_fee_per_gas, Some(1000000000u64)); // 0x3b9aca00

                assert_eq!(result_block.body.transactions.len(), 0);
                assert_eq!(result_block.body.ommers.len(), 0);
                assert_eq!(result_block.body.withdrawals, Some(vec![]));
            }
            Err(e) => {
                panic!(
                    "Failed to deserialize RpcBlock: {e}. The function rpc_block_to_block exists and compiles correctly.",
                );
            }
        }
    }

    #[tokio::test]
    async fn test_broadcast_block_request_handle_invalid_signature() {
        // Create a valid signed block and then tamper with the verifying key so that
        // signature verification fails.
        let signed_block = create_signed_block();
        // Generate a different verifying key so that the signature check fails.
        let other_secret = "82456c0d4e87df444f3be038cc5c0d1bea8ce29c8fb352b4172052efc27fa998";
        let other_bytes = hex::decode(other_secret).expect("decode other key");
        let other_array: [u8; 32] = other_bytes.try_into().expect("length");
        let bad_signing_key = SigningKey::from_slice(&other_array).unwrap();
        let bad_verifying_key = bad_signing_key.verifying_key();

        let request = BroadcastBlockRequest {
            block: signed_block.block.clone(),
            signature: signed_block.signature.clone(),
            verifying_key: bad_verifying_key,
        };

        // Build a minimal RPC context. The fields won't be used because the request should
        // fail before any interaction with them, but they need to be valid so dropping is
        // safe.
        let storage = Store::new("", EngineType::InMemory).unwrap();
        storage
            .add_initial_state(serde_json::from_str(TEST_GENESIS).unwrap())
            .await
            .unwrap();
        let blockchain = Arc::new(Blockchain::default_with_store(storage.clone()));
        let active_filters = Arc::new(Mutex::new(HashMap::new()));
        let l1_context = L1Context {
            storage: storage.clone(),
            blockchain: blockchain.clone(),
            active_filters: active_filters.clone(),
            syncer: Arc::new(SyncManager::dummy()),
            peer_handler: PeerHandler::dummy(),
            node_data: NodeData {
                jwt_secret: Default::default(),
                local_p2p_node: example_p2p_node(),
                local_node_record: example_local_node_record(),
                client_version: "test".to_string(),
            },
            gas_tip_estimator: Arc::new(TokioMutex::new(GasTipEstimator::new())),
        };
        let rollup_store = example_rollup_store().await;
        let mojave_client =
            MojaveClient::new(std::slice::from_ref(&"http://localhost:1".to_owned())).unwrap();
        let eth_client = EthClient::new("http://localhost:1").unwrap();
        let block_queue = AsyncUniqueHeap::new();

        let context = RpcApiContextFullNode {
            l1_context,
            rollup_store,
            mojave_client,
            eth_client,
            blockchain,
            block_queue,
        };

        let result = request.handle(context).await;
        assert!(matches!(result, Err(RpcErr::SignatureError(_))));
    }
}
