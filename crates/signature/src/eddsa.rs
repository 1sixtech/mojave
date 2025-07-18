use crate::{Signature, SignatureError, SignatureScheme};
use ed25519_dalek::{
    Signature as EddsaSignature, Signer, SigningKey as PrivateKey, Verifier,
    VerifyingKey as PublicKey,
};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Clone, Debug)]
pub struct SigningKey(PrivateKey);

impl FromStr for SigningKey {
    type Err = SignatureError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = hex::decode(s).map_err(|error| Error::CreateSigningKey(error.into()))?;
        let secret_key = PrivateKey::try_from(bytes.as_slice())
            .map_err(|error| Error::CreateSigningKey(error.into()))?;
        Ok(Self(secret_key))
    }
}

impl super::Signer for SigningKey {
    fn from_slice(slice: &[u8]) -> Result<Self, SignatureError> {
        let secret_key =
            PrivateKey::try_from(slice).map_err(|error| Error::CreateSigningKey(error.into()))?;
        Ok(Self(secret_key))
    }

    fn sign<T: Serialize>(&self, message: &T) -> Result<Signature, SignatureError> {
        let message_bytes =
            bincode::serialize(message).map_err(|error| Error::Sign(error.into()))?;
        let signature = self.0.sign(&message_bytes);
        Ok(Signature {
            bytes: signature.to_vec(),
            scheme: SignatureScheme::Ed25519,
        })
    }
}

impl SigningKey {
    pub fn verifying_key(&self) -> VerifyingKey {
        VerifyingKey(PublicKey::from(&self.0))
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
        hex::encode(value.0.as_bytes())
    }
}

impl FromStr for VerifyingKey {
    type Err = SignatureError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = hex::decode(s).map_err(|error| Error::CreateVerifyingKey(error.into()))?;
        let public_key = PublicKey::try_from(bytes.as_slice())
            .map_err(|error| Error::CreateVerifyingKey(error.into()))?;
        Ok(Self(public_key))
    }
}

impl super::Verifier for VerifyingKey {
    fn from_slice(slice: &[u8]) -> Result<Self, SignatureError> {
        let public_key =
            PublicKey::try_from(slice).map_err(|error| Error::CreateVerifyingKey(error.into()))?;
        Ok(Self(public_key))
    }

    fn verify<T: Serialize>(
        &self,
        message: &T,
        signature: &Signature,
    ) -> Result<bool, SignatureError> {
        if signature.scheme != SignatureScheme::Ed25519 {
            return Err(Error::InvalidSignatureScheme)?;
        }

        let message_bytes =
            bincode::serialize(message).map_err(|error| Error::Verify(error.into()))?;
        let signature = EddsaSignature::from_slice(&signature.bytes)
            .map_err(|error| Error::Verify(error.into()))?;

        match self.0.verify(&message_bytes, &signature) {
            Ok(()) => Ok(true),
            Err(_error) => Ok(false),
        }
    }
}

