use crate::rpc::{
    RpcHandler, SignedBlock,
    full_node::{RpcApiContextFullNode, types::ordered_block::OrderedBlock},
    utils::RpcErr,
};

use ethrex_common::types::{Block, BlockBody, Transaction};
use ethrex_rpc::{clients::eth::BlockByNumber, types::block::RpcBlock};
use serde_json::Value;

#[derive(Debug)]
pub struct BroadcastBlockRequest {
    block: Block,
    signature: String,
}

impl RpcHandler<RpcApiContextFullNode> for BroadcastBlockRequest {
    fn parse(params: &Option<Vec<Value>>) -> Result<Self, RpcErr> {
        let signed_block = get_block_data(params)?;
        Ok(Self {
            block: signed_block.block,
            signature: signed_block.signature,
        })
    }

    async fn handle(&self, context: RpcApiContextFullNode) -> Result<Value, RpcErr> {
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
        Ok(Value::Null)
    }
}

fn get_block_data(req: &Option<Vec<Value>>) -> Result<SignedBlock, RpcErr> {
    let params = req
        .as_ref()
        .ok_or(RpcErr::EthrexRPC(ethrex_rpc::RpcErr::BadParams(
            "No params provided".to_owned(),
        )))?;

    if params.len() < 1 || params.len() > 2 {
        return Err(RpcErr::EthrexRPC(ethrex_rpc::RpcErr::BadParams(format!(
            "Expected 1 or 2 params and {} were provided",
            params.len()
        ))));
    };

    let block_value = params[0].get("block").ok_or_else(|| {
        RpcErr::EthrexRPC(ethrex_rpc::RpcErr::BadParams(
            "Missing 'block' field".to_string(),
        ))
    })?;
    let block = serde_json::from_value::<Block>(block_value.clone()).map_err(|e| {
        RpcErr::EthrexRPC(ethrex_rpc::RpcErr::BadParams(format!(
            "Block deserialization error: {e}"
        )))
    })?;

    let signature = params[0]
        .get("signature")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_default();
    Ok(SignedBlock { block, signature })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethrex_common::{
        Address, Bloom, Bytes, H256, U256,
        types::{Block, BlockBody, BlockHeader},
    };

    use serde_json::json;

    fn create_signed_block() -> SignedBlock {
        SignedBlock {
            block: create_test_block(),
            signature: "test_signature".to_string(),
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
        let block_json = serde_json::to_value(signed_block.clone()).unwrap();
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
            assert_eq!(msg, "Expected 1 or 2 params and 0 were provided");
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
            assert_eq!(msg, "Expected 1 or 2 params and 3 were provided");
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
}
