use crate::{Signature, SignatureError, SignatureScheme};
use secp256k1::{
    ecdsa::Signature as EcdsaSignature, Message, PublicKey, Secp256k1, SecretKey as PrivateKey,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{str::FromStr, sync::LazyLock};

static SECP256K1_SIGNING: LazyLock<Secp256k1<secp256k1::SignOnly>> =
    LazyLock::new(|| Secp256k1::signing_only());
static SECP256K1_VERIFY: LazyLock<Secp256k1<secp256k1::VerifyOnly>> =
    LazyLock::new(|| Secp256k1::verification_only());

#[derive(Clone)]
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
