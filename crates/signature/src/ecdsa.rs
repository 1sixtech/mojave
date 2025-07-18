use crate::{Signature, SignatureError, SignatureScheme};
use secp256k1::{
    ecdsa::Signature as EcdsaSignature, Message, PublicKey, Secp256k1, SecretKey as PrivateKey,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{str::FromStr, sync::LazyLock};
use tiny_keccak::{Hasher, Keccak};

static SECP256K1_SIGNING: LazyLock<Secp256k1<secp256k1::SignOnly>> =
    LazyLock::new(|| Secp256k1::signing_only());
static SECP256K1_VERIFY: LazyLock<Secp256k1<secp256k1::VerifyOnly>> =
    LazyLock::new(|| Secp256k1::verification_only());

#[derive(Clone, Debug)]
pub struct SigningKey(PrivateKey);

impl FromStr for SigningKey {
    type Err = SignatureError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let private_key =
            PrivateKey::from_str(s).map_err(|error| Error::CreateSigningKey(error.into()))?;
        Ok(Self(private_key))
    }
}

impl super::Signer for SigningKey {
    fn from_slice(slice: &[u8]) -> Result<Self, SignatureError> {
        let private_key =
            PrivateKey::from_slice(slice).map_err(|error| Error::CreateSigningKey(error.into()))?;
        Ok(Self(private_key))
    }

    fn sign<T: Serialize>(&self, message: &T) -> Result<Signature, SignatureError> {
        let message_bytes =
            bincode::serialize(message).map_err(|error| Error::Sign(error.into()))?;
        let msg_hash = Sha256::digest(message_bytes);
        let message = Message::from_digest_slice(msg_hash.as_slice())
            .map_err(|error| Error::Sign(error.into()))?;
        let secp256k1 = &SECP256K1_SIGNING;
        let signature = secp256k1.sign_ecdsa(&message, &self.0).serialize_compact();
        Ok(Signature {
            bytes: signature.to_vec(),
            scheme: SignatureScheme::Secp256k1,
        })
    }
}

impl SigningKey {
    pub fn verifying_key(&self) -> VerifyingKey {
        let secp = Secp256k1::new();
        VerifyingKey(PublicKey::from_secret_key(&secp, &self.0))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(try_from = "String")]
#[serde(into = "String")]
pub struct VerifyingKey(PublicKey);

impl TryFrom<String> for VerifyingKey {
    type Error = SignatureError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_str(&value)
    }
}

impl From<VerifyingKey> for String {
    fn from(value: VerifyingKey) -> Self {
        value.0.to_string()
    }
}

impl FromStr for VerifyingKey {
    type Err = SignatureError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let public_key =
            PublicKey::from_str(s).map_err(|error| Error::CreateVerifyingKey(error.into()))?;
        Ok(Self(public_key))
    }
}

impl super::Verifier for VerifyingKey {
    fn from_slice(slice: &[u8]) -> Result<Self, SignatureError> {
        let public_key = PublicKey::from_slice(slice)
            .map_err(|error| Error::CreateVerifyingKey(error.into()))?;
        Ok(Self(public_key))
    }

    fn verify<T: Serialize>(
        &self,
        message: &T,
        signature: &Signature,
    ) -> Result<bool, SignatureError> {
        if signature.scheme != SignatureScheme::Secp256k1 {
            return Err(Error::InvalidSignatureScheme)?;
        }

        let secp = &SECP256K1_VERIFY;
        let message_bytes =
            bincode::serialize(message).map_err(|error| Error::Verify(error.into()))?;
        let digest = Sha256::digest(message_bytes);
        let msg =
            Message::from_digest_slice(&digest).map_err(|error| Error::Verify(error.into()))?;
        let sig = EcdsaSignature::from_compact(&signature.bytes)
            .map_err(|error| Error::Verify(error.into()))?;

        match secp.verify_ecdsa(&msg, &sig, &self.0) {
            Ok(()) => Ok(true),
            Err(_error) => Ok(false),
        }
    }
}

