//! Bridge event definitions

use crate::{Utxo, UtxoSource};
use serde::{Deserialize, Serialize};

/// Bridge contract events
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum BridgeEvent {
    UtxoRegistered(UtxoRegisteredEvent),
    UtxoSpent(UtxoSpentEvent),
    WithdrawalRequested(WithdrawalRequestedEvent),
    WithdrawalFinalized(WithdrawalFinalizedEvent),
}

/// UTXO registered event
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UtxoRegisteredEvent {
    pub utxo_id: String,
    pub txid: String,
    pub vout: u32,
    pub amount: u64,
    pub source: UtxoSource,
    pub timestamp: u64,
    pub block_number: u64,
    pub transaction_hash: String,
}

impl From<UtxoRegisteredEvent> for Utxo {
    fn from(event: UtxoRegisteredEvent) -> Self {
        use chrono::{DateTime, Utc};
        Self {
            utxo_id: event.utxo_id,
            txid: event.txid,
            vout: event.vout,
            amount: event.amount.to_string(),
            source: event.source,
            spent: false,
            created_at: DateTime::from_timestamp(event.timestamp as i64, 0)
                .unwrap_or_else(Utc::now),
            spent_in_withdrawal: None,
            spent_at: None,
        }
    }
}

/// UTXO spent event
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UtxoSpentEvent {
    pub utxo_id: String,
    pub withdrawal_id: String,
    pub timestamp: u64,
    pub block_number: u64,
    pub transaction_hash: String,
}

/// Withdrawal requested event
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WithdrawalRequestedEvent {
    pub withdrawal_id: String,
    pub recipient: String,
    pub amount: u64,
    pub bitcoin_address: String,
    pub timestamp: u64,
    pub block_number: u64,
}

/// Withdrawal finalized event
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WithdrawalFinalizedEvent {
    pub withdrawal_id: String,
    pub bitcoin_txid: String,
    pub timestamp: u64,
    pub block_number: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_serialization() {
        let event = BridgeEvent::UtxoRegistered(UtxoRegisteredEvent {
            utxo_id: "0x123".to_string(),
            txid: "0xabc".to_string(),
            vout: 0,
            amount: 50000,
            source: UtxoSource::Deposit,
            timestamp: 1234567890,
            block_number: 100,
            transaction_hash: "0xdef".to_string(),
        });

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: BridgeEvent = serde_json::from_str(&json).unwrap();

        match deserialized {
            BridgeEvent::UtxoRegistered(e) => {
                assert_eq!(e.utxo_id, "0x123");
                assert_eq!(e.amount, 50000);
            }
            _ => panic!("Wrong event type"),
        }
    }
}
