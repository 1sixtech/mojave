use crate::{Signature, SignatureError, SignatureScheme};
use ed25519_dalek::{
    Signature as EddsaSignature, Signer, SigningKey as PrivateKey, Verifier,
    VerifyingKey as PublicKey,
};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

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
    fn verifying_key(&self) -> VerifyingKey {
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
