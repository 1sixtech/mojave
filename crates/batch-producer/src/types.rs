use ethrex_common::{
    H256,
    types::{BlobsBundle, Block, BlockHeader, BlockNumber},
};

pub enum Request {
    BuildBatch,
}

pub(crate) struct BatchData {
    pub(crate) last_block: BlockNumber,
    pub(crate) state_root: H256,
    pub(crate) message_hashes: Vec<H256>,
    pub(crate) privileged_tx_hashes: Vec<H256>,
    pub(crate) blobs_bundle: BlobsBundle,
}

pub(crate) struct BlockData {
    pub(crate) block: Block,
    pub(crate) header: BlockHeader,
}
