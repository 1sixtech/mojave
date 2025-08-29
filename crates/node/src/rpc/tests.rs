use ethrex_common::{
    Address, Bloom, Bytes, H256, U256,
    constants::DEFAULT_OMMERS_HASH,
    types::{
        Block, BlockBody, BlockHeader, ChainConfig, ELASTICITY_MULTIPLIER, Genesis,
        INITIAL_BASE_FEE, calculate_base_fee_per_gas, compute_receipts_root,
        compute_transactions_root,
    },
};
use std::collections::BTreeMap;

#[allow(dead_code)]
fn build_genesis() -> Genesis {
    Genesis {
        config: ChainConfig {
            chain_id: 1,
            london_block: Some(0),
            ..Default::default()
        },
        alloc: BTreeMap::new(),
        coinbase: Address::zero(),
        difficulty: U256::zero(),
        extra_data: Bytes::new(),
        gas_limit: 30_000_000,
        nonce: 0,
        mix_hash: H256::zero(),
        timestamp: 0,
        base_fee_per_gas: None,
        blob_gas_used: None,
        excess_blob_gas: None,
        requests_hash: None,
    }
}

#[allow(dead_code)]
fn next_block(parent: &Block) -> Block {
    let parent_header = &parent.header;
    let base_fee = calculate_base_fee_per_gas(
        parent_header.gas_limit,
        parent_header.gas_limit,
        parent_header.gas_used,
        parent_header.base_fee_per_gas.unwrap_or(INITIAL_BASE_FEE),
        ELASTICITY_MULTIPLIER,
    )
    .unwrap();

    let header = BlockHeader {
        parent_hash: parent.hash(),
        ommers_hash: *DEFAULT_OMMERS_HASH,
        coinbase: parent_header.coinbase,
        state_root: parent_header.state_root,
        transactions_root: compute_transactions_root(&[]),
        receipts_root: compute_receipts_root(&[]),
        logs_bloom: Bloom::zero(),
        difficulty: U256::zero(),
        number: parent_header.number + 1,
        gas_limit: parent_header.gas_limit,
        gas_used: 0,
        timestamp: parent_header.timestamp + 1,
        extra_data: Bytes::new(),
        prev_randao: parent_header.prev_randao,
        nonce: 0,
        base_fee_per_gas: Some(base_fee),
        withdrawals_root: None,
        blob_gas_used: None,
        excess_blob_gas: None,
        parent_beacon_block_root: None,
        requests_hash: None,
        ..Default::default()
    };
    let body = BlockBody {
        transactions: vec![],
        ommers: vec![],
        withdrawals: None,
    };
    Block::new(header, body)
}