impl VerifyingKey {
    pub fn to_address(&self) -> String {
        let publick_key_byte = PublicKey::serialize_uncompressed(&self.0);

        let mut hasher = Keccak::v256();
        // remove from
        hasher.update(&publick_key_byte[1..]);
        let mut hash = [0u8; 32];
        hasher.finalize(&mut hash);

        let address = hex::encode(&hash[12..32]);

        address
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to create a signing key: {0}")]
    CreateSigningKey(ErrorKind),
    #[error("Failed to sign the message: {0}")]
    Sign(ErrorKind),
    #[error("Failed to create a verifying key: {0}")]
    CreateVerifyingKey(ErrorKind),
    #[error("Failed to verify the message: {0}")]
    Verify(ErrorKind),
    #[error("Invalid signature scheme")]
    InvalidSignatureScheme,
}

#[derive(Debug, thiserror::Error)]
pub enum ErrorKind {
    #[error("{0}")]
    Secp256k1(#[from] secp256k1::Error),
    #[error("{0}")]
    Bincode(#[from] bincode::Error),
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::{Signer, Verifier};
    /// use anvil 0 account for test in here: https://getfoundry.sh/anvil/overview/
    /// address: 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
    /// private_key: 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80

    const ANVIL_ACC0_KEY: &str = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

    #[test]
    fn test_secp256k1_address_from_anvil_acc0_pk() {
        let signing_key = SigningKey::from_str(&ANVIL_ACC0_KEY).unwrap();
        let verifying_key = signing_key.verifying_key();

        let address = verifying_key.to_address();
        assert_eq!(
            address.to_lowercase(),
            "f39Fd6e51aad88F6F4ce6aB8827279cffFb92266".to_lowercase()
        );
        print!("address expected  : \"f39Fd6e51aad88F6F4ce6aB8827279cffFb92266\"\naddress calculated: {:?}", address);
    }

    #[test]

    fn test_secp256k1_sign_and_verify() {
        use crate::{Signer, Verifier};

        let anvil_signing_key = SigningKey::from_str(&ANVIL_ACC0_KEY).unwrap();

        let verifying_key = anvil_signing_key.verifying_key();
        let msg = b"Hello World";

        let signature = anvil_signing_key.sign(msg).unwrap();
        let res = verifying_key.verify(msg, &signature);
        assert!(res.is_ok())
    }

    // Negative test cases

    #[test]

    fn test_secp256k1_invalid_signing_key() {
        // Test with invalid hex characters
        let invalid_hex = "invalid_hex_string_not_valid_ecdsa";
        let result = SigningKey::from_str(invalid_hex);
        assert!(result.is_err());

        // Test with odd-length hex string
        let odd_hex = &ANVIL_ACC0_KEY[1..];
        let result = SigningKey::from_str(odd_hex);
        assert!(result.is_err());

        // Test with empty string
        let empty_hex = "";
        let result = SigningKey::from_str(empty_hex);
        assert!(result.is_err());

        // Test with too short byte array (16 bytes instead of 32)
        let short_key: [u8; 16] = [
            0xac, 0x09, 0x74, 0xbe, 0xc3, 0x9a, 0x17, 0xe3, 0x6b, 0xa4, 0xa6, 0xb4, 0xd2, 0x38,
            0xff, 0x94,
        ];
        let result = SigningKey::from_slice(&short_key);
        assert!(result.is_err());

        // Test with too long byte array (64 bytes instead of 32)
        let long_key: [u8; 64] = [0u8; 64];
        let result = SigningKey::from_slice(&long_key);
        assert!(result.is_err());

        // Test with empty byte array
        let empty_key: [u8; 0] = [];
        let result = SigningKey::from_slice(&empty_key);
        assert!(result.is_err());

        // Test with zero key (invalid for secp256k1)
        let zero_key: [u8; 32] = [0u8; 32];
        let result = SigningKey::from_slice(&zero_key);
        assert!(result.is_err());
    }

    #[test]

    fn test_secp256k1_invalid_verifying_key() {
        // Test with invalid hex characters
        let invalid_hex = "zzinvalid_hex_string_not_valid_for_public_key_ecdsa";
        let result = VerifyingKey::from_str(invalid_hex);
        assert!(result.is_err());

        // Test with wrong format (not a valid public key format)
        let wrong_format = "04invalid_public_key_format_for_secp256k1";
        let result = VerifyingKey::from_str(wrong_format);
        assert!(result.is_err());

        // Test with too short byte array (16 bytes instead of 33/65)
        let short_key: [u8; 16] = [
            0x02, 0x79, 0xbe, 0x66, 0x7e, 0xf9, 0xdc, 0xbb, 0xac, 0x55, 0xa0, 0x62, 0x95, 0xce,
            0x87, 0x0b,
        ];
        let result = VerifyingKey::from_slice(&short_key);
        assert!(result.is_err());

        // Test with invalid compressed public key prefix
        let invalid_compressed: [u8; 33] = [
            0x05, // Invalid prefix (should be 0x02 or 0x03)
            0x79, 0xbe, 0x66, 0x7e, 0xf9, 0xdc, 0xbb, 0xac, 0x55, 0xa0, 0x62, 0x95, 0xce, 0x87,
            0x0b, 0x15, 0x7c, 0x23, 0xc4, 0x7a, 0xee, 0x36, 0x6e, 0x80, 0xd6, 0xc5, 0xd4, 0x88,
            0xdf, 0x68, 0x4c, 0x1,
        ];
        let result = VerifyingKey::from_slice(&invalid_compressed);
        assert!(result.is_err());

        // Test with empty byte array
        let empty_key: [u8; 0] = [];
        let result = VerifyingKey::from_slice(&empty_key);
        assert!(result.is_err());
    }

    #[test]
    fn test_secp256k1_verify_with_invalid_signature_bytes() {
        let signing_key = SigningKey::from_str(ANVIL_ACC0_KEY).unwrap();
        let verifying_key = signing_key.verifying_key();
        let msg = b"Hello World";

        // Test with invalid signature length (too short)
        let invalid_signature_short = Signature {
            bytes: vec![0u8; 32], // Too short (32 bytes instead of 64)
            scheme: SignatureScheme::Secp256k1,
        };
        let result = verifying_key.verify(msg, &invalid_signature_short);
        assert!(result.is_err());

        // Test with invalid signature length (too long)
        let invalid_signature_long = Signature {
            bytes: vec![0u8; 128], // Too long
            scheme: SignatureScheme::Secp256k1,
        };
        let result = verifying_key.verify(msg, &invalid_signature_long);
        assert!(result.is_err());

        // Test with invalid signature content (all zeros with correct length)
        let invalid_signature_zeros = Signature {
            bytes: vec![0u8; 64], // All zeros - invalid signature
            scheme: SignatureScheme::Secp256k1,
        };
        let result = verifying_key.verify(msg, &invalid_signature_zeros);
        assert!(result.is_ok() && !result.unwrap()); // Should return Ok(false)

        // Test with invalid signature content (all 255s)
        let invalid_signature_max = Signature {
            bytes: vec![255u8; 64], // All 255s - invalid signature
            scheme: SignatureScheme::Secp256k1,
        };
        let result = verifying_key.verify(msg, &invalid_signature_max);
        assert!(result.is_err());
    }

    #[test]

    fn test_secp256k1_verify_with_modified_message() {
        let signing_key = SigningKey::from_str(ANVIL_ACC0_KEY).unwrap();
        let verifying_key = signing_key.verifying_key();

        let original_msg = b"Hello World";
        let modified_msg = b"Hello World!"; // Modified message

        let signature = signing_key.sign(original_msg).unwrap();

        // Verification should fail with modified message
        let result = verifying_key.verify(modified_msg, &signature);
        assert!(result.is_ok() && !result.unwrap()); // Should return Ok(false)
    }

    #[test]

    fn test_secp256k1_verify_with_corrupted_signature() {
        let signing_key = SigningKey::from_str(ANVIL_ACC0_KEY).unwrap();
        let verifying_key = signing_key.verifying_key();
        let msg = b"Hello World";

        let mut signature = signing_key.sign(msg).unwrap();

        // Corrupt the signature by flipping a bit
        signature.bytes[0] ^= 1;

        // Verification should fail with corrupted signature
        let result = verifying_key.verify(msg, &signature);
        assert!(result.is_ok() && !result.unwrap()); // Should return Ok(false)
    }

    #[test]

    fn test_secp256k1_serialization_deserialization_errors() {
        // Test VerifyingKey deserialization with invalid public key string
        let invalid_json = "\"invalid_public_key_string\"";
        let result: Result<VerifyingKey, _> = serde_json::from_str(invalid_json);
        assert!(result.is_err());

        // Test VerifyingKey deserialization with malformed public key
        let malformed_json = "\"02invalid_public_key_hex\"";
        let result: Result<VerifyingKey, _> = serde_json::from_str(malformed_json);
        assert!(result.is_err());
    }

    #[test]

    fn test_secp256k1_address_generation_edge_cases() {
        // Test address generation with a known public key to ensure consistency
        let signing_key = SigningKey::from_str(ANVIL_ACC0_KEY).unwrap();
        let verifying_key = signing_key.verifying_key();

        let address1 = verifying_key.to_address();
        let address2 = verifying_key.to_address(); // Should be deterministic

        assert_eq!(address1, address2);
        assert_eq!(address1.len(), 40); // Should be 40 hex characters (20 bytes)

        // Address should be valid hex
        let _decoded = hex::decode(&address1).expect("Address should be valid hex");
    }

    #[test]

    fn test_secp256k1_wrong_private_key_range() {
        // Test with private key that's too large for secp256k1 (> curve order)
        // secp256k1 curve order is 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141
        let invalid_key_str = "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364142"; // curve order + 1
        let result = SigningKey::from_str(invalid_key_str);
        assert!(result.is_err());
    }
}
