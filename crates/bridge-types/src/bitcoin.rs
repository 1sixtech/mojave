//! Bitcoin-related type definitions

use serde::{Deserialize, Serialize};

/// Bitcoin transaction ID (32 bytes hex)
pub type Txid = String;

/// Bitcoin block hash (32 bytes hex)
pub type BlockHash = String;

/// Bitcoin script pubkey
pub type ScriptPubKey = Vec<u8>;

/// Bitcoin address types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AddressType {
    /// Pay-to-Public-Key-Hash (Legacy)
    P2PKH,
    /// Pay-to-Script-Hash
    P2SH,
    /// Pay-to-Witness-Public-Key-Hash (Native SegWit)
    P2WPKH,
    /// Pay-to-Witness-Script-Hash
    P2WSH,
    /// Pay-to-Taproot
    P2TR,
}

/// Bitcoin network types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Network {
    /// Bitcoin mainnet
    Mainnet,
    /// Bitcoin testnet
    Testnet,
    /// Bitcoin regtest (local testing)
    Regtest,
    /// Bitcoin signet
    Signet,
}

impl Default for Network {
    fn default() -> Self {
        Network::Mainnet
    }
}

/// Bitcoin transaction output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxOut {
    /// Amount in satoshis
    pub value: u64,
    /// Script pubkey
    pub script_pubkey: ScriptPubKey,
}

/// Bitcoin UTXO reference
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutPoint {
    /// Transaction ID
    pub txid: Txid,
    /// Output index
    pub vout: u32,
}

impl OutPoint {
    /// Create new outpoint
    pub fn new(txid: Txid, vout: u32) -> Self {
        Self { txid, vout }
    }

    /// Get unique identifier
    pub fn to_id(&self) -> String {
        format!("{}:{}", self.txid, self.vout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_outpoint_id() {
        let outpoint = OutPoint::new("abc123".to_string(), 0);
        assert_eq!(outpoint.to_id(), "abc123:0");
    }

    #[test]
    fn test_network_serialization() {
        let network = Network::Regtest;
        let json = serde_json::to_string(&network).unwrap();
        assert_eq!(json, "\"regtest\"");
    }
}