impl VerifyingKey{
    pub fn to_address(&self) -> String{
        String::from(self.clone())
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
    Ed25519(#[from] ed25519_dalek::ed25519::Error),
    #[error("{0}")]
    Hex(#[from] hex::FromHexError),
    #[error("{0}")]
    Bincode(#[from] bincode::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{Signer, Verifier};
    /// made key pair using `solana-keygen new --no-passphrase`
    ///
    /// [
    ///   144,  45, 220,  66,  89, 201,   7, 239,
    ///    86, 173, 155, 227,  31, 102,  64, 151,
    ///   142, 184, 211, 146, 225, 143, 253, 224,
    ///   165, 105, 222, 216,   4, 223,  35, 225,
    ///
    ///   104, 129, 238,  30, 109,  80,  35,  40,
    ///   222, 122, 189, 203, 126, 168,  28, 216,
    ///   229, 110, 167,  57, 192, 114, 219, 225,
    ///   233, 104,   3,  71,   9, 159, 103, 127
    /// ]
    ///
    /// first 32 bytes for secret key,
    /// second 32 bytes for public key.
    
    const PRIVATE_KEY: [u8; 32] = [
        144, 45, 220, 66, 89, 201, 7, 239, 86, 173, 155, 227, 31, 102, 64, 151, 142, 184, 211,
        146, 225, 143, 253, 224, 165, 105, 222, 216, 4, 223, 35, 225,
    ];
    const PUBLIC_KEY: [u8;32] = [
        104, 129, 238, 30, 109, 80, 35, 40, 222, 122, 189, 203, 126, 168, 28, 216, 229, 110,
        167, 57, 192, 114, 219, 225, 233, 104, 3, 71, 9, 159, 103, 127,
    ];

    #[test]
    fn test_ed25519_get_public_key_from_private_key() {
        let signing_key = SigningKey::from_slice(&PRIVATE_KEY).unwrap();
        let verifying_key = signing_key.verifying_key();
        let expected_pub_key: String = hex::encode(&PUBLIC_KEY);

        let pub_key = String::from(verifying_key);

        print!(
            "expected  : {:?}\ncalculated: {:?}",
            expected_pub_key, pub_key
        );
        assert_eq!(expected_pub_key, pub_key);
    }

    #[test]
    fn test_ed25519_sign_and_verify() {
        let signing_key = SigningKey::from_slice(&PRIVATE_KEY).unwrap();
        
        let verifying_key= signing_key.verifying_key();
        let msg = b"Hello World";

        let signature = signing_key.sign(msg).unwrap();
        let res = verifying_key.verify(msg, &signature);

        assert!(res.is_ok())
    }

    // Negative test cases
    
    #[test]
    fn test_ed25519_invalid_signing_key() {
        // Test with invalid hex characters
        let invalid_hex = "invalid_hex_string_not_valid";
        let result = SigningKey::from_str(invalid_hex);
        assert!(result.is_err());
        
        // Test with odd-length hex string
        let odd_hex = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0";
        let result = SigningKey::from_str(odd_hex);
        assert!(result.is_err());

        // Test with too short byte array (16 bytes instead of 32)
        let short_key: [u8; 16] = [
            144, 45, 220, 66, 89, 201, 7, 239,
            86, 173, 155, 227, 31, 102, 64, 151
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
    }

    #[test]
    fn test_ed25519_invalid_verifying_key() {
        // Test with invalid hex characters
        let invalid_hex = "invalid_hex_string_not_valid";
        let result = VerifyingKey::from_str(invalid_hex);
        assert!(result.is_err());
        
        // Test with odd-length hex string
        let odd_hex = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0";
        let result = VerifyingKey::from_str(odd_hex);
        assert!(result.is_err());

        // Test with too short byte array (16 bytes instead of 32)
        let short_key: [u8; 16] = [
            104, 129, 238, 30, 109, 80, 35, 40,
            222, 122, 189, 203, 126, 168, 28, 216
        ];
        let result = VerifyingKey::from_slice(&short_key);
        assert!(result.is_err());
        
        // Test with too long byte array (64 bytes instead of 32)
        let long_key: [u8; 64] = [0u8; 64];
        let result = VerifyingKey::from_slice(&long_key);
        assert!(result.is_err());
        
        // Test with empty byte array
        let empty_key: [u8; 0] = [];
        let result = VerifyingKey::from_slice(&empty_key);
        assert!(result.is_err());
    }

    #[test]
    fn test_ed25519_verify_with_invalid_signature_bytes() {
        let signing_key = SigningKey::from_slice(&PRIVATE_KEY).unwrap();
        let verifying_key = signing_key.verifying_key();
        let msg = b"Hello World";

        // Test with invalid signature length (too short)
        let invalid_signature_short = Signature {
            bytes: vec![0u8; 32], // Too short (32 bytes instead of 64)
            scheme: SignatureScheme::Ed25519,
        };
        let result = verifying_key.verify(msg, &invalid_signature_short);
        assert!(result.is_err());

        // Test with invalid signature length (too long)
        let invalid_signature_long = Signature {
            bytes: vec![0u8; 128], // Too long
            scheme: SignatureScheme::Ed25519,
        };
        let result = verifying_key.verify(msg, &invalid_signature_long);
        assert!(result.is_err());

        // Test with invalid signature content (all zeros) - Ed25519 returns Ok(false)
        let invalid_signature_zeros = Signature {
            bytes: vec![0u8; 64], // All zeros - invalid signature
            scheme: SignatureScheme::Ed25519,
        };
        let result = verifying_key.verify(msg, &invalid_signature_zeros);
        // Ed25519 returns Ok(false) for invalid signatures that parse correctly
        assert!(result.is_ok() && !result.unwrap());
    }

    #[test]
    fn test_ed25519_verify_with_modified_message() {
        let signing_key = SigningKey::from_slice(&PRIVATE_KEY).unwrap();
        let verifying_key = signing_key.verifying_key();
        
        let original_msg = b"Hello World";
        let modified_msg = b"Hello World!"; // Modified message

        let signature = signing_key.sign(original_msg).unwrap();
        
        // Verification should fail with modified message
        let result = verifying_key.verify(modified_msg, &signature);
        assert!(result.is_ok() && !result.unwrap()); // Should return Ok(false)
    }

    #[test]
    fn test_ed25519_verify_with_corrupted_signature() {
        let signing_key = SigningKey::from_slice(&PRIVATE_KEY).unwrap();
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
    fn test_ed25519_serialization_deserialization_errors() {
        // Test VerifyingKey deserialization with invalid hex
        let invalid_json = "\"invalid_hex_string\"";
        let result: Result<VerifyingKey, _> = serde_json::from_str(invalid_json);
        assert!(result.is_err());
        
        // Test VerifyingKey deserialization with wrong length hex
        let wrong_length_json = "\"6881ee1e6d502328de7abdcb7ea81cd8\""; // Too short
        let result: Result<VerifyingKey, _> = serde_json::from_str(wrong_length_json);
        assert!(result.is_err());
    }

}