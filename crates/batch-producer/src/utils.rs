use ethrex_common::types::{
    AccountUpdate, BlobsBundle, BlockHeader, PrivilegedL2Transaction, blobs_bundle,
};
use ethrex_l2_common::{l1_messages::L1Message, state_diff::StateDiff};
use ethrex_vm::VmDatabase;

use crate::error::{Error, Result};

/// Prepare the state diff for the block.
pub(crate) fn prepare_state_diff(
    _last_header: BlockHeader,
    _db: &impl VmDatabase,
    _l1messages: &[L1Message],
    _privileged_transactions: &[PrivilegedL2Transaction],
    _account_updates: Vec<AccountUpdate>,
) -> Result<StateDiff> {
    Ok(StateDiff::default())
}

pub(crate) fn get_privileged_transactions() -> Vec<PrivilegedL2Transaction> {
    vec![]
}

pub(crate) fn get_block_l1_messages() -> Vec<L1Message> {
    vec![]
}

pub(crate) fn generate_blobs_bundle(state_diff: &StateDiff) -> Result<(BlobsBundle, usize)> {
    let blob_data = state_diff.encode().map_err(Error::from)?;
    let blob_size = blob_data.len();
    let blob = blobs_bundle::blob_from_bytes(blob_data).map_err(Error::from)?;
    Ok((
        BlobsBundle::create_from_blobs(&[blob]).map_err(Error::from)?,
        blob_size,
    ))
}
