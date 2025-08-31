pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[cfg(feature = "secp256k1")]
    #[error("{0}")]
    Ecdsa(#[from] EcdsaError),
    #[cfg(feature = "secp256k1")]
    #[error("secp256k1 signature verification failed")]
    Secp256k1(#[from] secp256k1::Error),
    #[cfg(feature = "ed25519")]
    #[error("{0}")]
    Eddsa(#[from] EddsaError),
    #[cfg(feature = "ed25519")]
    #[error("ed25519 signature verification failed")]
    Ed25519(#[from] ed25519_dalek::SignatureError),
}

#[cfg(feature = "secp256k1")]
#[derive(Debug, thiserror::Error)]
pub enum EcdsaError {
    #[error("Failed to create a signing key: {0}")]
    CreateSigningKey(EcdsaErrorKind),
    #[error("Failed to sign the message: {0}")]
    Sign(EcdsaErrorKind),
    #[error("Failed to create a verifying key: {0}")]
    CreateVerifyingKey(EcdsaErrorKind),
    #[error("Failed to verify the message: {0}")]
    Verify(EcdsaErrorKind),
    #[error("Invalid signature scheme")]
    InvalidSignatureScheme,
}

#[cfg(feature = "secp256k1")]
#[derive(Debug, thiserror::Error)]
pub enum EcdsaErrorKind {
    #[error("{0}")]
    Secp256k1(#[from] secp256k1::Error),
    #[error("{0}")]
    Bincode(#[from] bincode::Error),
    #[error("{0}")]
    InvalidHex(hex::FromHexError),
}

#[cfg(feature = "ed25519")]
#[derive(Debug, thiserror::Error)]
pub enum EddsaError {
    #[error("Failed to create a signing key: {0}")]
    CreateSigningKey(EddsaErrorKind),
    #[error("Failed to sign the message: {0}")]
    Sign(EddsaErrorKind),
    #[error("Failed to create a verifying key: {0}")]
    CreateVerifyingKey(EddsaErrorKind),
    #[error("Failed to verify the message: {0}")]
    Verify(EddsaErrorKind),
    #[error("Invalid signature scheme")]
    InvalidSignatureScheme,
}

#[cfg(feature = "ed25519")]
#[derive(Debug, thiserror::Error)]
pub enum EddsaErrorKind {
    #[error("{0}")]
    Ed25519(#[from] ed25519_dalek::ed25519::Error),
    #[error("{0}")]
    Hex(#[from] hex::FromHexError),
    #[error("{0}")]
    Bincode(#[from] bincode::Error),
}
