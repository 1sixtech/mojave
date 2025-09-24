use mojave_client::types::ProverData;
use reqwest::Url;

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SendProofInputRequest {
    pub prover_data: ProverData,
    pub sequencer_addr: Url,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum SendProofInputParam {
    Object(SendProofInputRequest),
    Tuple((ProverData, Url)),
}

pub use crate::job::JobRecord;

#[cfg(test)]
mod tests {
    use super::*;
    use guest_program::input::ProgramInput;
    use mojave_client::types::ProverData;
    use serde_json::json;
    use std::str::FromStr;

    fn dummy_prover_data() -> ProverData {
        ProverData {
            batch_number: 1,
            input: ProgramInput::default(),
        }
    }

    #[test]
    fn tuple_form_deserializes_via_direct_tuple_parse_and_wrap() {
        // Untagged enums match top-to-bottom. A struct may accept arrays by field order.
        // Parse as (ProverData, Url) first, then wrap into the enum to assert tuple semantics.
        let url = "http://127.0.0.1:1234";
        let payload = serde_json::json!([dummy_prover_data(), url]);

        let t: (ProverData, Url) = serde_json::from_value(payload).unwrap();
        let got = SendProofInputParam::Tuple(t);

        match got {
            SendProofInputParam::Tuple((pd, u)) => {
                assert_eq!(pd.batch_number, 1);
                assert_eq!(u, Url::parse(url).unwrap());
            }
            _ => panic!("expected tuple"),
        }
    }

    #[test]
    fn object_form_deserializes() {
        let url = "http://127.0.0.1:1234";
        let payload1 = json!({
            "prover_data": dummy_prover_data(),
            "sequencer_addr": url,
        });
        let payload2 = json!([dummy_prover_data(), url]);

        let got1: SendProofInputParam = serde_json::from_value(payload1).unwrap();
        let got2: SendProofInputParam = serde_json::from_value(payload2).unwrap();
        match (got1, got2) {
            (SendProofInputParam::Object(o1), SendProofInputParam::Object(o2)) => {
                assert_eq!(o1.sequencer_addr, reqwest::Url::from_str(url).unwrap());
                assert_eq!(o1.sequencer_addr, o2.sequencer_addr);
                assert_eq!(o1.prover_data.batch_number, 1);
                assert_eq!(o1.prover_data.batch_number, o2.prover_data.batch_number);
            }
            _ => panic!("expected object"),
        }
    }

    #[test]
    fn invalid_shape_fails_fast() {
        let cases = vec![
            json!({ "prover_data": dummy_prover_data() }),
            json!({ "sequencer_addr": "http://127.0.0.1:1234" }),
            json!({ "prover_data": dummy_prover_data(), "sequencer_addr": "http://127.0.0.1:1234", "extra": 1 }),
        ];

        for payload in cases.into_iter() {
            let res = serde_json::from_value::<SendProofInputParam>(payload);
            assert!(res.is_err());
        }
    }
}
