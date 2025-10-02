use crate::rlpx::{
    error::RLPxError,
    message::RLPxMessage,
    utils::{snappy_compress, snappy_decompress},
};
use bytes::BufMut;
use ethrex_common::{Signature, types::Block};
use ethrex_rlp::{
    error::{RLPDecodeError, RLPEncodeError},
    structs::{Decoder, Encoder},
};
use serde::{Deserialize, Serialize};

/// The reason for data being [String] is that JSON string easily allows us to
/// deserialize into an enum unlike byte vector.
#[derive(Debug, Clone)]
pub struct MojaveMessage {
    pub data: String,
}

impl From<String> for MojaveMessage {
    fn from(value: String) -> Self {
        Self { data: value }
    }
}

impl RLPxMessage for MojaveMessage {
    const CODE: u8 = 0x0;

    fn encode(&self, buf: &mut dyn BufMut) -> Result<(), RLPEncodeError> {
        let mut encoded_data = vec![];
        Encoder::new(&mut encoded_data)
            .encode_field(&self.data.clone())
            .finish();
        let msg_data = snappy_compress(encoded_data)?;
        buf.put_slice(&msg_data);
        Ok(())
    }

    fn decode(msg_data: &[u8]) -> Result<Self, RLPDecodeError> {
        let decompressed_data = snappy_decompress(msg_data)?;
        let decoder = Decoder::new(&decompressed_data)?;
        let (data, decoder) = decoder.decode_field("data")?;
        decoder.finish()?;
        Ok(Self { data })
    }
}

impl MojaveMessage {
    pub fn from_payload(payload: &MojavePayload) -> Result<Self, RLPxError> {
        let data = serde_json::to_string(payload)
            .map_err(|error| RLPxError::InternalError(error.to_string()))?;
        Ok(Self { data })
    }
}

/// TODO: add enum variant on demand.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
#[allow(clippy::large_enum_variant)]
pub enum MojavePayload {
    Block(MojaveBlock),
    Proof(MojaveProof),
}

impl MojavePayload {
    pub fn from_mojave_message(msg: &MojaveMessage) -> Result<Self, RLPxError> {
        serde_json::from_str(&msg.data).map_err(|error| RLPxError::InternalError(error.to_string()))
    }

    pub async fn handle(&self) -> Result<(), RLPxError> {
        // TODO: handle message per enum variant.
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MojaveBlock {
    block: Block,
    signature: Signature,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MojaveProof {
    proof: String,
}
