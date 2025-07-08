use crate::rpc::{
    RpcHandler, SignedBlock,
    full_node::{RpcApiContextFullNode, types::ordered_block::OrderedBlock},
    utils::RpcErr,
};

use ethrex_common::types::{Block, BlockBody, Transaction};
use ethrex_rpc::{clients::eth::BlockByNumber, types::block::RpcBlock};
use std::env;

use ed25519_dalek::{Signature, Verifier, VerifyingKey, ed25519::SignatureBytes};
use serde_json::Value;

#[derive(Debug)]
pub struct BroadcastBlockRequest {
    block: Block,
    signature: SignatureBytes,
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
        // Verify the signature before processing the block
        verifying_signature(&self.block, &self.signature)?;

        let latest_block_number = context.l1_context.storage.get_latest_block_number().await? + 1;
        for block_number in latest_block_number..self.block.header.number {
            let block = context
                .eth_client
                .get_block_by_number(BlockByNumber::Number(block_number))
                .await?;
            let block = rpc_block_to_block(block);

            context.block_queue.push(OrderedBlock(block)).await;
        }
        // Verify the signature before broadcasting

        context
            .block_queue
            .push(OrderedBlock(self.block.clone()))
            .await;
        Ok(Value::Null)
    }
}

fn verifying_signature(block: &Block, signature_bytes: &SignatureBytes) -> Result<(), RpcErr> {
    let public = env::var("PUBLIC_KEY").map_err(|_| {
        RpcErr::EthrexRPC(ethrex_rpc::RpcErr::Internal(
            "Missing PUBLIC_KEY environment variable".to_string(),
        ))
    })?;

    let bytes = hex::decode(public).map_err(|_| {
        RpcErr::EthrexRPC(ethrex_rpc::RpcErr::Internal(
            "Invalid PUBLIC_KEY format".to_string(),
        ))
    })?;

    let hash = block.hash();

    let verifying_key =
        VerifyingKey::from_bytes(<&[u8; 32]>::try_from(bytes.as_slice()).map_err(|_| {
            RpcErr::EthrexRPC(ethrex_rpc::RpcErr::Internal(
                "Failed to convert PUBLIC_KEY to [u8; 32]".to_string(),
            ))
        })?)
        .map_err(|_| {
            RpcErr::EthrexRPC(ethrex_rpc::RpcErr::Internal(
                "Failed to create VerifyingKey from PUBLIC_KEY".to_string(),
            ))
        })?;

    let signature = Signature::from_bytes(signature_bytes);

    verifying_key
        .verify(hash.as_bytes(), &signature)
        .map_err(|_| {
            RpcErr::EthrexRPC(ethrex_rpc::RpcErr::BadParams(
                "Signature verification failed".to_string(),
            ))
        })?;
    Ok(())
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

    let signature_value = params.get(0).and_then(|v| v.get("signature"));
    let array = signature_value.and_then(|v| v.as_array()).ok_or_else(|| {
        RpcErr::EthrexRPC(ethrex_rpc::RpcErr::BadParams(
            "Invalid or missing signature array".to_string(),
        ))
    })?;
    let mut signature = [0u8; Signature::BYTE_SIZE];
    for (i, v) in array.iter().enumerate() {
        signature[i] = v.as_u64().ok_or_else(|| {
            RpcErr::EthrexRPC(ethrex_rpc::RpcErr::BadParams(
                "Invalid signature byte".to_string(),
            ))
        })? as u8;
    }

    Ok(SignedBlock { block, signature })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ctor::ctor;
    use ed25519_dalek::SigningKey;
    use ethrex_common::{
        Address, Bloom, Bytes, H256, U256,
        types::{Block, BlockBody, BlockHeader},
    };

    use k256::ecdsa::signature::SignerMut;
    use serde_json::json;

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
        let mut signing_key = SigningKey::from_bytes(&private_key_array);
        let signature: Signature = signing_key.sign(hash.as_bytes());
        SignedBlock {
            block: block,
            signature: signature.to_bytes(),
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

    #[test]
    fn test_verifying_signature_success() {
        let block = create_signed_block();
        let result = verifying_signature(&block.block, &block.signature);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verifying_signature_invalid_signature() {
        let block = create_test_block();
        let invalid_signature = [0u8; 64]; // Invalid signature bytes

        let result = verifying_signature(&block, &invalid_signature);
        assert!(result.is_err());
        if let Err(RpcErr::EthrexRPC(ethrex_rpc::RpcErr::BadParams(msg))) = result {
            assert_eq!(msg, "Signature verification failed");
        } else {
            panic!("Expected BadParams error for signature verification failure");
        }
    }

    #[test]
    fn test_verifying_signature_wrong_signature() {
        let block = create_test_block();
        let different_block = Block {
            header: BlockHeader {
                number: 2u64, // Different from the original block
                ..block.header.clone()
            },
            ..block.clone()
        };

        // Create signature for different block
        let different_hash = different_block.hash();
        let secret = "433887ac4e37c40872643b0f77a5919db9c47b0ad64650ed5a79dd05bbd6f197";
        let private_key_bytes = hex::decode(secret).expect("Failed to decode private key from hex");
        let private_key_array: [u8; 32] = private_key_bytes
            .try_into()
            .expect("invalid length for private key");
        let mut signing_key = SigningKey::from_bytes(&private_key_array);
        let signature: Signature = signing_key.sign(different_hash.as_bytes());

        // Try to verify signature of different block against original block
        let result = verifying_signature(&block, &signature.to_bytes());
        assert!(result.is_err());
        if let Err(RpcErr::EthrexRPC(ethrex_rpc::RpcErr::BadParams(msg))) = result {
            assert_eq!(msg, "Signature verification failed");
        } else {
            panic!("Expected BadParams error for signature verification failure");
        }
    }

    #[test]
    fn test_verifying_signature_missing_public_key() {
        let block = create_test_block();
        let signature = [0u8; 64];

        // Temporarily remove PUBLIC_KEY
        let original_key = std::env::var("PUBLIC_KEY").ok();
        unsafe {
            std::env::remove_var("PUBLIC_KEY");
        }

        let result = verifying_signature(&block, &signature);

        // Restore PUBLIC_KEY if it existed
        if let Some(key) = original_key {
            unsafe {
                std::env::set_var("PUBLIC_KEY", key);
            }
        }

        assert!(result.is_err());
        if let Err(RpcErr::EthrexRPC(ethrex_rpc::RpcErr::Internal(msg))) = result {
            assert_eq!(msg, "Missing PUBLIC_KEY environment variable");
        } else {
            panic!("Expected Internal error for missing PUBLIC_KEY");
        }
    }

    #[test]
    fn test_verifying_signature_invalid_public_key_format() {
        let block = create_test_block();
        let signature = [0u8; 64];

        // Set invalid PUBLIC_KEY
        let original_key = std::env::var("PUBLIC_KEY").ok();
        unsafe {
            std::env::set_var("PUBLIC_KEY", "invalid_hex");
        }

        let result = verifying_signature(&block, &signature);

        // Restore original PUBLIC_KEY
        if let Some(key) = original_key {
            unsafe {
                std::env::set_var("PUBLIC_KEY", key);
            }
        }

        assert!(result.is_err());
        if let Err(RpcErr::EthrexRPC(ethrex_rpc::RpcErr::Internal(msg))) = result {
            assert_eq!(msg, "Invalid PUBLIC_KEY format");
        } else {
            panic!("Expected Internal error for invalid PUBLIC_KEY format");
        }
    }

    #[test]
    fn test_verifying_signature_wrong_public_key_length() {
        let block = create_test_block();
        let signature = [0u8; 64];

        // Set PUBLIC_KEY with wrong length (too short)
        let original_key = std::env::var("PUBLIC_KEY").ok();
        unsafe {
            std::env::set_var("PUBLIC_KEY", "1234567890abcdef"); // Only 16 hex chars = 8 bytes, need 32 bytes
        }

        let result = verifying_signature(&block, &signature);

        // Restore original PUBLIC_KEY
        if let Some(key) = original_key {
            unsafe {
                std::env::set_var("PUBLIC_KEY", key);
            }
        }

        assert!(result.is_err());
        if let Err(RpcErr::EthrexRPC(ethrex_rpc::RpcErr::Internal(msg))) = result {
            assert_eq!(msg, "Failed to convert PUBLIC_KEY to [u8; 32]");
        } else {
            panic!("Expected Internal error for wrong PUBLIC_KEY length");
        }
    }

    #[test]
    fn test_verifying_signature_invalid_verifying_key() {
        let block = create_test_block();
        let signature = [0u8; 64];

        // Set PUBLIC_KEY with correct length but invalid key data
        let original_key = std::env::var("PUBLIC_KEY").ok();
        unsafe {
            std::env::set_var(
                "PUBLIC_KEY",
                "0000000000000000000000000000000000000000000000000000000000000002",
            ); // All zeros
        }

        let result = verifying_signature(&block, &signature);

        // Restore original PUBLIC_KEY
        if let Some(key) = original_key {
            unsafe {
                std::env::set_var("PUBLIC_KEY", key);
            }
        }

        assert!(result.is_err());
        if let Err(RpcErr::EthrexRPC(ethrex_rpc::RpcErr::Internal(msg))) = result {
            assert_eq!(msg, "Failed to create VerifyingKey from PUBLIC_KEY");
        } else {
            panic!("Expected Internal error for invalid VerifyingKey");
        }
    }
}
