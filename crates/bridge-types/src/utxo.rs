use serde::{Deserialize, Serialize};

/// UTXO source type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum UtxoSource {
    Deposit,
    Change,
    Collateral,
}

impl std::fmt::Display for UtxoSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UtxoSource::Deposit => write!(f, "DEPOSIT"),
            UtxoSource::Change => write!(f, "CHANGE"),
            UtxoSource::Collateral => write!(f, "COLLATERAL"),
        }
    }
}

/// UTXO representation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Utxo {
    pub utxo_id: String,
    pub txid: String,
    pub vout: u32,
    pub amount: String,
    pub source: UtxoSource,
    pub spent: bool,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub spent_in_withdrawal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[serde(with = "ts_milliseconds_option_default")]
    pub spent_at: Option<chrono::DateTime<chrono::Utc>>,
}

// Custom serde module for Option<DateTime> with default
mod ts_milliseconds_option_default {
    use chrono::{DateTime, TimeZone, Utc};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(date: &Option<DateTime<Utc>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match date {
            Some(dt) => serializer.serialize_some(&dt.timestamp_millis()),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<DateTime<Utc>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt = Option::<i64>::deserialize(deserializer)?;
        Ok(opt.and_then(|millis| Utc.timestamp_millis_opt(millis).single()))
    }
}

impl Utxo {
    pub fn amount_sats(&self) -> Result<u64, std::num::ParseIntError> {
        self.amount.parse()
    }
}

/// UTXO selection policy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UtxoPolicy {
    Largest,
    Smallest,
    Oldest,
    BestFit,
}

/// UTXO statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UtxoStats {
    pub total: usize,
    pub available: usize,
    pub spent: usize,
    pub total_amount: String,
    pub available_amount: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_utxo_serialization() {
        let utxo = Utxo {
            utxo_id: "0x123".to_string(),
            txid: "0xabc".to_string(),
            vout: 0,
            amount: "50000".to_string(),
            source: UtxoSource::Deposit,
            spent: false,
            created_at: Utc::now(),
            spent_in_withdrawal: None,
            spent_at: None,
        };

        let json = serde_json::to_string(&utxo).unwrap();
        let deserialized: Utxo = serde_json::from_str(&json).unwrap();

        assert_eq!(utxo.utxo_id, deserialized.utxo_id);
        assert_eq!(utxo.source, deserialized.source);
    }

    #[test]
    fn test_utxo_amount() {
        let utxo = Utxo {
            utxo_id: "0x123".to_string(),
            txid: "0xabc".to_string(),
            vout: 0,
            amount: "50000".to_string(),
            source: UtxoSource::Deposit,
            spent: false,
            created_at: Utc::now(),
            spent_in_withdrawal: None,
            spent_at: None,
        };

        assert_eq!(utxo.amount_sats().unwrap(), 50000);
    }
}
