#[derive(Debug, thiserror::Error)]
pub enum SignatureError {
    #[cfg(any(feature = "default", feature = "secp256k1"))]
    #[error("{0}")]
    Ecdsa(#[from] crate::ecdsa::Error),
    #[cfg(feature = "ed25519")]
    #[error("{0}")]
    Eddsa(#[from] crate::eddsa::Error),
}
