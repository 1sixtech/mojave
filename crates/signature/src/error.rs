#[derive(Debug, thiserror::Error)]
pub enum SignatureError {
    #[cfg(feature = "secp256k1")]
    #[error("{0}")]
    Ecdsa(#[from] crate::ecdsa::Error),
    #[cfg(feature = "ed25519")]
    #[error("{0}")]
    Eddsa(#[from] crate::eddsa::Error),
    #[cfg(feature = "secp256k1")]
    #[error("secp256k1 signature verification failed")]
    Secp256k1(#[from] secp256k1::Error),
}
